//! `AG:刻入天穹傲岸的烈光` -- C# `CardProudLight` (MatchHost.cs:1681-1721):
//! swap half of each side's most expensive deed price when you pass a player.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:刻入天穹傲岸的烈光`）:
//! > 刻入天穹傲岸的烈光：
//! > [反击] 当你经过一名角色时，你可以打出此卡，你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const PROUD_LIGHT: CardDef = CardDef::new("AG:刻入天穹傲岸的烈光", &[
    On::React(&[TriggerKind::PassPlayer], can_react, react),
]);

/// The player's most expensive deed's base purchase price (C# `CardProudLight.Best`).
fn best_price(player_id: i32) -> i32 {
    // 规则书[反击]: 「对方最贵格子基础购买价格」-- max of `H._tiles[t].price`
    // (`ctx::tile_price`, the land price alone).
    ctx::owned_tiles(player_id).into_iter().map(ctx::tile_price).max().unwrap_or(0)
}

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你经过一名角色时，你可以打出此卡」
    if trigger::kind() != TriggerKind::PassPlayer {
        return false;
    }
    if trigger::player_id() != player_id {
        return false;
    }
    let o = trigger::target();
    // C# `t.Target >= 0 && !H.Out(t.Target)`.
    o >= 0 && !ctx::player_out(o)
}

fn react(player_id: i32) {
    let o = trigger::target();
    if o < 0 {
        return;
    }
    // 规则书[反击]: 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
    let from_them = best_price(o) / 2;
    if from_them > 0 {
        ctx::transfer(o, player_id, from_them, &Msg::new(key!("proud_light_why")));
    }
    // 规则书[反击]: 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金」
    let from_us = best_price(player_id) / 2;
    if from_us > 0 && !ctx::player_out(o) {
        ctx::transfer(player_id, o, from_us, &Msg::new(key!("proud_light_why")));
    }
}