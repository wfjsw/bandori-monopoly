//! Unit `r` / Roselia skills: reference specs FROM the skill sheet text only.
//!
//! Sources: the roster character row `text` (band == "Roselia"), `the roster`
//! Roselia band skill 对音乐的纯粹, and `docs/rulebook/spec/cards/r.md`
//! (clause IDs `C-R-*`). OPEN items stay unasserted.

use crate::world::Effect;

// ===========================================================================
// C-R-踏上荆棘之路的觉悟-01 -- 冰川纱夜
// 「（1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个[火罐]
//  （初始10，上限10）」
// 「（3）第一个玩家被淘汰，经过Circle时获得的火罐加1」
// ===========================================================================

/// CiRCLE-pass gain: 2, or 3 once any player is out.
pub fn sayo_circle_gain(someone_out: bool) -> i32 {
    if someone_out {
        3
    } else {
        2
    }
}

/// Being-passed gain: 1.
pub fn sayo_passed_gain() -> i32 {
    1
}

/// Cap the sheet pins (host `charge_max`; Kani `gain_charge` does not clamp).
pub fn sayo_fire_cap() -> i32 {
    10
}

/// Exactly one `GainFire` of `n`.
pub fn sayo_gain_charge_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n: m } if p == player && m == n)
}

// ===========================================================================
// C-R-即使1cm也要前进-01 -- 白金燐子
// 「（1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）」
// ===========================================================================

pub fn rinko_circle_gain() -> i32 {
    3
}

pub fn rinko_fire_cap() -> i32 {
    3
}

pub fn rinko_gain_charge_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n } if p == player && n == rinko_circle_gain())
}

// ===========================================================================
// C-R-慈爱女神-01 -- 今井莉莎
// 「（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）」
// ===========================================================================

pub fn lisa_circle_gain() -> i32 {
    1
}

pub fn lisa_fire_cap() -> i32 {
    2
}

pub fn lisa_gain_charge_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n } if p == player && n == lisa_circle_gain())
}

// ===========================================================================
// C-R-来练习吧-04 / -05 -- 凑友希那
// 「（3）移动终点为任意"RiNG"时，获得一个火罐（上限1）。任何时刻当你不位于RiNG时，
//  失去所有的火罐。」
// ===========================================================================

pub fn yukina_ring_gain() -> i32 {
    1
}

pub fn yukina_fire_cap() -> i32 {
    1
}

/// Off every RiNG: fire is 0.
pub fn yukina_drop_ok(post_fire: i32) -> bool {
    post_fire == 0
}

// ===========================================================================
// C-R-对音乐的纯粹-01/03 -- band skill (BLOCKED-HOST on `trig_price`)
// 「（1）本局游戏购买的第一个非Live House格子购买价格减半」
// 「（2）购买任何Live House格子的地契时购买价格减半」
// ===========================================================================

pub fn band_half(price: i32) -> i32 {
    price / 2
}

/// Standing rule: a Livehouse buy is always half; a non-Livehouse buy is half
/// only while the one-shot (`skill.roselia.first`) is unused.
pub fn band_buy_price(price: i32, is_livehouse: bool, first_done: bool) -> i32 {
    if is_livehouse {
        band_half(price)
    } else if first_done {
        price
    } else {
        band_half(price)
    }
}

/// MUTANT: three-quarters.
pub fn band_half_mutant(price: i32) -> i32 {
    price * 3 / 4
}

// ===========================================================================
// OPEN refs (greppable)
// ===========================================================================

pub const OPEN_BAND_PRETEND: &str = "OPEN-BAND-PRETEND";
pub const OPEN_TRIG_SETTLE: &str = "OPEN-触发结算";
pub const OPEN_QIANGZHI: &str = "OPEN-强制停下";
pub const OPEN_SAYO_TIMING: &str = "OPEN-SAYO-TIMING";
