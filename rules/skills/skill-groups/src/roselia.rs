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

use card_sdk::abi::{ActionKind, FilterKind};
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
        // per-player colour, not CEL's tile-kind `is_venue`).
        On::Filter(&[FilterKind::BuyMul], "actor == owner && tile.id >= 0", Some(half_price_applies), half_price),
        // （1）'s one-shot marker: a real buy of a non-Live-House tile.
        On::Action(
            &[ActionKind::Bought],
            "actor == owner && slot('skill.roselia.first') == 0 && tile.id >= 0",
            Some(on_bought_applies),
            on_bought,
        ),
        On::Action(
            &[ActionKind::Bought],
            "actor == owner && slot('skill.roselia.first') != 0 && tile.id >= 0",
            Some(on_live_bought_applies),
            on_live_bought,
        ),
        On::Action(&[ActionKind::TurnStartBefore], "actor == owner && slot('skill.roselia.first') != 0", None, at_turn_start),
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
fn half_price(player_id: i32, value: i32) -> i32 {
    value / 2
}

/// Residual guard for [`on_bought`]: the tile is a real non-Live-House buy
/// (`tile_group` is a derived read).
fn on_bought_applies(player_id: i32) -> bool {
    ctx::tile_group(ctx::trigger::tile()) != LIVE
}

/// The one-shot marker stores the half-priced tile (`tile + 1`, so 0 stays
/// "not yet taken") and recolours it right away.
fn on_bought(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    state::set(player_id, DONE, t + 1);
    sync_colour(player_id);
    Ok(())
}

/// Residual guard for [`on_live_bought`]: the bought tile is an initial
/// (own-group) Live House.
fn on_live_bought_applies(_player_id: i32) -> bool {
    ctx::tile_group(ctx::trigger::tile()) == LIVE
}

/// （1）「在你拥有任意初始Live House格子前」 -- ends the moment the player
/// obtains one.
fn on_live_bought(player_id: i32) -> card_sdk::Asked {
    sync_colour(player_id);
    Ok(())
}

/// Turn-start sweep: also catches Live Houses obtained other than by buying.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    sync_colour(player_id);
    Ok(())
}

/// （1）「在你拥有任意初始Live House格子前，那个格子对你视为Live House」 --
/// the `colorFor:<player_id>` tile prop on the half-priced tile's board
/// instance: set while the player owns no initial Live House, cleared once
/// they do.
fn sync_colour(player_id: i32) {
    let marker = state::get(player_id, DONE);
    if marker == 0 {
        return;
    }
    let t = marker - 1;
    let owns_live = ctx::owned_tiles(player_id)
        .into_iter()
        .any(|o| ctx::tile_group(o) == LIVE);
    let key = alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id);
    if owns_live {
        ctx::set_tile_prop(t, &key, -1);
    } else if ctx::owned_tiles(player_id).contains(&t) {
        ctx::set_tile_prop(t, &key, LIVE);
    }
}

/// Roster band row for this band skill (ABI v63).
pub const BAND: ruleset_core::roster::GroupDef =
ruleset_core::roster::GroupDef::new(r#"Roselia"#)
    .short_name(r#"R"#)
    .color(r#"#3344AA"#)
    .skill(r#"对音乐的纯粹"#)
    .text(r#"（1）本局游戏购买的第一个非Live House格子购买价格减半，在你拥有任意初始Live House格子前，那个格子对你视为Live House。（2）购买任何Live House格子的地契时购买价格减半"#)
    .logo(r#"005"#);

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn half_price_kani(player_id: i32, value: i32) -> i32 {
    half_price(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_bought_kani(player_id: i32) -> card_sdk::Asked {
    on_bought(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_live_bought_kani(player_id: i32) -> card_sdk::Asked {
    on_live_bought(player_id)
}
