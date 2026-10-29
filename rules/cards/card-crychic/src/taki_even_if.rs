//! `CRYCHIC:（立希）即便比不上...` -- C# `CardTakiEvenIf`: reroll the move dice
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（立希）即便比不上...`）:
//! > （立希）即便比不上...：
//! >   [反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。
//!
//! until the face is new this turn.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TAKI_EVEN_IF: CardDef = CardDef::new(
    "CRYCHIC:（立希）即便比不上...",
    &[On::Counteract(
        &[ChainKind::DiceRollResult],
        // 规则书[反击]: 「[反击] 进入移动阶段后，触发结算前可打出」 -- the counter
        // rides the turn's main move roll.
        // TODO(规则书)（进入移动阶段后，触发结算前可打出）: 事件引起的强制移动
        //   (`plan.forced`) 不是主要移动 -- `move.main` 把「进入移动阶段后」读成
        //   主要移动，事件「移动1d20」的出目不能重骰。
        "actor == owner && move.main && move.roll != null",
        None,
        counteract).counter_reason(key!("taki_even_if_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"即便比不上..."#)
        .raw_name(r#"（立希）即便比不上..."#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。"#))
        // sheet cell M16
        .fields(&[("group", r#"CRYCHIC"#), ("owner", r#"椎名立希（CRYCHIC）"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    let Some(before) = trigger::move_roll() else {
        return Ok(());
    };
    // 规则书[反击]: 「进行一次重骰」 -- `H.DoMoveRoll`, which sums the move's
    // whole dice table.
    // 规则书[反击]: 「与本回合内你骰出过的所有骰点都不同」 -- `H._turnCtx.Rolls`:
    // the turn's recorded roll totals (one summed total per roll, every seat's
    // effect rolls included; the move's own face is pushed below by hand).
    // TODO(规则书)（本回合内你骰出过的所有骰点）: 只该是「你」骰出的骰点 --
    //   `turn_rolls` 是本回合所有玩家的效果骰，别人骰出的点也会挡你的重骰。
    let mut seen: alloc::vec::Vec<i32> = ctx::turn_rolls().into_iter().map(|r| r.abs()).collect();
    seen.push(before.abs());
    let mut x = ctx::do_move_roll(player_id)?;
    ctx::log(
        player_id,
        &Msg::new(key!("taki_reroll"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    // 规则书[反击]: 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止」
    // -- keep offering the reroll while the face repeats one rolled this turn.
    let mut guard = 0;
    while seen.contains(&x.abs()) {
        let again = ctx::ask_yes(
            player_id,
            &Msg::new(key!("taki_again_title")),
            &Msg::new(key!("taki_again_text")).i("n", x as i64),
        )?;
        if !again {
            break;
        }
        seen.push(x.abs());
        // Same shape as the first reroll: the move plan's whole dice table.
        x = ctx::do_move_roll(player_id)?;
        ctx::log(
            player_id,
            &Msg::new(key!("taki_reroll"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
        guard += 1;
        // TODO(规则书)（你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止）: 无次数
        //   上限 -- 100 次只是骰表恒定时防死循环的护栏。
        if guard >= 100 {
            break;
        }
    }
    trigger::set_move_roll(x); // 规则书[反击]: 「进行一次重骰」-- the move uses the new face
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
