//! `Mor:纯真振翅` -- C# `CardPureWings` (MatchHost.cs:5156-5177): teleport 20 tiles
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const PURE_WINGS: CardDef = CardDef::new("Mor:纯真振翅", &[
    On::Play(pure_wings),
]);

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
    // 规则书: 「（不触发结算）」 -- C# `H.ForceTeleport(i, to, resolve: false, ...)`.
    ctx::teleport_to(player_id, to);
    if ctx::player_out(player_id) {
        return;
    }
    // 规则书: 「立刻进行移动掷骰」
    // TODO(规则书): 「立刻进行移动掷骰」 -- needs the main-move routine (C#
    // `H.MainMove(i, H.State.roller >= 0 ? H.State.roller : i)` when
    // `!H._turnCtx.MainMoved`) so the teleport is followed by that turn's move
    // roll; today the player still gets its normal main move separately.
    // 规则书: 「立刻进行移动掷骰」 -- until the main-move hook lands this is only a log.
    ctx::log(player_id, &Msg::new(key!("pure_wings_roll")).player_id("who", player_id));
}