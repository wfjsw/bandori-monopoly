//! Unit `skills_mujica`: clause proofs for the five Ave Mujica character
//! skills and the band skill 假面之下的真实.
//!
//! Multi-turn / overlay / flip / interrupt machinery is PROTOCOL-LEVEL
//! (`docs/rulebook/spec/kani/mujica.md`). What is proved here is the
//! arithmetic and the hook bodies that the host can actually record.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn install_and_new() -> World {
    host::install();
    World::new()
}

fn effects_of() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

// ===========================================================================
// C-MUJICA-初华技能-01
// 「状态1：移动改为10+1d10」
// Pure arithmetic: the move length is 10 + d10, range 11..=20.
// The RollPlan hook body (`on_plan`) rewrites the plan dice; the host's
// `plan::set_base_dice` / `add_extra_dice` are structure-only stubs, so the
// walk itself is PROTOCOL-LEVEL. The arithmetic is the text's determinate part.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn uika_state1_roll_is_10_plus_d10() {
    let d10: i32 = kani::any();
    kani::assume((1..=10).contains(&d10));
    let mv = spec::skills_mujica::uika_state1_move(d10);
    let (lo, hi) = spec::skills_mujica::uika_state1_move_range();
    kani::cover!(d10 == 1, "d10 = 1 → move 11");
    kani::cover!(d10 == 10, "d10 = 10 → move 20");
    kassert!(
        mv == 10 + d10 && mv >= lo && mv <= hi,
        "C-MUJICA-初华技能-01: move is 10+1d10 in 11..=20"
    );
}

/// MUTANT: 9 + d10.
#[kani::proof]
#[kani::unwind(8)]
pub fn uika_state1_move_mutant_is_detected() {
    let d10: i32 = kani::any();
    kani::assume((1..=10).contains(&d10));
    kassert!(
        spec::skills_mujica::uika_state1_move(d10) != spec::skills_mujica::uika_state1_move_mutant(d10),
        "mutant: 10+d10 ≠ 9+d10"
    );
}

// ===========================================================================
// C-MUJICA-团技能-00
// 「你拥有"状态1"和"状态2"两种状态，初始处于状态1。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn band_starts_in_state1() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    // Fresh match: skillState unset (0). The band skill's turn-start
    // initialiser writes 1.
    w.symbolic_answers = false;
    let skill_state = crate::intern::intern("skillState");
    w.key_set(user, skill_state, 0);
    let pre_state = w.key_get(user, skill_state);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Entry 1 = On::Hook(TurnStartBefore) `at_turn_start`.
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:Ave Mujica:假面之下的真实"), 1, user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let now = world::with_world(|ww| ww.key_get(user, skill_state));

    kani::cover!(true, "fresh init");
    kassert!(pre_state == 0, "precondition: skillState unset");
    kassert!(
        now == spec::skills_mujica::band_initial_state(),
        "C-MUJICA-团技能-00: initial 状态1"
    );
    kassert!(
        spec::skills_mujica::band_init_effects(effects, user, skill_state.0),
        "C-MUJICA-团技能-00: SetState(skillState, 1)"
    );
    kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

// ===========================================================================
// C-MUJICA-团技能-01
// 「你在状态2时从[收取]与[支付]的资金改为1.5倍
//  （只对自己结算，对应的其他玩家收支不变）」
// The PayMul hook body rewrites the trigger amount. The one-sided
// application (「只对自己结算」) is a pay-pipeline property: under the
// current host `pay_to` moves a single amount both ways, so the full
// one-sided transfer is PROTOCOL-LEVEL / the known FAILS finding. Here we
// pin the hook's amount rewrite and show the one-sided spec is what the
// book demands (and that a symmetric rewrite is distinguishable).
// ===========================================================================

fn band_bend_world(user_on_from_side: bool) -> World {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 3_000;
    }
    let user = 0i32;
    w.trig_player = user;
    let skill_state = crate::intern::intern("skillState");
    w.key_set(user, skill_state, 2);
    // trigger: a 300 payment between the two seats.
    if user_on_from_side {
        w.trig_value = 300;
        w.pay_amount = 300;
    } else {
        w.trig_value = 300;
        w.pay_amount = 300;
    }
    w
}

/// Pure amount arithmetic: 1.5x on the user's side, printed amount unchanged
/// for the other party.
#[kani::proof]
#[kani::unwind(8)]
pub fn band_state2_pay_is_150_only_on_self() {
    let printed: i32 = kani::any();
    kani::assume((1..=1_000).contains(&printed));
    let user_is_payer: bool = kani::any();
    let bent = spec::skills_mujica::band_state2_bend(printed);
    kani::cover!(user_is_payer, "user is payer");
    kani::cover!(!user_is_payer, "user is payee");
    kani::cover!(printed % 2 == 0, "even printed (exact 1.5x)");
    // One-sided: user moves bent, other moves printed.
    let (user_delta, other_delta) = if user_is_payer {
        (-bent, printed)
    } else {
        (bent, -printed)
    };
    kassert!(
        spec::skills_mujica::band_state2_one_sided_ok(user_delta, other_delta, printed, user_is_payer),
        "C-MUJICA-团技能-01: 1.5x on the user's side only"
    );
    // A symmetric 1.5x is NOT the book (the known engine finding).
    kassert!(
        !spec::skills_mujica::band_state2_one_sided_ok_mutant(user_delta, other_delta, printed, user_is_payer)
            || bent == printed,
        "mutant: symmetric 1.5x must not satisfy the one-sided spec when bent ≠ printed"
    );
}

/// Hook body: `bend` rewrites `pay_amount` to 1.5x when the user is a side.
/// Entry 2 = On::Hook(PayMul) `bend`.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_state2_bend_hook_rewrites_amount() {
    let mut w = band_bend_world(true);
    let user = w.trig_player;
    let pre_amount = w.pay_amount;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:Ave Mujica:假面之下的真实"), 2, user));
    let post = world::snapshot();

    kani::cover!(pre_amount > 0, "positive amount");
    kani::cover!(pre_amount == 300, "300 → 450");
    // The body uses `(amount * 3 + 1) / 2` which for even amounts is exact
    // 1.5x; for odd amounts it is ceil-ish. The book does not specify the
    // odd rounding (OPEN-取整). Pin the even case as the clause's meaning
    // and accept the body's formula on odds as the executable reading.
    if pre_amount > 0 {
        let expected = (pre_amount * 3 + 1) / 2;
        kassert!(
            post.pay_amount == expected,
            "C-MUJICA-团技能-01: PayMul rewrites the amount to 1.5x"
        );
    }
    kassert!(world::arr4_eq(post.money, pre.money), "frame: money (hook only rewrites the amount)");
    kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// MUTANT: the hook must not leave the amount unchanged when the user is a side.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_state2_bend_mutant_is_detected() {
    let mut w = band_bend_world(true);
    let user = w.trig_player;
    let pre_amount = w.pay_amount;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:Ave Mujica:假面之下的真实"), 2, user));
    let post = world::snapshot();
    if pre_amount > 0 && pre_amount != (pre_amount * 3 + 1) / 2 {
        kassert!(
            post.pay_amount != pre_amount,
            "mutant: amount must change"
        );
    }
}

// ===========================================================================
// C-MUJICA-若麦技能-01 arithmetic
// 「每次其他玩家向你支付资金时，你立即获得1火罐（初始0，上限5），
//  并使那次支付的金额提高X*100，X为你拥有的火罐数。」
// X is the pot count **after** the +1.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn nyamu_pay_raise_uses_new_pot_count() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=5).contains(&pots_before));
    let after = spec::skills_mujica::nyamu_pots_after(pots_before);
    let raise = spec::skills_mujica::nyamu_pay_raise(pots_before);
    kani::cover!(pots_before == 0, "0 → 1 pot, raise 100");
    kani::cover!(pots_before == 4, "4 → 5 pots, raise 500");
    kani::cover!(pots_before == 5, "already at cap");
    kassert!(
        after == (pots_before + 1).min(5),
        "C-MUJICA-若麦技能-01: pots +1, cap 5"
    );
    kassert!(
        raise == after * 100,
        "C-MUJICA-若麦技能-01: raise is X*100 with X the NEW pot count"
    );
}

/// MUTANT: X is the pot count before the +1.
#[kani::proof]
#[kani::unwind(8)]
pub fn nyamu_pay_raise_mutant_is_detected() {
    let pots_before: i32 = kani::any();
    kani::assume(pots_before == 0);
    kassert!(
        spec::skills_mujica::nyamu_pay_raise(pots_before) != spec::skills_mujica::nyamu_pay_raise_mutant(pots_before),
        "mutant: new-count raise (100) ≠ old-count raise (0)"
    );
}

// ===========================================================================
// C-MUJICA-祥子技能-02 / C-MUJICA-海铃技能-02 pot arithmetic
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn sakiko_fire_cap_is_three() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=3).contains(&pots_before));
    let after = spec::skills_mujica::sakiko_pots_after(pots_before);
    kani::cover!(pots_before == 2, "2 → 3 (at cap)");
    kassert!(
        after == (pots_before + 1).min(3),
        "C-MUJICA-祥子技能-02: pots +1, cap 3"
    );
    kassert!(
        spec::skills_mujica::sakiko_state2_offered(after) == (after >= 3),
        "C-MUJICA-祥子技能-02: 状态2 offered at cap"
    );
}

#[kani::proof]
#[kani::unwind(8)]
pub fn umiri_fire_cap_is_four() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=4).contains(&pots_before));
    let after = spec::skills_mujica::umiri_pots_after(pots_before);
    kani::cover!(pots_before == 3, "3 → 4 (at cap)");
    kassert!(
        after == (pots_before + 1).min(4),
        "C-MUJICA-海铃技能-02: pots +1, cap 4"
    );
    kassert!(
        spec::skills_mujica::umiri_state2_offered(after) == (after >= 4),
        "C-MUJICA-海铃技能-02: 状态2 offered at cap"
    );
}

// ===========================================================================
// C-MUJICA-海铃技能-03 arithmetic
// 「每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数）」
// 「每通过此法消耗4个火罐，你抽1张卡。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn umiri_deed_pay_and_draw_arithmetic() {
    let houses: i32 = kani::any();
    kani::assume((0..=5).contains(&houses));
    let pots: i32 = kani::any();
    kani::assume((0..=8).contains(&pots));
    let pay = spec::skills_mujica::umiri_deed_pay(houses);
    let draws = spec::skills_mujica::umiri_draw_per_pots(pots);
    kani::cover!(houses == 0, "0 houses → 100");
    kani::cover!(houses == 3, "3 houses → 400");
    kani::cover!(pots == 4, "4 pots → 1 draw");
    kani::cover!(pots == 7, "7 pots → 1 draw");
    kassert!(
        pay == 100 + houses * 100,
        "C-MUJICA-海铃技能-03: pay is 100+n*100"
    );
    kassert!(
        draws == pots / 4,
        "C-MUJICA-海铃技能-03: 1 draw per 4 pots spent"
    );
}

// ===========================================================================
// C-MUJICA-睦技能-01 gate
// 「若你的回合开始时手牌数为5，可选择进入状态2。」
// Live sheet 「手牌数为5」 supersedes rulebook-doc 「手牌数4或以上」.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn mutsumi_state2_gate_is_hand_exactly_5() {
    let hand: i32 = kani::any();
    kani::assume((0..=8).contains(&hand));
    kani::cover!(hand == 5, "hand 5 → gate open");
    kani::cover!(hand == 4, "hand 4 → gate closed (sheet supersedes)");
    kassert!(
        spec::skills_mujica::mutsumi_state2_gate(hand) == (hand == 5),
        "C-MUJICA-睦技能-01: gate is hand size exactly 5"
    );
}

// ===========================================================================
// C-MUJICA-团技能-03 arithmetic
// 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn band_halve_pay_amount() {
    let amount: i32 = kani::any();
    kani::assume((0..=2_000).contains(&amount));
    kani::cover!(amount == 320, "even 320 → 160");
    kani::cover!(amount == 321, "odd 321 → 160 (OPEN-取整, floor)");
    kassert!(
        spec::skills_mujica::band_halve_pay(amount) == amount / 2,
        "C-MUJICA-团技能-03: halve the payment"
    );
    kassert!(
        spec::skills_mujica::band_halve_crystal_spend(-1),
        "C-MUJICA-团技能-03: spend exactly 1 crystal"
    );
}
