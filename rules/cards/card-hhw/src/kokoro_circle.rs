//! `HHW:（kkr）前往笑容集结的地方！` -- C# `CardKokoroCircle` (MatchHost.cs:3900-3964).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（kkr）前往笑容集结的地方！`）:
//! > （kkr）前往笑容集结的地方！：
//! >  支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const KOKORO_CIRCLE: CardDef = CardDef {
    id: "HHW:（kkr）前往笑容集结的地方！",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardKokoroCircle.WhyNot`: refuses without 10,000 money, or when a
    // `CardKokoroCircle` is already placed on CiRCLE.
    if ctx::money(seat) < 10000 {
        return Some(Msg::new(key!("x_no_money_10000")));
    }
    // TODO(规则书): the C# also refuses when any `CardKokoroCircle` is already
    // placed (`H._placed.Any(p => p is CardKokoroCircle)`); placed-card lookup
    // by type is still missing, so the gate covers only the money check.
    None
}

fn play(seat: i32) {
    // 规则书: 「支付10000资金（视为买地花费）」 -- C# `PayCtx { amount = 10000, kind = "buy" }`.
    let paid = ctx::pay(seat, 10000, &Msg::new(key!("kokoro_circle_why")));
    if paid < 10000 {
        // TODO(规则书): the C# sets `c.Effective = false` when the payment does not
        // go through (the play is wasted, `H.ToDiscard(..., wasted: true)`); the ABI
        // has no set_effective hook.
        return;
    }
    // 规则书: 「并将此卡置于CiRCLE上」 -- C# `H.PlaceFromPlay(c, i, 0)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "HHW:（kkr）前往笑容集结的地方！", &Msg::new(key!("kokoro_circle_note")));
    ctx::log(seat, &Msg::new(key!("kokoro_circle_placed")).seat("who", seat));
    // TODO(规则书): 「并将此卡置于CiRCLE上」 -- the placement is bound to tile #0
    // (CiRCLE) rather than the seat's field; needs field-card tile placement
    // (`H.PlaceFromPlay(c, owner, tile)`).
    // TODO(规则书): 「若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款」
    // -- needs the Fx.SettleAfter hook (C# `CardKokoroCircle.SettleAfter`) so a
    // settle on that tile pays the owner 6,000 (`ctx::transfer`).
}