//! `tile:ring` -- RiNG's [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, line 102, the parenthetical）:
//! > 每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。
//!
//! RiNG is a [可购买格子] (line 19: 「[可购买格子]：非CiRCLE，CiRCLE 咖啡厅，江户川
//! 乐器店，流星堂，或[地产商]的格子」 -- RiNG is none of those), so the whole of
//! lines 100–106 applies to it with one exception: **it cannot be upgraded**.
//! That is `prop::BUILD_MAX = 0` (its `rent` table is empty).
//!
//! The rent itself is **not** rulebook text. `data/match_rules.json`'s note
//! says 「RiNG 地租 = 地主拥有的 RiNG 数量 × ringMultiplier × 1d20」, and the
//! engine's `pay_rent` computes it. TODO(规则书): the dice-rent table and the
//! 「至少1个」 floor on the ring count have no book clause -- `data/rules.txt`
//! only says 「所有RiNG不可升级」.
//!
//! So this body is [`super::property::settle`] (the four-way branch) with the
//! RiNG facts already on the instance: `buildMax = 0`, `ringMult` = the match
//! rule. `ctx::pay_rent` picks the dice formula because the tile's `kind` is
//! `ring`.

use card_sdk::abi::prop;
use card_sdk::{CardDef, On};

/// Default dice-rent multiplier (match_rules note 「× ringMultiplier」).
/// The engine charges `count_owned_kind(owner, kind) × rentDiceMult × 1d20`.
pub const RENT_DICE_MULT: i32 = 10;

pub const RING: CardDef = CardDef::new("tile:ring", &[On::Settle("", None, settle)])
    .board_role("tile")
    .props(&[
        (prop::NO_MORTGAGE, 1),
        (prop::RENT_DICE_MULT, RENT_DICE_MULT),
    ]);

/// 规则书: the [可购买格子] branch (lines 100–106) with 「所有RiNG不可升级」
/// folded in as `buildMax = 0`. Dice rent is `prop::RENT_DICE_MULT` (see above).
fn settle(player_id: i32) -> card_sdk::Asked {
    super::property::settle(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn settle_kani(player_id: i32) -> card_sdk::Asked {
    settle(player_id)
}
