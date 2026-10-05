//! `HHW:（美咲）` -- C# `CardMisakiCard` (MatchHost.cs:4387-4461): [反击] a fire-pot
//! move roll, teleport to a rival standing between start and end, then collect 500.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（美咲）`）:
//! > （美咲）[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const MISAKI_CARD: CardDef = CardDef {
    id: "HHW:（美咲）",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `CardMisakiCard.Between` -- the other seats standing in the move's span.
/// Direction (`t.Move.Dir`) is missing from the trigger, so this is the forward
/// span only (see the TODO in [`can_react`]).
fn between(seat: i32) -> Vec<i32> {
    let Some(roll) = trigger::move_roll() else {
        return Vec::new();
    };
    let roll = roll.abs();
    let start = ctx::seat_pos(seat);
    // 规则书: `H.Forward(start, pos)` -- forward distance around the ring.
    ctx::others(seat)
        .into_iter()
        .filter(|&p| {
            let d = ctx::tile_forward(start, ctx::seat_pos(p));
            (1..=roll).contains(&d)
        })
        .collect()
}

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「使用火罐进行移动掷骰后，触发结算前可打出此卡」 -- C#
    // `CanReact`: `t.Kind == "moveRoll" && t.Seat == seat && t.Move != null &&
    // t.Move.FireRoll && Between(t.Move).Count > 0`.
    if trigger::kind() != TriggerKind::MoveRoll || trigger::seat() != seat {
        return false;
    }
    // TODO(ABI): `t.Move.FireRoll` (the move used a fire pot) -- the trigger
    // carries no move flags, so every move roll of yours matches. Never faked.
    // TODO(ABI): `t.Move.Dir` -- `Between` uses forward distance only; a
    // backward move's span is not representable.
    !between(seat).is_empty()
}

fn react(seat: i32) {
    let list = between(seat);
    if list.is_empty() {
        return;
    }
    // 规则书[反击]: 「使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子」
    // -- C# `H.AskSeat(i, "另一个我", ..., list)`.
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("misaki_card_title")),
        &Msg::new(key!("misaki_card_ask")),
        &list,
    );
    // 规则书[反击]: 「消耗所有火罐」 -- C# `H.SpendFire(i, H.Fire(i), "（美咲）")`.
    let fire = ctx::fire(seat);
    if fire > 0 {
        ctx::spend_fire(seat, fire, &Msg::new(key!("misaki_card_pay")));
    }
    // 规则书[反击]: 「传送至...玩家所在的格子并触发结算」 -- C# `H.TeleportMove`
    // (teleport + settle). `ctx::teleport_to` moves without settling.
    let to = ctx::seat_pos(who);
    ctx::teleport_to(seat, to);
    ctx::log(seat, &Msg::new(key!("misaki_card_moved")).seat("who", seat).tile("tile", to));
    // TODO(规则书): 「并触发结算」 -- needs a teleport-with-settle routine (C#
    // `H.TeleportMove(m2)`). `ctx::teleport_to` is `H.ForceTeleport(..., resolve:
    // false)`.
    // TODO(规则书): 「（视为你的主要移动）」 -- needs the H.CardMove / main-move
    // routine (C# `m2.Main = m.Main`) so this teleport consumes the turn's main
    // move.
    if ctx::seat_out(seat) {
        return;
    }
    // 规则书[反击]: 「随后那格及相邻2格上的所有其他玩家[支付]你500资金」 -- C#
    // `H.PayR(item, i, 500, "（美咲）", i)` for every `H.Others(i)` on tiles
    // `to-2 ..= to+2` of the 60-tile ring.
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    for d in -2..=2 {
        let t = (to + d).rem_euclid(n);
        for p in ctx::seats_on(t, seat) {
            ctx::transfer(p, seat, 500, &Msg::new(key!("misaki_card_pay")));
        }
    }
}