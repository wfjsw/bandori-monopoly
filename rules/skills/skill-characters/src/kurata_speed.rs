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

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan};
use card_sdk::{key, CardDef, Msg, On};

pub const KURATA_SPEED: CardDef =
    CardDef::new("skill:仓田真白:向后全速前进", &[On::RollPlan(roll_plan)]);

/// （1）「[主动移动]时移动掷骰变为2d20」, （2）「反方向移动」.
fn roll_plan(player_id: i32) -> card_sdk::Asked {
    if ctx::turn_player() != player_id {
        return Ok(());
    }
    // （1） -- the face is 2d20, not the default 1d20. `set_base_dice` clears
    // whatever the plan started from.
    plan::set_base_dice(2, 20, "向后全速前进");
    // （2） -- the walk runs backwards.
    plan::set_reverse(true);
    ctx::log(player_id, &Msg::new(key!("kurata_speed_plan")));
    Ok(())
}
