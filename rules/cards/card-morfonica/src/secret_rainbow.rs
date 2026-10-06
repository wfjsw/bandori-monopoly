//! `Mor:秘密与青春的虹彩` -- C# `CardSecretRainbow` (MatchHost.cs:5178-5220): halve a
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:秘密与青春的虹彩`）:
//! > 秘密与青春的虹彩：[反击]
//! > （1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。
//! > （2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。
//!
//! payment to a junior peer, or boost a senior peer's payment to you.
//!
//! Reaction-only (`Normal => false`). The C# grades are
//! `MatchHost.cs:17294` (`Grades[character]`, bigger = older).

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const SECRET_RAINBOW: CardDef = CardDef::new("Mor:秘密与青春的虹彩", &[
    On::CounterAct(&[ChainKind::Effect], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]（1）: 「当你向学妹或同级生支付时」
    // 规则书[反击]（2）: 「当学姐或同级生向你支付的时候」
    // C# `t.Kind == "pay" && t.Pay.PayToOther && !t.Pay.cancel && t.Pay.amount > 0`,
    // then `t.Pay.from == seat` + `GradeOf(t.Pay.to) <= GradeOf(player_id)` or
    // `t.Pay.to == seat` + `GradeOf(t.Pay.from) >= GradeOf(player_id)`.
    if trigger::kind() != ChainKind::Effect || trigger::value() <= 0 {
        return false;
    }
    let from = trigger::player_id();
    let to = trigger::target();
    if to < 0 || from == to {
        return false;
    }
    // C# `!t.Pay.cancel` -- a payment an earlier reaction already reduced to 0
    // reads as `value() == 0`, so the >0 guard above covers it.
    // TODO(规则书)[judgement](ABI): 「学妹或同级生」/「学姐或同级生」 needs a grade query
    //   the clause under-specifies -- see the note above it
    //   (`H.GradeOf`, `MatchHost.cs:18282`), so the window opens on every
    //   player-to-player pay that involves `player_id` (over-permissive).
    from == player_id || to == player_id
}

fn react(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value() as i64;
    let from = trigger::player_id();
    // 规则书[反击]（1）: 「此次支付金额减半」 -- C# `pay.amount = CeilTo(amount / 2.0, 10)`.
    // 规则书[反击]（2）: 「使此次支付资金变成1.5倍」 -- C# `pay.amount = CeilTo(amount * 1.5, 10)`.
    // `CeilTo` rounds up to a multiple of 10.
    if from == player_id {
        let half = (amount + 1) / 2;
        let half = ((half + 9) / 10) * 10;
        trigger::set_pay_amount(half as i32);
        ctx::log(player_id, &Msg::new(key!("secret_rainbow_half")).n("money", amount).n("n", half));
    } else {
        let boosted = (amount * 3 + 1) / 2;
        let boosted = ((boosted + 9) / 10) * 10;
        trigger::set_pay_amount(boosted as i32);
        ctx::log(player_id, &Msg::new(key!("secret_rainbow_boost")).n("money", amount).n("n", boosted));
    }
    Ok(())
}