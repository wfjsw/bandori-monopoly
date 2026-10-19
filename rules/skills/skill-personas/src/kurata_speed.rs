//! `skill:仓田真白:向后全速前进`
//!
//! 规则书（skill sheet, 仓田真白）:
//! > （1）[主动移动]时移动掷骰变为2d20
//! > （2）反方向移动
//!
//! Both halves shape the move as it is being planned, which is exactly
//! [`On::RollPlan`]: the plan's dice table and its direction. (1) replaces the
//! default 1d20, (2) reverses the walk. 「[主动移动]」 is the turn's main move,
//! so a card-driven side move keeps its own dice.

use card_sdk::abi::FilterKind;
use bandori_std::ctx::{self, plan};
use card_sdk::{key, CardDef, Msg, On};

pub const KURATA_SPEED: CardDef = CardDef::new(
    "skill:仓田真白:向后全速前进",
    // 「[主动移动]」 is the turn's main move, whose mover is `turn_player` --
    // the body-top `turn_player != player_id` early-out is the condition now.
    &[On::FilterValue(&[FilterKind::DiceRollPlan], "turn_player == owner", None, roll_plan)],
);

/// （1）「[主动移动]时移动掷骰变为2d20」, （2）「反方向移动」.
fn roll_plan(player_id: i32, v: card_sdk::abi::FilterValue) -> card_sdk::abi::FilterValue {
    let _ = player_id;
    // （1） -- the face is 2d20, not the default 1d20.
    // （2） -- the walk runs backwards.
    ctx::log(player_id, &Msg::new(key!("kurata_speed_plan")));
    card_sdk::abi::FilterValue {
        base_count: 2,
        base_sides: 20,
        reverse: 1,
        ..v
    }
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn roll_plan_kani(player_id: i32, v: card_sdk::abi::FilterValue) -> card_sdk::abi::FilterValue {
    roll_plan(player_id, v)
}
