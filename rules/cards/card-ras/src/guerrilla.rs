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
use card_sdk::{key, CardDef, On, Msg};

pub const GUERRILLA: CardDef = CardDef::new("RAS:游击演出", &[
    On::React(&[TriggerKind::Paid], can_react, react),
    On::AtEnd(check),
]);

const ID: &str = "RAS:游击演出";

/// C# `H._turnCtx.GuerrillaArmed` -- the turn-end check is already scheduled.
const SLOT_ARMED: &str = "guerrilla_armed";

/// C# `Free()` -- buyable tiles with no owner (`IsBuyable && owners[t] < 0`).
fn free_tiles() -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::tile_owner(t) < 0 && ctx::is_buyable(t))
        .collect()
}

/// C# `lhBuildable` -- a buildable Live House (group 6, not a RiNG agent).
const LIVEHOUSE_PROPS: [&str; 4] = ["DUB MUSIC EXPERIMENT", "武道馆", "Space", "Live House Galaxy"];

fn lh_buildable(t: i32) -> bool {
    LIVEHOUSE_PROPS.iter().any(|&n| ctx::tile_named(n) == t) && ctx::is_buyable(t)
}

/// 规则书[手][反击]: 「当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后」
fn can_react(player_id: i32) -> bool {
    // 规则书[手][反击]: 「当次[结算]时被其他玩家的格子收取资金后」 -- C# kind
    // "paid" with `t.Pay.IsRent && t.Pay.to != seat` (this player paid someone else).
    if trigger::kind() != TriggerKind::Paid || trigger::player_id() != player_id {
        return false;
    }
    if trigger::target() == player_id {
        return false;
    }
    if !trigger::pay_is_rent() {
        return false;
    }
    // TODO(规则书)[手][反击]: 「当你经过属于其他玩家的livehouse格子后」 -- C#
    // `PassedOthersLH(player_id, H.Settling)` walks the move path for a tile owned by
    // someone else that is a Live House of this player; needs the move path /
    // `H.Settling` on the trigger.
    // C# also requires `Free().Count > 0`; `free_tiles` uses `is_buyable` now.
    !free_tiles().is_empty()
}

fn react(player_id: i32) {
    // 规则书[手][反击]: 「传送至任意无主可购买的格子并[结算]，且必须购买」
    let free = free_tiles();
    if free.is_empty() {
        return;
    }
    // C# prefers tiles the player can afford, falling back to all free ones;
    // `H.AskTileOf`'s AI default is the most expensive.
    let affordable: Vec<i32> = free
        .iter()
        .copied()
        .filter(|&t| ctx::money(player_id) >= ctx::buy_price(t))
        .collect();
    let pool = if affordable.is_empty() { &free } else { &affordable };
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("guerrilla_title")),
        &Msg::new(key!("guerrilla_ask")),
        pool,
    );
    // 规则书[手][反击]: 「传送至任意无主可购买的格子」 -- C# `H.ForceTeleport(i,
    // to, resolve: false, ...)`.
    ctx::teleport_to(player_id, to);
    // 规则书[手][反击]: 「并[结算]，且必须购买」 -- C# `H.BuyRoutine(i, to, free:
    // false, ...)` after the teleport's settle, or a "资金不够，买不下" log.
    // TODO(规则书)[手][反击]: 「并[结算]」 -- needs the settle routine on the
    // teleported tile (C# `H.ForceTeleport` with `resolve: true`-style settle,
    // here forced by the `BuyRoutine` follow-up).
    // TODO(规则书)[手][反击]: 「且必须购买」 -- needs `H.BuyRoutine` (the buy
    // itself; `ctx::buy_price` only reads the price).
    if ctx::money(player_id) < ctx::buy_price(to) {
        ctx::log(
            player_id,
            &Msg::new(key!("guerrilla_broke")).player_id("who", player_id).tile("tile", to),
        );
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("guerrilla_moved")).player_id("who", player_id).tile("tile", to),
        );
    }
    // C# `OnDiscarded` schedules `Check` at turn end once per turn (the card
    // lands in the discard after this reaction resolves).
    if ctx::turn_player() == player_id && ctx::slot(player_id, SLOT_ARMED) == 0 {
        ctx::set_slot(player_id, SLOT_ARMED, 1);
        ctx::before_turn_end(player_id);
    }
}

/// C# `CardGuerrilla.Check` -- at turn end, offer to revive the card from the
/// discard as a fake Live House over the player's most expensive deed.
fn check(player_id: i32) {
    ctx::set_slot(player_id, SLOT_ARMED, 0);
    // 规则书[特]: 「此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金
    // 且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子」
    // TODO(规则书)[特]: 「本回合的[结算]向其他玩家支付了至少1000资金」 -- needs
    // `H._turnCtx.PaidInSettle` (the turn's settle-to-others total); no such
    // query in the vocabulary. The check below is the rest of the gate and stays
    // inert until that counter is readable.
    let paid_in_settle_ok = false; // TODO: `H._turnCtx.PaidInSettle >= 1000`
    if !paid_in_settle_ok {
        return;
    }
    // 「你不拥有任何可盖房的live house格子」 -- owns no buildable Live House.
    if ctx::owned_tiles(player_id).into_iter().any(lh_buildable) {
        return;
    }
    // 「且场上已不存在可购买的此类格子」 -- no unowned buildable Live House.
    if (0..ctx::tile_count()).any(|t| lh_buildable(t) && ctx::tile_owner(t) < 0) {
        return;
    }
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() || ctx::discard_count(player_id, ID) <= 0 {
        return;
    }
    // 「则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子」
    if !ctx::ask_yes(player_id, &Msg::new(key!("guerrilla_title")), &Msg::new(key!("guerrilla_revive"))) {
        return;
    }
    if !ctx::take_card(player_id, ctx::CardPile::Discard, ID) {
        return;
    }
    // C# picks the most expensive owned deed (`Mem["tile"] = num + 1`).
    let best = mine
        .into_iter()
        .max_by_key(|&t| ctx::tile_price(t))
        .unwrap_or(-1);
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("guerrilla_note")));
    ctx::set_slot(player_id, "guerrilla_tile", best);
    ctx::log(
        player_id,
        &Msg::new(key!("guerrilla_revived")).player_id("who", player_id).tile("tile", best),
    );
    // TODO(规则书)[特]: 「使其对你视为live house格子」 -- needs the `ExtraColor`
    // override (C# `CardGuerrilla.ExtraColor`) that makes the designated deed a
    // Live House for this player; `H.IsLiveHouse` does not see it yet.
}
