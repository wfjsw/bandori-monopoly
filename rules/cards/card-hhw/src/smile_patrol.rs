//! `HHW:微笑巡逻队` -- C# `CardSmilePatrol` (MatchHost.cs:3872-3899): pay to build
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:微笑巡逻队`）:
//! > 微笑巡逻队：
//! >  付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）
//!
//! on one of your tiles, then roll 3d20 and maybe build free on the tile the
//! roll points at (a different tile, which need not be yours).

use bandori_std::ctx;
use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const SMILE_PATROL: CardDef =
    CardDef::new("HHW:微笑巡逻队", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"微笑巡逻队"#)
        .raw_name(r#"微笑巡逻队"#)
        .tags(&[])
        .text(CardText::para(r#"付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）"#))
        // sheet cell F6
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「付款并在任意自己的格子加盖一层房屋」 -- the destination must be
    // one the player could build on: the same `H.WhyNotBuildOn` gate the build
    // step uses (ring / rent-tier cap / house cap / ownership / mortgage
    // included -- C# `CardSmilePatrol.WhyNot`).
    // TODO(规则书)（付款并在任意自己的格子加盖一层房屋）: 规则书未给打出条件 --
    // 我们要求至少有一格自己的可加盖且付得起的格子才可打出，否则拒绝；免费
    // 加盖那一半其实单独打也有效。
    let money = ctx::money_of(player_id);
    for t in ctx::owned_tiles(player_id) {
        // TODO(规则书)（付款）: 付款额就是该格房屋造价 -- 但半价/减免的回合
        // 实际要付的更少，这里仍按原价判「付得起」.
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
    // TODO(规则书)（付款）: 规则书未给付款额 -- 我们按该格房屋表的正常造价收费
    // (OPEN-HHW-PATROL-PRICE).
    ctx::card_offer_build(player_id, &ctx::owned_tiles(player_id));
    // 规则书: 「投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）」
    // -- C# `H.Roll(i, 3, 20, CardName)`; the tile is `(roll - 1) % tiles.Length`.
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // TODO(规则书)（投掷3d20）: 三个面可读作各对应一格 -- 我们只取三面之和，
    // 用它的取模定位一格并只加盖一层 (OPEN-HHW-PATROL-3D20).
    let r = ctx::roll(player_id, 3, 20, RollPurpose::Effect, roll_source::CARD)?;
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
    // cover the buyable/mortgage half and the ring / rent-tier / house-cap
    // caps. They are **not** the whole 可建造 test: the tile-level vetoes
    // (`prop::NO_BUILD`, `prop::NO_BUILD_ABOVE`, `prop::BUILD_MAX`) are still
    // unchecked here, so a house rises where the paid build would refuse.
    // TODO(规则书)（若为可建造格子）: 不可盖房的格子不能加盖 -- 免费的这一层
    // 只查买得起/抵押，NO_BUILD 与 NO_BUILD_ABOVE 一类否决没查.
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
