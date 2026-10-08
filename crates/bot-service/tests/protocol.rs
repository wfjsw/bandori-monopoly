//! Protocol round-trip: one seat's view in, one proposed `NetMessage` out.
//!
//! The service must take the **view frame** the client gets (the worker's
//! `view` op shape) and answer with a legal command, never a `World` / seed /
//! another seat's hand (`docs/BOT.md` §1). StubRules is enough here -- this
//! pins the wire shape and the decision plumbing, not playing strength.

use std::sync::Arc;
use std::time::Duration;

use bot_service::{decide_request, handle, ponder_request, BotAnswer, Ctx, SearchOpts};
use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use serde_json::{json, Value};

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

fn ctx() -> Ctx {
    Ctx::new(data(), Arc::new(StubRules) as Arc<dyn CardRules>)
}

/// A 3-seat match parked at one member's decision: seat 1 is a human from the
/// engine's point of view (so it waits), seats 2-3 are standard bots.
fn parked_match() -> (Match, i32) {
    let members = vec![
        RoomMember {
            id: 1,
            player: "Searcher".into(),
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "BotA".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "BotB".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    // Tick until member 1 is at a real decision -- an open prompt waiting on
    // it, or the turn surface (运营 / 结束). Same predicate the server's
    // `botsvc::decision_at` uses.
    for _ in 0..8_000 {
        if m.ended() {
            break;
        }
        let st = m.state();
        let me = st.player_of(1);
        let prompt = st.prompt.id > 0 && st.prompt.waiting(me);
        let turn = !st.busy
            && st.turn == me
            && (st.step == game_core::state::stage::OPS || st.step == game_core::state::stage::END);
        if st.phase == "play" && (prompt || turn) {
            break;
        }
        m.tick(0.25);
    }
    (m, 1)
}

/// The exact frame the server sends: the worker's `view` op shape.
fn view_frame(m: &Match, member: i32) -> Value {
    let st = m.state();
    let player_id = st.player_of(member);
    let extra = m.view_extra(member);
    json!({
        "state": st,
        "hand": m.hand_of(member),
        "handNotes": m.hand_notes_of(member),
        "draw": m.draw_of(member),
        "you": member,
        "playerId": player_id,
        "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(Value::Null),
        "playable": extra.get("playable").cloned().unwrap_or(Value::Null),
    })
}

#[test]
fn ping_and_info_answer() {
    let ctx = ctx();
    let pong = handle(&ctx, json!({"id": 1, "op": "ping"}));
    assert_eq!(pong["ok"], true);
    assert_eq!(pong["id"], 1);
    let info = handle(&ctx, json!({"id": 2, "op": "info"}));
    assert_eq!(info["ok"], true);
    assert_eq!(info["service"], "bot-service");
    assert!(info["modules"].is_string());
}

#[test]
fn decide_round_trips_a_view_frame() {
    let ctx = ctx();
    let (m, member) = parked_match();
    let view = view_frame(&m, member);
    let st = m.state();
    let prompt_id = if st.prompt.id > 0 && st.prompt.waiting(st.player_of(member)) {
        st.prompt.id
    } else {
        0
    };

    let req = decide_request(9, "TESTROOM", member, &view, prompt_id, 200, 42);
    let resp = handle(&ctx, req);
    assert_eq!(resp["ok"], true, "{resp}");
    assert_eq!(resp["id"], 9, "the id is echoed for out-of-order matching");

    let ans = BotAnswer::from_response(&resp).expect("usable answer");
    assert!(!ans.answer.act.is_empty(), "a command came back: {:?}", ans.answer);
    // The reply reports the search work so the server can log it.
    assert!(resp.get("iterations").is_some());
    assert!(resp.get("elapsed_ms").is_some());
    // Applying the answer is the server's job -- the service only proposes.
    // It must be something `Match::act` would at least recognise.
    let mut m = m;
    let ok = m.act(member, &ans.answer).is_ok();
    let st2 = m.state();
    assert!(
        ok || st2.seq != st.seq || st2.turn != st.turn || st2.prompt.id != prompt_id,
        "answer {:?} was refused and the decision is still open",
        ans.answer
    );
}

#[test]
fn decide_accepts_the_worker_view_shape_verbatim() {
    // `handNotes` is in the frame the client gets and not in `SeatView`; it
    // must be ignored, not rejected.
    let ctx = ctx();
    let (m, member) = parked_match();
    let view = view_frame(&m, member);
    assert!(view.get("handNotes").is_some());
    let resp = handle(&ctx, decide_request(1, "R", member, &view, 0, 50, 1));
    assert_eq!(resp["ok"], true, "{resp}");
}

#[test]
fn decide_reports_the_heuristic_on_a_delegated_surface() {
    // Roll / end / discard are heuristic-delegated: the service answers them
    // without searching and says so (`iterations == 0`, `heuristic`).
    let ctx = ctx();
    let (m, member) = parked_match();
    let view = view_frame(&m, member);
    let resp = handle(&ctx, decide_request(3, "R", member, &view, 0, 1, 1));
    assert_eq!(resp["ok"], true, "{resp}");
    let ans = BotAnswer::from_response(&resp).unwrap();
    // Either the surface was searchable (a real search ran) or it was
    // delegated -- both are legal; the shape must hold either way.
    assert!(!ans.answer.act.is_empty());
    if ans.heuristic {
        assert_eq!(ans.iterations, 0);
    }
}

#[test]
fn bad_requests_are_errors_not_panics() {
    let ctx = ctx();
    let r = handle(&ctx, json!({"id": 4, "op": "decide"}));
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("view"));
    let r = handle(&ctx, json!({"id": 5, "op": "decide", "view": {"nope": 1}}));
    assert_eq!(r["ok"], false);
    let r = handle(&ctx, json!({"id": 6, "op": "nonsense"}));
    assert_eq!(r["ok"], false);
    // `id` is still echoed on failure so the client can match it.
    assert_eq!(r["id"], 6);
}

#[test]
fn budget_is_clamped_not_trusted() {
    // A wild `budget_ms` must not park a search thread for minutes.
    let ctx = ctx();
    let (m, member) = parked_match();
    let view = view_frame(&m, member);
    let started = std::time::Instant::now();
    let resp = handle(
        &ctx,
        decide_request(8, "R", member, &view, 0, 3_600_000, 1),
    );
    assert_eq!(resp["ok"], true, "{resp}");
    // The hard cap is `MAX_BUDGET_MS` (5 s) plus overhead -- nowhere near the
    // hour the request asked for.
    assert!(
        started.elapsed() < Duration::from_secs(12),
        "a huge budget_ms must be capped (took {:?})",
        started.elapsed()
    );
}

#[test]
fn the_search_never_sees_another_seats_hand() {
    // The frame carries exactly one seat's hand; the reply must not need any
    // more than that. Reconstructing the request from the frame alone (as the
    // server does) is the contract -- this test pins the frame's fields.
    let (m, member) = parked_match();
    let view = view_frame(&m, member);
    assert_eq!(view["you"], member);
    assert!(view["hand"].as_array().is_some());
    assert!(view["draw"].as_array().is_some());
    assert!(view["state"].get("players").is_some());
    // No `world`, no `seed`, no other seat's `hand`.
    assert!(view.get("world").is_none());
    assert!(view.get("seed").is_none());
    for p in view["state"]["players"].as_array().unwrap() {
        assert!(p.get("hand").is_none() || p["hand"].is_number(), "hand size only");
    }
}

/// Park at a *searchable* decision (the abstracted action list is non-empty),
/// answering mulligan / roll / end with the heuristic on the way.
fn parked_searchable() -> (Match, i32) {
    let members = vec![
        RoomMember {
            id: 1,
            player: "Searcher".into(),
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "BotA".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "BotB".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];
    let data = data();
    let mut m = Match::new(
        data.clone(),
        Arc::new(StubRules),
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    for _ in 0..20_000 {
        if m.ended() {
            break;
        }
        let st = m.state();
        let me = st.player_of(1);
        let at = (st.prompt.id > 0 && st.prompt.waiting(me))
            || (!st.busy
                && st.turn == me
                && (st.step == game_core::state::stage::OPS
                    || st.step == game_core::state::stage::END));
        if st.phase == "play" && at {
            let view = bot_service::BotSeatView::from_match(&m, 1);
            let acts = bot_service::bot_action::legal_actions(
                &data,
                &view.state,
                &view.hand,
                &view.playable,
                me as usize,
            );
            if !acts.is_empty() {
                return (m, 1);
            }
            let msg = bot_service::heuristic_message_view(&data, &view);
            let _ = m.act(1, &msg);
        }
        m.tick(0.25);
    }
    panic!("no searchable decision for member 1");
}

#[test]
fn ponder_is_cached_and_reused_by_the_next_decide() {
    // BOT-RESEARCH #5: a speculative `ponder` while the other seats act is
    // keyed by the view's decision key; a `decide` for the same information
    // set must reuse it (`reused: true`) without spending its budget.
    let ctx = ctx();
    let (m, member) = parked_searchable();
    let view = view_frame(&m, member);

    let p = handle(&ctx, ponder_request(1, "R", member, &view, 200, 5));
    assert_eq!(p["ok"], true, "{p}");
    assert_eq!(p["reused"], false, "first ponder searches: {p}");
    let ponder_iters = p["iterations"].as_u64().unwrap_or(0);
    let key = p["decisionKey"].as_str().unwrap_or("").to_string();
    assert!(!key.is_empty(), "the ponder reports its decision key: {p}");

    let started = std::time::Instant::now();
    let d = handle(&ctx, decide_request(2, "R", member, &view, 0, 200, 5));
    let dt = started.elapsed();
    assert_eq!(d["ok"], true, "{d}");
    assert_eq!(d["reused"], true, "the decide reuses the pondered result: {d}");
    let ans = BotAnswer::from_response(&d).expect("usable answer");
    assert_eq!(ans.iterations, ponder_iters, "the cached work is reported");
    // Reuse is free: no new search runs (the wall time is far under budget).
    assert!(
        dt < Duration::from_millis(150),
        "a reused decision must not search again (took {dt:?})"
    );
    assert!(!ans.answer.act.is_empty());
}

#[test]
fn decide_reuses_the_same_ponder_only_for_the_same_information_set() {
    let ctx = ctx();
    let (m, member) = parked_searchable();
    let view = view_frame(&m, member);
    let _ = handle(&ctx, ponder_request(1, "R", member, &view, 100, 5));
    // A different budget / seed / id does not change the information set.
    let d = handle(&ctx, decide_request(2, "R", member, &view, 0, 500, 99));
    assert_eq!(d["reused"], true, "{d}");
}

#[test]
fn root_parallel_decide_answers_from_the_service() {
    // BOT-RESEARCH #2: one decision may use several search threads; the
    // reply shape is unchanged.
    let mut opts = SearchOpts::default();
    opts.search_threads = 2;
    let ctx = ctx().with_opts(opts);
    let (m, member) = parked_searchable();
    let view = view_frame(&m, member);
    let resp = handle(&ctx, decide_request(4, "R", member, &view, 0, 200, 11));
    assert_eq!(resp["ok"], true, "{resp}");
    let ans = BotAnswer::from_response(&resp).expect("usable answer");
    if !ans.heuristic {
        assert!(ans.iterations >= 2, "two threads x >=1 iteration: {}", ans.iterations);
    }
    assert!(!ans.answer.act.is_empty());
}

#[test]
fn info_reports_the_search_options() {
    let ctx = ctx();
    let info = handle(&ctx, json!({"id": 1, "op": "info"}));
    assert_eq!(info["ok"], true);
    assert!(info.get("search_threads").is_some());
    assert!(info.get("bias_weight").is_some());
    assert!(info.get("eval_weight").is_some());
    assert!(info.get("horizon_rounds").is_some());
    assert!(info.get("ponder").is_some());
}

#[test]
fn legacy_opts_answer_without_the_new_behaviour() {
    let ctx = ctx().with_opts(SearchOpts::legacy());
    let (m, member) = parked_searchable();
    let view = view_frame(&m, member);
    let p = handle(&ctx, ponder_request(1, "R", member, &view, 50, 5));
    assert_eq!(p["ok"], true);
    assert_eq!(p["disabled"], true, "legacy turns the ponder cache off: {p}");
    let d = handle(&ctx, decide_request(2, "R", member, &view, 0, 50, 5));
    assert_eq!(d["ok"], true, "{d}");
    assert_eq!(d["reused"], false, "legacy never reuses: {d}");
}