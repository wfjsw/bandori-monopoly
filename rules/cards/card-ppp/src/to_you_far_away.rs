//! `PPP:献给远方的你` -- C# `CardToYouFarAway` (MatchHost.cs:8873-8918): teleport to
//! a farthest player and settle, then maybe raise a house on your farthest deed.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:献给远方的你`）:
//! > 献给远方的你：
//! > [手]：
//! > [传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖（例：[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10）。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg};

pub const TO_YOU_FAR_AWAY: CardDef = CardDef {
    id: "PPP:献给远方的你",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardToYouFarAway.WhyNot`: refuses with 「没有别的玩家」 when `H.Others` is
    // empty (no other present seat at all).
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("to_you_far_away_none")).seat("who", seat));
    }
    None // playable
}

fn play(seat: i32) {
    // 规则书: 「[传送]至任意与[使用者]绝对距离最远的玩家的格子」 -- C# `H.Farthest(i)`
    // keeps every other seat sitting at the maximum absolute distance.
    let pos = ctx::seat_pos(seat);
    let others = ctx::others(seat);
    // C# `Play` yields break when `H.Farthest` is empty (WhyNot refuses earlier).
    if others.is_empty() {
        return;
    }
    let best = others.iter().map(|&p| ctx::dist(pos, ctx::seat_pos(p))).max().unwrap_or(0);
    let mut far: Vec<i32> = others
        .into_iter()
        .filter(|&p| ctx::dist(pos, ctx::seat_pos(p)) == best)
        .collect();
    if far.is_empty() {
        return;
    }
    // C# `H.AskSeat` only when several seats share the maximum.
    let who = if far.len() > 1 {
        far.sort();
        ctx::ask_seat(
            seat,
            &Msg::new(key!("to_you_far_away_title")),
            &Msg::new(key!("to_you_far_away_ask")),
            &far,
        )
    } else {
        far[0]
    };
    let to = ctx::seat_pos(who);
    // 规则书: 「[传送]至…玩家的格子」
    ctx::teleport_to(seat, to);
    ctx::log(
        seat,
        &Msg::new(key!("to_you_far_away_moved"))
            .seat("who", seat)
            .seat("target", who)
            .tile("tile", to),
    );
    // TODO(ABI): 「并[结算]」 -- `ctx::teleport_to` is `H.ForceTeleport(..., resolve:
    //   false)` (no settle). Needs `H.ForceTeleport(..., resolve: true)` (C#
    //   `CardToYouFarAway.Play`) so the landing settles.
    if ctx::seat_out(seat) {
        return;
    }
    // 规则书: 「然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖」
    // -- C# filters `H.OwnedBy` by `H.WhyNotBuildOn == null`, keeps those at the
    // maximum `H.Dist` from the new position, then `H.OfferBuildAmong`.
    let at = ctx::seat_pos(seat);
    let mine = ctx::owned_tiles(seat);
    let best = mine.iter().map(|&t| ctx::dist(at, t)).max().unwrap_or(0);
    let far_tiles: Vec<i32> = mine.into_iter().filter(|&t| ctx::dist(at, t) == best).collect();
    // TODO(ABI): 「加盖」 -- needs `H.OfferBuildAmong` / `H.WhyNotBuildOn` (pay
    //   `ctx::build_cost` and raise one house). `ctx::build_cost` is a query only.
    //   Candidates today would be `far_tiles`.
    let _ = far_tiles;
}