//! `RAS:Change the world` -- C# `CardChangeWorld` (MatchHost.cs:9441-9531):
//! place on a house-bearing Live House and turn the move dice into 3d20.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Change the world`）:
//! > Change the world：
//! >   将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const CHANGE_WORLD: CardDef = CardDef {
    id: "RAS:Change the world",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses (board.json
/// `rent` non-empty). `ExtraColor` tiles (e.g. 「游击演出」's deed) are missing.
const LIVEHOUSE_PROPS: [&str; 4] = ["DUB MUSIC EXPERIMENT", "武道馆", "Space", "Live House Galaxy"];

/// C# `Spots(seat)` = owned Live Houses with `houses[t] > 0`.
fn spots(seat: i32) -> Vec<i32> {
    LIVEHOUSE_PROPS
        .iter()
        .filter_map(|&n| {
            let t = ctx::tile_named(n);
            (t >= 0 && ctx::tile_owner(t) == seat && ctx::houses_of(t) > 0).then_some(t)
        })
        .collect()
}

/// C# `CardChangeWorld.WhyNot`: 「你没有盖了房屋的 Live House 格子」 or
/// `H.MoveWhyNot(seat)`.
fn why_not(seat: i32) -> Option<Msg> {
    if spots(seat).is_empty() {
        return Some(Msg::new(key!("change_world_no_houses")));
    }
    // TODO(规则书): `H.MoveWhyNot` (refuse after the main move / on skip-move /
    // off-turn) is still missing; only the house-bearing check is gated here.
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("change_world_not_your_turn")));
    }
    None
}

fn play(seat: i32) {
    // 规则书: 「将此卡放置于你的一个有房屋的livehouse格子上」
    // -- C# `Spots(seat)` = owned Live Houses with `houses[t] > 0`, then
    // `H.AskTileOf`. The gate is now `why_not` above.
    let spots = spots(seat);
    if spots.is_empty() {
        return;
    }
    let tile = ctx::ask_tile(
        seat,
        &Msg::new(key!("change_world_title")),
        &Msg::new(key!("change_world_ask")),
        &spots,
    );
    // 规则书: 「将此卡放置于你的一个有房屋的livehouse格子上」
    // -- C# `H.PlaceFromPlay(c, i, tile)` binds the card to that tile.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "RAS:Change the world", &Msg::new(key!("change_world_note")));
    ctx::log(
        seat,
        &Msg::new(key!("change_world_placed")).seat("who", seat).tile("tile", tile),
    );
    // TODO(规则书): 「将此卡放置于你的一个有房屋的livehouse格子上」 -- the
    // placement is bound to `tile` rather than the seat's field; needs field-card
    // tile placement (`H.PlaceFromPlay(c, owner, tile)`).
    // TODO(规则书): 「你的本次移动掷骰变为3d20」 -- needs the move dice plan
    // (C# `H._turnCtx.Plan.Base.Clear()` + `Add((3, 20, ...))`).
    // TODO(规则书): 「期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶」
    // -- needs the Fx.PassTile hook (C# `CardChangeWorld.PassTile`) and the
    // per-card crystal counter (`Card.Crystals`).
    // TODO(规则书): 「此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数」
    // -- needs the Fx.PayAdd hook (C# `CardChangeWorld.PayAdd`, bonus
    // `50 * (Crystals + 1) * houses[Tile]` added to the rent and paid out).
    // TODO(规则书): 「触发后将该卡放入弃牌堆」 -- needs the Fx.PayAfter hook
    // (C# `CardChangeWorld.PayAfter` -> `H.Unplace(this, "discard", ...)`).
}
