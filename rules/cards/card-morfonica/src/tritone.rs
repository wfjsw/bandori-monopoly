//! `Mor:迷茫之蝶们的三全音` -- C# `CardTritone` (MatchHost.cs:4524-4583): loan the
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:迷茫之蝶们的三全音`）:
//! > 迷茫之蝶们的三全音：
//! > [反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金
//!
//! pay amount back in three turns (crystal decay).
//!
//! Reaction-only (`Normal => false`): the seat is about to lose or pay money.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const TRITONE: CardDef = CardDef {
    id: "Mor:迷茫之蝶们的三全音",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「任意时刻当你将要失去或支付资金时打出此卡」
    // C# `t.Kind == "pay" && t.Pay.from == seat && t.Pay.amount > 0 && !t.Pay.cancel`.
    if trigger::kind() != TriggerKind::Pay || trigger::seat() != seat {
        return false;
    }
    // TODO(ABI): `t.Pay.cancel` (the pay is already being cancelled) is not on the
    // trigger payload; a cancelled pay can still open this window.
    trigger::value() > 0
}

fn react(seat: i32) {
    let amount = trigger::value();
    // 规则书[反击]: 「立刻获得此次失去的资金金额」
    // C# `H.Money(new PayCtx { to = seat, amount, kind = "gain", fixedAmount = true })`.
    // TODO(ABI): `fixedAmount` (skills / crits may not modify this gain) is not
    // expressible; the gain goes through the normal `H.GainR` path.
    ctx::gain(seat, amount, &Msg::new(key!("tritone_why")).n("money", amount as i64));
    // 规则书[反击]: 「此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mor:迷茫之蝶们的三全音", &Msg::new(key!("tritone_note")).n("money", amount as i64));
    ctx::log(seat, &Msg::new(key!("tritone_placed")).seat("who", seat).n("money", amount as i64));
    // TODO(规则书)[反击]: 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡
    // 获得的资金」 -- needs the field-card crystal counter (`H.PlaceFromPlay(..., 3)`,
    // `H.AddCrystals`) and the Fx.TurnEnd decay (C# `DecayCard.TurnEnd` -> `Empty` ->
    // `H.Charge(Seat, -1, Owed, "lose", ...)`) plus this card's `Mem["owed"]`.
}