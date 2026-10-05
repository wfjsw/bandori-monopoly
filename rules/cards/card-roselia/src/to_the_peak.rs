//! `R:向着顶点` -- C# `CardToThePeak` (MatchHost.cs:10360-10422): walk to the next
//!
//! 规则书（docs/rulebook/cards.json, id `R:向着顶点`）:
//! > 向着顶点：
//! >  移动到下一个可被购买的livehouse格子。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。
//!
//! free Livehouse, or build one of yours at 1.5x cost.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const TO_THE_PEAK: CardDef = CardDef {
    id: "R:向着顶点",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// The buyable Livehouse deeds (C# `H.LiveHouses`: `IsBuyable && IsColor(6)`).
fn livehouses() -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::tile_group(t) == 6)
        .collect()
    // TODO(规则书): `H.IsColor` also reads `_tileColors[t]` and every live
    //   `Fx.ExtraColor` (band skills that re-colour a tile); neither is in the
    //   vocabulary, so those swaps are not seen here.
}

/// C# `WhyNotBuildOn`'s 「RiNG 不能加盖房屋」: a RiNG deed is the buyable
/// group-6 tile with no rent table, so `rent_of <= 0` is the test (the
/// vocabulary has no `TileData.kind` query).
fn is_ring(t: i32) -> bool {
    ctx::rent_of(t) <= 0
}

/// C# `H.WhyNotBuildOn(i, t)` -- the checks the vocabulary can see.
fn why_not_build_on(seat: i32, t: i32) -> Option<&'static str> {
    if ctx::tile_owner(t) != seat {
        return Some("only_own");
    }
    if is_ring(t) {
        return Some("ring");
    }
    if ctx::mortgaged_of(t) {
        return Some("mortgaged");
    }
    // C# `houses[t] >= rent.Length - 1` (「这块地已经盖满了」). The vocabulary has
    // no rent-table length; every buildable tile in `data/board.json` holds 3.
    if ctx::houses_of(t) >= MAX_HOUSES {
        return Some("full");
    }
    if !ctx::can_pay(seat) {
        return Some("cannot_pay");
    }
    None
    // Not checked (no hook): `_turnCtx.NoBuild` (学生会的检查's build lock),
    // the 「卡池BUG」 event, and `Fx.CanBuild`.
}

/// C# `tileData.rent.Length - 1` for the board's buildable tiles (all 4-step
/// rent tables); a rent-length query would generalise this.
const MAX_HOUSES: i32 = 3;

/// C# `CardToThePeak.WhyNot`.
fn why_not(seat: i32) -> Option<Msg> {
    let free: Vec<i32> = livehouses()
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        // C# `H.Moving(seat)` -- the walk branch needs a movable seat.
        if ctx::stun_of(seat) > 0 {
            return Some(Msg::new(key!("to_the_peak_stunned")));
        }
        if ctx::stay_of(seat) > 0 {
            // TODO(规则书): `H.Moving` lets a `[停留]` seat through when
            //   `_turnCtx.Unstoppable` (「不可阻挡」) is up -- no hook for it.
            return Some(Msg::new(key!("to_the_peak_stuck")));
        }
        return None;
    }
    // C# the build branch: `H.LiveHouses(i, t => owners[t] == i &&
    // H.WhyNotBuildOn(i, t) == null)` (no money check here -- that is in Play).
    if livehouses().into_iter().any(|t| {
        ctx::tile_owner(t) == seat && why_not_build_on(seat, t).is_none()
    }) {
        return None;
    }
    Some(Msg::new(key!("to_the_peak_nothing")))
}

fn play(seat: i32) {
    let pos = ctx::seat_pos(seat);
    // 规则书: 「移动到下一个可被购买的livehouse格子」 -- unowned Livehouses, nearest ahead.
    let free: Vec<i32> = livehouses()
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        let to = free
            .into_iter()
            .min_by_key(|&t| {
                let d = ctx::tile_forward(pos, t);
                if d == 0 { ctx::tile_count() } else { d }
            })
            .unwrap_or(-1);
        // 规则书: 「移动到下一个可被购买的livehouse格子」 -- C# `H.Walk(i, Forward(pos, to),
        // resolve: true, null, "向着顶点")`.
        ctx::log(seat, &Msg::new(key!("to_the_peak_walk")).seat("who", seat).tile("tile", to));
        // TODO(规则书): 「移动到」 -- needs the H.Walk movement routine (walk the ring
        // and settle at the destination, C# `H.Walk(..., resolve: true)`); the
        // vocabulary has no movement routine, so the seat is not moved at all.
        return;
    }
    // 规则书: 「若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋」
    let mut mine: Vec<i32> = Vec::new();
    for t in livehouses() {
        if why_not_build_on(seat, t).is_some() {
            continue;
        }
        if ctx::money(seat) >= ctx::build_cost(t) * 3 / 2 {
            mine.push(t);
        }
    }
    if mine.is_empty() {
        ctx::log(seat, &Msg::new(key!("to_the_peak_no_money")));
        return;
    }
    let title = Msg::new(key!("to_the_peak_ask_title"));
    let text = Msg::new(key!("to_the_peak_ask_text"));
    // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
    if !ctx::ask_yes(seat, &title, &text) {
        return;
    }
    let tile = ctx::ask_tile(seat, &title, &text, &mine);
    // 规则书: 「花费1.5倍价格」 -- C# `CeilTo(H.BuildCostFor(i, r.index) * 1.5, 10)`.
    let amount = ceil_to(ctx::build_cost(tile) as i64 * 3 / 2, 10);
    let paid = ctx::pay(seat, amount, &Msg::new(key!("to_the_peak_why")).tile("tile", tile));
    if paid < amount {
        return;
    }
    // 规则书: 「加盖一层房屋」 -- C# `H.BuildRoutine(i, r.index, free: true, "向着顶点")`:
    // the pay above covers the 1.5x cost and the raise is free (`H.AddHouse`).
    if why_not_build_on(seat, tile).is_none() {
        ctx::add_house(tile, 1);
        ctx::log(seat, &Msg::new(key!("to_the_peak_build")).seat("who", seat).tile("tile", tile));
    }
    // TODO(规则书): the C# `BuildRoutine` then fires `H.Each((Fx f) => f.Built(i, t,
    //   full))` / `f.HouseAdded(...)` -- needs those persistent Fx hooks (and the
    //   build is still a bare `H.AddHouse`, free of the `BuildRoutine` bookkeeping).
}

/// C# `CeilTo(x, unit)` -- round up to a multiple of `unit`.
fn ceil_to(x: i64, unit: i64) -> i32 {
    if unit <= 0 {
        return x as i32;
    }
    (((x + unit - 1) / unit) * unit) as i32
}