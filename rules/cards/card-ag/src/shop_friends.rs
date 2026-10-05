//! `AG:商店街的青梅竹马` -- C# `CardShopFriends` (MatchHost.cs:988-1013):
//! 1d6-th own shop-street tile from 商店街, else 商店街, as the main move.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:商店街的青梅竹马`）:
//! > 商店街的青梅竹马：
//! >  投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数），如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街，视为你的主要移动
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const SHOP_FRIENDS: CardDef = CardDef {
    id: "AG:商店街的青梅竹马",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardShopFriends.WhyNot` -> `H.MoveWhyNot`: refuses off the main move.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「视为你的主要移动」 -- C# `H.MoveWhyNot` refuses when it is not
    // the seat's turn.
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("shop_friends_not_your_turn")));
    }
    // TODO(规则书): the rest of `H.MoveWhyNot` (`H._turnCtx.MainMoved` ->
    // 「这回合已经移动过了」, `State.skipMove` -> 「本回合不能移动」) is still
    // missing from the vocabulary, so those refusals cannot be expressed yet.
    None
}

fn play(seat: i32) {
    // 规则书: 「从商店街格子开始数」 -- C# walks `(street + d) % n` for d in 1..n,
    // keeping `H.IsShop(t) && H.State.owners[t] == i`.
    let street = ctx::tile_named("商店街");
    let n = ctx::tile_count();
    let mut list: Vec<i32> = Vec::new();
    if street >= 0 && n > 0 {
        for d in 1..n {
            let t = (street + d).rem_euclid(n);
            if ctx::is_shop(t) && ctx::tile_owner(t) == seat {
                list.push(t);
            }
        }
    }
    // 规则书: 「投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数）」
    let roll = ctx::roll(seat, 1, 6);
    let to = if (1..=list.len() as i32).contains(&roll) {
        list[(roll - 1) as usize]
    } else {
        // 规则书: 「如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街」
        street
    };
    if to < 0 {
        return;
    }
    // 规则书: 「[传送]」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo = to })`.
    ctx::teleport_to(seat, to);
    let why = if (1..=list.len() as i32).contains(&roll) {
        Msg::new(key!("shop_friends_to_own")).tile("tile", to).i("roll", roll as i64)
    } else {
        Msg::new(key!("shop_friends_to_street")).i("roll", roll as i64).i("count", list.len() as i64)
    };
    ctx::log(seat, &why);
    // TODO(规则书): 「视为你的主要移动」 -- needs the H.CardMove / main-move routine
    // (C# `H.CardMove(c, new MoveCtx { TeleportTo = to })`) so the teleport consumes
    // the turn's main move and runs settle on the destination.
}