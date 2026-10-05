//! `AG:刻入天穹傲岸的烈光` -- C# `CardProudLight` (MatchHost.cs:1681-1721):
//! swap half of each side's most expensive deed price when you pass a seat.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:刻入天穹傲岸的烈光`）:
//! > 刻入天穹傲岸的烈光：
//! > [反击] 当你经过一名角色时，你可以打出此卡，你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const PROUD_LIGHT: CardDef = CardDef {
    id: "AG:刻入天穹傲岸的烈光",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// The seat's most expensive deed's base purchase price (C# `CardProudLight.Best`).
fn best_price(seat: i32) -> i32 {
    // 规则书[反击]: 「对方最贵格子基础购买价格」-- max of `H._tiles[t].price`
    // (`ctx::tile_price`, the land price alone).
    ctx::owned_tiles(seat).into_iter().map(ctx::tile_price).max().unwrap_or(0)
}

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当你经过一名角色时，你可以打出此卡」
    if trigger::kind() != TriggerKind::PassSeat {
        return false;
    }
    if trigger::seat() != seat {
        return false;
    }
    let o = trigger::target();
    // C# `t.Target >= 0 && !H.Out(t.Target)`.
    o >= 0 && !ctx::seat_out(o)
}

fn react(seat: i32) {
    let o = trigger::target();
    if o < 0 {
        return;
    }
    // 规则书[反击]: 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
    let from_them = best_price(o) / 2;
    if from_them > 0 {
        ctx::transfer(o, seat, from_them, &Msg::new(key!("proud_light_why")));
    }
    // 规则书[反击]: 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金」
    let from_us = best_price(seat) / 2;
    if from_us > 0 && !ctx::seat_out(o) {
        ctx::transfer(seat, o, from_us, &Msg::new(key!("proud_light_why")));
    }
}