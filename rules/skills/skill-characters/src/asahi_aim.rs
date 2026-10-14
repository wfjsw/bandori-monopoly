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
use ruleset_core::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const DONE: &str = "skill.asahiAim.first";

/// The `colorFor:<p>` tile prop key -- the group `player_id` treats this tile as.
fn color_for(player_id: i32) -> String {
    format!("{}{}", prop::COLOR_FOR_PREFIX, player_id)
}

pub const ASAHI_AIM: CardDef = CardDef::new(
    "skill:朝日六花:瞄准目标",
    &[
        // （1）「非"旭汤澡堂"或任意"Live House"」 -- the tile is a real buy,
        // not the bathhouse. `is_live_house_for` is the per-player colour check
        // (not CEL's tile-kind `is_live_house`), so it is the residual.
        On::Hook(
            &[HookKind::Bought],
            "actor == owner && slot('skill.asahiAim.first') == 0 && tile.id >= 0 && tile.id != tile_named('旭汤澡堂')",
            Some(not_live_house_for_me),
            on_bought,
        ),
        On::Hook(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, Some(owns_bath_with_lh), at_turn_start),
    ],
);

/// Residual guard for [`on_bought`]: the tile is not already a Live House for
/// this player (`ctx::is_live_house_for` -- per-player colour, not CEL's
/// tile-kind `is_live_house`).
fn not_live_house_for_me(player_id: i32) -> bool {
    !ctx::is_live_house_for(player_id, ctx::trigger::tile())
}

/// Residual guard for [`at_turn_start`]: the owner holds 旭汤澡堂 and still has
/// an initial Live House. Both are derived scans (named-tile owner + owned-tile
/// group), outside the CEL schema (docs/GUARDS.md §4.2).
fn owns_bath_with_lh(player_id: i32) -> bool {
    let bath = ctx::tile_named("旭汤澡堂");
    if bath < 0 || ctx::tile_owner(bath) != player_id {
        return false;
    }
    ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| is_initial_livehouse(t))
}

/// （1）「你购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色」.
fn on_bought(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
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
/// -- restated each turn while [`owns_bath_with_lh`] holds.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    let bath = ctx::tile_named("旭汤澡堂");
    ctx::set_tile_prop(bath, &color_for(player_id), 6);
    Ok(())
}

/// 「初始"Live House"格子」 -- one that is a Live House by its own group, not
/// because this skill recoloured it.
fn is_initial_livehouse(t: i32) -> bool {
    ctx::is_buyable(t) && ctx::tile_group(t) == 6
}

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::CharacterDef =
ruleset_core::roster::CharacterDef::new(r#"朝日六花"#)
    .display(r#"朝日六花"#)
    .band(r#"RAISE A SUILEN"#)
    .color(r#"#AAEE22"#)
    .skill(r#"瞄准目标"#)
    .text(r#"（1）你购买的第一个非“旭汤澡堂”或任意“Live House”格子获得“Live House”的颜色（2）在你有初始“Live House”格子时你拥有的“旭汤澡堂”格子获得“Live House”的颜色"#)
    .cn_id(r#"032"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"追逐梦想的步伐"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}
