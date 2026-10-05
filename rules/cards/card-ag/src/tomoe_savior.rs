//! `AG:（巴）商店街的救世主` -- C# `CardTomoeSavior` (MatchHost.cs:1722-1791):
//! [反击] on a shop-street mortgage: buy it cheap, or double your own mortgage.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（巴）商店街的救世主`）:
//! > （巴）商店街的救世主：【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的价格从该玩家处收购该地契。当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TOMOE_SAVIOR: CardDef = CardDef::new("AG:（巴）商店街的救世主", &[
    On::React(&[ChainKind::Mortgage], can_react, react),
]);

/// The buyable shop-street deeds (C# `H.IsShop`: `IsBuyable && group == 10`).
fn is_shop(t: i32) -> bool {
    t >= 0 && ctx::is_shop(t)
}

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当其他玩家抵押商店街地契时」 / 「当你抵押商店街地契时」
    if trigger::kind() != TriggerKind::Mortgage {
        return false;
    }
    let t = trigger::tile();
    if !is_shop(t) {
        return false;
    }
    if trigger::player_id() != player_id {
        // 规则书[反击]: 「立刻支付常规收购价一半的价格」 -- C# also requires
        // `money >= H._tiles[t].price / 2` before the reaction is offered.
        ctx::money(player_id) >= ctx::tile_price(t) / 2
    } else {
        true
    }
}

fn react(player_id: i32) {
    let t = trigger::tile();
    let from = trigger::player_id();
    if from == player_id {
        // 规则书[反击]: 「额外获得一份抵押收益并将地契翻回」 -- C# only fires when
        // the deed is still the player's and still mortgaged
        // (`H.State.owners[t] == i && H.State.mortgaged[t]`).
        if ctx::tile_owner(t) != player_id || !ctx::mortgaged_of(t) {
            return;
        }
        // 规则书[反击]: 「额外获得一份抵押收益」 -- C# adds `H.MortgageValue(t)`
        // (= `_tiles[t].price / 2`) with a raw money add (not `H.GainR`).
        let v = ctx::mortgage_value(t);
        if v > 0 {
            ctx::gain(player_id, v, &Msg::new(key!("tomoe_savior_why")));
        }
        // 规则书[反击]: 「将地契翻回」 -- C# `H.State.mortgaged[t] = false`.
        ctx::set_mortgaged(t, false);
        // TODO(ABI): the C# uses a raw `H.State.seats[i].money += num` (bypasses
        // pay/gain effects) rather than `H.GainR`.
        return;
    }
    // 规则书[反击]: 「立刻支付常规收购价一半的价格从该玩家处收购该地契」 -- C#
    // `PayCtx { from = i, to = from, amount = H._tiles[t].price / 2, must = false }`,
    // then `H.State.owners[t] = i`.
    if ctx::tile_owner(t) != from {
        return;
    }
    let price = ctx::tile_price(t) / 2;
    if price <= 0 {
        return;
    }
    let why = Msg::new(key!("tomoe_savior_buy")).tile("tile", t);
    ctx::transfer(player_id, from, price, &why);
    // 规则书[反击]: 「从该玩家处收购该地契」 -- C# `if (p.paid) H.State.owners[t] = i`
    // (`H.Money` marks a positive-amount run paid even when `must: false` clamps
    // the loss to what the player has).
    ctx::set_owner(t, player_id);
    ctx::log(player_id, &Msg::new(key!("tomoe_savior_bought")).tile("tile", t).player_id("from", from).player_id("who", player_id));
    // TODO(规则书[反击]): the C# also runs `f.Bought(i, t)` over the Fx chain
    // after the hand-over. The `TriggerKind::Bought` listen-side exists (the
    // engine raises `bought` from `buy()` / auction), but this card's direct
    // `set_owner` hand-over is outside those routines and there is no card-side
    // raise, so the buy here fires no Bought.
}