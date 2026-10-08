//! `skill:朝日六花:瞄准目标`
//!
//! 规则书（skill sheet, 朝日六花）:
//! > （1）你购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色
//! > （2）在你有初始"Live House"格子时你拥有的"旭汤澡堂"格子获得"Live House"的颜色
//!
//! Both halves are a re-colour, and the vocabulary already has the write
//! (the `colorFor:<p>` tile prop, per player). (1) is a one-shot on the
//! first qualifying purchase; (2) is restated while the condition holds, the
//! same way the fire-cap skills restate their bound -- a standing claim, not a
//! one-time grant.
//!
//! 「非"旭汤澡堂"或任意"Live House"」 reads as: the tile is neither 旭汤澡堂 nor
//! already a Live House. A tile that is already group 6 needs no recolour.

use alloc::format;
use alloc::string::String;

use card_sdk::abi::{prop, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const DONE: &str = "skill.asahiAim.first";

/// The `colorFor:<p>` tile prop key -- the group `player_id` treats this tile as.
fn color_for(player_id: i32) -> String {
    format!("{}{}", prop::COLOR_FOR_PREFIX, player_id)
}

/// The tile the clause names out.
fn is_bathhouse(t: i32) -> bool {
    t >= 0 && t == ctx::tile_named("旭汤澡堂")
}

pub const ASAHI_AIM: CardDef = CardDef::new(
    "skill:朝日六花:瞄准目标",
    &[
        On::Hook(&[HookKind::Bought], card_sdk::pre::MINE, None, on_bought),
        On::Hook(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「你购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色」.
fn on_bought(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, DONE) != 0 {
        return Ok(());
    }
    let t = ctx::trigger::tile();
    if t < 0 || is_bathhouse(t) || ctx::is_live_house_for(player_id, t) {
        return Ok(());
    }
    state::set(player_id, DONE, 1);
    // 「获得"Live House"的颜色」 -- the `colorFor:<p>` tile prop on the tile's
    // board instance: group 6 for this player.
    ctx::set_tile_prop(t, &color_for(player_id), 6);
    ctx::log(
        player_id,
        &Msg::new(key!("asahi_aim_first")).tile("tile", t),
    );
    Ok(())
}

/// （2）「在你有初始"Live House"格子时你拥有的"旭汤澡堂"格子获得"Live House"的颜色」
/// -- restated each turn while the condition holds.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    let bath = ctx::tile_named("旭汤澡堂");
    if bath < 0 || ctx::tile_owner(bath) != player_id {
        return Ok(());
    }
    if !ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| is_initial_livehouse(t))
    {
        return Ok(());
    }
    ctx::set_tile_prop(bath, &color_for(player_id), 6);
    Ok(())
}

/// 「初始"Live House"格子」 -- one that is a Live House by its own group, not
/// because this skill recoloured it.
fn is_initial_livehouse(t: i32) -> bool {
    ctx::is_buyable(t) && ctx::tile_group(t) == 6
}
