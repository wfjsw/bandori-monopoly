//! `MyGO:轮符雨` -- C# `CardRinneRain` (MatchHost.cs:6348-6363): gain one
//! [停留] layer and book an extra [触发结算] at this turn's end.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:轮符雨`）:
//! > 轮符雨：
//! >  使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const RINNE_RAIN: CardDef = CardDef::new("MyGO:轮符雨", &[
    On::Play(rinne_rain),
]);

fn rinne_rain(player_id: i32) {
    // 规则书: 「使自己获得一层[停留]」 -- C# `H.GiveStay(i, 1, i, CardName)`.
    ctx::give_stay(player_id, 1);
    // 规则书: 「并在回合结束时额外进行一次[触发结算]」 -- C# `H._turnCtx.SettleAtEnd++`.
    ctx::inc_slot(player_id, "settleAtEnd", 1);
    ctx::log(player_id, &Msg::new(key!("rinne_rain_log")));
    // TODO(规则书): 「并在回合结束时额外进行一次[触发结算]」 -- the engine must honour
    // the `settleAtEnd` counter and run one extra [触发结算] on the player's tile at
    // the end of this turn (C# `H._turnCtx.SettleAtEnd`); nothing reads it yet.
}