//! `HHW:（美咲）` -- C# `CardMisakiCard` (MatchHost.cs:4387-4461): [反击] a fire-pot
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（美咲）`）:
//! > （美咲）[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。
//!
//! move roll, teleport to a rival standing between start and end, then collect 500.

use alloc::vec::Vec;

use card_sdk::abi::{ChainKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MISAKI_CARD: CardDef = CardDef::new(
    "HHW:（美咲）",
    &[On::Counteract(
        &[ChainKind::MoveRoll],
        // 规则书[反击]: 「使用火罐进行移动掷骰后」 -- the [火罐] roll is
        // card-owned state: the card that armed one tagged the move
        // (`ctx::plan::set_tag("fireRoll", 1)`).
        "actor == owner && move.tag('fireRoll') != 0 && between(owner) > 0",
        None,
        counteract,
    )],
)
.legacy(&[(0, legacy_can_counteract)]);

/// C# `CardMisakiCard.Between` -- the other players standing in the move's span,
/// in the direction the move actually travels (`t.Move.Dir`).
fn between(player_id: i32) -> Vec<i32> {
    let Some(roll) = trigger::move_roll() else {
        return Vec::new();
    };
    let roll = roll.abs();
    let start = ctx::player_pos(player_id);
    let n = ctx::tile_count();
    let backward = trigger::move_dir() < 0;
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| {
            // 规则书: `H.Forward(start, pos)` -- forward distance around the ring.
            let fwd = ctx::tile_forward(start, ctx::player_pos(p));
            let d = if backward { (n - fwd) % n } else { fwd };
            (1..=roll).contains(&d)
        })
        .collect()
}

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「使用火罐进行移动掷骰后，触发结算前可打出此卡」 -- C#
    // `CanCounteract`: `t.Kind == "moveRoll" && t.Seat == seat && t.Move != null &&
    // t.Move.FireRoll && Between(t.Move).Count > 0`.
    if trigger::player_id() != player_id {
        return false;
    }
    // The [火罐] roll is card-owned state: the card that armed one tagged the
    // move (see `ctx::plan::set_tag`).
    if trigger::move_tag("fireRoll") == 0 {
        return false;
    }
    !between(player_id).is_empty()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.tag('fireRoll') != 0 && between(owner) > 0` is
    // the pre.
    let list = between(player_id);
    // 规则书[反击]: 「使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子」
    // -- C# `H.AskSeat(i, "另一个我", ..., list)`.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("misaki_card_title")),
        &Msg::new(key!("misaki_card_ask")),
        &list,
    )?;
    // 规则书[反击]: 「消耗所有火罐」 -- C# `H.SpendFire(i, H.Fire(i), "（美咲）")`.
    let fire = ctx::fire(player_id);
    if fire > 0 {
        ctx::spend_fire(player_id, fire, &Msg::new(key!("misaki_card_pay")))?;
    }
    // 规则书[反击]: 「传送至...玩家所在的格子并触发结算」
    let to = ctx::player_pos(who);
    // C# `m.Cancelled = true` -- the original fire-pot move roll is voided.
    trigger::set_cancelled();
    // C# `H.TeleportMove(m2)` with `TeleportTo = to`, `Resolve` default true,
    // `Main = m.Main` (「视为你的主要移动」). `card_move` runs the teleport.
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    // 规则书[反击]: 「（视为你的主要移动）」 -- C# `m2.Main = m.Main`; `card_move`
    // (`MainMoveAs`) runs the teleport now.
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("misaki_card_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书[反击]: 「随后那格及相邻2格上的所有其他玩家[支付]你500资金」 -- C#
    // `H.PayR(item, i, 500, "（美咲）", i)` for every `H.Others(i)` on tiles
    // `to-2 ..= to+2` of the 60-tile ring.
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    for d in -2..=2 {
        let t = (to + d).rem_euclid(n);
        for p in ctx::players_on(t, player_id) {
            ctx::transfer(p, player_id, 500, &Msg::new(key!("misaki_card_pay")))?;
        }
    }
    Ok(())
}
