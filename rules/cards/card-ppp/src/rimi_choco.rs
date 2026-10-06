//! `PPP:（里美）我的心就像巧克力螺` -- C# `CardRimiChoco` (MatchHost.cs:9241-9285):
//! jump 1-4 tiles (either way) as the main move, unstoppable.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（里美）我的心就像巧克力螺`）:
//! > （里美）我的心就像巧克力螺：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const RIMI_CHOCO: CardDef = CardDef::new(
    "PPP:（里美）我的心就像巧克力螺",
    &[On::Play(Some(cant_play), play)],
);

/// C# `CardRimiChoco.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 || pos < 0 {
        return Ok(());
    }
    // 规则书: 「移动到当前格子绝对距离1到4格或以内的任何格子」 -- C#
    // `CardRimiChoco.Play` offers the eight tiles at offsets ±1..±4.
    let mut tiles: Vec<i32> = Vec::new();
    for i in -4i32..=4 {
        if i != 0 {
            tiles.push((pos + i).rem_euclid(n));
        }
    }
    if tiles.is_empty() {
        return Ok(());
    }
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("rimi_choco_title")),
        &Msg::new(key!("rimi_choco_ask")),
        &tiles,
    )?;
    // 规则书: 「期间[不可阻挡]」 -- a blocker cannot stop this walk. The window is
    // the move, and the move is the turn's main move and consumes it, so a
    // turn-end expiry is the same window here.
    ctx::state::set(player_id, "unstoppable", 1);
    ctx::state::set_expires(player_id, "unstoppable", ctx::state::TURN_END);
    // 规则书: 「并[结算]」 / 「本回合的[主要移动]改为…」 -- C# `H.CardMove` with
    // `Steps` (forward, or `Reverse` with `n - steps` when the target is behind).
    // `MoveCtx.Resolve` defaults to true, so the landing settles.
    let steps = ctx::tile_forward(pos, to);
    let reverse = steps > 4;
    let len = if reverse { n - steps } else { steps };
    ctx::plan::set_steps(len);
    ctx::plan::set_reverse(reverse);
    ctx::log(
        player_id,
        &Msg::new(key!("rimi_choco_moved"))
            .player_id("who", player_id)
            .tile("tile", to)
            .i("n", len as i64),
    );
    ctx::card_move(player_id);
    Ok(())
}
