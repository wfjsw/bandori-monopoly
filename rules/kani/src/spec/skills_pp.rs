//! Unit `skills_pp`: reference specs FROM the clause docs / sheet text only.
//!
//! Sources: `the roster` band `Pastel✽Palettes` (`text` field),
//! `the roster` skill `与偶像一起`. Quotes in
//! `docs/rulebook/spec/cards/pp.md` § Character skills / Band skill.
//! Never derived from engine behaviour.

use crate::world::Effect;

// ===========================================================================
// Shared (1) on every PP character (C-PP-<slug>-01):
// 「（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得<角色>的（2）技能」
// ===========================================================================

/// The 5-fan half: the character gains exactly 5 positive fans.
pub fn char_start_five_fans(pre_pos: i32) -> i32 {
    pre_pos + 5
}

/// Exact effect of the 5-fan half: one AddToken(+5) on the positive fan key.
pub fn char_start_five_fans_log_ok(
    effects: &[Effect],
    player: i32,
    pos_key: u16,
) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key, n: 5 } if p == player && key == pos_key
        )
}

// ===========================================================================
// C-PP-aya-with-02 / chisato-frank-02 / maya-dawn-02 / eve-unify-02
// share the trailing fan-side flip:
// 「如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，
//  否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正」
// ===========================================================================

/// How many reverse fans flip positive on `target` when the actor chose `y`
/// and `actor_is_pp` selects the branch.
/// PP branch: each of `y` reverse fans on *other-band* players (OPEN: how
/// they split across those players is not printed — the per-player cap is
/// `min(y, that player's down)` in the live reading, applied to each).
/// Non-PP branch: effect X = 1 per PP character (「1个反面…变正」) — the
/// `fansFlip` handler flips `min(1, down)`. When `down == 0` the flip is 0
/// and band clause （2） sees `X=1 > down=0`, granting 1 overflow crystal.
pub fn skill2_side_flip(target_down: i32, y: i32, actor_is_pp: bool, target_is_pp: bool) -> i32 {
    if actor_is_pp {
        if target_is_pp {
            0
        } else {
            target_down.min(y.max(0))
        }
    } else if target_is_pp {
        target_down.min(1)
    } else {
        0
    }
}

/// Band clause （2） overflow for the non-PP branch's per-seat `X = 1`.
/// `min(1 - down, 10)` when `1 > down`, else 0.
pub fn skill2_all_pp_overflow(target_down: i32) -> i32 {
    band_idol_02_overflow(1, target_down)
}

// ===========================================================================
// C-PP-aya-with-02
// 「（2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前
//  资金减少Y×100（最少0）；…」
// ===========================================================================

/// Post pay amount after the optional discount.
pub fn aya_with_02_amount(pre_amount: i32, y: i32) -> i32 {
    (pre_amount - y * 100).max(0)
}

/// Flip `y` positive fans reverse (capped at the actor's positives).
pub fn aya_with_02_flip_pos(up: i32, y: i32) -> i32 {
    up.min(y.max(0))
}

/// Y is chosen in `0..=up` (「自己Y个正面」).
pub fn aya_with_02_y_ok(y: i32, up: i32) -> bool {
    y >= 0 && y <= up
}

// ===========================================================================
// C-PP-chisato-frank-02
// 「（2）自己每次收取资金时可选择将自己Y个正面[P✽P粉丝]变反，此次获得的分摊前数量
//  增加Y×100；…」
// ===========================================================================

/// Post receive amount after the optional boost.
pub fn chisato_frank_02_amount(pre_amount: i32, y: i32) -> i32 {
    pre_amount + y * 100
}

pub fn chisato_frank_02_flip_pos(up: i32, y: i32) -> i32 {
    up.min(y.max(0))
}

// ===========================================================================
// C-PP-hina-lottery-02
// 「（2）每回合开始时必须进行1次投掷1d4，根据最后投掷的结果在下回合开始前获得以下
//  角色的（2）技能：结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；
//  结果为4是大和麻弥。」
// ===========================================================================

/// Borrowed character index from a 1d4 result (0=aya, 1=chisato, 2=eve, 3=maya).
pub fn hina_lottery_02_borrow(roll: i32) -> i32 {
    // 1->0, 2->1, 3->2, 4->3
    roll - 1
}

pub fn hina_lottery_02_roll_ok(roll: i32) -> bool {
    (1..=4).contains(&roll)
}

// ===========================================================================
// C-PP-maya-dawn-02
// 「（2）自己抽卡时时可选择将自己Y个正面[P✽P粉丝]变反，此次抽卡改为观看卡组顶端
//  Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择一张牌（不公开）加入手牌
//  （此次加手视为抽卡动作），剩余观看的牌洗回卡组；…」
// ===========================================================================

/// Watch size = `min(Y + 1, deck_n)`.
pub fn maya_dawn_02_watch(y: i32, deck_n: i32) -> i32 {
    (y + 1).min(deck_n).max(0)
}

/// Exactly one watched card joins the hand (counts as a draw).
pub fn maya_dawn_02_hand_n() -> i32 {
    1
}

// ===========================================================================
// C-PP-eve-unify-02
// 「（2）当你因任意原因投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加
//  Yd4；…」
// BLOCKED-HOST: `trig_move_roll` is unmodeled (KANI-COVERAGE).
// ===========================================================================

/// Extra dice added = Y (each a d4). Sum range of the extra is `Y..=4Y`.
pub fn eve_unify_02_extra_dice(y: i32) -> i32 {
    y.max(0)
}

pub fn eve_unify_02_extra_sum_ok(extra_sum: i32, y: i32) -> bool {
    extra_sum >= y && extra_sum <= 4 * y
}

// ===========================================================================
// C-PP-band-idol-01
// 「（1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
//  Pastel✽Palettes乐队卡时此效果不重复发动）。」
// ===========================================================================

/// Each non-PP living character gains exactly 1 reverse fan; fires once.
pub fn band_idol_01_fan() -> i32 {
    1
}

/// Non-repeat: at most one dealing pass even with several PP band cards.
pub fn band_idol_01_fires_once(copies: i32) -> i32 {
    if copies >= 1 {
        1
    } else {
        0
    }
}

// ===========================================================================
// C-PP-band-idol-02
// 「（2）你因任意原因受到“将X个反面[P✽P粉丝]变正”的效果且X大于拥有的反面的
//  [P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。」
// ===========================================================================

/// Overflow crystals = `min(X - down, 10)`, and only when `X > down`.
pub fn band_idol_02_overflow(x: i32, down: i32) -> i32 {
    if x > down {
        (x - down).min(10).max(0)
    } else {
        0
    }
}

// ===========================================================================
// C-PP-band-idol-03
// 「（3）[经过]CiRCLE时不获得[CiRCLE奖励]。」
// ===========================================================================

/// No money / draw from the CiRCLE reward step.
pub fn band_idol_03_no_reward(effects: &[Effect]) -> bool {
    // The clause forbids the reward; a passing player with this band skill
    // must not gain the CiRCLE money or draw. `effects` here is the reward
    // step's log — it must be empty.
    effects.is_empty()
}

// ===========================================================================
// C-PP-band-idol-04
// 「（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。」
// ===========================================================================

/// Post crystals after the automatic +1 (before any optional spend).
pub fn band_idol_04_crystals_after_tick(pre: i32) -> i32 {
    pre + 1
}

/// If the player takes the option: crystals -5, draw 1. Requires >= 5 after
/// the tick (the text says 「移除此卡5个」 — the spend needs 5 present).
pub fn band_idol_04_spend_ok(pre: i32, take: bool) -> bool {
    if !take {
        true
    } else {
        pre + 1 >= 5
    }
}

/// Post crystals when the option is taken.
pub fn band_idol_04_crystals_after_spend(pre: i32) -> i32 {
    (pre + 1) - 5
}

// ===========================================================================
// Mutants
// ===========================================================================

pub fn aya_with_02_amount_mutant(pre_amount: i32, y: i32) -> i32 {
    (pre_amount - y * 99).max(0)
}

pub fn band_idol_02_overflow_mutant(x: i32, down: i32) -> i32 {
    if x > down {
        (x - down).min(11).max(0)
    } else {
        0
    }
}

pub fn char_start_five_fans_mutant(pre_pos: i32) -> i32 {
    pre_pos + 4
}