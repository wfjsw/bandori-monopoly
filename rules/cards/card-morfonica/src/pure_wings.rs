//! `Mor:纯真振翅` -- C# `CardPureWings` (MatchHost.cs:5156-5177): teleport 20 tiles
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const PURE_WINGS: CardDef = CardDef::new("Mor:纯真振翅", &[
    On::Play(None, pure_wings)]);

fn pure_wings(player_id: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    // 规则书: 「传送到移动方向20格后」 -- C# `num2 = Plan.Reverse ? -1 : 1`,
    // `to = ((pos + 20 * num2) % n + n) % n`.
    // 「移动方向」 is the turn's planned direction (`H._turnCtx.Plan.Reverse`) --
    // `ctx::plan::dir()` (+1 forward / -1 backward).
    let dir = ctx::plan::dir();
    let to = ctx::tile_steps_ahead(player_id, 20 * dir);
    if to < 0 {
        return;
    }
    // 规则书: 「（不触发结算）」 -- C# `H.ForceTeleport(i, to, resolve: false, ...)`
    //   = `set_kind(Teleport)` + `set_teleport_to(to)` + `set_resolve(false)` +
    //   `card_move(player_id)`.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(false);
    ctx::card_move(player_id);
    if ctx::player_out(player_id) {
        return;
    }
    // 规则书: 「立刻进行移动掷骰」 -- C# `H.MainMove(i, ...)` when `!MainMoved`.
    // A `card_move` with no fixed step count rolls the plan's dice and walks
    // that far; the teleport above did not settle, so this is the movement.
    // It consumes the turn's main move, which is what 「主要移动」 means.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
    ctx::plan::set_steps(-1);
    ctx::card_move(player_id);
    ctx::log(player_id, &Msg::new(key!("pure_wings_roll")).player_id("who", player_id));
}