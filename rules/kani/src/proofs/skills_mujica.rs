//! Unit `skills_mujica`: clause proofs for the five Ave Mujica character
//! skills and the band skill 假面之下的真实.
//!
//! Multi-turn / overlay / flip / interrupt machinery is PROTOCOL-LEVEL
//! (`docs/rulebook/spec/kani/mujica.md`). What is proved here is the
//! arithmetic and the hook bodies that the host can actually record.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Narration (`ctx::log`) is not part of the frame.
//! Pure-formula harnesses never call a body and keep a low unwind.

use crate::host;
use crate::invoke;
use crate::spec::skills_mujica as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-body host-op subsets.
// ---------------------------------------------------------------------------

/// `skill:Ave Mujica:假面之下的真实` `at_turn_start` — init + 状态2 tick.
crate::host_subset!(ave_turn_start_dispatch {
    state_get => arm_state_get,
    state_set => arm_state_set,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
});

/// `skill:Ave Mujica:假面之下的真实` `bend` (PayMul) — rewrite the amount.
crate::host_subset!(ave_bend_dispatch {
    trig_value => arm_trig_value,
    trig_player => arm_trig_player,
    trig_target => arm_trig_target,
    trig_set_pay_amount => arm_trig_set_pay_amount,
});

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

/// In-place 2-seat world on `dispatch`, then `f` for the card-specific fields.
fn sparse2(
    dispatch: card_sdk::kani_host::Backend,
    user: i32,
    f: impl FnOnce(&mut world::World),
) {
    host::install_backend(dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.money[p] = 0;
            w.pos[p] = 0;
            w.stay[p] = 0;
            w.stun[p] = 0;
        }
        w.trig_player = user;
        f(w);
    });
}

// ===========================================================================
// C-MUJICA-初华技能-01
// 「状态1：移动改为10+1d10」
// Pure arithmetic: the move length is 10 + d10, range 11..=20.
// The RollPlan hook body (`on_plan`) rewrites the plan dice; the host's
// `plan::set_base_dice` / `add_extra_dice` are structure-only stubs, so the
// walk itself is PROTOCOL-LEVEL. The arithmetic is the text's determinate part.
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn uika_state1_roll_is_10_plus_d10() {
    let d10: i32 = kani::any();
    kani::assume((1..=10).contains(&d10));
    let mv = spec::uika_state1_move(d10);
    let (lo, hi) = spec::uika_state1_move_range();
    kani::cover!(d10 == 1, "d10 = 1 → move 11");
    kani::cover!(d10 == 10, "d10 = 10 → move 20");
    crate::kassert!(
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
    crate::kassert!(
        spec::uika_state1_move(d10) != spec::uika_state1_move_mutant(d10),
        "mutant: 10+d10 ≠ 9+d10"
    );
}

// ===========================================================================
// C-MUJICA-团技能-00
// 「你拥有"状态1"和"状态2"两种状态，初始处于状态1。」
// Seat-symmetric (the initialiser writes the user's `skillState`).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn band_init_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let skill_state = crate::intern::intern("skillState");
    sparse2(ave_turn_start_dispatch, user, |w| {
        // Fresh match: skillState unset (0). The band skill's turn-start
        // initialiser writes 1.
        w.symbolic_answers = false;
        w.key_set(user, skill_state, 0);
    });
    let pre_state = world::with_world(|ww| ww.key_get(user, skill_state));
    let pre = world::snapshot();
    invoke::unwrap_done(skill_bands::ave_mujica::at_turn_start_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let now = world::with_world(|ww| ww.key_get(user, skill_state));

    kani::cover!(true, "fresh init");
    crate::kassert!(pre_state == 0, "precondition: skillState unset");
    crate::kassert!(
        now == spec::band_initial_state(),
        "C-MUJICA-团技能-00: initial 状态1"
    );
    crate::kassert!(
        spec::band_init_effects(effects, user, skill_state.0),
        "C-MUJICA-团技能-00: SetState(skillState, 1)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// Band init iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_starts_in_state1() {
    band_init_iff_at(0);
}

/// Band init iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_user1_starts_in_state1() {
    band_init_iff_at(1);
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
// Seat-symmetric (the hook fires for the skill owner, who is a side of the
// 0↔1 payment).
// ===========================================================================

/// Pure amount arithmetic: 1.5x on the user's side, printed amount unchanged
/// for the other party. Pure-formula harness — no body call.
#[kani::proof]
#[kani::unwind(8)]
pub fn band_state2_pay_is_150_only_on_self() {
    let printed: i32 = kani::any();
    kani::assume((1..=1_000).contains(&printed));
    let user_is_payer: bool = kani::any();
    let bent = spec::band_state2_bend(printed);
    kani::cover!(user_is_payer, "user is payer");
    kani::cover!(!user_is_payer, "user is payee");
    kani::cover!(printed % 2 == 0, "even printed (exact 1.5x)");
    // One-sided: user moves bent, other moves printed.
    let (user_delta, other_delta) = if user_is_payer {
        (-bent, printed)
    } else {
        (bent, -printed)
    };
    crate::kassert!(
        spec::band_state2_one_sided_ok(user_delta, other_delta, printed, user_is_payer),
        "C-MUJICA-团技能-01: 1.5x on the user's side only"
    );
    // A symmetric 1.5x is NOT the book (the known engine finding).
    crate::kassert!(
        !spec::band_state2_one_sided_ok_mutant(user_delta, other_delta, printed, user_is_payer)
            || bent == printed,
        "mutant: symmetric 1.5x must not satisfy the one-sided spec when bent ≠ printed"
    );
}

/// Pending pay `0 → 1` of a symbolic `amount`, skill owner at `user`.
fn bend_world(user: i32) -> i32 {
    debug_assert!(user == 0 || user == 1);
    let amount: i32 = kani::any();
    kani::assume(amount > 0 && amount <= 10_000);
    let skill_state = crate::intern::intern("skillState");
    sparse2(ave_bend_dispatch, user, |w| {
        w.key_set(user, skill_state, 2);
        w.trig_player = 0;
        w.trig_target = 1;
        w.trig_value = amount;
        w.pay_amount = amount;
    });
    amount
}

/// Hook body: `bend` rewrites `pay_amount` to 1.5x when the user is a side.
/// Entry 2 = On::Hook(PayMul) `bend`. Direct body call.
fn band_bend_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let pre_amount = bend_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_bands::ave_mujica::bend_kani(user));
    let post = world::snapshot();

    kani::cover!(pre_amount == 300, "300 → 450");
    kani::cover!(pre_amount % 2 == 1, "odd amount (OPEN-取整)");
    // The body uses `(amount * 3 + 1) / 2` which for even amounts is exact
    // 1.5x; for odd amounts it is ceil-ish. The book does not specify the
    // odd rounding (OPEN-取整). Pin the even case as the clause's meaning
    // and accept the body's formula on odds as the executable reading.
    if pre_amount > 0 {
        let expected = (pre_amount * 3 + 1) / 2;
        crate::kassert!(
            post.pay_amount == expected,
            "C-MUJICA-团技能-01: PayMul rewrites the amount to 1.5x"
        );
    }
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money (hook only rewrites the amount)"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Bend hook iff at seat 0 (user is the payer side of the 0→1 payment).
#[kani::proof]
#[kani::unwind(32)]
pub fn band_state2_bend_hook_rewrites_amount() {
    band_bend_iff_at(0);
}

/// Bend hook iff at seat 1 (user is the payee side of the 0→1 payment).
#[kani::proof]
#[kani::unwind(32)]
pub fn band_state2_user1_bend_hook_rewrites_amount() {
    band_bend_iff_at(1);
}

/// MUTANT: the hook must not leave the amount unchanged when the user is a side.
#[kani::proof]
#[kani::unwind(32)]
pub fn band_state2_bend_mutant_is_detected() {
    let user = 0i32;
    let pre_amount = bend_world(user);
    invoke::unwrap_done(skill_bands::ave_mujica::bend_kani(user));
    let post = world::snapshot();
    if pre_amount > 0 && pre_amount != (pre_amount * 3 + 1) / 2 {
        crate::kassert!(
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
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn nyamu_pay_raise_uses_new_pot_count() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=5).contains(&pots_before));
    let after = spec::nyamu_pots_after(pots_before);
    let raise = spec::nyamu_pay_raise(pots_before);
    kani::cover!(pots_before == 0, "0 → 1 pot, raise 100");
    kani::cover!(pots_before == 4, "4 → 5 pots, raise 500");
    kani::cover!(pots_before == 5, "already at cap");
    crate::kassert!(
        after == (pots_before + 1).min(5),
        "C-MUJICA-若麦技能-01: pots +1, cap 5"
    );
    crate::kassert!(
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
    crate::kassert!(
        spec::nyamu_pay_raise(pots_before) != spec::nyamu_pay_raise_mutant(pots_before),
        "mutant: new-count raise (100) ≠ old-count raise (0)"
    );
}

// ===========================================================================
// C-MUJICA-祥子技能-02 / C-MUJICA-海铃技能-02 pot arithmetic
// Pure-formula harnesses — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn sakiko_fire_cap_is_three() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=3).contains(&pots_before));
    let after = spec::sakiko_pots_after(pots_before);
    kani::cover!(pots_before == 2, "2 → 3 (at cap)");
    crate::kassert!(
        after == (pots_before + 1).min(3),
        "C-MUJICA-祥子技能-02: pots +1, cap 3"
    );
    crate::kassert!(
        spec::sakiko_state2_offered(after) == (after >= 3),
        "C-MUJICA-祥子技能-02: 状态2 offered at cap"
    );
}

#[kani::proof]
#[kani::unwind(8)]
pub fn umiri_fire_cap_is_four() {
    let pots_before: i32 = kani::any();
    kani::assume((0..=4).contains(&pots_before));
    let after = spec::umiri_pots_after(pots_before);
    kani::cover!(pots_before == 3, "3 → 4 (at cap)");
    crate::kassert!(
        after == (pots_before + 1).min(4),
        "C-MUJICA-海铃技能-02: pots +1, cap 4"
    );
    crate::kassert!(
        spec::umiri_state2_offered(after) == (after >= 4),
        "C-MUJICA-海铃技能-02: 状态2 offered at cap"
    );
}

// ===========================================================================
// C-MUJICA-海铃技能-03 arithmetic
// 「每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数）」
// 「每通过此法消耗4个火罐，你抽1张卡。」
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn umiri_deed_pay_and_draw_arithmetic() {
    let houses: i32 = kani::any();
    kani::assume((0..=5).contains(&houses));
    let pots: i32 = kani::any();
    kani::assume((0..=8).contains(&pots));
    let pay = spec::umiri_deed_pay(houses);
    let draws = spec::umiri_draw_per_pots(pots);
    kani::cover!(houses == 0, "0 houses → 100");
    kani::cover!(houses == 3, "3 houses → 400");
    kani::cover!(pots == 4, "4 pots → 1 draw");
    kani::cover!(pots == 7, "7 pots → 1 draw");
    crate::kassert!(
        pay == 100 + houses * 100,
        "C-MUJICA-海铃技能-03: pay is 100+n*100"
    );
    crate::kassert!(
        draws == pots / 4,
        "C-MUJICA-海铃技能-03: 1 draw per 4 pots spent"
    );
}

// ===========================================================================
// C-MUJICA-睦技能-01 gate
// 「若你的回合开始时手牌数为5，可选择进入状态2。」
// Live sheet 「手牌数为5」 supersedes rulebook-doc 「手牌数4或以上」.
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn mutsumi_state2_gate_is_hand_exactly_5() {
    let hand: i32 = kani::any();
    kani::assume((0..=8).contains(&hand));
    kani::cover!(hand == 5, "hand 5 → gate open");
    kani::cover!(hand == 4, "hand 4 → gate closed (sheet supersedes)");
    crate::kassert!(
        spec::mutsumi_state2_gate(hand) == (hand == 5),
        "C-MUJICA-睦技能-01: gate is hand size exactly 5"
    );
}

// ===========================================================================
// C-MUJICA-团技能-03 arithmetic
// 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。」
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn band_halve_pay_amount() {
    let amount: i32 = kani::any();
    kani::assume((0..=2_000).contains(&amount));
    kani::cover!(amount == 320, "even 320 → 160");
    kani::cover!(amount == 321, "odd 321 → 160 (OPEN-取整, floor)");
    crate::kassert!(
        spec::band_halve_pay(amount) == amount / 2,
        "C-MUJICA-团技能-03: halve the payment"
    );
    crate::kassert!(
        spec::band_halve_crystal_spend(-1),
        "C-MUJICA-团技能-03: spend exactly 1 crystal"
    );
}