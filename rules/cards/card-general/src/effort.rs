//! `通用:尽力后的收获` -- C# `CardEffort` (MatchHost.cs:2304-2332): this turn's
//! main move becomes 1-6 steps and settles.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:尽力后的收获`）:
//! > 尽力后的收获：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const EFFORT: CardDef = CardDef {
    id: "通用:尽力后的收获",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardEffort.WhyNot`: refuses the card after the main move
/// (`这回合已经移动过了`) and while `H.State.skipMove` (`本回合不能移动`).
fn why_not(_seat: i32) -> Option<Msg> {
    // TODO(规则书): `H.MoveWhyNot` -- the C# refuses 「这回合已经移动过了」
    //   (`_turnCtx.MainMoved`) and 「本回合不能移动」 (`State.skipMove`); those
    //   move-state reads are still engine holes (`H.MoveWhyNot` in the
    //   still-missing list), so neither refusal can be expressed yet and the
    //   gate is over-permissive on the move window.
    None
}

fn play(seat: i32) {
    // 规则书[手]: 「本回合的[主要移动]改为移动1到6以内的任意整数」 -- C#
    // `H.AskNumber(c.Seat, ..., 1, max=6, ...)` picks the step count.
    let steps = ctx::ask_number(
        seat,
        &Msg::new(key!("effort_title")),
        &Msg::new(key!("effort_ask")),
        1,
        6,
    );
    // 规则书[手]: 「立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]」
    // -- C# `H.CardMove(c, new MoveCtx { Steps = Math.Max(1, r.value) })`.
    let steps = steps.max(1);
    let to = ctx::tile_steps_ahead(seat, steps);
    if to >= 0 {
        ctx::teleport_to(seat, to);
    }
    // TODO(规则书): 「立刻进入移动阶段…并[结算]」 -- needs the H.CardMove / main-move
    // routine (C# `H.CardMove(c, new MoveCtx { Steps = ... })`) so the walk is the
    // turn's main move and settles on arrival; `teleport_to` moves without settling
    // and leaves the normal main move in place.
}