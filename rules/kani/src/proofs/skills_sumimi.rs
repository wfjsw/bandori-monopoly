//! Unit `skills_sumimi`: clause proofs.
//!
//! Character skills (纯田真奈 / 三角初华（Sumimi）) and the band skill
//! 人气偶像组合. Specs live in `crate::spec::skills_sumimi`.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Isolation assumes are named here and `kani::assume`d.
//! Narration (`ctx::log`) is not part of the frame.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-body host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `skill:Sumimi:人气偶像组合` `bend` (PaySplitMul) — crystal read + pay rewrite.
crate::host_subset!(sumimi_bend_dispatch {
    counter_self => arm_counter_self,
    trig_target => arm_trig_target,
    trig_value => arm_trig_value,
    trig_set_pay_amount => arm_trig_set_pay_amount,
});

/// `skill:Sumimi:人气偶像组合` `at_turn_end` (TurnEndBefore) — latch + crystal
/// add/clear. Isolation: the entry guard (`slot('skill.sumimi.got') == 0`)
/// never admits the add half — that LESS is pinned below; the body still
/// declares every op the unmodified `at_turn_end` reaches on either half.
crate::host_subset!(sumimi_turn_end_dispatch {
    state_get => arm_state_get,
    state_set => arm_state_set,
    add_counter_self => arm_add_counter_self,
    counter_self => arm_counter_self,
    log | effect => arm_log,
});

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn log_slice() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn effects_of(logbuf: &[Effect; world::MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

/// Miracle crystals of the running instance (`counter_self` / `add_counter_self`
/// path — `field_crystals(last_uid)`, which falls back to `self_crystals` when
/// no `field[]` slot matches). The player-key map is a different store.
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

fn set_self_crystals(v: i32) {
    world::with_world_mut(|w| w.self_crystals = v);
}

fn crystals_key() -> intern::NameId {
    intern::intern("crystals")
}

// ===========================================================================
// C-SUMIMI-band-02 -- rent bonus +100 per crystal
// 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
// Entry: `On::Filter(PaySplitMul)` on `skill:Sumimi:人气偶像组合`.
// Convention: `crystals()` is the running instance's counter (host: keyed on
// `trig_player`); the body admits when `trigger.target == owner` (money
// flowing to this player — 「向其他人收费」).
// Seat-symmetric (the owner is the collector).
// ===========================================================================

/// In-place 2-seat world for the PaySplitMul rewrite. Crystals live on the
/// running instance (`self_crystals`).
fn band_02_world(owner: i32, crystals: i32, pre_amount: i32) {
    debug_assert!(owner == 0 || owner == 1);
    host::install_backend(sumimi_bend_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = owner;
        w.trig_target = owner;
        w.trig_value = pre_amount;
        w.pay_amount = pre_amount;
        w.out[owner as usize] = false;
    });
    set_self_crystals(crystals);
}

/// Shared iff at a **concrete** owner seat.
fn band_02_rent_bonus_iff_at(owner: i32) {
    debug_assert!(owner == 0 || owner == 1);
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    band_02_world(owner, crystals, pre_amount);

    let pre = world::snapshot();
    let crystals = self_crystals();

    kani::cover!(crystals > 0, "crystals > 0 (bonus applies)");
    kani::cover!(crystals == 0, "crystals == 0 (no-op)");

    // DIRECT body call (ppcost).
    invoke::unwrap_done(skill_groups::sumimi::bend_kani(owner));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let expected = spec::sumimi::band_rent_bonus_amount(pre_amount, crystals);
    crate::kassert!(
        spec::sumimi::band_rent_bonus_log_ok(effects, crystals, expected),
        "C-SUMIMI-band-02: exactly one SetPayAmount to pre + crystals*100 (or empty at 0)"
    );
    crate::kassert!(
        post.pay_amount == expected || crystals <= 0,
        "C-SUMIMI-band-02: post pay_amount must be pre + crystals*100"
    );
    if crystals <= 0 {
        crate::kassert!(post.pay_amount == pre_amount, "no crystals: amount untouched");
    }
    // Frame: money of every seat untouched at rewrite time.
    for p in 0..MAX_PLAYERS {
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
    }
}

/// Iff at owner seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_02_rent_bonus_iff() {
    band_02_rent_bonus_iff_at(0);
}

/// Same iff at owner seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_02_user1_rent_bonus_iff() {
    band_02_rent_bonus_iff_at(1);
}

/// Mutant companion: +100 total is not what the card does.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_02_rent_bonus_mutant_is_detected() {
    let owner = 0i32;
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    band_02_world(owner, 3, pre_amount);
    invoke::unwrap_done(skill_groups::sumimi::bend_kani(owner));
    let post = world::snapshot();
    let wrong = spec::sumimi::band_rent_bonus_amount_mutant(pre_amount, 3);
    kani::cover!(post.pay_amount != wrong, "disagreement live");
    crate::kassert!(
        post.pay_amount != wrong,
        "mutant: bonus is +100 per crystal, not +100 total"
    );
}

// ===========================================================================
// C-SUMIMI-band-01 (clear half) -- 「否则，移除此卡上的所有奇迹水晶。」
// Entry guard: `"actor == owner && slot('skill.sumimi.got') == 0"` — the
// clear half. The add half (「添加1个奇迹水晶」) is gated OUT by that same
// string (it only admits GOT == 0); see the unit table for the FAILS row.
// Seat-symmetric (the owner's turn-end).
// ===========================================================================

/// In-place 2-seat world for the TurnEnd body. Latch `got` + crystals on the
/// running instance.
fn band_01_world(owner: i32, got: i32, crystals: i32) {
    debug_assert!(owner == 0 || owner == 1);
    host::install_backend(sumimi_turn_end_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    let got_key = intern::intern("skill.sumimi.got");
    world::with_world_mut(|w| {
        w.trig_player = owner;
        w.turn_player = owner;
        w.out[owner as usize] = false;
        w.key_set(owner, got_key, got);
    });
    set_self_crystals(crystals);
}

/// Shared iff body at a **concrete** owner seat. Both halves of （1）:
/// 规则书「这回合从别人那里拿到过钱：回合结束时团卡 +1 奇迹水晶；没拿到就清空
/// 团卡上的水晶。」 — GOT set ⇒ add 1 crystal; GOT clear ⇒ remove all crystals.
fn band_01_clear_iff_at(owner: i32) {
    debug_assert!(owner == 0 || owner == 1);
    let got: i32 = kani::any();
    kani::assume(got == 0 || got == 1);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    band_01_world(owner, got, crystals);

    // Source guard is `pre::MINE` ("actor == owner") — both halves live in
    // the body. The old `slot('skill.sumimi.got') == 0` guard only ever
    // admitted the clear branch (FINDINGS C-SUMIMI-band-01).
    let gid = crate::guard_id!("actor == owner");
    let ov = ir_fill::TriggerOverlay::default();
    let admitted = world::with_world(|w| {
        let ctx = ir_fill::fill_for_guard(w, gid, owner, 0, true, -1, &ov);
        crate::guard!(gid, &ctx)
    });
    let pre_crystals = self_crystals();

    kani::cover!(got == 1, "add half (GOT set)");
    kani::cover!(got == 0, "clear half (GOT clear)");
    kani::cover!(got == 0 && pre_crystals > 0, "clear half with crystals");
    kani::cover!(got == 0 && pre_crystals == 0, "clear half with no crystals");

    crate::kassert!(admitted, "guard admits when actor == owner");

    let pre = world::snapshot();
    // DIRECT body call (ppcost).
    invoke::unwrap_done(skill_groups::sumimi::at_turn_end_kani(owner));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    if got != 0 {
        // Book: 「回合结束时团卡 +1 奇迹水晶」 — add half.
        let post_crystals = self_crystals();
        crate::kassert!(
            post_crystals == pre_crystals + 1,
            "C-SUMIMI-band-01 add half: +1 crystal when GOT is set"
        );
    } else {
        // Book: 「没拿到就清空团卡上的水晶」 — clear half.
        crate::kassert!(
            spec::sumimi::band_crystal_clear_ok(effects, owner, crystals_key().0, pre_crystals),
            "C-SUMIMI-band-01 clear: remove every crystal when the latch is clear"
        );
    }
    // Frame: money / pos / charge untouched by either half.
    for p in 0..MAX_PLAYERS {
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
        crate::kassert!(post.charge[p] == pre.charge[p], "frame: charge");
    }
}

/// Clear-half iff at owner seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_01_clear_iff() {
    band_01_clear_iff_at(0);
}

/// Same iff at owner seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_01_user1_clear_iff() {
    band_01_clear_iff_at(1);
}

/// Mutant companion: clear removes one crystal, not all.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_01_clear_mutant_is_detected() {
    let owner = 0i32;
    band_01_world(owner, 0, 3);
    invoke::unwrap_done(skill_groups::sumimi::at_turn_end_kani(owner));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let wrong = spec::sumimi::band_crystal_clear_ok_mutant(effects, owner, crystals_key().0, 3);
    crate::kassert!(!wrong, "mutant: clear removes all crystals, not one");
}

// ===========================================================================
// C-SUMIMI-mana-01 -- pass CiRCLE → +1 [火罐]
// The body's CiRCLE test is `t == ctx::tile_named("CiRCLE")`. The Kani host
// `tile_named` resolves only 「江户川乐器店」 and returns -1 for every other
// name, so a legal "passing CiRCLE" setup cannot be built (BLOCKED-HOST
// `tile_named`). The shared fire spec is proven on `hina_01_pass_circle_iff`
// (whose body is the plain `gain_charge` under the real `is_start` guard).
// This harness covers the no-op half that does NOT need `tile_named`: when
// the Pass body is entered on a non-CiRCLE, non-shop tile the shop branch
// would hit `rent_of` (also unmodeled) — so the only safe entry is the
// decline/no-prompt path. We pin the determined no-prompt structure here
// and leave the CiRCLE branch to the hina twin.
// ===========================================================================

/// C-SUMIMI-mana-02 structure (declined offer): no effects.
/// The accept path is BLOCKED-HOST (`rent_of` for the open 「原本收费」).
/// Pure-formula (no body call).
#[kani::proof]
#[kani::unwind(8)]
pub fn mana_02_decline_is_empty() {
    // Determined part of the offer: a refusal leaves the world untouched.
    // The body's `ask_yes` false path is `return Ok(())` with no effects.
    // We do not invoke the body here (its entry would need `rent_of` for the
    // accept amount); this harness documents the declined-side contract and
    // is a pure-spec cover of `mana_shop_pay_declined`.
    let empty: [Effect; 0] = [];
    crate::kassert!(
        spec::skills_sumimi::mana_shop_pay_declined(&empty),
        "declined offer must be a no-op"
    );
}

// ===========================================================================
// C-SUMIMI-band-03 -- exclusive play gate
// 「你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出。」
// The gate is the playability query on a character exclusive card (not a
// body effect). Covered here as a pure predicate on the character match.
// Pure-formula (no body).
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn band_03_exclusive_gate_predicate() {
    let user_is_owner: bool = kani::any();
    kani::cover!(user_is_owner, "matching character — play allowed");
    kani::cover!(!user_is_owner, "mismatch — play denied");
    crate::kassert!(
        spec::skills_sumimi::band_exclusive_play_allowed(user_is_owner) == user_is_owner,
        "C-SUMIMI-band-03: play iff the character matches the exclusive's owner"
    );
    // Mutant: the dual must be distinguishable.
    crate::kassert!(
        spec::skills_sumimi::band_exclusive_play_allowed(user_is_owner) != !user_is_owner,
        "mutant: gate is identity, not negation"
    );
}

/// Keep the mana skill id greppable next to the BLOCKED-HOST row.
#[allow(dead_code)]
fn mana_id() -> &'static str {
    "skill:纯田真奈:甜甜圈爱好者"
}