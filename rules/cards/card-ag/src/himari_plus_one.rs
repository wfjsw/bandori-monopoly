//! `AG:（绯玛丽）如果并非没问题` -- C# `CardHimariPlusOne` (MatchHost.cs:1792-1816).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（绯玛丽）如果并非没问题`）:
//! > （绯玛丽）如果并非没问题：【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1
//!
//! Reaction: when one of your own rolls is under 6, it goes up by 1.

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const HIMARI_PLUS_ONE: CardDef = CardDef::new("AG:（绯玛丽）如果并非没问题", &[
    On::React(&[ChainKind::Roll, ChainKind::MoveRoll], can_react, react),
]);

/// 规则书: 「【反击】当你的一次掷骰小于6时，你可以打出此卡」 -- reaction-only.
fn can_react(player_id: i32) -> bool {
    // 规则书: 「【反击】当你的一次掷骰」 -- a roll window, not a hand play.
    if !matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll) {
        return false;
    }
    // 规则书: 「你的一次掷骰」 -- only the roller's own roll.
    if trigger::player_id() != player_id {
        return false;
    }
    // 规则书: 「小于6时」
    matches!(trigger::move_roll(), Some(r) if r < 6)
}

fn react(player_id: i32) {
    // 规则书: 「使结果+1」
    let Some(before) = trigger::move_roll() else { return };
    trigger::set_move_roll(before + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("himari_plus_one"))
            .player_id("who", player_id)
            .card("card", "AG:（绯玛丽）如果并非没问题")
            .i("total", (before + 1) as i64),
    );
}