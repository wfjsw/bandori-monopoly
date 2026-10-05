//! `MyGO:轮符雨` -- C# `CardRinneRain` (MatchHost.cs:6348-6363): gain one
//! [停留] layer and book an extra [触发结算] at this turn's end.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:轮符雨`）:
//! > 轮符雨：
//! >  使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const RINNE_RAIN: CardDef = CardDef {
    id: "MyGO:轮符雨",
    play: Some(rinne_rain),
    can_react: None,
    react: None,
    why_not: None,
};

fn rinne_rain(seat: i32) {
    // 规则书: 「使自己获得一层[停留]」 -- C# `H.GiveStay(i, 1, i, CardName)`.
    ctx::give_stay(seat, 1);
    // 规则书: 「并在回合结束时额外进行一次[触发结算]」 -- C# `H._turnCtx.SettleAtEnd++`.
    ctx::inc_slot(seat, "settleAtEnd", 1);
    ctx::log(seat, &Msg::new(key!("rinne_rain_log")));
    // TODO(规则书): 「并在回合结束时额外进行一次[触发结算]」 -- the engine must honour
    // the `settleAtEnd` counter and run one extra [触发结算] on the seat's tile at
    // the end of this turn (C# `H._turnCtx.SettleAtEnd`); nothing reads it yet.
}