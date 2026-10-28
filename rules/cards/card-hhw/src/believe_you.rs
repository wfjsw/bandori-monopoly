//! `HHW:因为我一直相信着你` -- C# `CardBelieveYou` (MatchHost.cs:3529-3596): pay
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:因为我一直相信着你`）:
//! > 因为我一直相信着你：
//! > 至少有另一张手牌时可发动，消耗800资金，公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。
//!
//! 800, reveal hand + deck, a rival discards one hand card, then keep two from the deck.

use alloc::vec::Vec;

use bandori_std::ctx::{self, CardPile};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:因为我一直相信着你";

pub const BELIEVE_YOU: CardDef =
    CardDef::new("HHW:因为我一直相信着你", &[On::Play("", Some(cant_play), play)])
        .props(&[(card_sdk::abi::prop::EST_COST, 800)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"因为我一直相信着你"#)
        .raw_name(r#"因为我一直相信着你"#)
        .tags(&[])
        .text(CardText::para(r#"至少有另一张手牌时可发动，消耗800资金，公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。"#))
        // sheet cell F2
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardBelieveYou.WhyNot`: refuses without another hand card / 800 money
    // / a living rival. NEGATION-AUDIT V2 (settled): the **money** half is gone -- the
    // 「消耗800资金」 is effect content, not an activation cost.
    // 规则书: 「至少有另一张手牌时可发动」 -- a real [限] on hand count, kept.
    // The played copy is still in hand here (it leaves only after `cant_play`),
    // so drop exactly that one copy -- not every copy, which would miscount a
    // second copy of this same card as no other hand card.
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let others_in_hand = hand.len() - usize::from(hand.iter().any(|c| c == ID));
    if others_in_hand < 1 {
        return Some(Msg::new(key!("believe_you_no_hand")));
    }
    // 规则书: 「选择一名玩家」 -- C# `CardBelieveYou.WhyNot` also needs a living
    // rival to ask.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「消耗800资金」 -- C# `PayCtx { amount = 800, kind = "lose", must = false }`.
    let paid = ctx::pay(player_id, 800, &Msg::new(key!("believe_you_pay")))?;
    // TODO(规则书)（消耗800资金）: the book does not gate the rest on a full payment
    // -- a short/cancelled 800 drops the whole effect (C-Q5 / CH-07 wasted flag).
    if paid < 800 {
        // C# `c.Effective = false` when the payment does not go through (the play
        // is wasted, `H.ToDiscard(..., wasted: true)`); the ABI has no
        // set_effective hook.
        return Ok(());
    }
    // 规则书: 「公开你的卡组与手牌」 -- C# `H.Log` of both piles + `H.RevealSeen`
    // (the public peek itself is not in the vocabulary; the log names the sizes
    // and the picks below show each card's title).
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    // TODO(ABI)（公开你的卡组与手牌）: no reveal/publish-pile hostcall -- only the
    //   counts reach the table; the card identities stay with the prompt's seat.
    // TODO(规则书)（公开你的卡组与手牌）: the sizes are sampled before the rival
    //   acts, so an empty deck can log 0 here and the picks below then draw from
    //   a discard refill.
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
        // TODO(规则书)（选择一名玩家）: 「一名玩家」 does not say another player --
        //   whether the user may choose themself is open (OPEN-HHW-BELIEVE-CHOOSER).
        &ctx::others(player_id),
    )?;
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    if !hand.is_empty() {
        let ids: Vec<&str> = hand.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            who,
            &Msg::new(key!("believe_you_title")),
            &Msg::new(key!("believe_you_ask_discard")).player_id("who", player_id),
            &ids,
        )?;
        let id = ids[pick.min(ids.len() - 1)];
        ctx::discard_from_hand(player_id, id);
        ctx::log(
            player_id,
            &Msg::new(key!("believe_you_discarded"))
                .player_id("who", who)
                .card("card", id),
        );
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
        )?;
        let id = ids[pick.min(ids.len() - 1)];
        if ctx::take_card(player_id, id, CardPile::Deck) {
            ctx::add_to_dest(player_id, id, ctx::Dest::Hand);
            ctx::log(
                player_id,
                &Msg::new(key!("believe_you_kept")).card("card", id),
            );
        }
    }
    // 规则书: 「重洗你的抽牌堆」 -- C# `H.Shuffle(h.draw)`. `shuffle_into_deck`
    // (both flags off) shuffles the pile in place and raises `reshuffled` on
    // commit; the old take/`DeckRandom` loop let an emptied deck refill from the
    // discard and raised no hook at all on an empty pile.
    ctx::shuffle_into_deck(player_id, false, false);
    if ctx::deck_count(player_id) > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("believe_you_shuffled")).player_id("who", player_id),
        );
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
