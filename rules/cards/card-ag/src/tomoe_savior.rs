//! `AG:（巴）商店街的救世主` -- C# `CardTomoeSavior` (MatchHost.cs:1722-1791):
//! [反击] on a shop-street mortgage: buy it cheap, or double your own mortgage.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（巴）商店街的救世主`）:
//! > （巴）商店街的救世主：【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的价格从该玩家处收购该地契。当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回
//!

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const TOMOE_SAVIOR: CardDef = CardDef::new(
    "AG:（巴）商店街的救世主",
    &[On::CounterAct(&[ChainKind::Mortgage], can_react, react)],
);

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
        ctx::money_of(player_id) >= ctx::tile_price(t) / 2
    } else {
        true
    }
}

fn react(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    let from = trigger::player_id();
    if from == player_id {
        // 规则书[反击]: 「额外获得一份抵押收益并将地契翻回」 -- C# only fires when
        // the deed is still the player's and still mortgaged
        // (`H.State.owners[t] == i && H.State.mortgaged[t]`).
        if ctx::tile_owner(t) != player_id || !ctx::mortgaged_of(t) {
            return Ok(());
        }
        // 规则书[反击]: 「额外获得一份抵押收益」 -- C# adds `H.MortgageValue(t)`
        // (= `_tiles[t].price / 2`) with a raw money add (not `H.GainR`).
        let v = ctx::mortgage_value(t);
        if v > 0 {
            ctx::gain(player_id, v, &Msg::new(key!("tomoe_savior_why")));
        }
        // 规则书[反击]: 「将地契翻回」 -- C# `H.State.mortgaged[t] = false`.
        ctx::set_mortgaged(t, false);
        // TODO(规则书)[judgement]: 「额外获得一份抵押收益」 does not say whether the
        // extra payout goes through the gain pipeline or lands raw. The C# added
        // it raw (`H.State.seats[i].money += num`, no `H.GainR`), so a [拥有者] /
        // [支付] reaction cannot see it; this port routes it through `gain`, which
        // means such a reaction can. Not a missing capability -- the clause just
        // does not decide. Flagging rather than guessing.
        return Ok(());
    }
    // 规则书[反击]: 「立刻支付常规收购价一半的价格从该玩家处收购该地契」 -- C#
    // `PayCtx { from = i, to = from, amount = H._tiles[t].price / 2, must = false }`,
    // then `H.State.owners[t] = i`.
    if ctx::tile_owner(t) != from {
        return Ok(());
    }
    let price = ctx::tile_price(t) / 2;
    if price <= 0 {
        return Ok(());
    }
    let why = Msg::new(key!("tomoe_savior_buy")).tile("tile", t);
    ctx::transfer(player_id, from, price, &why)?;
    // 规则书[反击]: 「从该玩家处收购该地契」 -- C# `if (p.paid) H.State.owners[t] = i`
    // (`H.Money` marks a positive-amount run paid even when `must: false` clamps
    // the loss to what the player has).
    ctx::set_owner(t, player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("tomoe_savior_bought"))
            .tile("tile", t)
            .player_id("from", from)
            .player_id("who", player_id),
    );
    // TODO(ABI)[反击]: the C# also runs `f.Bought(i, t)` over the Fx chain after
    //   the hand-over. `TriggerKind::Bought` is raised by `buy()` / auction, but a
    //   forced hand-over off an owned tile is not a buy (`card_buy` -> `buy`
    //   refuses a tile with an owner), and there is no card-side `raise`. Running
    //   the hand-over through `set_owner` fires nothing.
    Ok(())
}
