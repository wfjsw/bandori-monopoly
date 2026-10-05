//! `RAS:游击演出` -- C# `CardGuerrilla` (MatchHost.cs:9710-9822): [反击] jump to
//! an unowned deed and must buy it; a [特] that revives it as a fake Live House.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:游击演出`）:
//! > 游击演出：\[特]：
//! > 此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。
//! > [手]：
//! > [反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const GUERRILLA: CardDef = CardDef {
    id: "RAS:游击演出",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `Free()` -- buyable tiles with no owner (`IsBuyable && owners[t] < 0`).
fn free_tiles() -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::tile_owner(t) < 0 && ctx::is_buyable(t))
        .collect()
}

/// 规则书[手][反击]: 「当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后」
fn can_react(seat: i32) -> bool {
    // 规则书[手][反击]: 「当次[结算]时被其他玩家的格子收取资金后」 -- C# kind
    // "paid" with `t.Pay.IsRent && t.Pay.to != seat` (this seat paid someone else).
    if trigger::kind() != TriggerKind::Paid || trigger::seat() != seat {
        return false;
    }
    if trigger::target() == seat {
        return false;
    }
    // TODO(ABI): `t.Pay.IsRent` is not in the Trigger payload -- any pay-to-other
    // currently counts, not only rent.
    // TODO(规则书)[手][反击]: 「当你经过属于其他玩家的livehouse格子后」 -- C#
    // `PassedOthersLH(seat, H.Settling)` walks the move path for a tile owned by
    // someone else that is a Live House of this seat; needs the move path /
    // `H.Settling` on the trigger.
    // C# also requires `Free().Count > 0`; `free_tiles` uses `is_buyable` now.
    !free_tiles().is_empty()
}

fn react(seat: i32) {
    // 规则书[手][反击]: 「传送至任意无主可购买的格子并[结算]，且必须购买」
    let free = free_tiles();
    if free.is_empty() {
        return;
    }
    // C# prefers tiles the seat can afford, falling back to all free ones;
    // `H.AskTileOf`'s AI default is the most expensive.
    let affordable: Vec<i32> = free
        .iter()
        .copied()
        .filter(|&t| ctx::money(seat) >= ctx::buy_price(t))
        .collect();
    let pool = if affordable.is_empty() { &free } else { &affordable };
    let to = ctx::ask_tile(
        seat,
        &Msg::new(key!("guerrilla_title")),
        &Msg::new(key!("guerrilla_ask")),
        pool,
    );
    // 规则书[手][反击]: 「传送至任意无主可购买的格子」 -- C# `H.ForceTeleport(i,
    // to, resolve: false, ...)`.
    ctx::teleport_to(seat, to);
    // 规则书[手][反击]: 「并[结算]，且必须购买」 -- C# `H.BuyRoutine(i, to, free:
    // false, ...)` after the teleport's settle, or a "资金不够，买不下" log.
    // TODO(规则书)[手][反击]: 「并[结算]」 -- needs the settle routine on the
    // teleported tile (C# `H.ForceTeleport` with `resolve: true`-style settle,
    // here forced by the `BuyRoutine` follow-up).
    // TODO(规则书)[手][反击]: 「且必须购买」 -- needs `H.BuyRoutine` (the buy
    // itself; `ctx::buy_price` only reads the price).
    if ctx::money(seat) < ctx::buy_price(to) {
        ctx::log(
            seat,
            &Msg::new(key!("guerrilla_broke")).seat("who", seat).tile("tile", to),
        );
    } else {
        ctx::log(
            seat,
            &Msg::new(key!("guerrilla_moved")).seat("who", seat).tile("tile", to),
        );
    }
    // TODO(规则书)[特]: 「此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。」
    // -- needs the Fx.OnDiscarded hook + turn-end callback (C#
    // `CardGuerrilla.OnDiscarded` / `Check`, `H._turnCtx.PaidInSettle`,
    // `H._turnCtx.AtEnd`), `H.WhyNotBuildOn` / tile kind "ring" for the
    // buildable-Live-House gate, and `H.PlaceCard` with `Mem["tile"]` plus the
    // `ExtraColor` override that makes that deed a Live House for this seat.
}
