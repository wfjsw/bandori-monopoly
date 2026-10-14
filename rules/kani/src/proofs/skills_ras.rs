//! Unit `skills_ras`: clause proofs.
//!
//! Character skills (RAISE A SUILEN) + band skill UNSTOPPABLE.
//! Classification lives in `docs/rulebook/spec/kani/ras.md`.
//!
//! Isolation: fire-pot pass gains assume `fire + 1 <= fire_max` so the
//! (unmodeled) cap does not bind. The printed cap itself is BLOCKED-HOST.

use crate::host;
use crate::spec::skills_ras as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

/// Call an `On` body without the registry string scan (memcmp cliff).
fn body_of(on: &card_sdk::On) -> fn(i32) -> card_sdk::Asked {
    match on {
        card_sdk::On::Play(_, _, f)
        | card_sdk::On::Counteract(_, _, _, f)
        | card_sdk::On::Hook(_, _, _, f)
        | card_sdk::On::Gate(_, _, _, f)
        | card_sdk::On::RollPlan(_, _, f)
        | card_sdk::On::AtEnd(_, _, f)
        | card_sdk::On::Settle(_, _, f)
        | card_sdk::On::Message(_, _, _, f) => *f,
    }
}

fn unwrap_done(r: card_sdk::Asked) {
    match r {
        Ok(()) => {}
        Err(_) => panic!("unexpected EXIT_NEED_INPUT"),
    }
}

fn symbolic_fire_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 2);
        // default model cap; skill declare_cap is BLOCKED-HOST
        w.fire_max[p] = 3;
        kani::assume(w.fire[p] + 1 <= w.fire_max[p]);
    }
    w
}

/// C-RAS-masking-01 leaf: `skill:佐藤益木` `on_pass` requests +1 [火罐].
/// The tile gate (`白雪学园` / `银河拉面馆`) is the hook's CEL pre and is not
/// re-checked here — this harness proves the leaf the body runs once admitted.
#[kani::proof]
#[kani::unwind(16)]
pub fn masking_01_fire_gain_leaf() {
    let w = symbolic_fire_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    // Hook entry 2 is `on_pass` (see `skill:佐藤益木` registration order).
    unwrap_done(body_of(
        &skill_characters::sato_red::SATO_RED.on[2],
    )(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    assert!(
        spec::fire_pass_log_ok(effects, player),
        "C-RAS-masking-01: leaf is exactly one +1 GainFire"
    );
    assert!(
        post.fire[player as usize] == pre.fire[player as usize] + spec::fire_pass_gain(),
        "C-RAS-masking-01: fire is pre+1"
    );
    assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: positions unchanged");
}

/// C-RAS-rio-01 leaf: `skill:和奏瑞依` `on_pass` requests +1 [火罐].
#[kani::proof]
#[kani::unwind(16)]
pub fn rio_01_fire_gain_leaf() {
    let w = symbolic_fire_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    // Hook entry 1 is `on_pass` (see `skill:和奏瑞依` registration order).
    unwrap_done(body_of(
        &skill_characters::raise_effort::RAISE_EFFORT.on[1],
    )(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    assert!(
        spec::fire_pass_log_ok(effects, player),
        "C-RAS-rio-01: leaf is exactly one +1 GainFire"
    );
    assert!(
        post.fire[player as usize] == pre.fire[player as usize] + spec::fire_pass_gain(),
        "C-RAS-rio-01: fire is pre+1"
    );
    assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
}

/// Mutant companion for the fire-pass leaf (+2 ≠ +1).
#[kani::proof]
#[kani::unwind(16)]
pub fn fire_pass_gain_mutant_is_detected() {
    let w = symbolic_fire_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);

    world::reset_world(w);
    unwrap_done(body_of(
        &skill_characters::sato_red::SATO_RED.on[2],
    )(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::fire_pass_log_ok_mutant(effects, player),
        "mutant: card must not match the +2 GainFire spec"
    );
}