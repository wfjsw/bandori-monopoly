//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（立希）即便比不上...`）:
//! > （立希）即便比不上...：
//! >   [反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。
//!
//! until the face is new this turn.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, ChainKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TAKI_EVEN_IF: CardDef = CardDef::new(
    "CRYCHIC:（立希）即便比不上...",
    &[On::Counteract(
        &[ChainKind::DiceRollResult],
        // 规则书[反击]: 「[反击] 进入移动阶段后，触发结算前可打出」 -- the counter
        // rides the turn's main move roll.
        // TODO(规则书)（进入移动阶段后，触发结算前可打出）: 事件引起的强制移动
        //   (`plan.forced`) 不是主要移动 -- `move.is_main_move` 把「进入移动阶段后」读成
        //   主要移动，事件「移动1d20」的出目不能重骰。
        "actor == owner && move.is_main_move && move.roll != null",
        None,
        counteract).counter_reason(key!("taki_even_if_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"即便比不上..."#)
        .raw_name(r#"（立希）即便比不上..."#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。"#))
        // sheet cell M16
        .fields(&[("group", r#"CRYCHIC"#), ("owner", r#"椎名立希（CRYCHIC）"#)]));

/// 规则书[反击]: 「与本回合内你骰出过的所有骰点都不同」 -- the per-roller cast
/// history minus the current face itself. The history already holds the face
/// (cast time), and the body replays over the same history after each prompt,
/// so comparing against the raw list would always match.
fn others(player_id: i32, x: i32) -> alloc::vec::Vec<i32> {
    let mut seen: alloc::vec::Vec<i32> = ctx::rolls_by_player(player_id)
        .into_iter()
        .map(|r| r.abs())
        .collect();
    let x = x.abs();
    if let Some(pos) = seen.iter().rposition(|&v| v == x) {
        seen.remove(pos);
    }
    seen
}

/// Runaway guard -- 「你可重骰至…为止」 states no cap, and a constant dice table
/// never reaches the exit. `latch:`-prefixed so the count crosses the ask
/// prompt's pause (the body re-runs and cannot carry a local), keyed to the roll
/// transaction so the next activation on another throw starts over.
const GUARD_TX: &str = "latch:taki_guard_tx";
const GUARD: &str = "latch:taki_guard";

fn counteract(player_id: i32) -> card_sdk::Asked {
    let Some(_before) = trigger::move_roll() else {
        return Ok(());
    };
    // 规则书[反击]: 「进行一次重骰」 -- a recast on the move's transaction, so the
    // new face inherits the shared producer (and a forced extreme).
    let id = trigger::roll_id();
    let x = ctx::roll_recast(id, roll_source::CARD)?;
    ctx::log(
        player_id,
        &Msg::new(key!("taki_reroll"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    reroll_to_new(player_id, id, x)
}

/// 规则书[反击]: 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止」
/// -- keep offering the reroll while the face repeats one rolled this turn.
/// Tail-recursive: every pass restarts the body, so the face rides the call.
fn reroll_to_new(player_id: i32, id: i32, x: i32) -> card_sdk::Asked {
    if ctx::slot(player_id, GUARD_TX) != id {
        ctx::set_slot(player_id, GUARD_TX, id);
        ctx::set_slot(player_id, GUARD, 0);
    }
    let guard = ctx::slot(player_id, GUARD);
    // TODO(规则书)（你可重骰至…为止）: 无次数上限 -- 100 次只是骰表恒定时
    //   防死循环的护栏。
    if guard >= 100 || !others(player_id, x).contains(&x.abs()) {
        // 规则书[反击]: 「进行一次重骰」-- the move uses the new face.
        trigger::set_move_roll(x);
        return Ok(());
    }
    let again = ctx::ask_yes(
        player_id,
        &Msg::new(key!("taki_again_title")),
        &Msg::new(key!("taki_again_text")).i("n", x as i64),
    )?;
    if !again {
        trigger::set_move_roll(x);
        return Ok(());
    }
    ctx::set_slot(player_id, GUARD, guard + 1);
    let y = ctx::roll_recast(id, roll_source::CARD)?;
    ctx::log(
        player_id,
        &Msg::new(key!("taki_reroll"))
            .player_id("who", player_id)
            .i("n", y as i64),
    );
    reroll_to_new(player_id, id, y)
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
