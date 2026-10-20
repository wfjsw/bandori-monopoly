//! `skill:松原花音:真正的迷子`
//!
//! 规则书（skill sheet, 松原花音）:
//! > (1) 游戏开始起点为#30弦卷豪宅
//! > (2) 移动阶段进行两次移动掷骰并将它们的结果相减作为你的移动格数
//! > (3) 如果三个回合内没有拿过Circle奖励，则下回合移动投掷改为1d20+1d4
//!
//! (1) is a starting square, which is a position write at game start.
//!
//! (2) 「两次移动掷骰并将它们的结果相减」 -- the face is `|a - b|`, not a sum.
//! `DiceRollAfter` is where the first face exists; a second roll and the difference
//! replace it.
//!
//! (3) 「三个回合内没有拿过Circle奖励」 is a three-turn silence, tracked as a
//! counter that a CiRCLE reward resets; 「下回合移动投掷改为1d20+1d4」 is the
//! dice table being rewritten when it fires.

use card_sdk::abi::{ActionKind, Dice, FilterKind};
use bandori_std::ctx::{self, state};
use card_sdk::{key, CardDef, FilterBody, Msg, On, PlanValue};

/// Turns since the last CiRCLE reward. Cleared on the reward.
const SILENT: &str = "skill.kanonLost.silent";

pub const KANON_LOST: CardDef = CardDef::new(
    "skill:松原花音:真正的迷子",
    &[
        // (1) is a starting square: the before-match-start point decides start
        // positions.
        On::Action(&[ActionKind::DeckBeforeGame], "", "", None, at_start),
        // (2) 「两次移动掷骰并将它们的结果相减」 -- a fixed face replaces the
        // dice, so there are not two faces to subtract.
        On::Action(&[ActionKind::DiceRollAfter], "actor == owner && plan.fixed_roll == null", "", None, on_roll),
        On::Action(&[bandori_std::kind::PassRewardAffected], "actor == owner && slot('skill.kanonLost.silent') >= 3", "", None, on_circle),
        On::Action(&[ActionKind::TurnEnd], "actor == owner && slot('skill.kanonLost.silent') >= 3", "", None, at_turn_end),
        // TODO(bug): possible engine issue — a `slot()` condition on a
        // FilterValue at DiceRollPlan did not observe a state write made
        // earlier in the same roll (seen while splitting eve_unify: the
        // condition evaluated before the mid-roll stash was visible, while
        // `state::get` inside the body saw it). This entry's gate is exactly
        // that pattern (`slot('skill.kanonLost.silent')` counting up during
        // the roll). Verify whether the gate ever mis-fires here; if it does,
        // fix the condition evaluation snapshot (or move the gate into the
        // body) and re-run the kanon tests + a two-roll scenario.
        On::FilterValue(&[FilterKind::DiceRollPlan], "actor == owner && slot('skill.kanonLost.silent') >= 3", None, FilterBody::Plan(on_plan)),
    ],
)
    .silent(&[2, 3]);

/// (1) 「游戏开始起点为#30弦卷豪宅」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_named("弦卷豪宅");
    if t >= 0 {
        ctx::teleport_to(player_id, t);
        ctx::log(
            player_id,
            &Msg::new(key!("kanon_lost_start")).tile("tile", t),
        );
    }
    Ok(())
}

/// (2) 「移动阶段进行两次移动掷骰并将它们的结果相减作为你的移动格数」.
/// Applicability is the entry's condition (a fixed face means no pair to subtract).
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let a = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    let b = ctx::do_move_roll(player_id).max(0);
    let face = (a - b).abs();
    ctx::trigger::set_move_roll(face);
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_lost_sub"))
            .i("a", a as i64)
            .i("b", b as i64)
            .i("n", face as i64),
    );
    Ok(())
}

/// (3) 「如果三个回合内没有拿过Circle奖励」 -- the reward resets the counter.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, SILENT, 0);
    Ok(())
}

fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, SILENT) + 1;
    state::set(player_id, SILENT, n);
    Ok(())
}

/// (3) 「下回合移动投掷改为1d20+1d4」 -- the plan's dice table, rewritten while
/// the silence holds. Pure: the base pair is `base` and the
/// extra d4 rides `add_base` (the wrapper appends it).
fn on_plan(player_id: i32, mut v: PlanValue) -> PlanValue {
    v.base = Some(Dice { count: 1, sides: 20 });
    v.add_base = Some(Dice { count: 1, sides: 4 });
    ctx::log(player_id, &Msg::new(key!("kanon_lost_dice")));
    v
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_kani(player_id: i32) -> card_sdk::Asked {
    on_roll(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_circle_kani(player_id: i32) -> card_sdk::Asked {
    on_circle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_kani(player_id: i32, v: PlanValue) -> PlanValue {
    on_plan(player_id, v)
}
