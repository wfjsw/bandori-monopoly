//! `AG:朝同一片天空迈进` -- C# `CardSameSky` (MatchHost.cs:1547-1582):
//! [反击] auto-plays when drawn: money by hand size, else draw.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:朝同一片天空迈进`）:
//! > 朝同一片天空迈进：
//! > [反击]抽出此卡时立刻打出，如果你手牌数大于等于3，获得手牌数*600的资金，如果你的手牌数小于3，抽一张卡（开局时抽到此卡洗回）
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const SAME_SKY: CardDef = CardDef::new("AG:朝同一片天空迈进", &[
    On::Hook(&[TriggerKind::Drawn], react),
    On::Hook(&[TriggerKind::DeckAtGameStart], return_at_opening),
]);

const ID: &str = "AG:朝同一片天空迈进";

/// 规则书[反击]: 「（开局时抽到此卡洗回）」 -- C# `ReturnAtOpening` /
/// `FixOpeningHand`. The opening deal and mulligan raise no `Drawn` hooks, so
/// the old `step() == 0` branch is unreachable; the brief maps this to
/// `On::Hook(&[TriggerKind::DeckAtGameStart], ...)`, which fires on this card
/// over draw + hand after the mulligan.
fn return_at_opening(player_id: i32) {
    // Pull every copy out of the opening hand and shuffle it back into the deck.
    let mut returned = 0;
    while ctx::take_from_hand(player_id, ID) {
        ctx::add_to_deck(player_id, ID, true);
        returned += 1;
    }
    if returned > 0 {
        ctx::log(player_id, &Msg::new(key!("same_sky_returned")).player_id("who", player_id));
    }
}

/// 规则书[反击]: 「抽出此卡时立刻打出」 -- C# `CardSameSky.Drawn` auto-plays it
/// the moment it is drawn; it never answers the [反击] window.
fn react(player_id: i32) {
    if trigger::kind() != TriggerKind::Drawn || !trigger::card_is(ID) {
        return;
    }
    // C# `Drawn` pulls the card out of hand first (`hand.Remove(Id)`).
    if !ctx::take_from_hand(player_id, ID) {
        return;
    }
    let n = ctx::hand_size(player_id);
    if n >= 3 {
        // 规则书[反击]: 「如果你手牌数大于等于3，获得手牌数*600的资金」
        ctx::gain(player_id, n * 600, &Msg::new(key!("same_sky_why")).i("n", n as i64));
    } else {
        // 规则书[反击]: 「如果你的手牌数小于3，抽一张卡」
        ctx::draw(player_id, 1);
    }
    // Played: it goes to the discard pile.
    ctx::to_discard(player_id, ID);
}