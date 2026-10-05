//! `AG:（绯玛丽）如果并非没问题` -- C# `CardHimariPlusOne` (MatchHost.cs:1792-1816).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（绯玛丽）如果并非没问题`）:
//! > （绯玛丽）如果并非没问题：【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1
//!
//! Reaction: when one of your own rolls is under 6, it goes up by 1.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const HIMARI_PLUS_ONE: CardDef = CardDef {
    id: "AG:（绯玛丽）如果并非没问题",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书: 「【反击】当你的一次掷骰小于6时，你可以打出此卡」 -- reaction-only.
fn can_react(seat: i32) -> bool {
    // 规则书: 「【反击】当你的一次掷骰」 -- a roll window, not a hand play.
    if !matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll) {
        return false;
    }
    // 规则书: 「你的一次掷骰」 -- only the roller's own roll.
    if trigger::seat() != seat {
        return false;
    }
    // 规则书: 「小于6时」
    matches!(trigger::move_roll(), Some(r) if r < 6)
}

fn react(seat: i32) {
    // 规则书: 「使结果+1」
    let Some(before) = trigger::move_roll() else { return };
    trigger::set_move_roll(before + 1);
    ctx::log(
        seat,
        &Msg::new(key!("himari_plus_one"))
            .seat("who", seat)
            .card("card", "AG:（绯玛丽）如果并非没问题")
            .i("total", (before + 1) as i64),
    );
}