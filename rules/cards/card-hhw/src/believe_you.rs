//! `HHW:因为我一直相信着你` -- C# `CardBelieveYou` (MatchHost.cs:3529-3596): pay
//! 800, reveal hand + deck, a rival discards one hand card, then keep two from the deck.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:因为我一直相信着你`）:
//! > 因为我一直相信着你：
//! > 至少有另一张手牌时可发动，消耗800资金，公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const BELIEVE_YOU: CardDef = CardDef {
    id: "HHW:因为我一直相信着你",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardBelieveYou.WhyNot`: refuses without another hand card / 800 money
    // / a living rival.
    // TODO(规则书): the hand-size check (`h.hand.Count(id => id != Id) >= 1`)
    // needs full hand enumeration, which is still missing; the gate covers only
    // the money and rival checks.
    if ctx::money(seat) < 800 {
        return Some(Msg::new(key!("x_no_money_800")));
    }
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(seat: i32) {
    // 规则书: 「消耗800资金」 -- C# `PayCtx { amount = 800, kind = "lose", must = false }`.
    let paid = ctx::pay(seat, 800, &Msg::new(key!("believe_you_pay")));
    if paid < 800 {
        // C# `c.Effective = false` when the payment does not go through (the play
        // is wasted, `H.ToDiscard(..., wasted: true)`); the ABI has no
        // set_effective hook.
        return;
    }
    // 规则书: 「公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，
    // 然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。」
    // TODO(规则书): the reveal / pick / shuffle body needs hand and deck
    // enumeration (C# `H._hidden[seat].hand` / `.draw`), `H.RevealSeen`, and
    // `H.Shuffle` of the draw pile. `discard_count` / `discard_size` /
    // `hand_count` (per-id) / `deck_count` are now queryable, and
    // `discard_from_hand` / `sweep_to_deck` / `add_to_deck_at` exist, but full
    // hand/deck *enumeration* is still missing -- neither the rival's pick from
    // your hand nor the 2-of-deck selection can be offered.
}