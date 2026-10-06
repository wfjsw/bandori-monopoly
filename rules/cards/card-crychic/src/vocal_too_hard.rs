//! `CRYCHIC:主唱太拼命了` -- C# `CardVocalTooHard` (MatchHost.cs:3120-3145): [反击]
//! that cancels one 5,000+ payment to another player.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:主唱太拼命了`）:
//! > 主唱太拼命了：
//! > [反击] 一次性向其他玩家支付5000以上资金时，免除此次支付。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const VOCAL_TOO_HARD: CardDef = CardDef::new("CRYCHIC:主唱太拼命了", &[
    On::CounterAct(&[ChainKind::Effect], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「一次性向其他玩家支付5000以上资金时」 -- C#
    // `t.Kind == "pay" && t.Pay.from == seat && t.Pay.PayToOther && t.Pay.amount >= 5000`.
    if trigger::kind() != ChainKind::Effect || trigger::player_id() != player_id {
        return false;
    }
    // 规则书[反击]: 「向其他玩家支付」 -- C# `t.Pay.PayToOther` (`Pay.from >= 0
    // && Pay.to >= 0`); on pay triggers `t.Pay.to` is `trigger::target()`.
    if trigger::target() < 0 {
        return false;
    }
    // 规则书[反击]: 「5000以上」
    trigger::value() >= 5000
    // C# `!t.Pay.cancel` -- a payment an earlier reaction already reduced to 0
    // reads as `value() == 0`, so the ≥5000 guard covers it.
}

fn react(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「免除此次支付」
    trigger::set_pay_amount(0);
    ctx::log(player_id, &Msg::new(key!("vocal_too_hard_note")).player_id("who", player_id));
    Ok(())
}