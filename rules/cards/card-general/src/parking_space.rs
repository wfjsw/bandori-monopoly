//! `通用:[都筑诗船]Parking Space` -- C# `CardParkingSpace` (MatchHost.cs:2430-2501):
//! place on Space, hand its houses to your other tiles, settle becomes a
//! turn-end [停留].
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[都筑诗船]Parking Space`）:
//! > [都筑诗船]Parking Space：
//! > [手]：
//! > 将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。
//! > [持续]：
//! >
//! > （1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。
//! >
//! > （2）位于此卡所在格子上的玩家无法使用角色及乐队技能。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const PARKING_SPACE: CardDef = CardDef {
    id: "通用:[都筑诗船]Parking Space",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardParkingSpace.WhyNot`: refuses when the board has no "Space" tile
/// (`没有 Space`).
fn why_not(_seat: i32) -> Option<Msg> {
    if ctx::tile_named("Space") >= 0 {
        return None;
    }
    Some(Msg::new(key!("parking_space_no_space")))
}

fn play(seat: i32) {
    // 规则书[手]: 「将此卡放置于“Space”格子上」 -- C# `H.PlaceFromPlay(c, i, s)`
    // with `s = H.TileNamed("Space")`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "通用:[都筑诗船]Parking Space",
        &Msg::new(key!("parking_space_note")),
    );
    let space = ctx::tile_named("Space");
    // TODO(规则书)[手]: 「将此卡放置于“Space”格子上」 -- the placement is bound to
    // the seat's field, not the Space tile; needs field-card tile placement
    // (`H.PlaceFromPlay(c, owner, tile)`), the same gap as `kokoro_circle`.
    // 规则书[手]: 「将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）」
    // -- C# takes `H.State.houses[s]` houses off the Space tile (only when the
    // seat owns it) and hands one to each of the seat's other buildable deeds
    // (`H.State.houses[s]--` then `H.AddHouse(i, item, ...)`, at most one per
    // destination).
    let mut num = if space >= 0 && ctx::tile_owner(space) == seat {
        ctx::houses_of(space)
    } else {
        0
    };
    for t in ctx::owned_tiles(seat) {
        if num <= 0 {
            break;
        }
        if t == space {
            continue;
        }
        // TODO(规则书)[手]: the C# destination filter is `H.WhyNotBuildOn(i, t) == null`
        // (kind != "ring", rent capacity, house cap, `H._turnCtx.NoBuild`, the
        // "卡池BUG" event, `Fx.CanBuild`) -- only buyable / not-mortgaged /
        // can-pay are expressible here, and `ctx::add_house` does not clamp at
        // the C# house cap either. Same gap as `hey_kids`.
        if !ctx::is_buyable(t) || ctx::mortgaged_of(t) || !ctx::can_pay(seat) {
            continue;
        }
        ctx::add_house(space, -1);
        ctx::add_house(t, 1);
        num -= 1;
    }
    // TODO(规则书)（1）[持续]: 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」
    // -- needs the Fx.SettleInstead + Fx.TurnEndAfter hooks (C#
    // `CardParkingSpace.SettleInstead` / `TurnEndAfter`, `H.IncV(who, "parkingStay")`
    // then `H.GiveStay(turn, layers, ...)`).
    // TODO(规则书)（2）[持续]: 「位于此卡所在格子上的玩家无法使用角色及乐队技能」
    // -- needs a skill-suppression hook (C# `CardParkingSpace.NoteText` /
    // `Fx` gate on skill use at this tile).
}