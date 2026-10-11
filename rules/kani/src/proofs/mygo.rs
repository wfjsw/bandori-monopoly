//! Unit `mygo`: clause proofs.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. `*_mutant_is_detected` reachable-disagreement companion (and
//!    `#[kani::should_panic]` only when it really reports panic-as-expected).
//!
//! Specs come from `crate::spec::mygo` — written from the clause docs only.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared symbolic pre-world (status / play bodies: no trigger overlay).
// Isolation: empty hand / discard / field so stacked hooks cannot fire.
// ---------------------------------------------------------------------------

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < world::TILE_COUNT);
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 8);
        w.stay[p] = kani::any();
        kani::assume(w.stay[p] >= 0 && w.stay[p] <= 4);
        w.out[p] = kani::any();
    }
    w
}

fn living_player(w: &World) -> i32 {
    let p = crate::harness::any_player(w.n_players);
    kani::assume(!w.out[p as usize]);
    p
}

// ===========================================================================
// C-MYGO-rinpuu-01
// 「使自己获得一层[停留]」 — `MyGO:轮符雨` On::Play.
// ===========================================================================

/// Iff: the unmodified play body grants exactly one [停留] layer to the user
/// and nothing else (narration is not the frame; `before_turn_end` is a
/// scheduling stub that logs no effect).
#[kani::proof]
#[kani::unwind(24)]
pub fn rinpuu_01_give_stay_iff() {
    let w = symbolic_play_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play("MyGO:轮符雨", player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    assert!(
        spec::mygo::rinpuu_01_log_ok(effects, player),
        "C-MYGO-rinpuu-01: effect log must be exactly one GiveStay(player, 1)"
    );
    assert!(
        spec::mygo::rinpuu_01_post_ok(&pre, &post, player),
        "C-MYGO-rinpuu-01: post must be pre + 1 stay on the user, frame unchanged"
    );

    kani::cover!(pre.stay[player as usize] == 0, "pre stay was 0");
    kani::cover!(pre.stay[player as usize] > 0, "pre stay was nonzero");
}

/// Mutant companion: a two-layer spec must be distinguishable from the card.
#[kani::proof]
#[kani::unwind(24)]
pub fn rinpuu_01_mutant_is_detected() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("MyGO:轮符雨", player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    // The card does n=1; the mutant wants n=2. They must disagree.
    assert!(
        !spec::mygo::rinpuu_01_log_ok_mutant(effects, player),
        "mutant: two-layer spec must not match the one-layer card"
    );
    let _ = pre;
}

/// `#[kani::should_panic]` twin of the mutant companion.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn rinpuu_01_mutant_two_layers() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("MyGO:轮符雨", player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        spec::mygo::rinpuu_01_log_ok_mutant(effects, player),
        "mutant: card must not match the two-layer spec"
    );
    let _ = pre;
}