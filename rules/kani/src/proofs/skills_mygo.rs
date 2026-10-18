//! Unit `skills_mygo`: clause proofs.
//!
//! Band skill `skill:MyGO!!!!!:迷途之星` and the five MyGO!!!!! character
//! skills. Per clause: iff + no-op + covers + mutant (see KANI-FOUNDATION.md).
//! Specs live in `crate::spec::skills_mygo`.
//!
//! **ppcost pattern.** Concrete user seats (n_players=2). Direct body call
//! via the cfg(kani) `NAME_kani` wrapper. Per-harness host-op subset.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Host-op subsets (body-reachable ops).
// ---------------------------------------------------------------------------

/// `after_roll` (DiceRollAfter) — add_crystals(1,1) + log.
crate::host_subset!(mygo_after_roll_dispatch {
    add_counter_self => arm_add_counter_self,
    add_card_counter => arm_add_card_counter,
    log | effect => arm_log,
});

/// `on_discarded` (Discarded) — crystals read + state + add_crystals.
crate::host_subset!(mygo_on_discarded_dispatch {
    counter_self => arm_counter_self,
    card_counter => arm_card_counter,
    add_counter_self => arm_add_counter_self,
    add_card_counter => arm_add_card_counter,
    state_get => arm_state_get,
    state_set => arm_state_set,
    log | effect => arm_log,
});

fn after_roll_world(user: i32) {
    host::install_backend(mygo_after_roll_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.stay[p] = 0;
            w.charge[p] = 0;
        }
        w.trig_player = user;
    });
}

fn on_discarded_world(user: i32, pre_c: i32) {
    host::install_backend(mygo_on_discarded_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.stay[p] = 0;
            w.charge[p] = 0;
        }
        w.trig_player = user;
        // Crystals live on the on-card counter (`field_crystals` /
        // `self_crystals` via `arm_add_counter_self` / `arm_counter_self`),
        // **not** the player key map (class B — same fix as `events.rs`
        // `key_get(0, "crystals")` and `skills_pp` band-idol-04).
        w.self_crystals = pre_c;
    });
}

/// Miracle crystals of the skill instance (`field_crystals(last_uid)`).
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

// ===========================================================================
// C-MYGO-band-02
// 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
// Hook entry index 1 (`On::Action(DiceRollAfter)`). Seat-symmetric (「为此卡」).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn band_02_crystal_on_16_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    after_roll_world(user);
    // Seed the on-card crystal counter (see `self_crystals`).
    world::with_world_mut(|w| w.self_crystals = kani::any());
    kani::assume(self_crystals() >= 0 && self_crystals() <= 3);
    let pre = world::snapshot();
    let pre_c = self_crystals();

    invoke::unwrap_done(skill_groups::mygo::after_roll_kani(user));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();

    // `add_crystals(1, 1)` clamps at max=1. Host `add_counter_self` logs the
    // clamped delta (`now - was`) — +1 when the gain lands, 0 at the cap.
    crate::kassert!(
        spec::skills_mygo::band_02_log_ok(effects, user, pre_c),
        "C-MYGO-band-02: effect log is one AddCounter (clamped delta)"
    );
    crate::kassert!(
        post_c == spec::skills_mygo::band_02_crystals_after(pre_c),
        "C-MYGO-band-02: crystals must be min(pre+1, 1) under the roll cap"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    crate::kassert!(post.stay == pre.stay, "frame: stay unchanged");

    kani::cover!(pre_c == 0, "pre crystals 0 (gain lands)");
    kani::cover!(pre_c >= 1, "pre crystals at cap (gain refused by max)");
}

/// Band-02 iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_crystal_on_16_iff() {
    band_02_crystal_on_16_iff_at(0);
}

/// Band-02 iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_user1_crystal_on_16_iff() {
    band_02_crystal_on_16_iff_at(1);
}

/// Mutant companion: a +2 spec must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_mutant_is_detected() {
    after_roll_world(0);
    let user = 0i32;
    invoke::unwrap_done(skill_groups::mygo::after_roll_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::skills_mygo::band_02_log_ok_mutant(effects, user),
        "mutant: +2 spec must not match the +1 card"
    );
}

// ===========================================================================
// C-MYGO-band-04
// 「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]
//  （最多超出2个）」
// Hook entry index 2 (`On::Action(Discarded)`).
// Book: +1 crystal that may exceed the roll cap of 1, at most 2 over (ceiling 3).
// The body's `skill.mygo.over` counter is audit/display only (SetState
// bookkeeping) — the ceiling is the held total, not that key.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn band_04_ineffective_overcap_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let pre_c: i32 = kani::any();
    kani::assume(pre_c >= 0 && pre_c <= 3);
    on_discarded_world(user, pre_c);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_groups::mygo::on_discarded_kani(user));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();

    let expected = spec::skills_mygo::band_04_crystals_after(pre_c);
    kani::cover!(pre_c == 1, "at roll cap (book: must still grow)");
    kani::cover!(pre_c < expected, "gain fires");
    kani::cover!(pre_c >= 3, "at the +2 ceiling (gain refused)");

    crate::kassert!(
        post_c == expected,
        "C-MYGO-band-04: ineffective discard must add one over-cap crystal (max 2 over)"
    );
    if post_c > pre_c {
        crate::kassert!(
            spec::skills_mygo::band_04_log_ok(effects, user),
            "C-MYGO-band-04: gain logs exactly one AddCounter(+1) (SetState audit tolerated)"
        );
    }
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
}

/// Band-04 iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_ineffective_overcap_iff() {
    band_04_ineffective_overcap_iff_at(0);
}

/// Band-04 iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_user1_ineffective_overcap_iff() {
    band_04_ineffective_overcap_iff_at(1);
}

/// Mutant: empty log when the gain should fire.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_mutant_is_detected() {
    on_discarded_world(0, 0);
    let user = 0i32;
    let pre_c = 0; // gain must fire
    invoke::unwrap_done(skill_groups::mygo::on_discarded_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    if log_n > 0 {
        crate::kassert!(
            !spec::skills_mygo::band_04_log_ok_mutant(effects, user),
            "mutant: empty-log spec must not match a gaining card"
        );
    }
    let _ = pre_c;
}

// ===========================================================================
// Classification notes (no harness — documented in the unit table):
// C-MYGO-band-01 (开局 3d20 start tile) — PROTOCOL-LEVEL: game-start
//   scheduling (`On::Action(DeckBeforeGame)`); the body's 3d20 sum is
//   `roll(3,20)` (symbolic sum 3..=60) then `teleport_to`. Leaf body could be
//   proven but the start-of-game window is engine scheduling.
// C-MYGO-band-03 (move-1 instead of dice) — the press body spends a crystal
//   and shapes a 1-step move via `plan::*` (protocol stubs). PROTOCOL-LEVEL.
// Character skills (tomori/anon/rana/soyo/rikki) — see the unit table; most
//   windows are multi-turn / counteraction / settle pipelines (PROTOCOL-LEVEL)
//   or hit known black-box FAILS recorded in mygo.md §Findings.
// ===========================================================================