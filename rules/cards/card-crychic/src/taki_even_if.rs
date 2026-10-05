//! `CRYCHIC:（立希）即便比不上...` -- C# `CardTakiEvenIf`: reroll the move dice
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（立希）即便比不上...`）:
//! > （立希）即便比不上...：
//! >   [反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。
//!
//! until the face is new this turn.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const TAKI_EVEN_IF: CardDef = CardDef {
    id: "CRYCHIC:（立希）即便比不上...",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「进入移动阶段后，触发结算前可打出」
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::seat() == seat
        && trigger::move_roll().is_some()
}

fn react(seat: i32) {
    let Some(before) = trigger::move_roll() else { return };
    // 规则书[反击]: 「进行一次重骰」
    // TODO: C# rerolls with H.DoMoveRoll (honours the move's dice plan / bonuses).
    let mut seen = vec![before.abs()];
    // TODO(规则书)[反击]: 「与本回合内你骰出过的所有骰点都不同」-- the C# seeds
    //   `seen` from H._turnCtx.Rolls (every die this turn). The ABI has no
    //   turn-roll history; only this card's own rerolls are compared.
    let mut x = ctx::roll(seat, 1, 20);
    ctx::log(seat, &Msg::new(key!("taki_reroll")).seat("who", seat).i("n", x as i64));
    for _ in 0..10 {
        // 规则书[反击]: 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止」
        if !seen.contains(&x.abs()) {
            break;
        }
        let again = ctx::ask_yes(
            seat,
            &Msg::new(key!("taki_again_title")),
            &Msg::new(key!("taki_again_text")).i("n", x as i64),
        );
        if !again {
            break;
        }
        seen.push(x.abs());
        x = ctx::roll(seat, 1, 20);
        ctx::log(seat, &Msg::new(key!("taki_reroll")).seat("who", seat).i("n", x as i64));
    }
    trigger::set_move_roll(x); // 规则书[反击]: 「进行一次重骰」-- the move uses the new face
}
