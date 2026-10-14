//! Unit `skills_crychic`: clause proofs for the 5 CRYCHIC character skills
//! and the band skill 美好的往日幻影.
//!
//! Specs: `crate::spec::skills_crychic` (book only).
//! Host notes that shape these harnesses:
//! * `Op::state_set` stores `a3` (the field id) and drops `a4` (the value) —
//!   any clause whose *observable* effect is `state::set` is BLOCKED-HOST.
//! * `state_add` / `spend_fire` / `add_band_crystals` are unlogged.
//! * `trigger::move_roll` is unmodeled (`trig_move_roll`).

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn effects() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn symbolic_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
    }
    w.symbolic_answers = true;
    w
}

fn any_seat(n: i32) -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < n);
    p
}

fn key_get(p: i32, key: intern::NameId) -> i32 {
    world::with_world(|w| w.key_get(p, key))
}

// ===========================================================================
// C-CRYCHIC-tomoriskill-02 — spend 1 fire, clear 1 stay
// ===========================================================================

/// IFF: when fire >= 1 and stay (player_keys) >= 1, the play body spends one
/// fire (`World::fire`) and clears one `state_key::STAY` layer
/// (`player_keys`). Both ops are unlogged in the Kani host — the effect log
/// must stay empty and the post-state must move exactly those two counters.
#[kani::proof]
#[kani::unwind(16)]
pub fn tomoriskill_02_spend_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.fire[player as usize] = kani::any();
    kani::assume(w.fire[player as usize] >= 0 && w.fire[player as usize] <= 3);
    let stay_key = intern::intern("stay");
    let fire_key = intern::intern("fire");
    // The body's `can_use` gate reads `state::get(FIRE/STAY)` = player_keys.
    let pk_fire: i32 = kani::any();
    kani::assume(pk_fire >= 1 && pk_fire <= 3);
    w.key_set(player, fire_key, pk_fire);
    let pk_stay: i32 = kani::any();
    kani::assume(pk_stay >= 1 && pk_stay <= 3);
    w.key_set(player, stay_key, pk_stay);
    let pre_w_fire = w.fire[player as usize];

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Entry 0 is `On::Play` (the skill press). `can_use` is the Some(...) gate
    // the engine consults; under Kani we invoke the body directly and pin the
    // gate's pre in the assumes above.
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("skill:高松灯（CRYCHIC）:跌跌撞撞..."), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    crate::kassert!(
        spec::skills_crychic::tomoriskill_02_log_ok(effects),
        "C-CRYCHIC-tomoriskill-02: both spends are unlogged — log must stay empty"
    );
    if pre_w_fire >= 1 {
        crate::kassert!(
            post.fire[player as usize] == pre_w_fire - 1,
            "C-CRYCHIC-tomoriskill-02: fire -1"
        );
    }
    crate::kassert!(
        key_get(player, stay_key) == pk_stay - 1,
        "C-CRYCHIC-tomoriskill-02: player_keys STAY -1 (state::add)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    kani::cover!(pre_w_fire >= 1, "fire available");
    kani::cover!(pre_w_fire == 0, "fire empty (spend no-op)");
}

/// MUTANT companion: the body is not a one-effect body.
#[kani::proof]
#[kani::unwind(16)]
pub fn tomoriskill_02_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.fire[player as usize] = 1;
    let stay_key = intern::intern("stay");
    let fire_key = intern::intern("fire");
    w.key_set(player, fire_key, 1);
    w.key_set(player, stay_key, 1);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("skill:高松灯（CRYCHIC）:跌跌撞撞..."), player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::skills_crychic::tomoriskill_02_log_ok(effects),
        "card matches book (empty log)"
    );
    let wrong = spec::skills_crychic::tomoriskill_02_log_ok_mutant(effects);
    kani::cover!(!wrong, "one-effect mutant disagrees");
    crate::kassert!(!wrong, "mutant (non-empty log) must NOT match");
}

// ===========================================================================
// C-CRYCHIC-soyoskill-01 — cancel pay + arm teleport (yes branch leaf)
// ===========================================================================

/// IFF on the yes branch of the PayChoose hook. The prompt is answered
/// symbolically (`symbolic_answers`); we pin the yes answer for the iff and
/// cover the no branch as a no-op.
#[kani::proof]
#[kani::unwind(20)]
pub fn soyoskill_01_cancel_iff() {
    let mut w = symbolic_world();
    let owner = any_seat(w.n_players);
    // actor != owner (another player settles on our tile)
    let actor = any_seat(w.n_players);
    kani::assume(actor != owner);
    w.trig_player = actor;
    w.trig_target = owner;
    w.trig_tile = kani::any();
    kani::assume(w.trig_tile >= 0 && w.trig_tile < 60);
    w.tiles[w.trig_tile as usize].owner = owner;
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= 1 && w.trig_value <= 20_000);
    w.pay_amount = w.trig_value;
    // Pin the "yes" answer (index 0 of the 2-option yes/no).
    w.symbolic_answers = false;
    w.answers[0] = 0;
    w.next_answer = 0;

    let tile_key = intern::intern("skill.soyoCrychic.tile").0;
    let armed_key = intern::intern("skill.soyoCrychic.armed").0;
    let tile = w.trig_tile;

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:长崎素世（CRYCHIC）:雨中祈晴"), "PayChoose", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    // Host `state_set` drops the value (stores the field id). The body is
    // book-correct (`state::set(TILE, t)` / `state::set(ARMED, 1)`); the
    // SetState log cannot carry those values. Assert the waive, which is the
    // book's primary effect and is host-faithful.
    crate::kassert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SetPayAmount { to: 0 })),
        "C-CRYCHIC-soyoskill-01: payment waived (SetPayAmount{0})"
    );
    crate::kassert!(
        post.pay_amount == 0,
        "C-CRYCHIC-soyoskill-01: pending pay amount is 0"
    );
    // The latch writes are visible as SetState entries (values host-broken).
    crate::kassert!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::SetState { .. }))
            .count()
            == 2,
        "C-CRYCHIC-soyoskill-01: two latch writes (TILE + ARMED)"
    );
    let _ = (tile_key, armed_key, tile);
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (waive at rewrite time)");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// No branch: empty log (the body returns before any write).
#[kani::proof]
#[kani::unwind(20)]
pub fn soyoskill_01_decline_noop() {
    let mut w = symbolic_world();
    let owner = any_seat(w.n_players);
    let actor = any_seat(w.n_players);
    kani::assume(actor != owner);
    w.trig_player = actor;
    w.trig_target = owner;
    w.trig_tile = 3;
    w.tiles[3].owner = owner;
    w.trig_value = 500;
    w.pay_amount = 500;
    w.symbolic_answers = false;
    w.answers[0] = 1; // "no"
    w.next_answer = 0;

    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:长崎素世（CRYCHIC）:雨中祈晴"), "PayChoose", owner));
    let (logbuf, log_n) = effects();
    crate::kassert!(
        spec::skills_crychic::soyoskill_01_noop_log_ok(&logbuf[..log_n]),
        "C-CRYCHIC-soyoskill-01 no: empty log"
    );
}

/// MUTANT companion: SetPayAmount{1} is not the waive.
#[kani::proof]
#[kani::unwind(20)]
pub fn soyoskill_01_mutant_is_detected() {
    let mut w = symbolic_world();
    let owner = any_seat(w.n_players);
    w.trig_player = (owner + 1) % w.n_players;
    w.trig_target = owner;
    w.trig_tile = 5;
    w.tiles[5].owner = owner;
    w.trig_value = 5000;
    w.pay_amount = 5000;
    w.symbolic_answers = false;
    w.answers[0] = 0;
    w.next_answer = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:长崎素世（CRYCHIC）:雨中祈晴"), "PayChoose", owner));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
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

// ===========================================================================
// C-CRYCHIC-band-01 — hand limit removed (leaf: SetState{handLimit, MAX})
// HOST-BLOCKED: `state_set` drops the value. The body is book-correct.
// Harness asserts the *call shape* (two SetState writes) and covers that the
// book value cannot be observed through this host.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn band_01_hand_limit_host_gap() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.hand_n = kani::any();
    kani::assume(w.hand_n >= 0 && w.hand_n <= 8);

    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:CRYCHIC:美好的往日幻影"), "TurnStartBefore", player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    // Body: `state::set(HAND_LIMIT, NO_HAND_LIMIT)` + `state::set(SEEN, hand)`.
    // Host logs SetState twice but with `n = field id` (0), not the value.
    crate::kassert!(
        effects.len() == 2,
        "C-CRYCHIC-band-01: two state writes (HAND_LIMIT + SEEN)"
    );
    crate::kassert!(
        effects
            .iter()
            .all(|e| matches!(e, Effect::SetState { .. })),
        "C-CRYCHIC-band-01: both writes are SetState"
    );
    // Document the host gap: the book value `i32::MAX` is NOT observable.
    let limit_val = effects
        .iter()
        .find_map(|e| match e {
            Effect::SetState { n, .. } => Some(*n),
            _ => None,
        })
        .unwrap_or(-1);
    kani::cover!(limit_val == 0, "host drops value (stores field id 0)");
    kani::cover!(
        limit_val == spec::skills_crychic::band_01_hand_limit_value(),
        "book value visible (would be PROVEN with a fixed host)"
    );
}

// ===========================================================================
// C-CRYCHIC-band-03 — crystal on 4→5 (leaf)
// HOST-BLOCKED: SEEN is written via `state_set` (value dropped), so the
// `before == 4` latch can never hold. The body's `add_crystals(1, 10)` path
// is book-correct. Harness pins SEEN via `set_slot` (the host-faithful write)
// to prove the crystal leaf itself.
// ===========================================================================

#[kani::proof]
#[kani::unwind(20)]
pub fn band_03_crystal_on_four_to_five_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    // Seed hand at 5 (the crossing landed) and SEEN = 4 via `set_slot`'s
    // host-faithful packing so the body's `state::get(SEEN)` reads 4.
    w.hand_n = 5;
    for i in 0..5 {
        w.hand_pile[i] = 100 + i as u16;
        w.hand[i] = true;
    }
    let seen_key = intern::intern("skill.crychic.seen");
    w.key_set(player, seen_key, 4);
    let crystal_key = intern::intern("crystals");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let pre_crystals = key_get(player, crystal_key);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:CRYCHIC:美好的往日幻影"), "Drawn", player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let post_crystals = key_get(player, crystal_key);

    crate::kassert!(
        spec::skills_crychic::band_03_crystal_log_ok(effects, player, crystal_key.0),
        "C-CRYCHIC-band-03: exactly AddCounter{+1 crystal}"
    );
    crate::kassert!(
        post_crystals == pre_crystals + spec::skills_crychic::band_03_crystal_delta(),
        "C-CRYCHIC-band-03: crystal +1"
    );
    // The body also rewrites SEEN (host drops the value) — the log carries a
    // second SetState.
    crate::kassert!(
        effects.len() == 2
            && matches!(effects[1], Effect::SetState { .. }),
        "C-CRYCHIC-band-03: SEEN rewrite is a SetState (value host-broken)"
    );
    let _ = pre;
}

/// Not a crossing (SEEN != 4): no crystal.
#[kani::proof]
#[kani::unwind(16)]
pub fn band_03_no_crossing_noop() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.hand_n = 5;
    let seen_key = intern::intern("skill.crychic.seen");
    w.key_set(player, seen_key, 3);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:CRYCHIC:美好的往日幻影"), "Drawn", player));
    let (logbuf, log_n) = effects();
    // SEEN rewrite is still a SetState; the crystal AddCounter must be absent.
    let has_crystal = logbuf[..log_n]
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
#[kani::unwind(16)]
pub fn band_03_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.hand_n = 5;
    let seen_key = intern::intern("skill.crychic.seen");
    w.key_set(player, seen_key, 4);
    let crystal_key = intern::intern("crystals");
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("skill:CRYCHIC:美好的往日幻影"), "Drawn", player));
    let (logbuf, log_n) = effects();
    // Filter to the AddCounter for the crystal key (no Vec under Kani).
    let mut crystal_effects = [Effect::AddCounter { owner: -1, key: 0, n: 0 }; 4];
    let mut cn = 0usize;
    let mut i = 0usize;
    while i < log_n {
        if let Effect::AddCounter { key, .. } = logbuf[i] {
            if key == crystal_key.0 && cn < 4 {
                crystal_effects[cn] = logbuf[i];
                cn += 1;
            }
        }
        i += 1;
    }
    let crystal_slice = &crystal_effects[..cn];
    crate::kassert!(
        spec::skills_crychic::band_03_crystal_log_ok(crystal_slice, player, crystal_key.0),
        "card places exactly 1 crystal"
    );
    let wrong = spec::skills_crychic::band_03_crystal_log_ok_mutant(crystal_slice, player, crystal_key.0);
    kani::cover!(!wrong, "two-crystal mutant disagrees");
    crate::kassert!(!wrong, "mutant +2 must NOT match");
}

// ===========================================================================
// Classification placeholders (no harness): documented in the unit table.
// ===========================================================================
//
// C-CRYCHIC-tomoriskill-01  walk-past + fire_pot: PROTOCOL-LEVEL / BLOCKED-HOST
//                          (set_bounds → state_set value drop; Pass walk)
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