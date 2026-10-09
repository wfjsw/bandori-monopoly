//! Root-parallel ISMCTS, tree reuse and the #3 selection flags
//! (`docs/BOT-RESEARCH` #2/#3/#5).
//!
//! * **Determinism**: given a seed and a thread count, the merged result is
//!   reproducible (the iteration cap binds; the wall budget is wide).
//! * **N = 1 equivalence**: the root-parallel path with one thread is exactly
//!   the single-threaded search.
//! * **Tree reuse**: [`Ismcts::retain_after`] keeps the subtree under the
//!   played action and drops the root's other edges.

use std::sync::Arc;
use std::time::Duration;

use bot_core::{
    action, Ismcts, MatchSim, SearchConfig, SeatView,
};
use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

/// A match parked at member 1's first *searchable* decision (the abstracted
/// action list is non-empty).
fn parked() -> (Match, i32) {
    let members = vec![
        RoomMember {
            id: 1,
            player: "Searcher".into(),
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "B".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "C".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];
    let data = data();
    let mut m = Match::new(
        data.clone(),
        Arc::new(StubRules) as Arc<dyn CardRules>,
        &members,
        7,
        MatchMode::Solo,
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
            let view = SeatView::from_match(&m, 1);
            let acts = action::legal_actions(
                &data,
                &view.state,
                &view.hand,
                &view.playable,
                me as usize,
            );
            if !acts.is_empty() {
                return (m, 1);
            }
            // Non-searchable decision (mulligan / roll / end): answer it with
            // the heuristic and move on -- the search tests need a surface
            // with something to branch on.
            let msg = bot_core::heuristic_message_view(&data, &view);
            let _ = m.act(1, &msg);
        }
        m.tick(0.25);
    }
    panic!("no searchable decision for member 1");
}

fn sim_of(m: &Match, member: i32) -> MatchSim {
    let data = data();
    let rules: Arc<dyn CardRules> = Arc::new(StubRules);
    let view = SeatView::from_match(m, member);
    let seat = view.player_id.max(0) as usize;
    MatchSim::new(data, rules, view, seat, member)
}

/// The searching seat's player index (member → player).
fn seat_of(m: &Match, member: i32) -> usize {
    m.state().player_of(member).max(0) as usize
}

/// Deterministic search config: the iteration cap binds, the wall budget
/// never does.
fn det_cfg(seed: u64, iters: u64) -> SearchConfig {
    SearchConfig {
        budget: Duration::from_secs(60),
        max_iterations: iters,
        seed,
        ..Default::default()
    }
}

#[test]
fn one_thread_root_parallel_equals_single_thread() {
    let (m, member) = parked();
    let seat = seat_of(&m, member);

    let mut a = Ismcts::new();
    let out_a = a.search(&mut sim_of(&m, member), seat, det_cfg(42, 8));

    let mut b = Ismcts::new();
    let out_b = b.search_root_parallel(|| sim_of(&m, member), seat, det_cfg(42, 8), 1);

    assert!(!out_a.heuristic, "the single search actually searched");
    assert!(out_a.iterations > 0);
    assert_eq!(out_a.action, out_b.action, "N=1 must pick the same action");
    assert_eq!(out_a.iterations, out_b.iterations);
    assert_eq!(out_a.root_stats, out_b.root_stats);
    assert_eq!(out_a.root_key, out_b.root_key);
}

#[test]
fn root_parallel_is_deterministic_given_seed_and_threads() {
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let run = |threads: usize| {
        let mut t = Ismcts::new();
        t.search_root_parallel(|| sim_of(&m, member), seat, det_cfg(7, 6), threads)
    };
    for threads in [1usize, 2, 4] {
        let a = run(threads);
        let b = run(threads);
        assert_eq!(a.action, b.action, "threads={threads}");
        assert_eq!(a.iterations, b.iterations, "threads={threads}");
        assert_eq!(
            a.root_stats_full.len(),
            b.root_stats_full.len(),
            "threads={threads}"
        );
        for (x, y) in a.root_stats_full.iter().zip(&b.root_stats_full) {
            assert_eq!(x.action, y.action, "threads={threads}");
            assert_eq!(x.visits, y.visits, "threads={threads}");
            assert!(
                (x.value_sum - y.value_sum).abs() < 1e-9,
                "threads={threads}: {} vs {}",
                x.value_sum,
                y.value_sum
            );
        }
        // The root key is a pure function of the view.
        assert_eq!(a.root_key, b.root_key);
    }
}

#[test]
fn parallel_iterations_scale_with_threads() {
    // The budget is the same wall clock; N threads must complete at least as
    // many iterations in total as one (usually ~N×).
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let cfg = SearchConfig {
        budget: Duration::from_millis(300),
        ..Default::default()
    };
    let one = Ismcts::new().search_root_parallel(|| sim_of(&m, member), seat, cfg, 1);
    let four = Ismcts::new().search_root_parallel(|| sim_of(&m, member), seat, cfg, 4);
    assert!(one.iterations >= 1, "{}", one.iterations);
    assert!(
        four.iterations > one.iterations,
        "4 threads did {four} vs 1 thread {one}",
        four = four.iterations,
        one = one.iterations
    );
}

#[test]
fn retain_after_keeps_only_the_played_subtree() {
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let mut tree = Ismcts::new();
    let out = tree.search(&mut sim_of(&m, member), seat, det_cfg(11, 20));
    let before = tree.node_count();
    assert!(before > 0, "the search built a tree");
    let root_edges = tree.edge_count(out.root_key);
    assert!(root_edges > 0);

    tree.retain_after(out.root_key, &out.action);
    let after = tree.node_count();
    assert!(after <= before, "retain cannot add nodes ({after} <= {before})");
    assert_eq!(
        tree.edge_count(out.root_key),
        1,
        "the root keeps only the played action"
    );
    // The kept subtree is exactly the material under the played action; with
    // several root actions in play this strictly shrinks the tree.
    if root_edges > 1 {
        assert!(after < before, "siblings were dropped ({after} < {before})");
    }
}

#[test]
fn parallel_tree_merges_and_reuses() {
    // The merged root-parallel tree must keep the played subtree too (the
    // threads' subtree bookkeeping is unioned by `merge_from`).
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let mut tree = Ismcts::new();
    let out = tree.search_root_parallel(|| sim_of(&m, member), seat, det_cfg(13, 8), 3);
    assert!(out.iterations >= 3, "three threads x >=1 iteration");
    let before = tree.node_count();
    assert!(before > 0, "the merged tree has nodes");
    tree.retain_after(out.root_key, &out.action);
    assert_eq!(tree.edge_count(out.root_key), 1);
    assert!(tree.node_count() <= before);
    // The kept subtree is non-trivial: the root itself is there.
    assert!(tree.node_count() >= 1);
}

#[test]
fn legacy_flags_still_search() {
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let cfg = SearchConfig {
        budget: Duration::from_millis(50),
        seed: 3,
        ..SearchConfig::legacy()
    };
    let out = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg);
    assert!(!out.heuristic, "legacy mode still searches: {:?}", out.action);
    assert!(out.iterations >= 1);
}

#[test]
fn search_flags_do_not_change_the_view_boundary() {
    // The search must never need anything beyond the one seat's view: the
    // determinizer + search run off `SeatView::from_match` alone (this is the
    // B4 contract; the new flags must not weaken it).
    let (m, member) = parked();
    let view = SeatView::from_match(&m, member);
    let seat = view.player_id.max(0) as usize;
    let out = Ismcts::new().search_root_parallel(|| sim_of(&m, member), seat, det_cfg(5, 4), 2);
    assert!(out.iterations >= 1);
}

// ---------------------------------------------------------- early stopping

#[test]
fn early_stop_is_off_in_deterministic_mode_and_the_iteration_cap_binds() {
    // The determinism contract (`docs/BOT.md` §3.4): with the iteration cap
    // binding and the wall budget wide, `early_stop` must not change the
    // result -- tests that pin seed+threads reproducibility stay valid.
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let cfg_off = det_cfg(42, 8);
    let cfg_on = SearchConfig {
        early_stop: true,
        ..det_cfg(42, 8)
    };
    let a = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg_off);
    let b = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg_on);
    assert_eq!(a.action, b.action, "early stop must not move the pick");
    assert_eq!(a.iterations, b.iterations);
    assert_eq!(a.root_stats, b.root_stats);
    assert!(!a.early_stopped && !b.early_stopped, "the cap bound first");
}

#[test]
fn early_stop_never_runs_past_the_budget_and_reports_itself() {
    // Anytime contract: with `early_stop` on the search still returns a
    // searched answer, and when the rule fires it says so. The rule is
    // allowed not to fire (an ambiguous root just runs the budget) -- the
    // assertion is on the contract, not on the machine's speed.
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let cfg = SearchConfig {
        budget: Duration::from_millis(120),
        seed: 7,
        early_stop: true,
        min_iterations: 2,
        ..Default::default()
    };
    let out = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg);
    assert!(!out.heuristic, "a searched answer came back: {:?}", out.action);
    assert!(out.iterations >= 1);
    if out.early_stopped {
        assert!(
            out.elapsed < Duration::from_millis(120),
            "early stop fired but the search still ran the budget: {:?}",
            out.elapsed
        );
    }
}

#[test]
fn early_stop_stops_no_later_than_the_full_budget_search() {
    // Strength-safe direction: the early-stop rule may only *remove* work
    // (stop at or before the budget), never add it. Same seed, same budget.
    let (m, member) = parked();
    let seat = seat_of(&m, member);
    let cfg = |early: bool| SearchConfig {
        budget: Duration::from_millis(200),
        seed: 9,
        early_stop: early,
        min_iterations: 2,
        ..Default::default()
    };
    let on = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg(true));
    let off = Ismcts::new().search(&mut sim_of(&m, member), seat, cfg(false));
    assert!(
        on.iterations <= off.iterations,
        "early stop added work: {on} > {off}",
        on = on.iterations,
        off = off.iterations
    );
    assert!(off.elapsed >= Duration::from_millis(190), "the full-budget run ran its budget");
}

// ------------------------------------------------- speculative next decision

#[test]
fn predict_upcoming_view_is_a_turn_start_ops_surface() {
    use bot_core::{next_turn_near, predict_upcoming_view};
    use game_core::state::{stage, MatchState};

    let (m, member) = parked();
    let view = SeatView::from_match(&m, member);
    // The parked match is at a live decision -- prediction is for the *next*
    // one. Clear the surface to model "another seat is acting".
    let mut idle = view.clone();
    idle.state.turn = (view.player_id + 1) % 3;
    idle.state.step = stage::END;
    idle.state.prompt.id = 0;
    idle.state.busy = false;
    assert!(
        next_turn_near(&idle.state, idle.player_id),
        "the current player's turn is ending, so ours is next"
    );
    let pred = predict_upcoming_view(&idle).expect("a predictable turn start");
    assert_eq!(pred.state.turn, idle.player_id);
    assert_eq!(pred.state.step, stage::OPS);
    assert!(!pred.state.busy);
    assert_eq!(pred.state.landed, -1);
    assert_eq!(pred.state.prompt.id, 0);
    assert!(pred.state.can_roll_here, "roll is owed at 运营");
    assert!(!pred.state.can_end_here, "end is gated until the move resolves");
    assert_eq!(pred.hand, idle.hand, "own hand is exact");
    assert_eq!(pred.draw, idle.draw, "own draw composition is exact");
    // The predicted frame is itself a searchable decision surface.
    let data = data();
    let acts = bot_core::action::legal_actions(
        &data,
        &pred.state,
        &pred.hand,
        &pred.playable,
        pred.player_id as usize,
    );
    // Card plays (StubRules hands have playable cards) or a forced / trivial
    // root -- either way `surface` recognised it as the turn surface.
    assert!(bot_core::action::surface(&pred.state, pred.player_id as usize).is_some());
    let _ = acts;

    // Mid-move: the 结束 surface depends on where the dice land -- no guess.
    let mut mid_move = idle.clone();
    mid_move.state.turn = idle.player_id;
    mid_move.state.step = stage::MOVE;
    assert!(predict_upcoming_view(&mid_move).is_none());

    // A prompt open on someone else is not a predictable turn start.
    let mut prompted = idle.clone();
    prompted.state.prompt.id = 7;
    assert!(predict_upcoming_view(&prompted).is_none());

    // A long wait (not next, not ending) is not near: the seat after us is
    // mid-turn (运营), and the ring's next after them is not us.
    let mut far = idle.clone();
    far.state.turn = (idle.player_id + 1) % 3;
    far.state.step = stage::OPS;
    assert!(!next_turn_near(&far.state, far.player_id));

    // A synthetic state: our own turn mid-routine is near (the next surface
    // is ours).
    let mut ours = MatchState::default();
    ours.phase = "play".into();
    ours.turn = 1;
    ours.step = stage::START;
    ours.players = idle.state.players.clone();
    assert!(next_turn_near(&ours, 1));
    assert!(predict_upcoming_view(&SeatView {
        state: ours,
        player_id: 1,
        ..idle.clone()
    })
    .is_some());
}