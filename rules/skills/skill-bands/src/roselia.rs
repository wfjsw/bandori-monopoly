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
use ruleset_core::ctx::{self, state};
use card_sdk::{CardDef, On};

const DONE: &str = "skill.roselia.first";

/// The Live House tile group.
const LIVE: i32 = 6;

pub const ROSELIA: CardDef = CardDef::new(
    "skill:Roselia:对音乐的纯粹",
    &[
        // （2）/（1）'s 「购买价格减半」 is the buy price's `BuyMul` stage
        // (`docs/PURCHASE.md`), so the quote a player sees is the half they pay.
        // The Live-House / one-shot split is residual (`is_live_house_for` is a
        // per-player colour, not CEL's tile-kind `is_live_house`).
        On::Hook(&[HookKind::BuyMul], "actor == owner && tile.id >= 0", Some(half_price_applies), half_price),
        // （1）'s one-shot marker: a real buy of a non-Live-House tile.
        On::Hook(
            &[HookKind::Bought],
            "actor == owner && slot('skill.roselia.first') == 0 && tile.id >= 0",
            Some(on_bought_applies),
            on_bought,
        ),
        On::Hook(&[HookKind::TurnStartBefore], "actor == owner && slot('skill.roselia.first') != 0", Some(at_turn_start_applies), at_turn_start),
    ],
)
    .silent(&[1]);

/// Residual guard for [`half_price`]: a Live House (always halves) or the
/// first not-yet-taken non-Live-House buy. `is_live_house_for` is a
/// per-player colour (docs/GUARDS.md §4.2).
fn half_price_applies(player_id: i32) -> bool {
    let t = ctx::trigger::tile();
    if ctx::is_live_house_for(player_id, t) {
        return true;
    }
    state::get(player_id, DONE) == 0
}

/// （2）「购买任何Live House格子的地契时购买价格减半」, and （1）'s one-shot.
/// Applicability is [`half_price_applies`]; this side only halves.
fn half_price(player_id: i32) -> card_sdk::Asked {
    ctx::trigger::set_price(ctx::trigger::price() / 2);
    Ok(())
}

/// Residual guard for [`on_bought`]: the tile is a real non-Live-House buy
/// (`tile_group` is a derived read).
fn on_bought_applies(player_id: i32) -> bool {
    ctx::tile_group(ctx::trigger::tile()) != LIVE
}

fn on_bought(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, DONE, 1);
    Ok(())
}

/// Residual guard for [`at_turn_start`]: still no initial Live House and the
/// recolour has not already been claimed. Derived scans (docs/GUARDS.md §4.2).
fn at_turn_start_applies(player_id: i32) -> bool {
    if ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| ctx::tile_group(t) == LIVE)
    {
        return false;
    }
    // The recoloured tile is the one this skill half-priced; find it by the
    // extra-colour claim the same way the colour cards do.
    for t in ctx::owned_tiles(player_id) {
        if ctx::is_live_house_for(player_id, t) {
            return false;
        }
    }
    true
}

/// （1）「在你拥有任意初始Live House格子前，那个格子对你视为Live House」 --
/// restated each turn while [`at_turn_start_applies`] holds.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if let Some(t) = ctx::owned_tiles(player_id).into_iter().next() {
        // 「那个格子对你视为Live House」 -- the `colorFor:<player_id>` tile prop
        // on the tile's board instance.
        ctx::set_tile_prop(
            t,
            &alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id),
            LIVE,
        );
    }
    Ok(())
}

/// `bands.json` row for this band skill (ABI v63).
pub const BAND: ruleset_core::roster::BandDef =
ruleset_core::roster::BandDef::new(r#"Roselia"#)
    .short_name(r#"R"#)
    .boot_art(r#"005"#)
    .color(r#"#3344AA"#)
    .skill(r#"对音乐的纯粹"#)
    .text(r#"（1）本局游戏购买的第一个非Live House格子购买价格减半，在你拥有任意初始Live House格子前，那个格子对你视为Live House。（2）购买任何Live House格子的地契时购买价格减半"#)
    .logo(r#"005"#);
