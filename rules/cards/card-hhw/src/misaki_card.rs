//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（美咲）`）:
//! > （美咲）[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。
//!
//! move roll, teleport to a rival standing between start and end, then collect 500.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, MoveKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const MISAKI_CARD: CardDef = CardDef::new(
    "HHW:（美咲）",
    &[On::Counteract(
        &[ChainKind::DiceRollResult],
        // 规则书[反击]: 「触发结算前可打出此卡」 -- the window is the
        // `diceRollResult` timing, after the roll and before the settle.
        // 「使用火罐进行移动掷骰后」 -- the [火罐] roll is meant to be card-owned
        // state: the fire reroll is meant to tag it (`plan::set_tag("fireRoll", 1)`)
        // and readers ask `move.tag`.
        // 规则书: 「位于你的移动起点与预定移动终点之间」 -- `between(owner) > 0`
        // (there must be a rival to teleport to).
        // TODO(规则书)（使用火罐进行移动掷骰后）: the book conditions on the fire-pot
        //   roll -- no shipped producer tags the move `fireRoll` (only the
        //   `TEST:fireRoll` fixture does) and `roll_source` never reaches this
        //   trigger, so this condition reads 0 in every real game.
        "actor == owner && move.tag('fireRoll') != 0 && between(owner) > 0",
        None,
        counteract).counter_reason(key!("misaki_card_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（美咲）"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。"#))
        // sheet cell F15
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"奥泽美咲"#)]));

fn between(player_id: i32) -> Vec<i32> {
    let Some(roll) = trigger::move_roll() else {
        return Vec::new();
    };
    let roll = roll.abs();
    // TODO(ABI)（位于你的移动起点与预定移动终点之间）: the span origin is the
    //   current tile, but a `plan::set_start` override (your_light / detour / …)
    //   only lands when the walk starts, after this window, and the trigger
    //   carries no start -- candidates then wrap around the wrong origin.
    let start = ctx::player_pos(player_id);
    let n = ctx::tile_count();
    let backward = trigger::move_dir() < 0;
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| {
            let fwd = ctx::tile_forward(start, ctx::player_pos(p));
            let d = if backward { (n - fwd) % n } else { fwd };
            // TODO(规则书)（预定移动终点）: 「之间」 may exclude the planned end --
            //   we include `d == roll` (the start, d == 0, is excluded), so the
            //   two ends are treated asymmetrically.
            (1..=roll).contains(&d)
        })
        .collect()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.tag('fireRoll') != 0 && between(owner) > 0` is
    // the condition.
    let list = between(player_id);
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("misaki_card_title")),
        &Msg::new(key!("misaki_card_ask")),
        &list,
    )?;
    let fire = bandori_std::fire::total(player_id);
    if fire > 0 {
        bandori_std::fire::spend(player_id, fire, &Msg::new(key!("misaki_card_pay")))?;
    }
    // 规则书[反击]: 「传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算」
    let to = ctx::player_pos(who);
    // TODO(规则书)（并触发结算（视为你的主要移动） ）: the book makes the teleport the move -- the
    // rolled walk is voided, the piece goes to the chosen tile and settles there; we
    // neither cancel the walk nor teleport (below).
    trigger::set_cancelled();
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_settles(true);
    // TODO(规则书)（视为你的主要移动）: `run_planned_move` runs as a *main* move while the turn's main
    // move already latched `main_moved` before this mid-roll window, so the engine
    // refuses it and drops `teleport_to`: the teleport never happens.
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("misaki_card_moved"))
            .player_id("who", player_id)
            // The refused `run_planned_move` can leave the piece elsewhere: log where it
            // actually is, not the chosen destination.
            .tile("tile", ctx::player_pos(player_id)),
    );
    if ctx::player_out(player_id) {
        return Ok(());
    }
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // TODO(规则书)（那格及相邻2格）: the book can be read as the two tiles adjacent
    //   to 那格 (3 tiles) -- we charge ±2 (5 tiles). OPEN-HHW-NEIGHBOUR.
    for d in -2..=2 {
        let t = (to + d).rem_euclid(n);
        for p in ctx::players_on(t, player_id) {
            ctx::transfer(p, player_id, 500, &Msg::new(key!("misaki_card_pay")))?;
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
