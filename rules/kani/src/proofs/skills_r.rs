//! Unit `r` / Roselia skills: clause proofs.
//!
//! Cheap fire-pot hooks first (C-R-踏上荆棘之路的觉悟-01, C-R-即使1cm也要前进-01,
//! C-R-慈爱女神-01). Move / settle / counteraction windows are PROTOCOL-LEVEL;
//! the band skill is BLOCKED-HOST on `trig_price`.
//!
//! Concrete-user pattern (TRIAGE-LOG `ppcost`): symbolic player index is a
//! BMC cliff. `n_players = 2`; iff harnesses are `*_iff_at(user)` at seats
//! 0/1 (clause is seat-symmetric 「自己的」 / `actor == owner`). Direct
//! `*_kani` body calls. Per-body host-op subset (outside → loud FAIL).

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::invoke;
use crate::spec::skills_r as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// Ops the sayo/rinko/lisa Pass hooks reach (`gain_charge`, `log`, `player_*`).
crate::host_subset!(skills_r_dispatch {
    log | effect => arm_log,
    gain_charge => arm_gain_charge,
    fire => arm_fire,
    charge_max => arm_charge_max,
    spend_charge => arm_spend_charge,
    player_count => arm_player_count,
    player_out => arm_player_out,
    player_pos => arm_player_pos,
    tile_named => arm_tile_named,
    tile_count => arm_tile_count,
    in_group => arm_in_band,
    persona_is => arm_character_is,
    raise => arm_raise,
    gate => arm_gate,
});

/// Sparse 2-seat world + fire caps. `user` ∈ {0, 1} concrete.
fn skills_r_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(skills_r_dispatch);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        money_lo: 0,
        money_hi: 10_000,
        fans_lo: 0,
        fans_hi: 0,
        resonance_in_hand: false,
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        for p in 0..MAX_PLAYERS {
            w.charge_max[p] = 10;
            w.stay[p] = 0;
        }
    });
}

fn effects_of() -> ([Effect; 24], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

// ===========================================================================
// C-R-踏上荆棘之路的觉悟-01 -- sayo CiRCLE / passed fire
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost).
fn sayo_fire_pass_iff_at(user: i32) {
    let user = user;
    skills_r_world(user);
    // The body's `circle_gain()` reads `player_out` over `0..player_count`.
    let someone_out: bool = kani::any();
    world::with_world_mut(|w| {
        if someone_out {
            w.out[0] = true;
        } else {
            for p in 0..MAX_PLAYERS {
                w.out[p] = false;
            }
        }
    });
    let n = spec::sayo_circle_gain(someone_out);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_personas::sayo_thorns::on_pass_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(someone_out, "someone out (gain 3)");
    kani::cover!(!someone_out, "nobody out (gain 2)");
    crate::kassert!(
        spec::sayo_gain_charge_log_ok(effects, user, n),
        "C-R-踏上荆棘之路的觉悟-01: GainFire 2 or 3"
    );
    crate::kassert!(
        post.charge[user as usize] == pre.charge[user as usize] + n,
        "C-R-踏上荆棘之路的觉悟-01: fire +n"
    );
}

#[kani::proof]
#[kani::unwind(24)]
pub fn sayo_fire_pass_iff() {
    sayo_fire_pass_iff_at(0);
}

#[kani::proof]
#[kani::unwind(24)]
pub fn sayo_fire_user1_pass_iff() {
    sayo_fire_pass_iff_at(1);
}

/// MUTANT companion (concrete user=0).
#[kani::proof]
#[kani::unwind(24)]
pub fn sayo_fire_mutant_is_detected() {
    let user = 0i32;
    skills_r_world(user);
    world::with_world_mut(|w| {
        for p in 0..MAX_PLAYERS {
            w.out[p] = false;
        }
    });
    invoke::unwrap_done(skill_personas::sayo_thorns::on_pass_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::sayo_circle_gain(false) + 1;
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 3 when nobody is out"
    );
}

// ===========================================================================
// C-R-即使1cm也要前进-01 -- rinko CiRCLE +3
// ===========================================================================

fn rinko_fire_pass_iff_at(user: i32) {
    let user = user;
    skills_r_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_personas::rinko_1cm::on_pass_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    crate::kassert!(
        spec::rinko_gain_charge_log_ok(effects, user),
        "C-R-即使1cm也要前进-01: exactly GainFire 3"
    );
    crate::kassert!(
        post.charge[user as usize] == pre.charge[user as usize] + spec::rinko_circle_gain(),
        "C-R-即使1cm也要前进-01: fire +3"
    );
}

#[kani::proof]
#[kani::unwind(16)]
pub fn rinko_fire_pass_iff() {
    rinko_fire_pass_iff_at(0);
}

#[kani::proof]
#[kani::unwind(16)]
pub fn rinko_fire_user1_pass_iff() {
    rinko_fire_pass_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(16)]
pub fn rinko_fire_mutant_is_detected() {
    let user = 0i32;
    skills_r_world(user);
    invoke::unwrap_done(skill_personas::rinko_1cm::on_pass_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::rinko_circle_gain() + 1;
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 4"
    );
}

// ===========================================================================
// C-R-慈爱女神-01 -- lisa CiRCLE +1
// ===========================================================================

fn lisa_fire_pass_iff_at(user: i32) {
    let user = user;
    skills_r_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_personas::lisa_goddess::on_pass_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    crate::kassert!(
        spec::lisa_gain_charge_log_ok(effects, user),
        "C-R-慈爱女神-01: exactly GainFire 1"
    );
    crate::kassert!(
        post.charge[user as usize] == pre.charge[user as usize] + spec::lisa_circle_gain(),
        "C-R-慈爱女神-01: fire +1"
    );
}

#[kani::proof]
#[kani::unwind(16)]
pub fn lisa_fire_pass_iff() {
    lisa_fire_pass_iff_at(0);
}

#[kani::proof]
#[kani::unwind(16)]
pub fn lisa_fire_user1_pass_iff() {
    lisa_fire_pass_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(16)]
pub fn lisa_fire_mutant_is_detected() {
    let user = 0i32;
    skills_r_world(user);
    invoke::unwrap_done(skill_personas::lisa_goddess::on_pass_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let wrong = spec::lisa_circle_gain() + 1;
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GainFire { n, .. }) if *n == wrong),
        "mutant: gain is not 2"
    );
}