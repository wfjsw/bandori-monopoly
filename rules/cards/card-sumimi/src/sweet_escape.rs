//! `Sumimi:Sweet Escape` -- C# `CardSweetEscape` (MatchHost.cs:11407-11480):
//! settle-teleport onto a free tile of the 周边精选 / 商店街 / 东京外 colour groups.
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sweet Escape`）:
//! > Sweet Escape：
//! >  回合开始时，若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡，传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子并触发结算，若为可购买格子则必须购买，视为你的主要移动。
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const SWEET_ESCAPE: CardDef = CardDef {
    id: "Sumimi:Sweet Escape",
    play: Some(sweet_escape),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `TileData.kind == "ring"` -- the ABI has no `tile_kind`, but the board's
/// tile-kind surface is `is_buyable` / `is_shop` / `tile_group`, and the RiNG
/// deeds are exactly the buyable tiles whose rent table is empty (`rent_of`
/// falls back to the land price). They never hold houses (`H.WhyNotBuildOn`
/// rejects `kind == "ring"`).
fn is_ring(t: i32) -> bool {
    ctx::is_buyable(t)
        && ctx::tile_price(t) > 0
        && ctx::houses_of(t) == 0
        && ctx::rent_of(t) == ctx::tile_price(t)
}

/// C# `H.PriceTag` -- the tile's 「收费标价」.
fn price_tag(t: i32) -> i32 {
    if t < 0 || !ctx::is_buyable(t) {
        return 0;
    }
    // 规则书: 「[收费标价]」 -- C# `H.PriceTag` returns `H.RingMultiplier` for
    // `kind == "ring"` tiles and the rent table otherwise.
    if is_ring(t) {
        return ctx::ring_multiplier();
    }
    ctx::rent_of(t)
}

/// C# `CardSweetEscape.Sum` -- price tags of the tiles 2 steps either side.
fn nearby_sum(seat: i32) -> i32 {
    let n = ctx::tile_count();
    if n <= 0 {
        return 0;
    }
    let pos = ctx::seat_pos(seat);
    let mut sum = 0;
    for d in [-2, -1, 1, 2] {
        let t = (pos + d).rem_euclid(n);
        // 规则书: 「自身前后两格内的地块[收费标价]之和」 -- C# `H.PriceTag`.
        sum += price_tag(t);
    }
    sum
}

/// C# `CardSweetEscape.Spots` -- buyable, not mine, not mortgaged, right colour.
/// 规则书: 「传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子」
fn spots(seat: i32) -> Vec<i32> {
    // C# `Agents.Select(H.TileNamed)` -> their `H._tiles[t].@group`; the agent
    // tiles themselves are `kind == "agent"` and not buyable, so 「除地产商以外」
    // falls out of `is_buyable`.
    let mut groups: Vec<i32> = Vec::new();
    for name in ["周边精选", "商店街", "东京外"] {
        let t = ctx::tile_named(name);
        if t >= 0 {
            groups.push(ctx::tile_group(t));
        }
    }
    let mut out: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        if !ctx::is_buyable(t) {
            continue;
        }
        if !groups.contains(&ctx::tile_group(t)) {
            continue;
        }
        if ctx::tile_owner(t) == seat {
            continue;
        }
        // 规则书: 「未抵押格子」 -- C# `!H.State.mortgaged[t]`.
        if ctx::mortgaged_of(t) {
            continue;
        }
        out.push(t);
    }
    out
}

/// C# `CardSweetEscape.WhyNot` -- turn-start, price-sum and destination gates.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「回合开始时」 / 「可打出」 -- C# `H.State.turn == seat && !H._turnCtx.MainMoved`.
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("sweet_escape_gate_start")));
    }
    // 规则书: 「若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡」
    let sum = nearby_sum(seat);
    if sum < 2000 {
        return Some(Msg::new(key!("sweet_escape_short")).i("n", sum as i64));
    }
    // C# `WhyNot`: 没有可以去的格子
    if spots(seat).is_empty() {
        return Some(Msg::new(key!("sweet_escape_none")));
    }
    // TODO(规则书): the C# also refuses after this turn's main move
    // (`!H._turnCtx.MainMoved`) and defers to `H.MoveWhyNot`; the gate currently
    // only covers 「回合开始时」's own-turn half.
    None // playable
}

fn sweet_escape(seat: i32) {
    let picks = spots(seat);
    if picks.is_empty() {
        // C# `WhyNot`: 没有可以去的格子 (kept so a forced play cannot prompt empty).
        ctx::log(seat, &Msg::new(key!("sweet_escape_none")));
        return;
    }
    // 规则书: 「传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子」
    let to = ctx::ask_tile(
        seat,
        &Msg::new(key!("sweet_escape_title")),
        &Msg::new(key!("sweet_escape_ask")),
        &picks,
    );
    // 规则书: 「…并触发结算」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo = to })`.
    // `teleport_to` is the resolve:false form; used here as the best-effort stand-in.
    ctx::teleport_to(seat, to);
    ctx::log(
        seat,
        &Msg::new(key!("sweet_escape_moved")).seat("who", seat).tile("tile", to),
    );
    // TODO(规则书): 「并触发结算」 -- needs the H.CardMove teleport-with-settle
    // routine (C# `H.CardMove(..., TeleportTo)` settles on arrival).
    // TODO(规则书): 「若为可购买格子则必须购买」 -- C# `H.BuyRoutine` when the tile is
    // unowned and `money >= H.BuyPriceFor`; needs a buy routine
    // (`H.BuyRoutine` / buy-price-for-seat).
    // TODO(规则书): 「视为你的主要移动」 -- needs the H.CardMove / main-move routine
    // so this teleport consumes the turn's main move.
}
