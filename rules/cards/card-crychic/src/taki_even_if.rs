//! `CRYCHIC:（立希）即便比不上...` -- C# `CardTakiEvenIf`: reroll the move dice
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（立希）即便比不上...`）:
//! > （立希）即便比不上...：
//! >   [反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。
//!
//! until the face is new this turn.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const TAKI_EVEN_IF: CardDef = CardDef::new(
    "CRYCHIC:（立希）即便比不上...",
    &[On::Counteract(&[ChainKind::MoveRoll], can_counteract, counteract)],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「进入移动阶段后，触发结算前可打出」
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::player_id() == player_id
        && trigger::move_roll().is_some()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let Some(before) = trigger::move_roll() else {
        return Ok(());
    };
    // 规则书[反击]: 「进行一次重骰」 -- `H.DoMoveRoll`, which sums the move's
    // whole dice table.
    // 规则书[反击]: 「与本回合内你骰出过的所有骰点都不同」 -- `H._turnCtx.Rolls`,
    // every face rolled this turn (not just this card's own rerolls).
    let mut seen: alloc::vec::Vec<i32> = ctx::turn_rolls().into_iter().map(|r| r.abs()).collect();
    seen.push(before.abs());
    let mut x = ctx::do_move_roll(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("taki_reroll"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    for _ in 0..10 {
        // 规则书[反击]: 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止」
        if !seen.contains(&x.abs()) {
            break;
        }
        let again = ctx::ask_yes(
            player_id,
            &Msg::new(key!("taki_again_title")),
            &Msg::new(key!("taki_again_text")).i("n", x as i64),
        )?;
        if !again {
            break;
        }
        seen.push(x.abs());
        x = ctx::roll(player_id, 1, 20);
        ctx::log(
            player_id,
            &Msg::new(key!("taki_reroll"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
    }
    trigger::set_move_roll(x); // 规则书[反击]: 「进行一次重骰」-- the move uses the new face
    Ok(())
}
