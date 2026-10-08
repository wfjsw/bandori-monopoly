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