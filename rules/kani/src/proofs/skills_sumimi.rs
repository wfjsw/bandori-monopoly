//! Unit `skills_sumimi`: clause proofs.
//!
//! Character skills (纯田真奈 / 三角初华（Sumimi）) and the band skill
//! 人气偶像组合. Specs live in `crate::spec::skills_sumimi`.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const BAND: usize = crate::card_id!("skill:Sumimi:人气偶像组合");
const MANA: &str = "skill:纯田真奈:甜甜圈爱好者";

fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 8);
        w.fire_max[p] = 2;
        w.out[p] = false;
    }
    w
}

fn log_slice() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn effects_of(logbuf: &[Effect; 24], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

// ===========================================================================
// C-SUMIMI-band-02 -- rent bonus +100 per crystal
// 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
// Entry: `On::Hook(PayMul)` on `skill:Sumimi:人气偶像组合`.
// Convention: `crystals()` is the running instance's counter (host: keyed on
// `trig_player`); the body admits when `trigger.target == owner` (money
// flowing to this player — 「向其他人收费」).
// ===========================================================================

/// C-SUMIMI-band-02 iff: rent amount += crystals * 100.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_02_rent_bonus_iff() {
    let mut w = base_world();
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    // The skill owner is the collector; crystals live on that instance.
    w.trig_player = owner;
    w.trig_target = owner;
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    w.pay_amount = pre_amount;
    w.trig_value = pre_amount;

    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    let ckey = intern::intern("crystals");
    w.key_set(owner, ckey, crystals);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let crystals = world::with_world(|ww| ww.key_get(owner, ckey));

    kani::cover!(crystals > 0, "crystals > 0 (bonus applies)");
    kani::cover!(crystals == 0, "crystals == 0 (no-op)");

    invoke::unwrap_done(invoke::hook_only_idx(BAND, "PayMul", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let expected = spec::sumimi::band_rent_bonus_amount(pre_amount, crystals);
    assert!(
        spec::sumimi::band_rent_bonus_log_ok(effects, crystals, expected),
        "C-SUMIMI-band-02: exactly one SetPayAmount to pre + crystals*100 (or empty at 0)"
    );
    assert!(
        post.pay_amount == expected || crystals <= 0,
        "C-SUMIMI-band-02: post pay_amount must be pre + crystals*100"
    );
    if crystals <= 0 {
        assert!(post.pay_amount == pre_amount, "no crystals: amount untouched");
    }
    // Frame: money of every seat untouched at rewrite time.
    for p in 0..MAX_PLAYERS {
        assert!(post.money[p] == pre.money[p], "frame: money");
    }
}

#[kani::proof]
#[kani::unwind(24)]
pub fn band_02_rent_bonus_mutant_is_detected() {
    let mut w = base_world();
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    w.trig_player = owner;
    w.trig_target = owner;
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    w.pay_amount = pre_amount;
    w.trig_value = pre_amount;
    let ckey = intern::intern("crystals");
    w.key_set(owner, ckey, 3);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(BAND, "PayMul", owner));
    let post = world::snapshot();
    let wrong = spec::sumimi::band_rent_bonus_amount_mutant(pre_amount, 3);
    kani::cover!(post.pay_amount != wrong, "disagreement live");
    assert!(
        post.pay_amount != wrong,
        "mutant: bonus is +100 per crystal, not +100 total"
    );
}

// ===========================================================================
// C-SUMIMI-band-01 (clear half) -- 「否则，移除此卡上的所有奇迹水晶。」
// Entry guard: `"actor == owner && slot('skill.sumimi.got') == 0"` — the
// clear half. The add half (「添加1个奇迹水晶」) is gated OUT by that same
// string (it only admits GOT == 0); see the unit table for the FAILS row.
// ===========================================================================

/// C-SUMIMI-band-01 clear half + add-half guard finding.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_01_clear_iff() {
    let mut w = base_world();
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    w.trig_player = owner;
    w.turn_player = owner;
    let got: i32 = kani::any();
    kani::assume(got == 0 || got == 1);
    let got_key = intern::intern("skill.sumimi.got");
    w.key_set(owner, got_key, got);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    let ckey = intern::intern("crystals");
    w.key_set(owner, ckey, crystals);

    let gid = crate::guard_id!("actor == owner && slot('skill.sumimi.got') == 0");
    let ov = ir_fill::TriggerOverlay::default();
    let ctx = ir_fill::fill_for_guard(&w, gid, owner, 0, true, -1, &ov);
    let admitted = crate::guard!(gid, &ctx);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let crystals = world::with_world(|ww| ww.key_get(owner, ckey));
    let got = world::with_world(|ww| ww.key_get(owner, got_key));

    kani::cover!(admitted && crystals > 0, "clear half with crystals");
    kani::cover!(admitted && crystals == 0, "clear half with no crystals");
    kani::cover!(!admitted, "guard rejects (GOT set — book's add half)");

    // C-SUMIMI-band-01 book: GOT set ⇒ add 1 crystal. The entry guard admits
    // only GOT == 0, so the add half can never run. This assert is the
    // plain FAILING proof of that LESS.
    if got != 0 {
        assert!(
            admitted,
            "C-SUMIMI-band-01 add half: book demands a turn-end add when money was gained; the entry guard never admits"
        );
        return;
    }

    assert!(admitted, "clear half must be admitted when GOT == 0");
    invoke::unwrap_done(invoke::hook_only_idx(BAND, "TurnEndBefore", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    assert!(
        spec::sumimi::band_crystal_clear_ok(effects, owner, ckey.0, crystals),
        "C-SUMIMI-band-01 clear: remove every crystal when the latch is clear"
    );
    let _ = pre;
    let _ = post;
}

#[kani::proof]
#[kani::unwind(24)]
pub fn band_01_clear_mutant_is_detected() {
    let mut w = base_world();
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    w.trig_player = owner;
    w.turn_player = owner;
    let got_key = intern::intern("skill.sumimi.got");
    w.key_set(owner, got_key, 0);
    let ckey = intern::intern("crystals");
    w.key_set(owner, ckey, 3);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(BAND, "TurnEndBefore", owner));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let wrong = spec::sumimi::band_crystal_clear_ok_mutant(effects, owner, ckey.0, 3);
    assert!(!wrong, "mutant: clear removes all crystals, not one");
}

// ===========================================================================
// C-SUMIMI-mana-01 -- pass CiRCLE → +1 [火罐]
// The body's CiRCLE test is `t == ctx::tile_named("CiRCLE")`. The Kani host
// `tile_named` resolves only 「江户川乐器店」 and returns -1 for every other
// name, so a legal "passing CiRCLE" setup cannot be built (BLOCKED-HOST
// `tile_named`). The shared fire spec is proven on `hina_01_pass_circle_iff`
// (whose body is the plain `gain_fire` under the real `is_circle` guard).
// This harness covers the no-op half that does NOT need `tile_named`: when
// the Pass body is entered on a non-CiRCLE, non-shop tile the shop branch
// would hit `rent_of` (also unmodeled) — so the only safe entry is the
// decline/no-prompt path. We pin the determined no-prompt structure here
// and leave the CiRCLE branch to the hina twin.
// ===========================================================================

/// C-SUMIMI-mana-02 structure (declined offer): no effects.
/// The accept path is BLOCKED-HOST (`rent_of` for the open 「原本收费」).
#[kani::proof]
#[kani::unwind(16)]
pub fn mana_02_decline_is_empty() {
    // Determined part of the offer: a refusal leaves the world untouched.
    // The body's `ask_yes` false path is `return Ok(())` with no effects.
    // We do not invoke the body here (its entry would need `rent_of` for the
    // accept amount); this harness documents the declined-side contract and
    // is a pure-spec cover of `mana_shop_pay_declined`.
    let empty: [Effect; 0] = [];
    assert!(
        spec::skills_sumimi::mana_shop_pay_declined(&empty),
        "declined offer must be a no-op"
    );
}

// ===========================================================================
// C-SUMIMI-band-03 -- exclusive play gate
// 「你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出。」
// The gate is the playability query on a character exclusive card (not a
// body effect). Covered here as a pure predicate on the character match.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn band_03_exclusive_gate_predicate() {
    let user_is_owner: bool = kani::any();
    kani::cover!(user_is_owner, "matching character — play allowed");
    kani::cover!(!user_is_owner, "mismatch — play denied");
    assert!(
        spec::skills_sumimi::band_exclusive_play_allowed(user_is_owner) == user_is_owner,
        "C-SUMIMI-band-03: play iff the character matches the exclusive's owner"
    );
    // Mutant: the dual must be distinguishable.
    assert!(
        spec::skills_sumimi::band_exclusive_play_allowed(user_is_owner) != !user_is_owner,
        "mutant: gate is identity, not negation"
    );
}

/// Keep `MANA` referenced so the id is greppable next to the BLOCKED-HOST row.
#[allow(dead_code)]
fn mana_id() -> &'static str {
    MANA
}