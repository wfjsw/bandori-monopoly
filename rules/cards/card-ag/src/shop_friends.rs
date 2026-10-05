//! `AG:商店街的青梅竹马` -- C# `CardShopFriends` (MatchHost.cs:988-1013):
//! 1d6-th own shop-street tile from 商店街, else 商店街, as the main move.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:商店街的青梅竹马`）:
//! > 商店街的青梅竹马：
//! >  投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数），如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街，视为你的主要移动
//!

use card_sdk::abi::MoveKind;
use card_sdk::ctx::{self, plan};
use alloc::vec::Vec;

use card_sdk::{key, CardDef, Msg, On};

pub const SHOP_FRIENDS: CardDef = CardDef::new("AG:商店街的青梅竹马", &[
    On::Play(Some(cant_play), play)]);

/// C# `CardShopFriends.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你的主要移动」 -- the teleport is the turn's main move, so
    // the C# `H.MoveWhyNot` gate applies (own turn, main move still available,
    // turn's move not skipped).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) {
    // 规则书: 「从商店街格子开始数」 -- C# walks `(street + d) % n` for d in 1..n,
    // keeping `H.IsShop(t) && H.State.owners[t] == i`.
    let street = ctx::tile_named("商店街");
    let n = ctx::tile_count();
    let mut list: Vec<i32> = Vec::new();
    if street >= 0 && n > 0 {
        for d in 1..n {
            let t = (street + d).rem_euclid(n);
            if ctx::is_shop(t) && ctx::tile_owner(t) == player_id {
                list.push(t);
            }
        }
    }
    // 规则书: 「投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数）」
    let roll = ctx::roll(player_id, 1, 6);
    let to = if (1..=list.len() as i32).contains(&roll) {
        list[(roll - 1) as usize]
    } else {
        // 规则书: 「如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街」
        street
    };
    if to < 0 {
        return;
    }
    let why = if (1..=list.len() as i32).contains(&roll) {
        Msg::new(key!("shop_friends_to_own")).tile("tile", to).i("roll", roll as i64)
    } else {
        Msg::new(key!("shop_friends_to_street")).i("roll", roll as i64).i("count", list.len() as i64)
    };
    ctx::log(player_id, &why);
    // 规则书: 「[传送]」 + 「视为你的主要移动」 -- one move, not a bare hop: it is
    // the turn's main move, it consumes it, and it settles at the destination.
    plan::set_kind(MoveKind::Teleport);
    plan::set_teleport_to(to);
    ctx::card_move(player_id);
}