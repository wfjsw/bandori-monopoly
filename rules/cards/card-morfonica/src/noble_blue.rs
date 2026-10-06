//! `Mor:高贵的微蓝` -- C# `CardNobleBlue` (MatchHost.cs:5079-5107): settle your own
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:高贵的微蓝`）:
//! > 高贵的微蓝：当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。
//!
//! expensive tile immediately and mark it untargetable.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const NOBLE_BLUE: CardDef = CardDef::new("Mor:高贵的微蓝", &[
    On::Play(Some(cant_play), noble_blue)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
    // C# `CardNobleBlue.WhyNot` refuses when `H._tiles[pos].price < 2200`
    // (land price only -- not `buy_price`, which adds houses).
    let pos = ctx::player_pos(player_id);
    if pos < 0 || ctx::tile_price(pos) < 2200 {
        return Some(Msg::new(key!("noble_blue_why_not")));
    }
    None
}

fn noble_blue(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    if pos < 0 {
        return Ok(());
    }
    // 规则书: 「如果此地块属于你」
    if ctx::tile_owner(pos) != player_id {
        // C# `c.Effective = false; H.Log(... 没有效果)`.
// TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
        // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
        // side channel and is not being ported (a routine should *return* whether
        // it took effect); but before that lands, what "not effective" changes has
        // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
        // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
        // implies no)? And what counts a use that this would suppress?
        ctx::log(player_id, &Msg::new(key!("noble_blue_not_yours")).tile("tile", pos));
        return Ok(());
    }
    // 规则书1: 「立刻进行一次触发结算」 -- C# `H.SettleAt(seat, pos, CardName)`.
    // The player does not move; the tile they are on resolves again.
    ctx::card_settle_at(player_id, pos, true);
    // 规则书2: 「在该地块上放置一个标记，有标记时此地块不能被指定」 -- the mark kind
    // is the gate: `target_tile` refuses any tile carrying `noTarget`.
    ctx::add_mark(pos, player_id, card_sdk::abi::mark::NO_TARGET, &Msg::new(key!("noble_blue_mark_note")));
    ctx::log(player_id, &Msg::new(key!("noble_blue_placed")).tile("tile", pos).player_id("who", player_id));
    Ok(())
}