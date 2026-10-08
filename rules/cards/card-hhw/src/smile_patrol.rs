//! `HHW:微笑巡逻队` -- C# `CardSmilePatrol` (MatchHost.cs:3872-3899): pay to build
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:微笑巡逻队`）:
//! > 微笑巡逻队：
//! >  付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）
//!
//! on one of your tiles, then roll 3d20 and maybe build free on that tile.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SMILE_PATROL: CardDef =
    CardDef::new("HHW:微笑巡逻队", &[On::Play(Some(cant_play), play, "")]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardSmilePatrol.WhyNot`: refuses without a tile you can afford to
    // build on (`H.WhyNotBuildOn(seat, t) == null && money >= BuildCostFor`).
    // 规则书: the destination must be one the player could build on -- the same
    // `WhyNotBuildOn` gate the build step uses (ring / rent-tier cap / house cap
    // / ownership / mortgage / can-pay included).
    let money = ctx::money_of(player_id);
    for t in ctx::owned_tiles(player_id) {
        if ctx::can_build_on(player_id, t) && money >= ctx::build_cost(t) {
            return None;
        }
    }
    Some(Msg::new(key!("x_no_buildable")))
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「付款并在任意自己的格子加盖一层房屋」 -- C#
    // `H.OfferBuildAmong(i, H.OwnedBy(i), CardName)` pays the tile's build cost
    // and raises one house. Then the free build below is the second, separate
    // half of the clause.
    ctx::card_offer_build(player_id, &ctx::owned_tiles(player_id));
    // 规则书: 「投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）」
    // -- C# `H.Roll(i, 3, 20, CardName)`; the tile is `(roll - 1) % tiles.Length`.
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let r = ctx::roll(player_id, 3, 20);
    let tile = (r - 1).rem_euclid(n);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_patrol_rolled"))
            .player_id("who", player_id)
            .tile("tile", tile)
            .i("roll", r as i64),
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
        ctx::log(
            player_id,
            &Msg::new(key!("smile_patrol_built"))
                .tile("tile", tile)
                .i("n", after as i64),
        );
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("smile_patrol_cannot_build")).tile("tile", tile),
        );
    }
    Ok(())
}
