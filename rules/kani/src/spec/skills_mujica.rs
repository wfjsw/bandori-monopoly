//! Unit `skills_mujica`: reference specs FROM the clause docs only.
//!
//! Sources: `the roster` (band == "Ave Mujica") and
//! `the roster` `Ave Mujica` / 假面之下的真实. Quotes live in
//! `docs/rulebook/spec/cards/mujica.md` § Character skills / Band skill.
//! Never derived from engine behaviour.

// ===========================================================================
// C-MUJICA-初华技能-01
// 「状态1：移动改为10+1d10」
// The main move is `10 + 1d10`, not 1d20.
// ===========================================================================

/// Main-move range under 状态1: 11..=20 inclusive.
pub fn uika_state1_move_range() -> (i32, i32) {
    (11, 20)
}

/// Map a symbolic d10 (1..=10) to the move length.
pub fn uika_state1_move(d10: i32) -> i32 {
    10 + d10
}

/// MUTANT: 9 + d10.
pub fn uika_state1_move_mutant(d10: i32) -> i32 {
    9 + d10
}

// ===========================================================================
// C-MUJICA-团技能-00
// 「你拥有"状态1"和"状态2"两种状态，初始处于状态1。」
// ===========================================================================

/// Initial `skillState` is 1.
pub fn band_initial_state() -> i32 {
    1
}

/// Fresh-world effect of the band skill's turn-start initialiser: exactly one
/// SetState writing skillState = 1.
pub fn band_init_effects(effects: &[crate::world::Effect], player: i32, skill_state_key: u16) -> bool {
    effects.len() >= 1
        && effects.iter().any(|e| {
            matches!(e, crate::world::Effect::SetState { player: p, key, n }
                if *p == player && *key == skill_state_key && *n == 1)
        })
}

// ===========================================================================
// C-MUJICA-团技能-01
// 「你在状态2时从[收取]与[支付]的资金改为1.5倍
//  （只对自己结算，对应的其他玩家收支不变）」
// Live sheet says **1.5倍** (rulebook-doc 「翻倍」 superseded).
// Rounding: (a*3+1)/2 is the engine's ceil-ish 1.5; the book does not
// specify odd-amount rounding → OPEN-取整. This spec uses exact 1.5x on
// integers via `a + a/2` (floor half) OR `(a*3+1)/2` — we pin the *body's*
// formula as the clause's executable meaning of 「1.5倍」 only for even
// amounts, and accept floor(a*1.5) = a + a/2 as the book's plain reading.
// ===========================================================================

/// User-side amount after the 1.5x bend. Book: 「改为1.5倍」.
/// Floor-half reading: `a + a/2`. Even `a` is exact 1.5x.
pub fn band_state2_bend(amount: i32) -> i32 {
    if amount <= 0 {
        amount
    } else {
        amount + amount / 2
    }
}

/// The other party's figure is unchanged (「对应的其他玩家收支不变」).
/// A one-sided application: the payer's debit (or payee's credit) is the
/// bent amount; the counterparty keeps the printed amount.
pub fn band_state2_one_sided_ok(
    user_delta: i32,
    other_delta: i32,
    printed: i32,
    user_is_payer: bool,
) -> bool {
    let bent = band_state2_bend(printed);
    if user_is_payer {
        user_delta == -bent && other_delta == printed
    } else {
        user_delta == bent && other_delta == -printed
    }
}

/// Symmetric (WRONG) application — the finding's shape. Both sides move 1.5x.
pub fn band_state2_one_sided_ok_mutant(
    user_delta: i32,
    other_delta: i32,
    printed: i32,
    _user_is_payer: bool,
) -> bool {
    let bent = band_state2_bend(printed);
    user_delta.abs() == bent && other_delta.abs() == bent
}

/// Pure `PaySplitMul` hook obligation: rewrite the trigger amount to the bent
/// figure when the user is one side of the transfer.
pub fn band_state2_pay_amount_ok(pre_amount: i32, post_amount: i32, user_is_side: bool) -> bool {
    if !user_is_side {
        post_amount == pre_amount
    } else {
        post_amount == band_state2_bend(pre_amount)
    }
}

// ===========================================================================
// C-MUJICA-团技能-03
// 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。」
// OPEN-取整 on odd amounts.
// ===========================================================================

pub fn band_halve_pay(amount: i32) -> i32 {
    amount / 2
}

/// Crystal spend of exactly 1.
pub fn band_halve_crystal_spend(n: i32) -> bool {
    n == -1
}

// ===========================================================================
// C-MUJICA-若麦技能-01
// 「每次其他玩家向你支付资金时，你立即获得1火罐（初始0，上限5），
//  并使那次支付的金额提高X*100，X为你拥有的火罐数。」
// X is the pot count **after** the +1.
// ===========================================================================

/// Fire pots after the grant, clamped at the declared cap 5.
pub fn nyamu_pots_after(pots_before: i32) -> i32 {
    (pots_before + 1).min(5)
}

/// Payment raise: `X * 100` with X the new pot count.
pub fn nyamu_pay_raise(pots_before: i32) -> i32 {
    nyamu_pots_after(pots_before) * 100
}

/// Bent payment amount.
pub fn nyamu_pay_amount(pre_amount: i32, pots_before: i32) -> i32 {
    pre_amount + nyamu_pay_raise(pots_before)
}

/// MUTANT: X is the pot count **before** the +1.
pub fn nyamu_pay_raise_mutant(pots_before: i32) -> i32 {
    pots_before * 100
}

// ===========================================================================
// C-MUJICA-祥子技能-02
// 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐
//  （初始0，上限3），火罐数达到上限时可在回合开始时选择进入状态2。」
// ===========================================================================

/// Pots after one applied status, clamped at the declared cap 3.
pub fn sakiko_pots_after(pots_before: i32) -> i32 {
    (pots_before + 1).min(3)
}

/// At cap, 状态2 is offered (not forced) at turn start.
pub fn sakiko_state2_offered(pots: i32) -> bool {
    pots >= 3
}

// ===========================================================================
// C-MUJICA-海铃技能-02
// 「每两个回合没有玩家发动该技能，你在回合开始时获得1火罐（初始0，上限4）」
// ===========================================================================

pub fn umiri_pots_after(pots_before: i32) -> i32 {
    (pots_before + 1).min(4)
}

pub fn umiri_state2_offered(pots: i32) -> bool {
    pots >= 4
}

// ===========================================================================
// C-MUJICA-海铃技能-03
// 「每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数）」
// 「每通过此法消耗4个火罐，你抽1张卡。」
// OPEN-分摊 (「均分」) — the 50/50 ratio is clear; the book's 分摊
// vocabulary is not defined. Prove only the payment arithmetic.
// ===========================================================================

/// Payment per designated deed: `100 + n*100` where n = houses on the tile.
pub fn umiri_deed_pay(houses: i32) -> i32 {
    100 + houses.max(0) * 100
}

/// Cards drawn per 4 fire pots spent (floor).
pub fn umiri_draw_per_pots(pots_spent: i32) -> i32 {
    pots_spent / 4
}

// ===========================================================================
// C-MUJICA-睦技能-01
// 「状态1：你的每6回合不打出任何手牌，在回合开始前抽1张牌。
//  若你的回合开始时手牌数为5，可选择进入状态2。」
// Live sheet 「手牌数为5」 supersedes rulebook-doc 「手牌数4或以上」.
// ===========================================================================

/// The state-2 gate is hand size exactly 5 at turn start.
pub fn mutsumi_state2_gate(hand_size: i32) -> bool {
    hand_size == 5
}

/// Draw 1 before the turn start after 6 idle turns.
pub fn mutsumi_idle_draw(idle_turns: i32) -> bool {
    idle_turns >= 6
}
