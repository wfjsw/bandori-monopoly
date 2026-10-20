//! `PPP:向着未来的路标` -- C# `CardSignpost` (MatchHost.cs:9018-9036): walk a full
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:向着未来的路标`）:
//! > 向着未来的路标：
//! > 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金
//!
//! lap without settling, then pay 1,000 at end of turn.

use card_sdk::{Action, CardDef, ctx, key, Msg, On};
use card_sdk::abi::ActionKind;

pub const SIGNPOST: CardDef = CardDef::new(
    "PPP:向着未来的路标",
    // The body-top `player_out(owner)` early-out is the condition now: a
    // bankrupt seat pays nobody, so the callback must not run (or flash).
    &[
        On::Play("", "", Some(cant_play), play),
        On::Action(Action::AtEnd("owner.out == 0", "", None, at_end)),
    ],
)
.props(&[(card_sdk::abi::prop::EST_COST, 1000)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"向着未来的路标"#)
        .raw_name(r#"向着未来的路标"#)
        .tags(&[])
        .text(r#"本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金"#)
        .cell(r#"B14"#)
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

/// C# `CardSignpost.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「本回合的主要移动设为移动60格子」 -- C# `H.CardMove(c, new MoveCtx
    // { Steps = 60, Resolve = false })`: the walk runs now and is the main move.
    ctx::plan::set_steps(60);
    // 规则书: 「并不触发结算」 -- C# `MoveCtx.Resolve = false` on the same move:
    // the 60-step walk passes every tile without landing on one.
    ctx::plan::set_resolve(false);
    ctx::log(
        player_id,
        &Msg::new(key!("signpost_move"))
            .player_id("who", player_id)
            .i("n", 60),
    );
    ctx::card_move(player_id);
    // 规则书: 「回合结束时[失去]1000资金」 -- C# `H._turnCtx.AtEnd.Add(() =>
    // H.LoseR(i, 1000, ...))`, scheduled onto `On::AtEnd`.
    ctx::before_turn_end(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("signpost_pending"))
            .player_id("who", player_id)
            .n("n", 1000),
    );
    Ok(())
}

/// `On::AtEnd` (C# `H._turnCtx.AtEnd` -> `H.LoseR(i, 1000, ...)`) -- scheduled by
/// `ctx::before_turn_end` in `play`; runs once when the turn ends. The
/// "still in the game" gate is the entry's condition (`owner.out == 0`).
fn at_end(player_id: i32) -> card_sdk::Asked {
    ctx::pay(player_id, 1000, &Msg::new(key!("signpost_lose")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}
