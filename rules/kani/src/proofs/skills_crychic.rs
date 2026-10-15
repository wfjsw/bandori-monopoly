//! Unit `skills_crychic`: clause proofs for the CRYCHIC character skills
//! and the band skill 美好的往日幻影.
//!
//! Specs: `crate::spec::skills_crychic` (book only).
//!
//! **ppcost pattern (TRIAGE-LOG `ppcost` FLOOR bisect).** Concrete user seats
//! (`n_players = 2`, user=0/1 for seat-symmetric clauses). Non-seat-symmetric
//! clauses cover the relevant seat combinations concretely and say so. Direct
//! body call via the cfg(kani) `NAME_kani` wrapper. Per-harness host-op subset
//! (`host_subset!`) — undeclared op FAILS loudly. Narration (`ctx::log`) is
//! not part of the frame.
//!
//! Isolation (module-level):
//! * no other skill / card hooks fire in the window (single-body call);
//! * `n_players = 2`, money `0..=10_000`, both seats living unless stated.
//!
//! Host notes:
//! * `Op::log` is narration only (not an effect-log entry).
//! * `Op::state_set` is host-faithful now (value in `a4`, logged as `n`).
//! * `state_add` / `spend_charge` / `add_group_pool` are unlogged.
//! * instance crystals ride `field_crystals(last_uid)` (not the player-key map).
//!
//! Feature gates: band-skill harnesses need `skills-groups` (the module's
//! gate); character-skill harnesses need `skills-personas` (the `*_kani`
//! wrappers live in that crate).
//!
//! Bounds: every harness that calls a body uses `#[kani::unwind(32)]`.
//! Each `unwrap_done` is followed by `kani::cover!(true, "body returned")`.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec::skills_crychic as spec;
use crate::world::{self, Effect};

// ---------------------------------------------------------------------------
// Shared helpers (local to this unit)
// ---------------------------------------------------------------------------

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

fn key_get(p: i32, key: intern::NameId) -> i32 {
    world::with_world(|w| w.key_get(p, key))
}

/// Miracle crystals of the running instance (`add_counter_self` path —
/// `field_crystals(last_uid)`, which falls back to `self_crystals` when no
/// `field[]` slot matches).
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

/// Sparse 2-seat play world on `dispatch`, then `f` for the skill-specific
/// fields. In-place only — no `reset_world` full-struct copy.
fn sparse2(
    dispatch: card_sdk::kani_host::Backend,
    user: i32,
    f: impl FnOnce(&mut world::World),
) {
    host::install_backend(dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        f(w);
    });
}

// ===========================================================================
// C-CRYCHIC-tomoriskill-02 — spend 1 fire, clear 1 stay
// `skill:高松灯（CRYCHIC）:跌跌撞撞...` use_skill. Seat-symmetric.
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod tomori_spend {
    use super::{effects_of, key_get, sparse2};
    use crate::intern;
    use crate::invoke;
    use crate::spec::skills_crychic as spec;
    use crate::world::{self, Effect};

    /// Unmodified `use_skill` reaches: raise (ChargePaying window), spend_charge,
    /// state_add(STAY, -1), log.
    crate::host_subset!(tomori_spend_dispatch {
        raise => arm_raise,
        spend_charge => arm_spend_charge,
        state_add | inc_slot => arm_state_add,
        log | effect => arm_log,
    });

    /// Shared iff at a **concrete** user seat. Fire store and STAY counter are
    /// symbolic so both the spend-yes and spend-no early-out are reachable.
    fn tomoriskill_02_iff_at(user: i32) {
        debug_assert!(user == 0 || user == 1);
        let charge: i32 = kani::any();
        kani::assume(fire >= 0 && fire <= 3);
        let stay: i32 = kani::any();
        kani::assume(stay >= 1 && stay <= 3);
        let stay_key = intern::intern("stay");
        sparse2(tomori_spend_dispatch, user, |w| {
            w.charge[user as usize] = fire;
            // The body's `state::add(STAY, -1)` writes the keyed store.
            w.key_set(user, stay_key, stay);
        });

        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::tomori_crychic::use_skill_kani(user));
        kani::cover!(true, "body returned");
        let post = world::snapshot();
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];

        crate::kassert!(
            spec::tomoriskill_02_log_ok(effects),
            "C-CRYCHIC-tomoriskill-02: both spends are unlogged — log must stay empty"
        );
        kani::cover!(fire >= 1, "fire available (spend lands)");
        kani::cover!(fire == 0, "fire empty (spend no-op, early-out)");

        if fire >= 1 {
            crate::kassert!(
                post.charge[user as usize] == fire - 1,
                "C-CRYCHIC-tomoriskill-02: fire -1"
            );
            crate::kassert!(
                key_get(user, stay_key) == stay - 1,
                "C-CRYCHIC-tomoriskill-02: player_keys STAY -1 (state::add)"
            );
        } else {
            // Early-out: neither store moves.
            crate::kassert!(
                post.charge[user as usize] == fire,
                "C-CRYCHIC-tomoriskill-02: fire unchanged when spend is refused"
            );
            crate::kassert!(
                key_get(user, stay_key) == stay,
                "C-CRYCHIC-tomoriskill-02: STAY unchanged when spend is refused"
            );
        }
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    }

    /// Spend iff at seat 0.
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn tomoriskill_02_spend_iff() {
        tomoriskill_02_iff_at(0);
    }

    /// Spend iff at seat 1.
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn tomoriskill_02_user1_spend_iff() {
        tomoriskill_02_iff_at(1);
    }

    /// MUTANT companion: the body is not a one-effect body (empty log).
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn tomoriskill_02_mutant_is_detected() {
        let stay_key = intern::intern("stay");
        sparse2(tomori_spend_dispatch, 0, |w| {
            w.charge[0] = 1;
            w.key_set(0, stay_key, 1);
        });
        let user = 0i32;
        invoke::unwrap_done(skill_personas::tomori_crychic::use_skill_kani(user));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        crate::kassert!(
            spec::tomoriskill_02_log_ok(effects),
            "card matches book (empty log)"
        );
        let wrong = spec::tomoriskill_02_log_ok_mutant(effects);
        kani::cover!(!wrong, "one-effect mutant disagrees");
        crate::kassert!(!wrong, "mutant (non-empty log) must NOT match");
    }

}

#[cfg(feature = "skills-personas")]
pub use tomori_spend::*;

// ===========================================================================
// C-CRYCHIC-soyoskill-01 — cancel pay + arm teleport (yes branch leaf)
// `skill:长崎素世（CRYCHIC）:雨中祈晴` on_pay.
// NOT seat-symmetric: the hook needs `actor != owner` and `tile.owner == owner`.
// Concrete seat combos in `n_players = 2`: (owner=0, actor=1) and (owner=1, actor=0).
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod soyo_cancel {
    use super::{effects_of, sparse2};
    use crate::intern;
    use crate::invoke;
    use crate::spec::skills_crychic as spec;
    use crate::world::{self, Effect};

    /// Unmodified `on_pay` reaches: trig_tile, ask (yes/no), trig_set_pay_amount,
    /// state_set (TILE + ARMED), log.
    crate::host_subset!(soyo_cancel_dispatch {
        trig_tile => arm_trig_tile,
        ask => arm_ask,
        opt_str | opt_int | opt_tile => arm_opt,
        trig_set_pay_amount => arm_trig_set_pay_amount,
        state_set => arm_state_set,
        log | effect => arm_log,
    });

    /// Pending pay on `owner`'s tile by `actor`. `answer` is the yes/no pick
    /// (0 = yes waive, 1 = no decline). Concrete seats.
    fn soyo_world(owner: i32, actor: i32, answer: i32) -> (i32, i32) {
        debug_assert!(owner == 0 || owner == 1);
        debug_assert!(actor == 0 || actor == 1);
        debug_assert!(actor != owner);
        let tile: i32 = kani::any();
        kani::assume(tile >= 0 && tile < 60);
        let value: i32 = kani::any();
        kani::assume(value >= 1 && value <= 20_000);
        sparse2(soyo_cancel_dispatch, owner, |w| {
            w.trig_player = actor;
            w.trig_target = owner;
            w.trig_tile = tile;
            w.tiles[tile as usize].owner = owner;
            w.trig_value = value;
            w.pay_amount = value;
            // Pin the yes/no: ask_yes does not register `opt_n`, so the
            // symbolic path would always answer 0. Pin instead.
            w.symbolic_answers = false;
            w.answers[0] = answer;
            w.next_answer = 0;
        });
        (tile, value)
    }

    /// Shared yes-branch iff at a **concrete** (owner, actor) pair.
    fn soyoskill_01_iff_at(owner: i32, actor: i32) {
        let (tile, _value) = soyo_world(owner, actor, 0);
        let tile_key = intern::intern("skill.soyoCrychic.tile").0;
        let armed_key = intern::intern("skill.soyoCrychic.armed").0;

        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::soyo_crychic::on_pay_kani(owner));
        kani::cover!(true, "body returned");
        let post = world::snapshot();
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];

        // Host-faithful `state_set`: values are logged. Book-exact three-effect
        // log: waive + TILE write + ARMED latch.
        crate::kassert!(
            spec::soyoskill_01_log_ok(effects, tile_key, armed_key, tile),
            "C-CRYCHIC-soyoskill-01: SetPayAmount{0} + SetState(TILE, t) + SetState(ARMED, 1)"
        );
        crate::kassert!(
            post.pay_amount == 0,
            "C-CRYCHIC-soyoskill-01: pending pay amount is 0"
        );
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (waive at rewrite time)");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    }

    /// Yes-branch iff at (owner=0, actor=1) — the two combos cover every
    /// ordered distinct pair of the 2-seat world.
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn soyoskill_01_cancel_iff() {
        soyoskill_01_iff_at(0, 1);
    }

    /// Yes-branch iff at (owner=1, actor=0).
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn soyoskill_01_user1_cancel_iff() {
        soyoskill_01_iff_at(1, 0);
    }

    /// No branch: empty log (the body returns before any write).
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn soyoskill_01_decline_noop() {
        soyo_world(0, 1, 1);
        let owner = 0i32;
        invoke::unwrap_done(skill_personas::soyo_crychic::on_pay_kani(owner));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        crate::kassert!(
            spec::soyoskill_01_noop_log_ok(&ebuf[..log_n]),
            "C-CRYCHIC-soyoskill-01 no: empty log"
        );
    }

    /// MUTANT companion: SetPayAmount{1} is not the waive.
    #[kani::proof]
    #[kani::unwind(32)]
    pub fn soyoskill_01_mutant_is_detected() {
        soyo_world(0, 1, 0);
        let owner = 0i32;
        invoke::unwrap_done(skill_personas::soyo_crychic::on_pay_kani(owner));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        crate::kassert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SetPayAmount { to: 0 })),
            "card waives to 0"
        );
        let wrong = effects
            .iter()
            .any(|e| matches!(e, Effect::SetPayAmount { to: 1 }));
        kani::cover!(!wrong, "SetPayAmount{1} mutant disagrees");
        crate::kassert!(!wrong, "mutant waive-to-1 must NOT match");
    }
}

#[cfg(feature = "skills-personas")]
pub use soyo_cancel::*;

// ===========================================================================
// C-CRYCHIC-band-01 — hand limit removed (leaf: SetState{handLimit, MAX})
// `skill:CRYCHIC:美好的往日幻影` at_turn_start. Seat-symmetric.
// `Op::state_set` is host-faithful now — the book value IS observable.
// ===========================================================================

/// Unmodified `at_turn_start` reaches: state_set (HAND_LIMIT + SEEN), hand_size.
crate::host_subset!(band_turn_start_dispatch {
    state_set => arm_state_set,
    hand_size => arm_hand_size,
});

/// Shared iff at a **concrete** user seat.
fn band_01_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let hand: i32 = kani::any();
    kani::assume(hand >= 0 && hand <= 8);
    sparse2(band_turn_start_dispatch, user, |w| {
        w.hand_n = hand as u8;
    });

    let limit_key = intern::intern("handLimit").0;
    let seen_key = intern::intern("skill.crychic.seen").0;
    invoke::unwrap_done(skill_groups::crychic::at_turn_start_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::band_01_log_ok(
            effects,
            user,
            seen_key,
            limit_key,
            hand
        ),
        "C-CRYCHIC-band-01: SetState{handLimit, i32::MAX} + SetState{SEEN, hand}"
    );
    // Book value is observable through the fixed host.
    crate::kassert!(
        key_get(user, intern::NameId(limit_key)) == spec::band_01_hand_limit_value(),
        "C-CRYCHIC-band-01: handLimit is i32::MAX (no cap)"
    );
    kani::cover!(hand == 0, "empty hand at turn start");
    kani::cover!(hand >= 1, "non-empty hand at turn start");
}

/// Hand-limit leaf iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_01_hand_limit_iff() {
    band_01_iff_at(0);
}

/// Hand-limit leaf iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_01_user1_hand_limit_iff() {
    band_01_iff_at(1);
}

// ===========================================================================
// C-CRYCHIC-band-03 — crystal on 4→5 (leaf)
// `skill:CRYCHIC:美好的往日幻影` on_drawn. Seat-symmetric.
// Instance crystals ride `field_crystals(last_uid)` / `self_crystals`.
// ===========================================================================

/// Unmodified `on_drawn` reaches: state_get, hand_size, state_set, add_counter_self, log.
crate::host_subset!(band_on_drawn_dispatch {
    state_get => arm_state_get,
    hand_size => arm_hand_size,
    state_set => arm_state_set,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
});

/// Shared iff at a **concrete** user seat with `seen` pinned and hand at `hand`.
fn band_03_crossing_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let seen_key = intern::intern("skill.crychic.seen");
    let crystal_key = intern::intern("crystals");
    sparse2(band_on_drawn_dispatch, user, |w| {
        // Seed hand at 5 (the crossing landed) and SEEN = 4 so the body's
        // `state::get(SEEN)` reads 4 and `hand_size` reads >= 5.
        w.hand_n = 5;
        w.key_set(user, seen_key, 4);
    });

    let pre_crystals = self_crystals();
    invoke::unwrap_done(skill_groups::crychic::on_drawn_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let post_crystals = self_crystals();

    // Filter to the AddCounter for the crystal key (the body also rewrites
    // SEEN via `state_set`, which is a second effect).
    let mut crystal_effects = [Effect::AddCounter { owner: -1, key: 0, n: 0 }; 4];
    let mut cn = 0usize;
    let mut i = 0usize;
    while i < log_n {
        if let Effect::AddCounter { key, .. } = effects[i] {
            if key == crystal_key.0 && cn < 4 {
                crystal_effects[cn] = effects[i];
                cn += 1;
            }
        }
        i += 1;
    }
    crate::kassert!(
        spec::band_03_crystal_log_ok(&crystal_effects[..cn], user, crystal_key.0),
        "C-CRYCHIC-band-03: exactly AddCounter{+1 crystal}"
    );
    crate::kassert!(
        post_crystals == pre_crystals + spec::band_03_crystal_delta(),
        "C-CRYCHIC-band-03: crystal +1"
    );
    // The body also rewrites SEEN (host-faithful now — value is the new hand).
    crate::kassert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SetState { player: p, .. } if *p == user)),
        "C-CRYCHIC-band-03: SEEN rewrite is a SetState"
    );
    crate::kassert!(
        key_get(user, seen_key) == 5,
        "C-CRYCHIC-band-03: SEEN rewritten to the new hand size"
    );
}

/// Crossing iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_03_crystal_on_four_to_five_iff() {
    band_03_crossing_iff_at(0);
}

/// Crossing iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_03_user1_crystal_on_four_to_five_iff() {
    band_03_crossing_iff_at(1);
}

/// Not a crossing (SEEN != 4): no crystal.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_03_no_crossing_noop() {
    let seen_key = intern::intern("skill.crychic.seen");
    sparse2(band_on_drawn_dispatch, 0, |w| {
        w.hand_n = 5;
        w.key_set(0, seen_key, 3);
    });
    let user = 0i32;
    invoke::unwrap_done(skill_groups::crychic::on_drawn_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // SEEN rewrite is still a SetState; the crystal AddCounter must be absent.
    let has_crystal = effects
        .iter()
        .any(|e| matches!(e, Effect::AddCounter { .. }));
    crate::kassert!(
        !has_crystal,
        "C-CRYCHIC-band-03: no crystal when SEEN != 4"
    );
    kani::cover!(true, "no-crossing path");
}

/// MUTANT companion: two crystals is not what the body does.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_03_mutant_is_detected() {
    let seen_key = intern::intern("skill.crychic.seen");
    let crystal_key = intern::intern("crystals");
    sparse2(band_on_drawn_dispatch, 0, |w| {
        w.hand_n = 5;
        w.key_set(0, seen_key, 4);
    });
    let user = 0i32;
    invoke::unwrap_done(skill_groups::crychic::on_drawn_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // Filter to the AddCounter for the crystal key.
    let mut crystal_effects = [Effect::AddCounter { owner: -1, key: 0, n: 0 }; 4];
    let mut cn = 0usize;
    let mut i = 0usize;
    while i < log_n {
        if let Effect::AddCounter { key, .. } = effects[i] {
            if key == crystal_key.0 && cn < 4 {
                crystal_effects[cn] = effects[i];
                cn += 1;
            }
        }
        i += 1;
    }
    let crystal_slice = &crystal_effects[..cn];
    crate::kassert!(
        spec::band_03_crystal_log_ok(crystal_slice, user, crystal_key.0),
        "card places exactly 1 crystal"
    );
    let wrong = spec::band_03_crystal_log_ok_mutant(crystal_slice, user, crystal_key.0);
    kani::cover!(!wrong, "two-crystal mutant disagrees");
    crate::kassert!(!wrong, "mutant +2 must NOT match");
}

// ===========================================================================
// Classification placeholders (no harness): documented in the unit table.
// ===========================================================================
//
// C-CRYCHIC-tomoriskill-01  walk-past + fire_pot: PROTOCOL-LEVEL / BLOCKED-HOST
//                          (set_bounds → state_set; Pass walk)
// C-CRYCHIC-tomoriskill-03  BLOCKED-HOST (`trig_move_roll`) + PROTOCOL-LEVEL
// C-CRYCHIC-rikiskill-01    BLOCKED-HOST (`trig_move_roll`)
// C-CRYCHIC-sakiskill-01    BLOCKED-HOST (fire_pot / set_bounds) — cap (1,2)
// C-CRYCHIC-sakiskill-02/03 PROTOCOL-LEVEL (within-5 step; plan stubs)
// C-CRYCHIC-sakiskill-04    PROTOCOL-LEVEL (band-skill swap)
// C-CRYCHIC-mutsumiskill-01 BLOCKED-HOST (fire_pot)
// C-CRYCHIC-mutsumiskill-02 BLOCKED-HOST (`trig_move_roll`)
// C-CRYCHIC-mutsumiskill-03 PROTOCOL-LEVEL (band-skill swap)
// C-CRYCHIC-band-01b        FAILS(LESS): no play gate body (see unit table)
// C-CRYCHIC-band-02         PROTOCOL-LEVEL (multi-zone transform); cash-out
//                           leaf is `spec::band_02_cashout_*` when crystals>0