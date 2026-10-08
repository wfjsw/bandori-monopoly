//! `PPP:抓到了` -- C# `CardCaught` (MatchHost.cs:8776-8813): walk to the next
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:抓到了`）:
//! > 抓到了：
//! > 移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。
//!
//! player ahead and treat the walk as the main move, build allowed.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const CAUGHT: CardDef = CardDef::new("PPP:抓到了", &[On::Play("", Some(cant_play), play)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardCaught.WhyNot`: `H.MoveWhyNot` first (main move still available),
    // then refuses with 「前方没有别的玩家」 unless another present player stands on
    // a different tile.
    if let Some(why) = ctx::cant_move(player_id) {
        return Some(why);
    }
    let pos = ctx::player_pos(player_id);
    if ctx::others(player_id)
        .iter()
        .all(|&p| ctx::player_pos(p) == pos)
    {
        return Some(Msg::new(key!("caught_none")).player_id("who", player_id));
    }
    None // playable
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书: 「移动到你前方一名玩家的格子」 -- C# `CardCaught.Play` offers the other
    // players at a different tile, nearest forward first (`orderby H.Forward`).
    let mut list: Vec<i32> = ctx::others(player_id)
        .into_iter()
        .filter(|&p| ctx::player_pos(p) != pos)
        .collect();
    // C# `Play` only prompts when the list is non-empty (`if (list.Count != 0)`).
    if list.is_empty() {
        return Ok(());
    }
    list.sort_by_key(|&p| ctx::tile_forward(pos, ctx::player_pos(p)));
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("caught_title")),
        &Msg::new(key!("caught_ask")).player_id("who", player_id),
        &list,
    )?;
    let steps = ctx::tile_forward(pos, ctx::player_pos(who));
    // 规则书: 「视为本回合的主要移动」 -- C# `H.CardMove(c, new MoveCtx { Steps =
    // H.Forward(...) })`: the walk passes intervening tiles and settles on the
    // target, and `card_move` consumes the turn's main move (`MainMoveAs`).
    ctx::plan::set_steps(steps);
    ctx::log(
        player_id,
        &Msg::new(key!("caught_target"))
            .player_id("who", player_id)
            .player_id("target", who)
            .i("n", steps as i64),
    );
    ctx::card_move(player_id);
    // 规则书: 「且可选择盖房」 -- C# arms `H._turnCtx.BuildOk` and lets the engine
    // offer afterwards. An explicit `H.OfferBuildAmong` on the tile reached is
    // the same effect without a hidden turn flag: ask, then build there.
    ctx::card_offer_build(player_id, &[ctx::player_pos(player_id)]);
    Ok(())
}
