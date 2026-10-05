//! `Mor:纯真振翅` -- C# `CardPureWings` (MatchHost.cs:5156-5177): teleport 20 tiles
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::{ctx, key, CardDef, Msg};

pub const PURE_WINGS: CardDef = CardDef {
    id: "Mor:纯真振翅",
    play: Some(pure_wings),
    can_react: None,
    react: None,
    why_not: None,
};

fn pure_wings(seat: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    // 规则书: 「传送到移动方向20格后」 -- C# `num2 = Plan.Reverse ? -1 : 1`,
    // `to = ((pos + 20 * num2) % n + n) % n`.
    // TODO(规则书): 「移动方向」 -- the turn's planned direction (`H._turnCtx.Plan.Reverse`)
    // is not on the ABI, so the teleport is always forward.
    let to = ctx::tile_steps_ahead(seat, 20);
    if to < 0 {
        return;
    }
    // 规则书: 「（不触发结算）」 -- C# `H.ForceTeleport(i, to, resolve: false, ...)`.
    ctx::teleport_to(seat, to);
    if ctx::seat_out(seat) {
        return;
    }
    // 规则书: 「立刻进行移动掷骰」
    // TODO(规则书): 「立刻进行移动掷骰」 -- needs the main-move routine (C#
    // `H.MainMove(i, H.State.roller >= 0 ? H.State.roller : i)` when
    // `!H._turnCtx.MainMoved`) so the teleport is followed by that turn's move
    // roll; today the seat still gets its normal main move separately.
    // 规则书: 「立刻进行移动掷骰」 -- until the main-move hook lands this is only a log.
    ctx::log(seat, &Msg::new(key!("pure_wings_roll")).seat("who", seat));
}