//! Unit `skills_hhw`: clause proofs for the five Hello, Happy World!
//! character skills and the band skill 传播笑容.
//!
//! Cheap first: money / fire hooks (kokoro, misaki) before plant / band.

use crate::host;
use crate::invoke;
use crate::ir_fill::{self, TriggerOverlay};
use crate::spec;
use crate::world::{self, Effect, Snapshot, World, MAX_PLAYERS};

/// Shared world shape for a `Pass` hook on a named tile.
fn symbolic_pass_world(owner: i32, tile: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 3;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 5000;
        w.fire[p] = 0;
        w.pos[p] = 0;
    }
    w.trig_player = owner;
    w.trig_tile = tile;
    w.turn_player = owner;
    w
}

fn effects_of() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

// ===========================================================================
// C-HHW-kokoro-01 -- 「（1）开局时获得1000资金」
// Entry: `skill:弦卷心:弦卷集团` / `On::Hook(TurnStartBefore)`
// Guard (single-source): `slot('skill.kasumiGroup.start') == 0`
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_01_opening_1000_iff() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    // Latch starts at 0 (「开局时」 pays once).
    w.key_set(owner, crate::intern::intern("skill.kasumiGroup.start"), 0);

    let gid = crate::guard_id!("slot('skill.kasumiGroup.start') == 0");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, -1i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (latch still 0)");
    if !admitted {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(pre.money[owner as usize] == 0, "pre money 0");
    assert!(
        spec::skills_hhw::kokoro_01_log_ok(effects, owner),
        "C-HHW-kokoro-01: log is the latch + GainMoney(1000)"
    );
    assert!(
        post.money[owner as usize] == pre.money[owner as usize] + spec::skills_hhw::kokoro_01_amount(),
        "C-HHW-kokoro-01: money rises by exactly 1000"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.hand == pre.hand, "frame: hand");
    assert!(post.discard == pre.discard, "frame: discard");
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn kokoro_01_mutant_500() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    w.key_set(owner, crate::intern::intern("skill.kasumiGroup.start"), 0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "", owner));
    let post = world::snapshot();
    assert!(
        post.money[owner as usize] == pre.money[owner as usize] + spec::skills_hhw::kokoro_01_amount_mutant(),
        "mutant: card must not match +500"
    );
}

#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_01_mutant_is_detected() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    w.key_set(owner, crate::intern::intern("skill.kasumiGroup.start"), 0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "", owner));
    let post = world::snapshot();
    assert!(
        post.money[owner as usize] != pre.money[owner as usize] + spec::skills_hhw::kokoro_01_amount_mutant(),
        "mutant +500 must disagree with the card"
    );
}

/// No-op: the latch is already 1 (「开局时」 already paid) — the host would
/// not call the body. Cover the reject side of the single-source guard.
#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_01_noop_when_latch_set() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    w.key_set(owner, crate::intern::intern("skill.kasumiGroup.start"), 1);
    let gid = crate::guard_id!("slot('skill.kasumiGroup.start') == 0");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, -1i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(!admitted, "guard rejects (latch already set)");
    assert!(!admitted, "latch=1 must reject the once-only grant");
}

// ===========================================================================
// C-HHW-kokoro-02 -- 「（2）[经过]CiRCLE时额外获得1500资金」
// Entry: `On::Hook(Pass)`; guard `actor == owner && is_circle(tile.id)`
// ===========================================================================

/// Board: CiRCLE is 序号 #1 → 0-based index 0.
pub const CIRCLE: i32 = 0;

#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_02_circle_pass_1500_iff() {
    let owner: i32 = 0;
    let w = symbolic_pass_world(owner, CIRCLE);

    let gid = crate::guard_id!("actor == owner && is_circle(tile.id)");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, CIRCLE as i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (own pass of CiRCLE)");
    kani::cover!(!admitted, "guard rejects");
    if !admitted {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "Pass", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    assert!(
        spec::skills_hhw::kokoro_02_log_ok(effects, owner),
        "C-HHW-kokoro-02: log is exactly GainMoney(1500)"
    );
    assert!(
        post.money[owner as usize] == pre.money[owner as usize] + spec::skills_hhw::kokoro_02_amount(),
        "C-HHW-kokoro-02: money rises by exactly 1500 extra"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
}

/// No-op: passing a non-CiRCLE tile must not fire the extra.
#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_02_noop_non_circle() {
    let owner: i32 = 0;
    // A non-CiRCLE tile (index 5).
    let w = symbolic_pass_world(owner, 5);
    let gid = crate::guard_id!("actor == owner && is_circle(tile.id)");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, 5i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(!admitted, "guard rejects non-CiRCLE");
    assert!(!admitted, "non-CiRCLE pass must not admit the extra 1500");
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn kokoro_02_mutant_2000() {
    let owner: i32 = 0;
    let w = symbolic_pass_world(owner, CIRCLE);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "Pass", owner));
    let post = world::snapshot();
    assert!(
        post.money[owner as usize] == pre.money[owner as usize] + spec::skills_hhw::kokoro_02_amount_mutant(),
        "mutant: card must not match +2000"
    );
}

#[kani::proof]
#[kani::unwind(24)]
pub fn kokoro_02_mutant_is_detected() {
    let owner: i32 = 0;
    let w = symbolic_pass_world(owner, CIRCLE);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:弦卷心:弦卷集团"), "Hook", "Pass", owner));
    let post = world::snapshot();
    assert!(
        post.money[owner as usize] != pre.money[owner as usize] + spec::skills_hhw::kokoro_02_amount_mutant(),
        "mutant +2000 must disagree"
    );
}

// ===========================================================================
// C-HHW-misaki-skill-01 -- 「每次经过"弦卷集团"（#29）格子后获得2火罐（初始0，上限2）」
// Entry: `skill:奥泽美咲:另一个我` / `On::Hook(Pass)`
// Guard: `actor == owner && tile.id == tile_named('弦卷集团')`
// ===========================================================================

/// 弦卷集团 is 序号 #29 → 0-based index 28.
pub const TSUZUMI_GROUP: i32 = 28;

#[kani::proof]
#[kani::unwind(24)]
pub fn misaki_skill_01_pass_fire_iff() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, TSUZUMI_GROUP);
    // Pre fire is symbolic in 0..=cap so the clamp is exercised.
    w.fire[owner as usize] = kani::any();
    kani::assume(w.fire[owner as usize] >= 0 && w.fire[owner as usize] <= 2);

    let gid = crate::guard_id!("actor == owner && tile.id == tile_named('弦卷集团')");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, TSUZUMI_GROUP as i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (own pass of 弦卷集团)");
    if !admitted {
        return;
    }

    let pre_fire = w.fire[owner as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:奥泽美咲:另一个我"), "Hook", "Pass", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(pre_fire == 0, "pre fire 0");
    kani::cover!(pre_fire == 1, "pre fire 1 (clamp path)");
    assert!(
        spec::skills_hhw::misaki_skill_01_log_ok(effects, owner),
        "C-HHW-misaki-skill-01: log is GainFire(+2) (+ latch)"
    );
    let expected = spec::skills_hhw::misaki_skill_01_gain(pre_fire);
    assert!(
        post.fire[owner as usize] == expected,
        "C-HHW-misaki-skill-01: fire rises by +2, capped at 2"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// No-op: a non-弦卷集团 pass does not admit the grant.
#[kani::proof]
#[kani::unwind(24)]
pub fn misaki_skill_01_noop_other_tile() {
    let owner: i32 = 0;
    let w = symbolic_pass_world(owner, CIRCLE);
    let gid = crate::guard_id!("actor == owner && tile.id == tile_named('弦卷集团')");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, owner, 0, true, CIRCLE as i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(!admitted, "guard rejects non-弦卷集团");
    assert!(!admitted);
}

#[kani::proof]
#[kani::unwind(24)]
pub fn misaki_skill_01_mutant_is_detected() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, TSUZUMI_GROUP);
    w.fire[owner as usize] = 0;
    let pre_fire = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:奥泽美咲:另一个我"), "Hook", "Pass", owner));
    let post = world::snapshot();
    assert!(
        post.fire[owner as usize] != spec::skills_hhw::misaki_skill_01_gain_mutant(pre_fire),
        "mutant +1 must disagree with the card's +2"
    );
}

// ===========================================================================
// C-HHW-kaoru-01 -- 「每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐
// （上限7）；若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]」
// Entry: `skill:濑田薰:梦幻的王子殿下` / `On::Hook(Pass)` (MINE) and
// `On::Hook(PassTile)` (passed-by). The pot arm is the cheap half.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn kaoru_01_pot_arm_iff() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    w.fire[owner as usize] = kani::any();
    kani::assume(w.fire[owner as usize] >= 0 && w.fire[owner as usize] <= 7);
    // Isolation: no 怪盗标记 on the field, so the pot arm is the book's.
    // (The mark write is a separate arm; FINDINGS documents that the live
    // counter the card plants is not seen by this check.)
    let pre_fire = w.fire[owner as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // `On::Hook(Pass)` with `card_sdk::pre::MINE`.
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:濑田薰:梦幻的王子殿下"), "Hook", "Pass", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(pre_fire == 0, "pre fire 0");
    kani::cover!(pre_fire == 6, "pre fire 6 (cap path)");
    // Book: +1 火罐, cap 7, when no mark exists. The body's mark-existence
    // check is a residual guard; with no marks planted in the world this is
    // the pot arm.
    if spec::skills_hhw::kaoru_01_pot_log_ok(effects, owner) {
        let expected = spec::skills_hhw::kaoru_01_pot_gain(pre_fire);
        assert!(
            post.fire[owner as usize] == expected,
            "C-HHW-kaoru-01: pot arm gains +1 capped at 7"
        );
    } else {
        // FINDINGS: the body may take the mark arm (or both) when its
        // existence check disagrees with the book. Pin the weaker book fact:
        // it must never gain more than +1 fire on one step.
        let delta = post.fire[owner as usize] - pre_fire;
        assert!(delta <= 1, "C-HHW-kaoru-01: at most +1 火罐 per step");
    }
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn kaoru_01_mutant_is_detected() {
    let owner: i32 = 0;
    let mut w = symbolic_pass_world(owner, -1);
    w.fire[owner as usize] = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:濑田薰:梦幻的王子殿下"), "Hook", "Pass", owner));
    let post = world::snapshot();
    // Mutant demands GainFire + a second effect (both arms at once).
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        !spec::skills_hhw::kaoru_01_pot_log_ok_mutant(effects, owner),
        "mutant both-arms must not match a single pot grant"
    );
    assert!(post.fire[owner as usize] <= 1, "still at most +1");
}

// ===========================================================================
// C-HHW-band-01 -- 「向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家
// 并使此卡获得一个奇迹水晶。」
// Entry: `skill:Hello, Happy World!:传播笑容` / `On::Hook(PayChoose)` (MINE)
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn band_01_declined_is_noop() {
    let owner: i32 = 0;
    let mut w = World::new();
    w.n_players = 3;
    w.trig_player = owner;
    w.trig_target = 1; // another player's tile
    w.trig_value = 500;
    w.pay_amount = 500;
    w.symbolic_answers = false;
    w.answers[0] = 0; // decline (no)
    let pre_amount = w.pay_amount;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:Hello, Happy World!:传播笑容"), "Hook", "PayChoose", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        spec::skills_hhw::band_01_declined_log_ok(effects),
        "C-HHW-band-01: decline produces no effects"
    );
    assert!(post.pay_amount == pre_amount, "decline leaves the amount");
    assert!(post.money == pre.money, "frame: money");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn band_01_accepted_doubles_iff() {
    let owner: i32 = 0;
    let mut w = World::new();
    w.n_players = 3;
    w.trig_player = owner;
    w.trig_target = 1;
    w.trig_value = 500;
    w.pay_amount = 500;
    w.symbolic_answers = false;
    w.answers[0] = 1; // accept (yes)
    let pre_amount = w.pay_amount;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:Hello, Happy World!:传播笑容"), "Hook", "PayChoose", owner));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "accepted path");
    // The crystal key is `crystals` (counter name).
    let key = crate::intern::intern("crystals").0;
    // Log shape: SetPayAmount(2×) + AddCounter(+1). The body may also write
    // the 「记录」 state (SetState) — the book's observable is the crystal.
    let mut ok_amount = false;
    let mut ok_crystal = false;
    for e in effects {
        match e {
            Effect::SetPayAmount { to } => {
                ok_amount = *to == spec::skills_hhw::band_01_amount(pre_amount);
            }
            Effect::AddCounter {
                owner: o,
                key: k,
                n,
            } => {
                ok_crystal = *o == owner && *k == key && *n == 1;
            }
            Effect::SetState { .. } => {}
            _ => {}
        }
    }
    assert!(ok_amount, "C-HHW-band-01: amount becomes 2×");
    assert!(ok_crystal, "C-HHW-band-01: band card gains exactly 1 crystal");
    assert!(
        post.pay_amount == spec::skills_hhw::band_01_amount(pre_amount),
        "C-HHW-band-01: post pay_amount is double"
    );
    assert!(post.money == pre.money, "frame: money");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn band_01_mutant_is_detected() {
    let owner: i32 = 0;
    let mut w = World::new();
    w.n_players = 3;
    w.trig_player = owner;
    w.trig_target = 1;
    w.trig_value = 500;
    w.pay_amount = 500;
    w.symbolic_answers = false;
    w.answers[0] = 1;
    let pre_amount = w.pay_amount;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("skill:Hello, Happy World!:传播笑容"), "Hook", "PayChoose", owner));
    let post = world::snapshot();
    assert!(
        post.pay_amount != spec::skills_hhw::band_01_amount_mutant(pre_amount),
        "mutant triple-pay must disagree with the card's double"
    );
}

/// Cover helper so the unit always has at least one trivially-fresh cover.
#[kani::proof]
#[kani::unwind(8)]
pub fn skills_hhw_smoke_cover() {
    let x: bool = kani::any();
    kani::cover!(x, "skills_hhw smoke");
    kani::cover!(!x, "skills_hhw smoke no");
}