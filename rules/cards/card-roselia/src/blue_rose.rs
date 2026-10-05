//! `R:蓝玫瑰的骄傲` -- C# `CardBlueRose` (MatchHost.cs:10848-10901): 20% of your
//!
//! 规则书（docs/rulebook/cards.json, id `R:蓝玫瑰的骄傲`）:
//! > 蓝玫瑰的骄傲：
//! >  打出此卡时，
//! > （1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。
//! > （2）若严格少于，则传送至下一个未被购买的Livehouse格子；若此时已没有未被购买的Livehouse格子，则传送至“RiNG 4”（不触发结算），均视为你的主要移动。
//!
//! Livehouse deeds, else teleport to the next free Livehouse (or RiNG 4) as your
//! main move.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const BLUE_ROSE: CardDef = CardDef {
    id: "R:蓝玫瑰的骄傲",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// The buyable Livehouse deeds (C# `H.IsLiveHouse`: `IsColor(seat, t, 6)`, and
/// `H.LiveHouses` also wants `IsBuyable`).
fn livehouses() -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::tile_group(t) == 6)
        .collect()
    // TODO(规则书): `H.IsColor` also reads `_tileColors[t]` and every live
    //   `Fx.ExtraColor` (band skills that re-colour a tile); neither is in the
    //   vocabulary, so those swaps are not seen here.
}

/// C# `CardBlueRose.Rich` -- Livehouses at least half of your deeds (and you have one).
fn is_rich(seat: i32) -> bool {
    let live = live_deeds(seat).len() as i32;
    let total = ctx::owned_count(seat);
    live > 0 && live * 2 >= total
}

fn live_deeds(seat: i32) -> Vec<i32> {
    let ids = livehouses();
    ctx::owned_tiles(seat)
        .into_iter()
        .filter(|t| ids.contains(t))
        .collect()
}

/// C# `CardBlueRose.WhyNot`: the 20% gain branch is always playable; the
/// teleport branch runs `H.MoveWhyNot`.
fn why_not(seat: i32) -> Option<Msg> {
    if is_rich(seat) {
        return None;
    }
    // TODO(规则书): `H.MoveWhyNot` -- the teleport branch is "你的主要移动", so
    //   the C# refuses 「这回合已经移动过了」 (`_turnCtx.MainMoved`) and
    //   「本回合不能移动」 (`State.skipMove`); those move-state reads are still
    //   engine holes (`H.MoveWhyNot` in the still-missing list). Off-turn
    //   ("只能在自己的回合") is already refused by the engine's play phase.
    None
}

fn play(seat: i32) {
    let pos = ctx::seat_pos(seat);
    let live = live_deeds(seat);
    let others = ctx::owned_count(seat) - live.len() as i32;
    // 规则书（1）: 「若你拥有的Livehouse格子多于或等于你拥有的其他格子」
    if !live.is_empty() && live.len() as i32 >= others {
        // 规则书（1）: 「获得相当于这些Livehouse格子地契价值20%的资金」 -- the land
        //   price, C# `H._tiles[t].price` (no standing houses).
        let sum: i64 = live.iter().map(|&t| ctx::tile_price(t) as i64).sum();
        // C# `CeilTo(sum * 0.2, 10)` -- 20% rounded up to a multiple of 10.
        let amount = (((sum + 49) / 50) * 10) as i32;
        ctx::gain(seat, amount, &Msg::new(key!("blue_rose_gain")).i("n", sum));
        return;
    }
    // 规则书（2）: 「传送至下一个未被购买的Livehouse格子」 -- nearest unowned ahead (C#
    //   `H.LiveHouses(i, t => owners[t] < 0)` ordered by `H.Forward`).
    let free: Vec<i32> = livehouses()
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    let mut fallback = false;
    let to = if !free.is_empty() {
        free.into_iter()
            .min_by_key(|&t| {
                let d = ctx::tile_forward(pos, t);
                if d == 0 {
                    ctx::tile_count()
                } else {
                    d
                }
            })
            .unwrap_or(-1)
    } else {
        // 规则书（2）: 「若此时已没有未被购买的Livehouse格子，则传送至“RiNG 4”（不触发结算）」
        fallback = true;
        ctx::tile_named("RiNG 4")
    };
    if to < 0 {
        return;
    }
    // 规则书（2）: 「传送至」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo = to })`.
    ctx::teleport_to(seat, to);
    let why = if fallback {
        Msg::new(key!("blue_rose_ring4")).seat("who", seat).tile("tile", to)
    } else {
        Msg::new(key!("blue_rose_teleport")).seat("who", seat).tile("tile", to)
    };
    ctx::log(seat, &why);
    // TODO(规则书)（2）: 「（不触发结算）」 applies only to the RiNG 4 fallback (C#
    //   `Resolve = false`); the free-Livehouse teleport settles on arrival. Needs the
    //   H.CardMove / main-move routine (C# `H.CardMove(c, new MoveCtx { TeleportTo })`)
    //   so the teleport is this turn's main move and settles (or not) as written;
    //   `teleport_to` is `H.ForceTeleport(..., resolve: false)` and never settles.
}