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

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BLUE_ROSE: CardDef = CardDef::new("R:蓝玫瑰的骄傲", &[On::Play(Some(cant_play), play)]);

/// The buyable Livehouse deeds (C# `H.IsLiveHouse`: `IsColor(player_id, t, 6)`, and
/// `H.LiveHouses` also wants `IsBuyable`).
fn livehouses(player_id: i32) -> Vec<i32> {
    // `H.IsLiveHouses` = `IsBuyable && IsColor(t, 6)`, and `IsColor` sees both
    // the global `_tileColors` re-colour and this player's `Fx.ExtraColor`.
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::is_color(player_id, t, 6))
        .collect()
}

/// C# `CardBlueRose.Rich` -- Livehouses at least half of your deeds (and you have one).
fn is_rich(player_id: i32) -> bool {
    let live = live_deeds(player_id).len() as i32;
    let total = ctx::owned_count(player_id);
    live > 0 && live * 2 >= total
}

fn live_deeds(player_id: i32) -> Vec<i32> {
    let ids = livehouses(player_id);
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|t| ids.contains(t))
        .collect()
}

/// C# `CardBlueRose.WhyNot`: the 20% gain branch is always playable; the
/// teleport branch runs `H.MoveWhyNot`.
fn cant_play(player_id: i32) -> Option<Msg> {
    if is_rich(player_id) {
        return None;
    }
    // 规则书（2）: 「均视为你的主要移动」 -- the teleport branch is the main move,
    // so the C# `H.MoveWhyNot` gate applies (own turn, main move still available,
    // turn's move not skipped).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    let live = live_deeds(player_id);
    let others = ctx::owned_count(player_id) - live.len() as i32;
    // 规则书（1）: 「若你拥有的Livehouse格子多于或等于你拥有的其他格子」
    if !live.is_empty() && live.len() as i32 >= others {
        // 规则书（1）: 「获得相当于这些Livehouse格子地契价值20%的资金」 -- the land
        //   price, C# `H._tiles[t].price` (no standing houses).
        let sum: i64 = live.iter().map(|&t| ctx::tile_price(t) as i64).sum();
        // C# `CeilTo(sum * 0.2, 10)` -- 20% rounded up to a multiple of 10.
        let amount = (((sum + 49) / 50) * 10) as i32;
        ctx::gain(
            player_id,
            amount,
            &Msg::new(key!("blue_rose_gain")).i("n", sum),
        );
        return Ok(());
    }
    // 规则书（2）: 「传送至下一个未被购买的Livehouse格子」 -- nearest unowned ahead (C#
    //   `H.LiveHouses(i, t => owners[t] < 0)` ordered by `H.Forward`).
    let free: Vec<i32> = livehouses(player_id)
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
        return Ok(());
    }
    // 规则书（2）: 「传送至」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo = to })`.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    // 规则书（2）: 「（不触发结算）」 -- C# `Resolve = false` on the RiNG 4 fallback
    // only; the free-Livehouse teleport settles on arrival (`Resolve` defaults true).
    ctx::plan::set_resolve(!fallback);
    // C# `H.CardMove` (`MainMoveAs`) consumes the turn's main move and runs the
    // teleport immediately.
    ctx::card_move(player_id);
    let why = if fallback {
        Msg::new(key!("blue_rose_ring4"))
            .player_id("who", player_id)
            .tile("tile", to)
    } else {
        Msg::new(key!("blue_rose_teleport"))
            .player_id("who", player_id)
            .tile("tile", to)
    };
    ctx::log(player_id, &why);
    Ok(())
}
