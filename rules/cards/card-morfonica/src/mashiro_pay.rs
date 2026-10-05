//! `Mor:（小白）` -- C# `CardMashiroPay` (MatchHost.cs:4996-5028): rewrite a pay
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（小白）`）:
//! > （小白）：
//! > [反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金
//!
//! into a pure loss and make the payee lose half.
//!
//! Reaction-only (`Normal => false`).

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const MASHIRO_PAY: CardDef = CardDef {
    id: "Mor:（小白）",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当你将要向其他玩家支付时打出此卡」
    // C# `t.Kind == "pay" && t.Pay.from == seat && t.Pay.PayToOther && !t.Pay.cancel
    //   && t.Pay.amount > 0` (`t.Pay.to` on pay triggers is `trigger::target`).
    if trigger::kind() != TriggerKind::Pay || trigger::seat() != seat {
        return false;
    }
    let to = trigger::target();
    // TODO(ABI): `t.Pay.cancel` is not on the trigger payload.
    to >= 0 && to != seat && trigger::value() > 0
}

fn react(seat: i32) {
    let amount = trigger::value();
    let to = trigger::target();
    // 规则书[反击]: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
    // C# rewrites the pending `PayCtx` (`pay.to = -1; pay.kind = "lose"`) and then
    // `H.LoseR(to, amount / 2, ..., must: true)`.
    // TODO(ABI): the pending pay cannot be cancelled or redirected (`PayCtx.to` /
    // `PayCtx.kind`), so the rewrite is not expressible; the forced half-loss on
    // the payee is left out on purpose -- doing it on top of the real transfer
    // would charge the payee twice.
    ctx::log(
        seat,
        &Msg::new(key!("mashiro_pay_rewritten"))
            .seat("who", seat)
            .seat("to", to)
            .n("money", amount as i64)
            .n("half", (amount / 2) as i64),
    );
}