//! `AG:刻入天穹傲岸的烈光` -- C# `CardProudLight` (MatchHost.cs:1681-1721):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:刻入天穹傲岸的烈光`）:
//! > 刻入天穹傲岸的烈光：
//! > [反击] 当你经过一名角色时，你可以打出此卡，你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。
//!
//! swap half of each side's most expensive deed price when you pass a player.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const PROUD_LIGHT: CardDef = CardDef::new(
    "AG:刻入天穹傲岸的烈光",
    // 规则书[反击]: 「当你经过一名角色时」 -- 行动阶段 12 [经过]
    // (`SETTLE-STAGES.md` §4 M4), a mid-route pass of a tile a character stands
    // on -- not the end-tile [重叠]. `ChainKind::PassTile` is the [反击] key
    // for one [经过] step (ABI v43).
    // G4: kind (PassTile) is the category; `mine` is the condition. The
    // "someone stands on the tile" check is a derived list (GUARDS.md §6) and
    // stays in the residual guard.
    &[On::Counteract(
        &[ChainKind::PassTile],
        Some(can_counteract),
        counteract,
        card_sdk::pre::MINE,
    )],
)
    .legacy(&[(0, legacy_can_counteract)]);

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    if trigger::kind() != TriggerKind::PassTile {
        return false;
    }
    if trigger::player_id() != player_id {
        return false;
    }
    ctx::players_on(trigger::tile(), player_id)
        .into_iter()
        .any(|o| o >= 0 && !ctx::player_out(o))
}

/// The player's most expensive deed's base purchase price (C# `CardProudLight.Best`).
fn best_price(player_id: i32) -> i32 {
    // 规则书[反击]: 「对方最贵格子基础购买价格」-- max of `H._tiles[t].price`
    // (`ctx::tile_price`, the land price alone).
    ctx::owned_tiles(player_id)
        .into_iter()
        .map(ctx::tile_price)
        .max()
        .unwrap_or(0)
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你经过一名角色时」 -- the actor rel is the condition
    // (`pre::MINE`); 「一名角色」 is a derived list (GUARDS.md §6) and stays here.
    let _ = player_id;
    // 「一名角色」 -- a player standing on the tile being entered.
    ctx::players_on(trigger::tile(), player_id)
        .into_iter()
        .any(|o| o >= 0 && !ctx::player_out(o))
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 「对方」 -- the character being passed (the first present player on the
    // tile). A `passTile` payload has no `target`; the tile's occupants are it.
    let Some(o) = ctx::players_on(trigger::tile(), player_id)
        .into_iter()
        .find(|&o| o >= 0 && !ctx::player_out(o))
    else {
        return Ok(());
    };
    // 规则书[反击]: 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
    let from_them = best_price(o) / 2;
    if from_them > 0 {
        ctx::transfer(o, player_id, from_them, &Msg::new(key!("proud_light_why")))?;
    }
    // 规则书[反击]: 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金」
    let from_us = best_price(player_id) / 2;
    if from_us > 0 && !ctx::player_out(o) {
        ctx::transfer(player_id, o, from_us, &Msg::new(key!("proud_light_why")))?;
    }
    Ok(())
}
