//! `PPP:向着未来的路标` -- C# `CardSignpost` (MatchHost.cs:9018-9036): walk a full
//! lap without settling, then pay 1,000 at end of turn.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:向着未来的路标`）:
//! > 向着未来的路标：
//! > 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金
//!

use card_sdk::{ctx, key, CardDef, Msg};

// TODO(规则书): C# `CardSignpost.WhyNot` is just `H.MoveWhyNot(seat)` (main move
//   already used / movement blocked) -- needs `H.MoveWhyNot`; `why_not` stays None.
pub const SIGNPOST: CardDef = CardDef {
    id: "PPP:向着未来的路标",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书: 「本回合的主要移动设为移动60格子并不触发结算」 -- C#
    // `H.CardMove(c, new MoveCtx { Steps = 60, Resolve = false })`.
    // TODO(ABI): needs the H.CardMove / main-move routine (a 60-step walk that
    //   passes every tile and does *not* settle on arrival). `ctx::teleport_to`
    //   jumps without passing tiles and is `ForceTeleport(resolve: false)`, so it
    //   is not a stand-in for the walk.
    ctx::log(seat, &Msg::new(key!("signpost_move")).seat("who", seat).i("n", 60));
    // 规则书: 「回合结束时[失去]1000资金」 -- C# `H._turnCtx.AtEnd.Add(() =>
    // H.LoseR(i, 1000, ...))`.
    // TODO(ABI): needs the turn-end callback list (C# `TurnCtx.AtEnd`) or an
    //   Fx.TurnEnd hook; the vocabulary has no end-of-turn sink. When it exists:
    //   `ctx::pay(seat, 1000, ...)` against a `signpost_lose` reason.
    ctx::log(seat, &Msg::new(key!("signpost_pending")).seat("who", seat).n("n", 1000));
}