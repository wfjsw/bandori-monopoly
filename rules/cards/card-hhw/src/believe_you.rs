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
    // NEGATION-AUDIT V2 (settled): the **money** half is gone -- the 「消耗800资金」 is
    // effect content, not an activation cost. 规则书: 「至少有另一张手牌时可发动」 -- a real [限] on
    // hand count, kept. The played copy is still in hand here (it leaves only after
    // `cant_play`), so drop exactly that one copy -- not every copy, which would
    // miscount a second copy of this same card as no other hand card.
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let others_in_hand = hand.len() - usize::from(hand.iter().any(|c| c == ID));
    if others_in_hand < 1 {
        return Some(Msg::new(key!("believe_you_no_hand")));
    }
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    let paid = ctx::pay(player_id, 800, &Msg::new(key!("believe_you_pay")))?;
    // TODO(规则书)（消耗800资金）: the book does not gate the rest on a full payment
    // -- a short/cancelled 800 drops the whole effect (C-Q5 / CH-07 wasted flag).
    if paid < 800 {
        return Ok(());
    }
    // 公开你的卡组与手牌: the hand is published once, the whole deck turns face
    // up in a public frame (每个 turned card is a public reveal of its own).
    ctx::reveal_pile(
        player_id,
        CardPile::Hand,
        &Msg::new(key!("believe_you_reveal_hand")),
    );
    let count = ctx::deck_count(player_id);
    ctx::inspection_begin(player_id, player_id, count, true)?;
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
    for k in 0..2 {
        let deck = ctx::cards_in(player_id, CardPile::Inspection);
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
        if ctx::take_card(player_id, id, CardPile::Inspection) {
            ctx::add_to_dest(player_id, id, ctx::Dest::Hand);
            ctx::log(
                player_id,
                &Msg::new(key!("believe_you_kept")).card("card", id),
            );
        }
    }
    // The leftovers go back in their revealed order, then the whole pile is
    // reshuffled (重洗你的抽牌堆).
    ctx::inspection_end(player_id, false);
    // `shuffle_into_deck` (both flags off) shuffles the pile in place and raises
    // `reshuffled` on commit; the old take/`DeckRandom` loop let an emptied deck
    // refill from the discard and raised no hook at all on an empty pile.
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
