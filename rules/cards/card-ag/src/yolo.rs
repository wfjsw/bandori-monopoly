//! `AG:Y.O.L.O` -- C# `CardYolo` (MatchHost.cs:956-987).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:Y.O.L.O`）:
//! > Y.O.L.O： 
//! >  掷骰结算前打出此卡，使结果增加1d4结果的数字
//!
//! Reaction: after a (move) roll, add 1d4 to the result.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const YOLO: CardDef = CardDef {
    id: "AG:Y.O.L.O",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(_seat: i32) -> bool {
    matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll) && trigger::move_roll().is_some()
}

fn react(seat: i32) {
    // TODO: C# uses H.CardRoll, which honours PlayCtx.Extreme (forced max/min dice).
    let Some(before) = trigger::move_roll() else { return };
    let n = ctx::roll(seat, 1, 4);
    trigger::set_move_roll(before + n);
    ctx::log(seat, &Msg::new(key!("yolo_boost")).seat("who", trigger::seat()).i("n", n as i64).i("total", (before + n) as i64));
}
