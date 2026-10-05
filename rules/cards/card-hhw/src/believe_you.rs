//! `HHW:因为我一直相信着你` -- C# `CardBelieveYou` (MatchHost.cs:3529-3596): pay
//! 800, reveal hand + deck, a rival discards one hand card, then keep two from the deck.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:因为我一直相信着你`）:
//! > 因为我一直相信着你：
//! > 至少有另一张手牌时可发动，消耗800资金，公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。
//!

use alloc::vec::Vec;

use card_sdk::ctx::{self, CardPile, DeckPos};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:因为我一直相信着你";

pub const BELIEVE_YOU: CardDef = CardDef::new("HHW:因为我一直相信着你", &[
    On::Play(play),
    On::CantPlay(cant_play),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardBelieveYou.WhyNot`: refuses without another hand card / 800 money
    // / a living rival.
    // 规则书: 「至少有另一张手牌时可发动」 -- C# `h.hand.Count(id => id != Id) >= 1`
    // (this card itself does not count; it is still in hand at the gate).
    let others_in_hand = ctx::cards_in(player_id, CardPile::Hand)
        .into_iter()
        .filter(|c| c != ID)
        .count();
    if others_in_hand < 1 {
        return Some(Msg::new(key!("believe_you_no_hand")));
    }
    if ctx::money(player_id) < 800 {
        return Some(Msg::new(key!("x_no_money_800")));
    }
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) {
    // 规则书: 「消耗800资金」 -- C# `PayCtx { amount = 800, kind = "lose", must = false }`.
    let paid = ctx::pay(player_id, 800, &Msg::new(key!("believe_you_pay")));
    if paid < 800 {
        // C# `c.Effective = false` when the payment does not go through (the play
        // is wasted, `H.ToDiscard(..., wasted: true)`); the ABI has no
        // set_effective hook.
        return;
    }
    // 规则书: 「公开你的卡组与手牌」 -- C# `H.Log` of both piles + `H.RevealSeen`
    // (the public peek itself is not in the vocabulary; the log names the sizes
    // and the picks below show each card's title).
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    ctx::log(
        player_id,
        &Msg::new(key!("believe_you_reveal"))
            .player_id("who", player_id)
            .i("hand", hand.len() as i64)
            .i("deck", deck.len() as i64),
    );
    // 规则书: 「选择一名玩家，使其从你的手牌中选择一张放入弃牌堆」 -- C#
    // `H.AskSeat` then `H.AskCard(who, ..., h.hand.ToList())` +
    // `H.DiscardFromHand`.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("believe_you_title")),
        &Msg::new(key!("believe_you_ask_who")),
        &ctx::others(player_id),
    );
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    if !hand.is_empty() {
        let ids: Vec<&str> = hand.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            who,
            &Msg::new(key!("believe_you_title")),
            &Msg::new(key!("believe_you_ask_discard")).player_id("who", player_id),
            &ids,
        );
        let id = ids[pick.min(ids.len() - 1)];
        ctx::discard_from_hand(player_id, id);
        ctx::log(player_id, &Msg::new(key!("believe_you_discarded")).player_id("who", who).card("card", id));
    }
    // 规则书: 「然后从你的卡组中选择两张加入你的手牌」 -- C# loops twice:
    // `H.AskCard(i, ..., h.draw.ToList())` then `h.draw.Remove` + `H.AddToHand`.
    for k in 0..2 {
        let deck = ctx::cards_in(player_id, CardPile::Deck);
        if deck.is_empty() {
            break;
        }
        let ids: Vec<&str> = deck.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            player_id,
            &Msg::new(key!("believe_you_title")),
            &Msg::new(key!("believe_you_ask_deck")).i("n", k as i64 + 1),
            &ids,
        );
        let id = ids[pick.min(ids.len() - 1)];
        if ctx::take_card(player_id, CardPile::Deck, id) {
            ctx::add_to_hand(player_id, id);
            ctx::log(player_id, &Msg::new(key!("believe_you_kept")).card("card", id));
        }
    }
    // 规则书: 「重洗你的抽牌堆」 -- C# `H.Shuffle(h.draw)`. There is no in-place
    // shuffle hook, so each remaining draw-pile card is taken out and mixed back
    // in (`DeckPos::Random`); reshuffles stay on `add_to_deck_at`.
    let rest = ctx::cards_in(player_id, CardPile::Deck);
    for id in rest {
        ctx::take_card(player_id, CardPile::Deck, &id);
        ctx::add_to_deck_at(player_id, &id, DeckPos::Random);
    }
    ctx::log(player_id, &Msg::new(key!("believe_you_shuffled")).player_id("who", player_id));
}
