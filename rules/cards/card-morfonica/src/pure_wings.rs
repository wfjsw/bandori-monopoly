//! `Mor:纯真振翅` -- C# `CardPureWings` (MatchHost.cs:5156-5177): teleport 20 tiles
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PURE_WINGS: CardDef = CardDef::new("Mor:纯真振翅", &[On::Play("", None, pure_wings)]);

fn pure_wings(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「传送到移动方向20格后」 -- C# `num2 = Plan.Reverse ? -1 : 1`,
    // `to = ((pos + 20 * num2) % n + n) % n`.
    // 「移动方向」 is the turn's planned direction (`H._turnCtx.Plan.Reverse`) --
    // `ctx::plan::dir()` (+1 forward / -1 backward).
    let dir = ctx::plan::dir();
    let to = ctx::tile_steps_ahead(player_id, 20 * dir);
    if to < 0 {
        return Ok(());
    }
    // 规则书: 「（不触发结算）」 -- C# `H.ForceTeleport(i, to, resolve: false, ...)`:
    // a raw position write (no settle, no move bookkeeping). The jump is not
    // 「视为你的主要移动」, so it must leave the main move free for the roll
    // below -- `card_move` (`H.CardMove` / MainMoveAs) would consume it.
    ctx::teleport_to(player_id, to);
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书: 「立刻进行移动掷骰」 -- C# `H.MainMove(i, ...)`: the turn's main
    // move roll, walking from the jump's destination (not from where the card
    // was played). A `card_move` with no fixed step count rolls the plan's dice
    // and walks that far; the landing settles (the default). Clear the plan's
    // destination so this is a walk, not another jump -- and leave `steps` at
    // its -1 "roll it" default (`set_steps(-1)` would clamp to 0).
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
    ctx::plan::set_teleport_to(-1);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("pure_wings_roll")).player_id("who", player_id),
    );
    Ok(())
}
