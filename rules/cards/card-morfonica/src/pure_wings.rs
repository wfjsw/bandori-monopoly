//! `Mor:纯真振翅` -- C# `CardPureWings` (MatchHost.cs:5156-5177): teleport 20 tiles
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PURE_WINGS: CardDef = CardDef::new("Mor:纯真振翅", &[On::Play("", None, pure_wings)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"纯真振翅"#)
            .raw_name(r#"纯真振翅"#)
            .tags(&[])
            .text(CardText::para(
                r#"传送到移动方向20格后（不触发结算），立刻进行移动掷骰"#,
            ))
            // sheet cell G13
            .fields(&[("group", r#"Morfonica"#), ("owner", "")]),
    );

fn pure_wings(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「传送到移动方向20格后」 -- C# `num2 = Plan.Reverse ? -1 : 1`,
    // `to = ((pos + 20 * num2) % n + n) % n`.
    // 「移动方向」 is the turn's planned direction (`H._turnCtx.Plan.Reverse`) --
    // `ctx::plan::dir()` (+1 forward / -1 backward).
    // TODO(规则书)（传送到移动方向20格后）: 「移动方向」 is the moving player's own
    //   direction -- we read the shared `plan.reverse`, which a previous forced move
    //   (saki_move's 祥，移动) leaves stale.
    let dir = ctx::plan::dir();
    let to = ctx::tile_steps_ahead(player_id, 20 * dir);
    if to < 0 {
        return Ok(());
    }
    // 规则书: 「（不触发结算）」 -- C# `H.ForceTeleport(i, to, resolve: false, ...)`:
    // the engine teleport with `resolve = false`: the settle stages are gated
    // off, while the move tail (moveBefore / pass / overlap / moveAfter /
    // moveResolved) raises like any other movement (a nested jump is a bare
    // write). The jump is not 「视为你的主要移动」, so it must leave the main move
    // free for the roll below -- `card_move` (`H.CardMove` / MainMoveAs) would
    // consume it.
    // The jump must not settle, and `teleport_as` re-reads the shared plan
    // after the pass window -- a stale `resolve` there would re-arm it. Clear
    // it before the request, then arm the walk below.
    ctx::plan::set_resolve(false);
    ctx::teleport_to(player_id, to);
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书: 「立刻进行移动掷骰」 -- C# `H.MainMove(i, ...)`: the turn's main
    // move roll, walking from the jump's destination (not from where the card
    // was played). A `card_move` with no fixed step count rolls the plan's dice
    // and walks that far; the landing settles (the default). Clear the plan's
    // destination so this is a walk, not another jump -- and leave `steps`
    // unset (it is `None` on a fresh plan; `set_steps(-1)` would clamp to
    // `Some(0)`, and the engine drops a count an earlier move left behind at
    // every move entry).
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
    ctx::plan::set_teleport_to(-1);
    // The landing settle also inherits the shared `plan.pay_factor` (a prior
    // 祥，移动 leaves 500 and halves this walk's rent); the engine resets it
    // to 1.0 at the move entry unless a body shaped this move.
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("pure_wings_roll")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn pure_wings_kani(player_id: i32) -> card_sdk::Asked {
    pure_wings(player_id)
}
