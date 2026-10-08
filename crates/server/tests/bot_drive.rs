//! The drive's refusal contract (`docs/BOT.md` §5 B6; `bot_cpu` §6).
//!
//! When the engine refuses the service's answer, the drive must fall back to
//! the heuristic for that decision **immediately** and drop the cached answer
//! so it is never replayed -- never re-ask the service for the same decision.
//! The 2026-10-08 stall (45–54% of match time) was exactly this loop: a cached
//! refused Buy re-applied every 200 ms until the turn bank expired.

use std::sync::Arc;
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality};
use game_core::MatchMode;
use server::botsvc::BotService;
use server::apply_bot_answer;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

/// Park at a searchable decision for member 1 (the abstracted action list is
/// non-empty), answering mulligan / roll / end with the heuristic on the way.
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
                && (st.step == stage::OPS || st.step == stage::END));
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

#[tokio::test]
async fn a_refused_service_answer_falls_back_without_reasking() {
    let data = data();
    let rules: Arc<dyn CardRules> = Arc::new(StubRules);
    let bots = BotService::in_process(data.clone(), rules);
    let (m, member) = parked_searchable();
    let seat = bot_service::BotSeatView::from_match(&m, member);
    let view = serde_json::to_value(&seat).expect("view serialises");

    // The service answers (and caches) one decision.
    let first = bots
        .decide("R", member, &view, 0, 300, 5, Duration::from_secs(5))
        .await
        .expect("decide answers");
    let refused = first.answer.clone();
    let heuristic = bot_service::heuristic_message_view(&data, &seat);

    // The engine refuses anything but the heuristic -- as it refused the
    // unaffordable Buy in the live-match stall.
    let mut seen: Vec<NetMessage> = Vec::new();
    let acted = apply_bot_answer(&bots, &data, &seat, member, refused.clone(), {
        let h = heuristic.clone();
        let seen = &mut seen;
        move |cmd| {
            seen.push(cmd.clone());
            let h = h.clone();
            async move {
                if cmd == h {
                    Ok(None)
                } else {
                    Ok(Some(Msg::new("err.buy_poor")))
                }
            }
        }
    })
    .await;

    assert!(acted, "the heuristic fallback must land");
    assert_eq!(
        seen.len(),
        2,
        "exactly one fallback -- never re-ask the service: {seen:?}"
    );
    assert_eq!(seen[0], refused, "the service's answer goes first");
    assert_eq!(seen[1], heuristic, "then the heuristic, not a second search");

    // The cached answer is gone: the next decide re-searches instead of
    // replaying the refused one (`reused: false`).
    let second = bots
        .decide("R", member, &view, 0, 300, 6, Duration::from_secs(5))
        .await
        .expect("decide answers");
    assert!(
        !second.reused,
        "a refused answer must not be replayed from the cache"
    );
}