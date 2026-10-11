//! Unit `r` / Roselia skills: clause proofs.
//!
//! Cheap fire-pot hooks first (C-R-踏上荆棘之路的觉悟-01, C-R-即使1cm也要前进-01,
//! C-R-慈爱女神-01). Move / settle / counteraction windows are PROTOCOL-LEVEL;
//! the band skill is BLOCKED-HOST on `trig_price`.

use crate::host;
use crate::invoke;
use crate::spec::skills_r as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = kani::any();
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 20);
        w.fire_max[p] = 10;
        w.stay[p] = 0;
    }
    w.trig_player = kani::any();
    kani::assume(w.trig_player >= 0 && w.trig_player < w.n_players);
    w
}

fn effects_of() -> ([Effect; 24], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

// ===========================================================================
// C-R-踏上荆棘之路的觉悟-01 -- sayo CiRCLE / passed fire
// `On::Hook(Pass)` is the Pass hook of `skill:冰川纱夜:踏上荆棘之路的觉悟`.
// ===========================================================================

/// IFF: a CiRCLE pass gains 2 (3 once someone is out).
#[kani::proof]
#[kani::unwind(24)]
pub fn sayo_fire_pass_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    // The body's `circle_gain()` reads `player_out` over `0..player_count`.
    let someone_out: bool = kani::any();
    if someone_out {
        w.out[0] = true;
    } else {
        for p in 0..MAX_PLAYERS {
            w.out[p] = false;
        }
    }
    let n = spec::sayo_circle_gain(someone_out);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:冰川纱夜:踏上荆棘之路的觉悟"), "Hook", "Pass", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(someone_out, "someone out (gain 3)");
    kani::cover!(!someone_out, "nobody out (gain 2)");
    assert!(
        spec::sayo_gain_fire_log_ok(effects, player, n),
        "C-R-踏上荆棘之路的觉悟-01: GainFire 2 or 3"
    );
    assert!(
        post.fire[player as usize] == pre.fire[player as usize] + n,
        "C-R-踏上荆棘之路的觉悟-01: fire +n"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(24)]
pub fn sayo_fire_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
    }
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:冰川纱夜:踏上荆棘之路的觉悟"), "Hook", "Pass", player));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::sayo_circle_gain(false) + 1;
    assert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 3 when nobody is out"
    );
}

// ===========================================================================
// C-R-即使1cm也要前进-01 -- rinko CiRCLE +3
// ===========================================================================

/// IFF: a CiRCLE pass gains exactly 3.
#[kani::proof]
#[kani::unwind(16)]
pub fn rinko_fire_pass_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:白金燐子:即使1cm也要前进"), "Hook", "Pass", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    assert!(
        spec::rinko_gain_fire_log_ok(effects, player),
        "C-R-即使1cm也要前进-01: exactly GainFire 3"
    );
    assert!(
        post.fire[player as usize] == pre.fire[player as usize] + spec::rinko_circle_gain(),
        "C-R-即使1cm也要前进-01: fire +3"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(16)]
pub fn rinko_fire_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:白金燐子:即使1cm也要前进"), "Hook", "Pass", player));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::rinko_circle_gain() + 1;
    assert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 4"
    );
}

// ===========================================================================
// C-R-慈爱女神-01 -- lisa CiRCLE +1
// ===========================================================================

/// IFF: a CiRCLE pass gains exactly 1.
#[kani::proof]
#[kani::unwind(16)]
pub fn lisa_fire_pass_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:今井莉莎:慈爱女神"), "Hook", "Pass", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    assert!(
        spec::lisa_gain_fire_log_ok(effects, player),
        "C-R-慈爱女神-01: exactly GainFire 1"
    );
    assert!(
        post.fire[player as usize] == pre.fire[player as usize] + spec::lisa_circle_gain(),
        "C-R-慈爱女神-01: fire +1"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(16)]
pub fn lisa_fire_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:今井莉莎:慈爱女神"), "Hook", "Pass", player));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::lisa_circle_gain() + 1;
    assert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 2"
    );
}