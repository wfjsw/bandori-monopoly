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
//! `RollAfter` is where the first face exists; a second roll and the difference
//! replace it.
//!
//! (3) 「三个回合内没有拿过Circle奖励」 is a three-turn silence, tracked as a
//! counter that a CiRCLE reward resets; 「下回合移动投掷改为1d20+1d4」 is the
//! dice table being rewritten when it fires.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// Turns since the last CiRCLE reward. Cleared on the reward.
const SILENT: &str = "skill.kanonLost.silent";

pub const KANON_LOST: CardDef = CardDef::new(
    "skill:松原花音:真正的迷子",
    &[
        // (1) is a starting square: the before-match-start point decides start
        // positions.
        On::Hook(&[HookKind::DeckBeforeGame], None, at_start, ""),
        On::Hook(&[HookKind::RollAfter], None, on_roll, card_sdk::pre::MINE),
        On::Hook(&[HookKind::CircleAffected], None, on_circle, card_sdk::pre::MINE),
        On::Hook(&[HookKind::TurnEnd], None, at_turn_end, card_sdk::pre::MINE),
        On::Hook(&[HookKind::RollPlan], None, on_plan, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine), (3, legacy_mine), (4, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

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
fn on_roll(player_id: i32) -> card_sdk::Asked {
    if ctx::fixed_roll().is_some() {
        return Ok(());
    }
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
/// the silence holds.
fn on_plan(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, SILENT) < 3 {
        return Ok(());
    }
    plan::set_base_dice(1, 20, "真正的迷子");
    plan::add_base_dice(1, 4, "真正的迷子");
    ctx::log(player_id, &Msg::new(key!("kanon_lost_dice")));
    Ok(())
}
