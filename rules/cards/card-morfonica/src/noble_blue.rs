//! `Mor:高贵的微蓝` -- C# `CardNobleBlue` (MatchHost.cs:5079-5107): settle your own
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:高贵的微蓝`）:
//! > 高贵的微蓝：当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。
//!
//! expensive tile immediately and mark it untargetable.

use card_sdk::{ctx, key, CardDef, Msg};

pub const NOBLE_BLUE: CardDef = CardDef {
    id: "Mor:高贵的微蓝",
    play: Some(noble_blue),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
    // C# `CardNobleBlue.WhyNot` refuses when `H._tiles[pos].price < 2200`
    // (land price only -- not `buy_price`, which adds houses).
    let pos = ctx::seat_pos(seat);
    if pos < 0 || ctx::tile_price(pos) < 2200 {
        return Some(Msg::new(key!("noble_blue_why_not")));
    }
    None
}

fn noble_blue(seat: i32) {
    let pos = ctx::seat_pos(seat);
    if pos < 0 {
        return;
    }
    // 规则书: 「如果此地块属于你」
    if ctx::tile_owner(pos) != seat {
        // C# `c.Effective = false; H.Log(... 没有效果)`.
        // TODO(ABI): `PlayCtx.Effective` is not writable; the card is spent anyway.
        ctx::log(seat, &Msg::new(key!("noble_blue_not_yours")).tile("tile", pos));
        return;
    }
    // 规则书1: 「立刻进行一次触发结算」 -- C# `H.SettleAt(seat, pos, CardName)`.
    // TODO(规则书)1: 「立刻进行一次触发结算」 -- needs the settle routine
    // (`H.SettleAt`) so landing on the tile runs a full [触发结算].
    // 规则书2: 「在该地块上放置一个标记，有标记时此地块不能被指定」
    ctx::add_mark(pos, seat, key!("noble_blue_mark"), &Msg::new(key!("noble_blue_mark_note")));
    ctx::log(seat, &Msg::new(key!("noble_blue_placed")).tile("tile", pos).seat("who", seat));
    // TODO(规则书)2: 「有标记时此地块不能被指定」 -- needs the H.Target / Untargetable
    // gate so a tile carrying this mark cannot be chosen as a target.
}