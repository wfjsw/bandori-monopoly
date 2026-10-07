//! `Mor:（小白）` -- C# `CardMashiroPay` (MatchHost.cs:4996-5028): rewrite a pay
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（小白）`）:
//! > （小白）：
//! > [反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金
//!
//! into a pure loss and make the payee lose half.
//!
//! Counteraction-only (`Normal => false`).

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MASHIRO_PAY: CardDef = CardDef::new(
    "Mor:（小白）",
    &[On::Counteract(&[ChainKind::Effect], can_counteract, counteract)],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你将要向其他玩家支付时打出此卡」
    // C# `t.Kind == "pay" && t.Pay.from == seat && t.Pay.PayToOther && !t.Pay.cancel
    //   && t.Pay.amount > 0` (`t.Pay.to` on pay triggers is `trigger::target`).
    if trigger::kind() != ChainKind::Effect || trigger::player_id() != player_id {
        return false;
    }
    // 规则书[反击]: 「支付」 -- the `pay` effect entry. The causer's own `abnormal`
    // also rides an `effect` link with `player_id` = the causer and `value` = its
    // `AbKind` (> 0); that is not a payment and must not open this window.
    if !ctx::effect::has(TriggerKind::Pay) {
        return false;
    }
    let to = trigger::target();
    // C# `!t.Pay.cancel` -- a payment an earlier counteraction already reduced to 0
    // reads as `value() == 0`, so the >0 guard covers it.
    to >= 0 && to != player_id && trigger::value() > 0
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    let to = trigger::target();
    // 规则书[反击]: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
    // C# rewrites the pending `PayCtx` (`pay.to = -1; pay.kind = "lose"`) and then
    // `H.LoseR(to, amount / 2, ..., must: true)`. Redirecting to the bank is what
    // turns the transfer into a pure loss; the forced half-loss is a separate
    // card-driven pay, which is itself a [反击] point.
    trigger::set_pay_target(-1);
    ctx::log(
        player_id,
        &Msg::new(key!("mashiro_pay_rewritten"))
            .player_id("who", player_id)
            .player_id("to", to)
            .n("money", amount as i64)
            .n("half", (amount / 2) as i64),
    );
    if to >= 0 {
        ctx::pay(
            to,
            amount / 2,
            &Msg::new(key!("mashiro_pay_half"))
                .player_id("who", to)
                .n("n", (amount / 2) as i64),
        )?;
    }
    Ok(())
}
