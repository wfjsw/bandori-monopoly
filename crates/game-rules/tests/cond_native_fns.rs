//! Every native condition name answers exactly what the guest-side `ctx::*`
//! reader returns on the same world (docs/GUARDS.md §11.4 / §11.6 -- past bugs
//! came from the snapshot fill disagreeing with the guest reader). Each test
//! builds a world, reads the value through the guest's host path
//! (`CardWorld::dist`-equivalent / `fill_window` + CEL) and asserts the two
//! agree.

#[path = "testworld.rs"]
mod testworld;
use testworld::*;

use game_rules::cond_pre::{self, CompiledPre};

/// Evaluate `src` against the window/candidate snapshots of `w`.
fn ask(w: &TestWorld, src: &str, owner: i32, card: &str, placed: bool) -> bool {
    let pre = CompiledPre::compile(src).expect("pre compiles");
    let win = cond_pre::fill_window(w);
    let cand = cond_pre::fill_candidate(w, owner, card, placed);
    pre.eval(&win, &cand)
}

/// A world with `n` seats on a `size`-tile ring at `pos`, owning `owners`.
fn world(_size: i32, pos: &[i32], owners: &[i32]) -> TestWorld {
    let mut w = TestWorld::new(1);
    w.pos = pos.to_vec();
    w.owners = owners.to_vec();
    w.out = vec![false; pos.len()];
    w.exile = vec![false; pos.len()];
    w.money = vec![10_000; pos.len()];
    w
}

// --------------------------------------------------------------- plan state

#[test]
fn plan_fixed_roll_matches_ctx() {
    // Guest `ctx::fixed_roll()` -> `Option<i32>`; the host reader is
    // `CardWorld::fixed_roll` (-1 = unset). CEL binds `null` when unset.
    let mut w = world(12, &[0, 1, 2, 3], &[-1; 12]);
    assert_eq!(w.fixed_roll(), -1, "guest: unset");
    assert!(ask(&w, "plan.fixed_roll == null", 0, "TEST:x", false), "CEL: null");

    w.fixed_roll = Some(10);
    assert_eq!(w.fixed_roll(), 10, "guest: 10");
    assert!(ask(&w, "plan.fixed_roll == 10", 0, "TEST:x", false), "CEL: 10");
    assert!(!ask(&w, "plan.fixed_roll == null", 0, "TEST:x", false));
}

// ---------------------------------------------------------- turn counters

#[test]
fn gains_and_targeted_match_ctx() {
    let mut w = world(12, &[0, 1, 2, 3], &[-1; 12]);
    w.gains = vec![3, 0, 1, 0];
    w.targeted = vec![0, 2, 0, 0];
    // Guest `ctx::gains_this_turn(p)` / `ctx::targeted_count(p)`.
    assert_eq!(w.gains_this_turn(0), 3);
    assert_eq!(w.targeted_count(1), 2);
    assert!(ask(&w, "gains_this_turn(0) == 3", 0, "TEST:x", false));
    assert!(ask(&w, "gains_this_turn(2) == 1", 0, "TEST:x", false));
    assert!(ask(&w, "targeted_count(1) == 2", 0, "TEST:x", false));
    assert!(ask(&w, "targeted_count(0) == 0", 0, "TEST:x", false));
}

// --------------------------------------------------------- instance tile

#[test]
fn card_tile_matches_ctx_self_tile() {
    // Guest `ctx::self_tile()` raw host value: -2 = not placed, -1 = with
    // owner, else the tile. `card.tile` binds the raw value.
    let mut w = world(12, &[0, 1, 2, 3], &[-1; 12]);
    w.field = vec![("MyGO:（乐奈）有趣的女人".to_string(), 5)];
    // The instance (uid 0) sits on tile 5.
    assert_eq!(w.tile_at(0), 5, "guest self_tile raw");
    let cand = cond_pre::fill_candidate(&w, 0, "MyGO:（乐奈）有趣的女人", true);
    assert_eq!(cand.card_tile, Some(5), "snapshot");
    assert!(ask(&w, "card.tile == 5", 0, "MyGO:（乐奈）有趣的女人", true));

    // With its owner: tile -1.
    w.field = vec![("MyGO:（乐奈）有趣的女人".to_string(), -1)];
    assert_eq!(w.tile_at(0), -1);
    assert!(ask(&w, "card.tile == -1", 0, "MyGO:（乐奈）有趣的女人", true));

    // Not placed: the fill leaves `card_tile` None -> binds -2.
    let cand = cond_pre::fill_candidate(&w, 0, "MyGO:（乐奈）有趣的女人", false);
    assert_eq!(cand.card_tile, None);
    assert!(ask(&w, "card.tile == -2", 0, "MyGO:（乐奈）有趣的女人", false));
}

// ------------------------------------------------------------- geometry

/// Reference `ctx::dist` -- the shorter way around the ring.
fn ref_dist(n: i32, a: i32, b: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let d = ((b - a) % n + n) % n;
    d.min(n - d)
}

#[test]
fn dist_matches_ctx() {
    let w = world(12, &[0, 1, 2, 3], &[-1; 12]);
    for (a, b) in [(0, 0), (0, 1), (0, 6), (0, 11), (11, 0), (3, 9)] {
        let d = ref_dist(12, a, b);
        assert!(
            ask(&w, &format!("dist({a}, {b}) == {d}"), 0, "TEST:x", false),
            "dist({a},{b}) should be {d}"
        );
    }
}

/// Reference `ctx::players_on(tile, except).len()` -- present (`!out &&
/// exile == 0`) standers.
fn ref_players_on(w: &TestWorld, tile: i32, except: i32) -> i32 {
    (0..w.pos.len() as i32)
        .filter(|&s| {
            s != except
                && w.player_out(s) == 0
                && w.state_get(s, game_core::state::key::EXILE) == 0
                && w.player_pos(s) == tile
        })
        .count() as i32
}

#[test]
fn players_on_matches_ctx() {
    let mut w = world(12, &[5, 5, 0, 5], &[-1; 12]);
    w.out = vec![false, false, true, false]; // seat 2 out
    for (tile, except) in [(5, 0), (5, -1), (5, 1), (0, -1), (0, 2)] {
        let c = ref_players_on(&w, tile, except);
        assert!(
            ask(&w, &format!("players_on({tile}, {except}) == {c}"), 0, "TEST:x", false),
            "players_on({tile},{except}) should be {c}"
        );
    }
}

/// Reference meet_again `next_dist`.
fn ref_next_dist(w: &TestWorld, p: i32, dir: i32) -> i32 {
    let n = w.tile_count();
    let pos = w.player_pos(p);
    if n <= 0 || pos < 0 {
        return -1;
    }
    let mut best = i32::MAX;
    for o in 0..w.pos.len() as i32 {
        if o == p || w.player_out(o) != 0 {
            continue;
        }
        let q = w.player_pos(o);
        if q < 0 {
            continue;
        }
        let f = if dir >= 0 {
            ((q - pos) % n + n) % n
        } else {
            ((pos - q) % n + n) % n
        };
        if f > 0 && f < best {
            best = f;
        }
    }
    if best == i32::MAX {
        -1
    } else {
        best
    }
}

#[test]
fn next_dist_matches_ctx() {
    let mut w = world(12, &[1, 3, 9, 6], &[-1; 12]);
    for (p, dir) in [(0, 1), (0, -1), (1, 1), (1, -1), (2, 1)] {
        let d = ref_next_dist(&w, p, dir);
        assert!(
            ask(&w, &format!("next_dist({p}, {dir}) == {d}"), 0, "TEST:x", false),
            "next_dist({p},{dir}) should be {d}"
        );
    }
    // Everyone co-located -> no rival at a positive forward distance.
    w.pos = vec![1, 1, 1, 1];
    assert_eq!(ref_next_dist(&w, 0, 1), -1);
    assert!(ask(&w, "next_dist(0, 1) == -1", 0, "TEST:x", false));
}

/// Reference haruhikage `within5` count.
fn ref_others_within(w: &TestWorld, p: i32, radius: i32) -> i32 {
    let pos = w.player_pos(p);
    if pos < 0 {
        return 0;
    }
    (0..w.pos.len() as i32)
        .filter(|&o| {
            if o == p || w.player_out(o) != 0 {
                return false;
            }
            let d = ref_dist(w.tile_count(), pos, w.player_pos(o));
            d > 0 && d <= radius
        })
        .count() as i32
}

#[test]
fn others_within_matches_ctx() {
    let w = world(12, &[0, 2, 5, 11], &[-1; 12]);
    for (p, r) in [(0, 0), (0, 1), (0, 2), (0, 5), (1, 3)] {
        let c = ref_others_within(&w, p, r);
        assert!(
            ask(&w, &format!("others_within({p}, {r}) == {c}"), 0, "TEST:x", false),
            "others_within({p},{r}) should be {c}"
        );
    }
}

/// Reference council_check `near` count.
fn ref_owned_within(w: &TestWorld, p: i32, radius: i32) -> i32 {
    let pos = w.player_pos(p);
    if pos < 0 {
        return 0;
    }
    (0..w.tile_count())
        .filter(|&t| w.tile_owner(t) == p && ref_dist(w.tile_count(), pos, t) <= radius)
        .count() as i32
}

#[test]
fn owned_within_matches_ctx() {
    let w = world(12, &[0, 3, 6, 9], &[0, 0, -1, -1, -1, -1, 0, -1, -1, -1, -1, -1]);
    for (p, r) in [(0, 0), (0, 3), (0, 6), (1, 1), (1, 4)] {
        let c = ref_owned_within(&w, p, r);
        assert!(
            ask(&w, &format!("owned_within({p}, {r}) == {c}"), 0, "TEST:x", false),
            "owned_within({p},{r}) should be {c}"
        );
    }
}

/// Reference repaint `on_path` count (reads the window's `move.roll`).
fn ref_on_path(w: &TestWorld, me: i32, them: i32, roll: i32) -> i32 {
    let steps = roll.abs();
    let n = w.tile_count();
    let pos = w.player_pos(them);
    if n <= 0 || pos < 0 {
        return 0;
    }
    (1..=steps)
        .filter(|i| {
            let t = ((pos + i) % n + n) % n;
            w.tile_owner(t) == me
        })
        .count() as i32
}

/// Set the world's move payload (`Trigger.mv`) -- roll face and direction.
/// The guest-visible values (`move.roll`, `move.dir`) are unchanged: `dir`
/// is `1` forward / `-1` backward (`Dir::as_i32`).
fn set_move(w: &mut TestWorld, roll: Option<i32>, dir: i32) {
    w.trigger.mv = Some(game_rules::TriggerMove {
        kind: game_rules::MoveKind::Walk,
        resolve: true,
        tags: vec![],
        main: true,
        dir: if dir < 0 {
            game_core::engine::rules::Dir::Backward
        } else {
            game_core::engine::rules::Dir::Forward
        },
        from: -1,
        remaining: roll.map(|r| r.abs()).unwrap_or(0),
        total: roll.map(|r| r.abs()).unwrap_or(0),
        roll,
    });
}

#[test]
fn on_path_matches_ctx() {
    let mut w = world(12, &[0, 2, 5, 9], &[-1, -1, -1, -1, 0, -1, -1, 0, -1, -1, -1, -1]);
    set_move(&mut w, Some(5), 1);
    for (me, them) in [(0, 1), (0, 2), (1, 0), (1, 1)] {
        let c = ref_on_path(&w, me, them, 5);
        assert!(
            ask(&w, &format!("on_path({me}, {them}) == {c}"), 0, "TEST:x", false),
            "on_path({me},{them}) should be {c}"
        );
    }
    // No face -> 0.
    set_move(&mut w, None, 1);
    assert!(ask(&w, "on_path(0, 1) == 0", 0, "TEST:x", false));
}

/// Reference misaki_card `between` count (reads the window's move roll/dir).
fn ref_between(w: &TestWorld, p: i32, roll: i32, dir: i32) -> i32 {
    let roll = roll.abs();
    let start = w.player_pos(p);
    let n = w.tile_count();
    if n <= 0 || start < 0 {
        return 0;
    }
    let backward = dir < 0;
    (0..w.pos.len() as i32)
        .filter(|&o| {
            if o == p || w.player_out(o) != 0 {
                return false;
            }
            let q = w.player_pos(o);
            if q < 0 {
                return false;
            }
            let fwd = ((q - start) % n + n) % n;
            let d = if backward { (n - fwd) % n } else { fwd };
            (1..=roll).contains(&d)
        })
        .count() as i32
}

#[test]
fn between_matches_ctx() {
    let mut w = world(12, &[0, 2, 5, 11], &[-1; 12]);
    set_move(&mut w, Some(5), 1);
    for p in 0..4 {
        let c = ref_between(&w, p, 5, 1);
        assert!(
            ask(&w, &format!("between({p}) == {c}"), 0, "TEST:x", false),
            "between({p}) forward should be {c}"
        );
    }
    set_move(&mut w, Some(5), -1);
    for p in 0..4 {
        let c = ref_between(&w, p, 5, -1);
        assert!(
            ask(&w, &format!("between({p}) == {c}"), 0, "TEST:x", false),
            "between({p}) backward should be {c}"
        );
    }
}

// -------------------------------------------------------- pure predicate

#[test]
fn repeated_digits_matches_reference() {
    // No guest reader -- a pure int predicate. Spot-check the C# `RepeatedDigits`
    // bit-twiddle no_breakup's `cant_play` uses.
    for (n, want) in [
        (0, false),
        (5, false),
        (44, true),
        (-444, true),
        (123, false),
        (1223, true),
        (1234, false),
        (100, true),
    ] {
        assert!(
            ask(&w_test(), &format!("repeated_digits({n}) == {want}"), 0, "TEST:x", false),
            "repeated_digits({n}) should be {want}"
        );
    }
}

fn w_test() -> TestWorld {
    world(12, &[0, 1, 2, 3], &[-1; 12])
}