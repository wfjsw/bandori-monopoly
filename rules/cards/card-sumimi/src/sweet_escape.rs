//! `Sumimi:Sweet Escape` -- C# `CardSweetEscape` (MatchHost.cs:11407-11480):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sweet Escape`）:
//! > Sweet Escape：
//! >  回合开始时，若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡，传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子并触发结算，若为可购买格子则必须购买，视为你的主要移动。
//!
//! settle-teleport onto a free tile of the 周边精选 / 商店街 / 东京外 colour groups.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SWEET_ESCAPE: CardDef = CardDef::new(
    "Sumimi:Sweet Escape",
    &[On::Play(Some(cant_play), sweet_escape)],
);

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
fn nearby_sum(player_id: i32) -> i32 {
    let n = ctx::tile_count();
    if n <= 0 {
        return 0;
    }
    let pos = ctx::player_pos(player_id);
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
fn spots(player_id: i32) -> Vec<i32> {
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
        if ctx::tile_owner(t) == player_id {
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
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「回合开始时」 / 「可打出」 -- C# `H.State.turn == seat &&
    // !H._turnCtx.MainMoved`, later `H.MoveWhyNot`; `ctx::cant_move` covers
    // off-turn, already-moved and skip-move.
    if let Some(why) = ctx::cant_move(player_id) {
        return Some(why);
    }
    // 规则书: 「若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡」
    let sum = nearby_sum(player_id);
    if sum < 2000 {
        return Some(Msg::new(key!("sweet_escape_short")).i("n", sum as i64));
    }
    // C# `WhyNot`: 没有可以去的格子
    if spots(player_id).is_empty() {
        return Some(Msg::new(key!("sweet_escape_none")));
    }
    None // playable
}

fn sweet_escape(player_id: i32) -> card_sdk::Asked {
    let picks = spots(player_id);
    if picks.is_empty() {
        // C# `WhyNot`: 没有可以去的格子 (kept so a forced play cannot prompt empty).
        ctx::log(player_id, &Msg::new(key!("sweet_escape_none")));
        return Ok(());
    }
    // 规则书: 「传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子」
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("sweet_escape_title")),
        &Msg::new(key!("sweet_escape_ask")),
        &picks,
    )?;
    // 规则书: 「…并触发结算」 / 「视为你的主要移动」 -- C# `H.CardMove(c, new MoveCtx
    // { TeleportTo = to })` (`Resolve` defaults to true) = `set_teleport_to(to)`
    // + `set_resolve(true)` + `card_move(player_id)`.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("sweet_escape_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // 规则书: 「视为你的主要移动」 -- `card_move` runs `MainMoveAs`
    // (MatchHost.cs:23102-23120), which sets `_turnCtx.MainMoved` on the turn
    // player; that is exactly `card_move`'s bookkeeping.
    // 规则书: 「若为可购买格子则必须购买」 -- C# `H.BuyRoutine` when the tile is
    // unowned and `money >= H.BuyPriceFor`. The settle already ran on arrival
    // (`set_resolve(true)`); this is the must-buy that follows it.
    if ctx::is_buyable(to)
        && ctx::tile_owner(to) < 0
        && ctx::money_of(player_id) >= ctx::buy_price(to)
    {
        ctx::card_buy(player_id, to);
    }
    Ok(())
}
