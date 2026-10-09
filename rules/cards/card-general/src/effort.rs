//! `通用:尽力后的收获` -- C# `CardEffort` (MatchHost.cs:2304-2332): this turn's
//!
//! 规则书（docs/rulebook/cards.json, id `通用:尽力后的收获`）:
//! > 尽力后的收获：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。
//!
//! main move becomes 1-6 steps and settles.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const EFFORT: CardDef = CardDef::new("通用:尽力后的收获", &[On::Play("", Some(cant_play), play)]);

/// C# `CardEffort.WhyNot` defers to `H.MoveWhyNot`: refuses after the main move
/// (`这回合已经移动过了`) and while `H.State.skipMove` (`本回合不能移动`).
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「本回合的[主要移动]改为移动1到6以内的任意整数」 -- the card
    // replaces the main move, so the C# `H.MoveWhyNot` gate applies (own turn,
    // main move still available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「本回合的[主要移动]改为移动1到6以内的任意整数」 -- C#
    // `int max = c.N(0, 6)` sizes the ask; `H.AskNumber(c.Seat, ..., 1, max, ...)`
    // picks the step count. Include the geometric landing in every label;
    // movement reactions (including Sayo's extension) still happen afterwards.
    let max = ctx::n(0, 6);
    let dir = ctx::plan::dir();
    let options: Vec<Msg> = (1..=max)
        .map(|n| Msg::new(key!("effort_steps"))
            .i("n", n as i64)
            .tile("tile", ctx::tile_steps_ahead(player_id, n * dir)))
        .collect();
    let steps = ctx::ask_pick(
        player_id,
        &Msg::new(key!("effort_title")),
        &Msg::new(key!("effort_ask")),
        &options,
    )? as i32 + 1;
    // 规则书[手]: 「立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]」
    // -- C# `H.CardMove(c, new MoveCtx { Steps = Math.Max(1, r.value) })`
    // (MatchHost.cs:2326-2329): `Steps` >= 0 walks exactly that many with no roll,
    // `Resolve` defaults to true (the landing settles), and `H.CardMove` is the
    // turn's main move (`MainMoveAs`).
    let steps = steps.max(1);
    ctx::plan::set_steps(steps);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    Ok(())
}
