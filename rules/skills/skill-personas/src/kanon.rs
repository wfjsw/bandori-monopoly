//! `skill:松原花音:真正的迷子`
//!
//! 规则书（skill sheet, 松原花音）:
//! > [自动]：
//! > （1）游戏开始时：自己的起点为“弦卷豪宅”（30号格子）。
//! > （2）自己的移动阶段：进行2次用于确定移动步数的[投掷]，将两次结果相减，作为
//! > 自己的移动格数（若为负数则反向移动）。
//! > （3）如果自己在三个回合内没有领取[CiRCLE奖励]：获得[呼诶诶]状态。
//! > [呼诶诶]状态：用于确定移动步数的[投掷]改为1d20+1d4。领取CiRCLE奖励后移除该状态。
//!
//! （1） is a starting square, which is a position write at game start.
//!
//! （2） 「将两次结果相减」 is a **signed** difference: a negative face walks
//! backwards (`plan::set_reverse`), so it is not `|a - b|`. `DiceRollAfter` is
//! where the first face exists; a second roll and the difference replace it.
//!
//! （3） 「三个回合内没有领取[CiRCLE奖励]」 is a three-turn silence, tracked as a
//! counter a CiRCLE reward resets; when it reaches three the [呼诶诶] latch is
//! gained (held until a reward, which clears both), and 「用于确定移动步数的
//! [投掷]改为1d20+1d4」 is the dice table being rewritten while it holds.

use bandori_std::ctx::{self, plan, state};
use bandori_std::kind::Actions;
use bandori_std::state_key;
use card_sdk::abi::{ActionKind, Dice, FilterKind};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PlanValue};

/// Turns since the last CiRCLE reward. Cleared on the reward.
const SILENT: &str = "skill.kanonLost.silent";
/// The [呼诶诶] latch: gained at three silent turns, spent by the next CiRCLE
/// reward.
const HEHE: &str = "skill.kanonLost.hehe";

pub const KANON: CardDef = CardDef::new(
    "skill:松原花音:真正的迷子",
    &[
        // （1） is a starting square: the before-match-start point decides start
        // positions.
        On::Action(Action::DeckBeforeGame("", None, at_start)),
        // （2） 「两次移动掷骰并将它们的结果相减」 -- a fixed face replaces the
        // dice, so there are not two faces to subtract.
        On::Action(Action::DiceRollAfter(
            "actor == owner && plan.fixed_roll == null",
            None,
            on_roll,
        )),
        // （3） 「领取CiRCLE奖励后移除该状态」 -- the reward resets the counter
        // and spends the [呼诶诶] latch.
        On::Action(Actions::PassRewardAffected(
            "actor == owner && slot('skill.kanonLost.silent') >= 3",
            None,
            on_circle,
        ))
        .silent(),
        // （3） the silence counts every own turn end; three of them latch.
        On::Action(Action::TurnEnd("actor == owner", None, at_turn_end)).silent(),
        // `state(seat, 'key')` is the reader for the plain player-state row the
        // latch is written to (`at_turn_end` / `on_circle` use `state::set`);
        // `slot()` only sees the fixed instance latch table, where this key is
        // absent. On a DiceRollPlan condition a `state()` read is live world
        // state (COND-01), so a write earlier in the same window is visible to
        // this gate exactly as `state::get` inside the body would see it.
        On::Filter(Filter::DiceRollPlan(
            "actor == owner && state(actor, 'skill.kanonLost.hehe') > 0",
            None,
            on_plan,
        )),
    ],
)
.rule();

/// （1） 「游戏开始起点为#30弦卷豪宅」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_named("弦卷豪宅");
    if t >= 0 {
        ctx::teleport_to(player_id, t);
        ctx::log(player_id, &Msg::new(key!("kanon_start")).tile("tile", t));
    }
    Ok(())
}

/// （2） 「进行两次移动掷骰并将它们的结果相减作为你的移动格数（若为负数则反向
/// 移动）」. Applicability is the entry's condition (a fixed face means no pair
/// to subtract).
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let a = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    let b = ctx::do_move_roll(player_id)?.max(0);
    // Signed difference: a negative result walks the other way.
    let face = if a < b {
        plan::set_reverse(true);
        b - a
    } else {
        a - b
    };
    ctx::trigger::set_move_roll(face);
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_sub"))
            .i("a", a as i64)
            .i("b", b as i64)
            .i("n", face as i64),
    );
    Ok(())
}

/// （3） 「领取CiRCLE奖励后移除该状态」 -- the reward resets the counter and
/// clears the latch.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, SILENT, 0);
    state::set(player_id, HEHE, 0);
    Ok(())
}

/// （3） the silence counts up; at three turns it latches [呼诶诶].
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, SILENT) + 1;
    state::set(player_id, SILENT, n);
    if n >= 3 {
        state::set(player_id, HEHE, 1);
    }
    Ok(())
}

/// （3） 「用于确定移动步数的[投掷]改为1d20+1d4」 -- the plan's dice table,
/// rewritten while the latch holds. Pure: the base is `1d20` and the extra d4
/// rides `add_base` (the wrapper appends it).
fn on_plan(player_id: i32, mut v: PlanValue) -> PlanValue {
    v.base = Some(Dice {
        count: 1,
        sides: 20,
    });
    v.add_base = Some(Dice { count: 1, sides: 4 });
    // Passive plan reshape: no header, no flash, no line of its own.
    v
}

//

//

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
pub fn on_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    on_plan(player_id, v)
}
