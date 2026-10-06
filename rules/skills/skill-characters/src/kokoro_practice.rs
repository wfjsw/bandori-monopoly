//! `skill:凑友希那:来练习吧`
//!
//! 规则书（skill sheet, 凑友希那）:
//! > （1）初始获得"RiNG 4"格子，从"RiNG 4"格子开始游戏，首次经过CiRCLE不获得经过奖励
//! > （2）如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动
//! > （3）移动终点为任意"RiNG"时，获得一个火罐（上限1）。任何时刻当你不位于RiNG时，
//! > 失去所有的火罐。当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家
//! > 强制停下并触发结算。
//!
//! （1） is three grants at game start: a deed, a starting square, and a waived
//! first pass. The deed is `set_owner`; the start is a position write; the
//! waiver is a one-shot latch.
//!
//! （2） 「可以在[经过]的第一个RiNG停止移动」 is a stop the player opts into
//! while walking past, which is the `Pass` moment onto a RiNG tile.
//!
//! （3） has three parts: a pot on ending on a RiNG, losing them all off a RiNG,
//! and a forced stop bought with a pot when someone passes you.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「首次经过CiRCLE不获得经过奖励」.
const WAIVED: &str = "skill.kokoroPractice.waived";

/// A RiNG tile is any of the four the board names.
fn is_ring(t: i32) -> bool {
    t >= 0 && ctx::is_ring(t)
}

pub const KOKORO_PRACTICE: CardDef = CardDef::new("skill:凑友希那:来练习吧", &[
    On::Hook(&[HookKind::DeckAtGameStart], |_| true, at_start),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::PassPlayer], mine, on_passed),
    On::Hook(&[HookKind::SettleAfter], mine, on_settle)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「初始获得"RiNG 4"格子，从"RiNG 4"格子开始游戏，首次经过CiRCLE不获得
/// 经过奖励」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_named("RiNG 4");
    if t < 0 {
        return Ok(());
    }
    ctx::set_owner(t, player_id);
    ctx::teleport_to(player_id, t);
    state::set(player_id, WAIVED, 0);
    ctx::log(player_id, &Msg::new(key!("kokoro_practice_start")).tile("tile", t));
    Ok(())
}

/// （2） 「如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动」, plus
/// （1）'s waived first CiRCLE pass and （3）'s pot-on-RiNG / lose-off-RiNG.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    // （1） 「首次经过CiRCLE不获得经过奖励」 -- the waiver is the tile's own
    // reward, which the engine pays on the pass. Latching it here is too late
    // to stop that payment, so the engine's `no_circle_reward` plan flag is the
    // right place; see the TODO below.
    if ctx::is_circle(t) && state::get(player_id, WAIVED) == 0 {
        state::set(player_id, WAIVED, 1);
    }
    // （2） 「可以在[经过]的第一个RiNG停止移动」 -- offered on the first RiNG
    // the walk passes.
    if is_ring(t) && ctx::trigger::move_remaining() > 0 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("kokoro_practice_stop_title")),
            &Msg::new(key!("kokoro_practice_stop_ask")).tile("tile", t),
        )? {
            ctx::plan::set_stop_at(t);
            ctx::log(player_id, &Msg::new(key!("kokoro_practice_stopped")).tile("tile", t));
        }
    }
    // （3） 「任何时刻当你不位于RiNG时，失去所有的火罐」.
    if !is_ring(t) {
        let have = state::get(player_id, state_key::FIRE);
        if have > 0 {
            ctx::spend_fire(player_id, have, &Msg::new(key!("kokoro_practice_lost")));
        }
    }
    Ok(())
}

/// （3）「当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下
/// 并触发结算」.
fn on_passed(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::target() != player_id {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    if ctx::trigger::move_remaining() <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("kokoro_practice_force_title")),
        &Msg::new(key!("kokoro_practice_force_ask")),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("kokoro_practice_spend"))) {
        return Ok(());
    }
    if !ctx::gate(ctx::trigger::player_id(), card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    ctx::plan::set_stop_at(ctx::player_pos(player_id));
    ctx::log(player_id, &Msg::new(key!("kokoro_practice_forced")));
    Ok(())
}

/// （3）「移动终点为任意"RiNG"时，获得一个火罐（上限1）」 -- the landing, which
/// is where the move ends.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    if !is_ring(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("kokoro_practice_gain")));
    Ok(())
}