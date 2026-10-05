//! `CRYCHIC:主唱太拼命了` -- C# `CardVocalTooHard` (MatchHost.cs:3120-3145): [反击]
//! that cancels one 5,000+ payment to another player.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:主唱太拼命了`）:
//! > 主唱太拼命了：
//! > [反击] 一次性向其他玩家支付5000以上资金时，免除此次支付。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const VOCAL_TOO_HARD: CardDef = CardDef {
    id: "CRYCHIC:主唱太拼命了",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「一次性向其他玩家支付5000以上资金时」 -- C#
    // `t.Kind == "pay" && t.Pay.from == seat && t.Pay.PayToOther && t.Pay.amount >= 5000`.
    if trigger::kind() != TriggerKind::Pay || trigger::seat() != seat {
        return false;
    }
    // 规则书[反击]: 「向其他玩家支付」 -- C# `t.Pay.PayToOther` (`Pay.from >= 0
    // && Pay.to >= 0`); on pay triggers `t.Pay.to` is `trigger::target()`.
    if trigger::target() < 0 {
        return false;
    }
    // 规则书[反击]: 「5000以上」
    trigger::value() >= 5000
    // C# `!t.Pay.cancel` -- the trigger payload carries no `cancel` flag yet, so
    // an already-cancelled payment still looks reactable (TODO(ABI)).
}

fn react(seat: i32) {
    // 规则书[反击]: 「免除此次支付」
    ctx::log(seat, &Msg::new(key!("vocal_too_hard_note")).seat("who", seat));
    // TODO(ABI): 「免除此次支付」 -- needs a pay-cancel op (C#
    // `c.Trigger.Pay.cancel = true` / `PayCtx.cancel`, which makes `H.Money`
    // skip the transfer). `ctx::pay` / `ctx::transfer` cannot un-pay a payment
    // the host is in the middle of raising.
}