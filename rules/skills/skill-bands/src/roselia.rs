//! `skill:Roselia:对音乐的纯粹`
//!
//! 规则书（band sheet, Roselia）:
//! > （1）本局游戏购买的第一个非Live House格子购买价格减半，在你拥有任意初始Live
//! > House格子前，那个格子对你视为Live House。（2）购买任何Live House格子的地契时
//! > 购买价格减半
//!
//! （1） is a one-shot on the first qualifying buy, plus a standing recolour
//! while the condition holds. 「初始Live House格子」 is a tile whose own group is
//! the Live House one -- not one this skill recoloured.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const DONE: &str = "skill.roselia.first";

/// The Live House tile group.
const LIVE: i32 = 6;

pub const ROSELIA: CardDef = CardDef::new("skill:Roselia:对音乐的纯粹", &[
    On::Hook(&[HookKind::BuyBefore], mine, before_buy),
    On::Hook(&[HookKind::Bought], mine, on_bought),
    On::Hook(&[HookKind::TurnStartBefore], mine, at_turn_start)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （2）「购买任何Live House格子的地契时购买价格减半」, and （1）'s one-shot.
fn before_buy(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if t < 0 {
        return Ok(());
    }
    if ctx::is_live_house_for(player_id, t) {
        // （2） -- always, for a Live House.
        ctx::set_buy_discount(ctx::buy_price(t) / 2);
        return Ok(());
    }
    // （1）「本局游戏购买的第一个非Live House格子购买价格减半」
    if state::get(player_id, DONE) != 0 {
        return Ok(());
    }
    ctx::set_buy_discount(ctx::buy_price(t) / 2);
    Ok(())
}

fn on_bought(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, DONE) != 0 {
        return Ok(());
    }
    let t = ctx::trigger::tile();
    if t < 0 || ctx::tile_group(t) == LIVE {
        return Ok(());
    }
    state::set(player_id, DONE, 1);
    ctx::log(player_id, &Msg::new(key!("roselia_first")).tile("tile", t));
    Ok(())
}

/// （1）「在你拥有任意初始Live House格子前，那个格子对你视为Live House」 --
/// restated each turn while the condition holds.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, DONE) == 0 {
        return Ok(());
    }
    if ctx::owned_tiles(player_id).into_iter().any(|t| ctx::tile_group(t) == LIVE) {
        return Ok(());
    }
    // The recoloured tile is the one this skill half-priced; find it by the
    // extra-colour claim the same way the colour cards do.
    for t in ctx::owned_tiles(player_id) {
        if ctx::is_live_house_for(player_id, t) {
            return Ok(());
        }
    }
    if let Some(t) = ctx::owned_tiles(player_id).into_iter().next() {
        ctx::set_extra_color(player_id, t, LIVE);
    }
    Ok(())
}