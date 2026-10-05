//! `PPP:向着未来的路标` -- C# `CardSignpost` (MatchHost.cs:9018-9036): walk a full
//! lap without settling, then pay 1,000 at end of turn.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:向着未来的路标`）:
//! > 向着未来的路标：
//! > 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SIGNPOST: CardDef = CardDef::new("PPP:向着未来的路标", &[On::Play(play), On::CantPlay(cant_play), On::AtEnd(at_end)]);

/// C# `CardSignpost.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

fn play(player_id: i32) {
    // 规则书: 「本回合的主要移动设为移动60格子」 -- C# `H.CardMove(c, new MoveCtx
    // { Steps = 60, Resolve = false })`: the walk runs now and is the main move.
    ctx::plan::set_steps(60);
    // 规则书: 「并不触发结算」 -- C# `MoveCtx.Resolve = false` on the same move:
    // the 60-step walk passes every tile without landing on one.
    ctx::plan::set_resolve(false);
    ctx::log(player_id, &Msg::new(key!("signpost_move")).player_id("who", player_id).i("n", 60));
    ctx::card_move(player_id);
    // 规则书: 「回合结束时[失去]1000资金」 -- C# `H._turnCtx.AtEnd.Add(() =>
    // H.LoseR(i, 1000, ...))`, scheduled onto `On::AtEnd`.
    ctx::before_turn_end(player_id);
    ctx::log(player_id, &Msg::new(key!("signpost_pending")).player_id("who", player_id).n("n", 1000));
}

/// `On::AtEnd` (C# `H._turnCtx.AtEnd` -> `H.LoseR(i, 1000, ...)`) -- scheduled by
/// `ctx::before_turn_end` in `play`; runs once when the turn ends.
fn at_end(player_id: i32) {
    if ctx::player_out(player_id) {
        return;
    }
    ctx::pay(player_id, 1000, &Msg::new(key!("signpost_lose")));
}
