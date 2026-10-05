//! `PPP:抓到了` -- C# `CardCaught` (MatchHost.cs:8776-8813): walk to the next
//! player ahead and treat the walk as the main move, build allowed.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:抓到了`）:
//! > 抓到了：
//! > 移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg};

pub const CAUGHT: CardDef = CardDef {
    id: "PPP:抓到了",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardCaught.WhyNot`: after `H.MoveWhyNot` passes, refuses with
    // 「前方没有别的玩家」 unless another present seat stands on a different tile.
    // TODO(规则书): the C# also refuses via `H.MoveWhyNot` (main move already used /
    //   movement blocked) -- needs `H.MoveWhyNot` (this gate currently only covers
    //   the player check).
    let pos = ctx::seat_pos(seat);
    if ctx::others(seat).iter().all(|&p| ctx::seat_pos(p) == pos) {
        return Some(Msg::new(key!("caught_none")).seat("who", seat));
    }
    None // playable
}

fn play(seat: i32) {
    let pos = ctx::seat_pos(seat);
    // 规则书: 「移动到你前方一名玩家的格子」 -- C# `CardCaught.Play` offers the other
    // seats at a different tile, nearest forward first (`orderby H.Forward`).
    let mut list: Vec<i32> = ctx::others(seat)
        .into_iter()
        .filter(|&p| ctx::seat_pos(p) != pos)
        .collect();
    // C# `Play` only prompts when the list is non-empty (`if (list.Count != 0)`).
    if list.is_empty() {
        return;
    }
    list.sort_by_key(|&p| ctx::tile_forward(pos, ctx::seat_pos(p)));
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("caught_title")),
        &Msg::new(key!("caught_ask")).seat("who", seat),
        &list,
    );
    let _steps = ctx::tile_forward(pos, ctx::seat_pos(who));
    // C# `H._turnCtx.BuildOk = true` -- the optional build after this move.
    // TODO(规则书): 「视为本回合的主要移动且可选择盖房」 -- needs the H.CardMove /
    //   main-move routine (C# `H.CardMove(c, new MoveCtx { Steps = H.Forward(...) })`)
    //   so the walk passes intervening tiles and settles on the target, plus the
    //   `BuildOk` turn flag (`H.OfferBuildAmong` afterwards). Until then the seat is
    //   not moved at all; only the target choice runs.
    ctx::log(
        seat,
        &Msg::new(key!("caught_target")).seat("who", seat).seat("target", who).i("n", _steps as i64),
    );
}