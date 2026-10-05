//! `AG:Y.O.L.O` -- C# `CardYolo` (MatchHost.cs:956-987).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:Y.O.L.O`）:
//! > Y.O.L.O： 
//! >  掷骰结算前打出此卡，使结果增加1d4结果的数字
//!
//! Reaction: after a (move) roll, add 1d4 to the result.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const YOLO: CardDef = CardDef::new("AG:Y.O.L.O", &[
    On::React(&[TriggerKind::Roll, TriggerKind::MoveRoll], can_react, react),
]);

fn can_react(_player: i32) -> bool {
    matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll) && trigger::move_roll().is_some()
}

fn react(player_id: i32) {
    // TODO: C# uses H.CardRoll, which honours PlayCtx.Extreme (forced max/min dice).
    let Some(before) = trigger::move_roll() else { return };
    let n = ctx::roll(player_id, 1, 4);
    trigger::set_move_roll(before + n);
    ctx::log(player_id, &Msg::new(key!("yolo_boost")).player_id("who", trigger::player_id()).i("n", n as i64).i("total", (before + n) as i64));
}
