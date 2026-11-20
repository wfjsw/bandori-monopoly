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

use bandori_std::ctx::{self, plan};
use card_sdk::abi::{Dice, FilterKind};
use card_sdk::{CardDef, Filter, On, PlanValue};

pub const MASHIRO: CardDef = CardDef::new(
    "skill:仓田真白:向后全速前进",
    // 「[主动移动]」 is the turn's main move, whose mover is `turn_player` --
    // the body-top `turn_player != player_id` early-out is the condition now.
    &[On::Filter(Filter::DiceRollPlan(
        "turn_player == owner",
        None,
        roll_plan,
    ))],
)
.rule();

/// （1）「[主动移动]时移动掷骰变为2d20」, （2）「反方向移动」. Passive on
/// every main move: the plan pass is bookkeeping, so the skill writes no
/// log line and does not flash -- the walk's own line is the outcome.
fn roll_plan(_player_id: i32, v: PlanValue) -> PlanValue {
    // （1） -- the face is 2d20, not the default 1d20.
    // （2） -- the walk runs backwards.
    PlanValue {
        base: Some(Dice {
            count: 2,
            sides: 20,
        }),
        reverse: Some(true),
        ..v
    }
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn roll_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    roll_plan(player_id, v)
}
