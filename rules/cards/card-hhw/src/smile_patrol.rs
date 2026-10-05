//! `HHW:微笑巡逻队` -- C# `CardSmilePatrol` (MatchHost.cs:3872-3899): pay to build
//! on one of your tiles, then roll 3d20 and maybe build free on that tile.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:微笑巡逻队`）:
//! > 微笑巡逻队：
//! >  付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SMILE_PATROL: CardDef = CardDef::new("HHW:微笑巡逻队", &[
    On::Play(play),
    On::CantPlay(cant_play),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardSmilePatrol.WhyNot`: refuses without a tile you can afford to
    // build on (`H.WhyNotBuildOn(seat, t) == null && money >= BuildCostFor`).
    // TODO(规则书): the C# `H.WhyNotBuildOn` also checks the ring/rent-tier cap,
    // the turn's NoBuild flag and live Fx blockers; those hooks are still
    // missing, so the gate covers only the expressible checks (buyable, owned,
    // unmortgaged, can-pay, affordable).
    let money = ctx::money(player_id);
    for t in ctx::owned_tiles(player_id) {
        if ctx::is_buyable(t)
            && !ctx::mortgaged_of(t)
            && ctx::can_pay(player_id)
            && money >= ctx::build_cost(t)
        {
            return None;
        }
    }
    Some(Msg::new(key!("x_no_buildable")))
}

fn play(player_id: i32) {
    // 规则书: 「付款并在任意自己的格子加盖一层房屋」 -- C#
    // `H.OfferBuildAmong(i, H.OwnedBy(i), CardName)` pays the tile's build cost
    // and raises one house. TODO(规则书): the paid-build routine (`H.BuildRoutine`
    // / `H.OfferBuildAmong`) is still missing -- only the free build below runs.
    // 规则书: 「投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）」
    // -- C# `H.Roll(i, 3, 20, CardName)`; the tile is `(roll - 1) % tiles.Length`.
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let r = ctx::roll(player_id, 3, 20);
    let tile = (r - 1).rem_euclid(n);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_patrol_rolled")).player_id("who", player_id).tile("tile", tile).i("roll", r as i64),
    );
    // 规则书: 「额外免费加盖一层房屋（若为可建造格子）」 -- C# checks `IsBuyable`,
    // kind != "ring", rent tiers, mortgage, house cap, then `H.AddHouse`.
    // `is_buyable` / `mortgaged_of` / `add_house` (which clamps at the C# max)
    // cover the expressible checks; the ring / rent-tier / house-cap tests fall
    // through `add_house`'s clamp (a no-op when the tile cannot take a house).
    let before = ctx::houses_of(tile);
    let can = ctx::is_buyable(tile) && !ctx::mortgaged_of(tile);
    let after = if can { ctx::add_house(tile, 1) } else { before };
    if after > before {
        ctx::log(player_id, &Msg::new(key!("smile_patrol_built")).tile("tile", tile).i("n", after as i64));
    } else {
        ctx::log(player_id, &Msg::new(key!("smile_patrol_cannot_build")).tile("tile", tile));
    }
}