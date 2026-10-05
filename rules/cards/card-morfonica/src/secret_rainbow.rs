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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const SECRET_RAINBOW: CardDef = CardDef {
    id: "Mor:秘密与青春的虹彩",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]（1）: 「当你向学妹或同级生支付时」
    // 规则书[反击]（2）: 「当学姐或同级生向你支付的时候」
    // C# `t.Kind == "pay" && t.Pay.PayToOther && !t.Pay.cancel && t.Pay.amount > 0`,
    // then `t.Pay.from == seat` + `GradeOf(t.Pay.to) <= GradeOf(seat)` or
    // `t.Pay.to == seat` + `GradeOf(t.Pay.from) >= GradeOf(seat)`.
    if trigger::kind() != TriggerKind::Pay || trigger::value() <= 0 {
        return false;
    }
    let from = trigger::seat();
    let to = trigger::target();
    if to < 0 || from == to {
        return false;
    }
    // TODO(ABI): `t.Pay.cancel` is not on the trigger payload, and there is no
    //   grade query (`H.GradeOf`, `MatchHost.cs:18282`) so the 「学妹或同级生」 /
    //   「学姐或同级生」 gates cannot be checked -- the window opens on every
    //   seat-to-seat pay that involves `seat` (over-permissive).
    from == seat || to == seat
}

fn react(seat: i32) {
    let amount = trigger::value() as i64;
    let from = trigger::seat();
    // 规则书[反击]（1）: 「此次支付金额减半」 -- C# `pay.amount = CeilTo(amount / 2.0, 10)`.
    // 规则书[反击]（2）: 「使此次支付资金变成1.5倍」 -- C# `pay.amount = CeilTo(amount * 1.5, 10)`.
    // TODO(ABI): the pending `PayCtx.amount` cannot be written, so neither the
    //   halving nor the ×1.5 takes effect (C# `CardSecretRainbow.React`). `CeilTo`
    //   rounds up to a multiple of 10.
    if from == seat {
        let half = (amount + 1) / 2;
        let half = ((half + 9) / 10) * 10;
        ctx::log(seat, &Msg::new(key!("secret_rainbow_half")).n("money", amount).n("n", half));
    } else {
        let boosted = (amount * 3 + 1) / 2;
        let boosted = ((boosted + 9) / 10) * 10;
        ctx::log(seat, &Msg::new(key!("secret_rainbow_boost")).n("money", amount).n("n", boosted));
    }
}