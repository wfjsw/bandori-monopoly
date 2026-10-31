//!
//! 规则书（docs/rulebook/cards.json, id `HHW:出发！后台之旅！`）:
//! > 出发！后台之旅！：
//! >  看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金
//!
//! at the top two deck cards, keep 0-2 in order, discard the rest for 1,000 each.

use alloc::vec::Vec;

use bandori_std::ctx::{self, CardPile};
use card_sdk::ctx::NameId;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const BACKSTAGE_TOUR: CardDef = CardDef::new(
    "HHW:出发！后台之旅！",
    &[On::Play("", Some(cant_play), play)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"出发！后台之旅！"#)
        .raw_name(r#"出发！后台之旅！"#)
        .tags(&[])
        .text(CardText::para(
            r#"看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金"#,
        ))
        // sheet cell F4
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]),
);

fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::deck_count(player_id) + ctx::discard_size(player_id) == 0 {
        return Some(Msg::new(key!("x_no_deck_cards")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（看牌堆顶2张牌）: the book names only the top 2 and not a short pile -- with
    // <2 we shuffle the whole discard in first (OPEN-HHW-SHORT-PILE).
    if ctx::deck_count(player_id) < 2 && ctx::discard_size(player_id) > 0 {
        let discard = ctx::cards_in(player_id, CardPile::Graveyard);
        for id in discard.iter().rev() {
            ctx::take_card(player_id, id.as_str(), CardPile::Graveyard);
            ctx::add_to_fate(player_id, id.as_str(), ctx::Fate::DeckBottom);
        }
        ctx::log(
            player_id,
            &Msg::new(key!("backstage_tour_reshuffled")).player_id("who", player_id),
        );
        // These are bare `DeckBottom` writes, so no `reshuffled` rides them -- raise it
        // here (`shuffle_into_deck` raises on commit, but it would also randomize and
        // change the peek order; cf. `card-roselia`'s 曲奇时间).
        ctx::raise(player_id, card_sdk::abi::ActionKind::Reshuffled, 0)?;
    }
    let top: Vec<NameId> = ctx::cards_in(player_id, CardPile::Deck)
        .into_iter()
        .take(2)
        .collect();
    if top.is_empty() {
        return Ok(());
    }
    for id in &top {
        ctx::take_card(player_id, id.as_str(), CardPile::Deck);
    }
    let mut keep: Vec<NameId> = Vec::new();
    for (i, id) in top.iter().enumerate() {
        // 规则书: 「看牌堆顶2张牌，选择0~2张以任意顺序放回」 -- both peeked cards
        // are named in each ask, so the keep/flip decision is made with the pair
        // already known.
        let text = if top.len() == 2 {
            Msg::new(key!("backstage_tour_ask_keep_both"))
                .card("card", id.as_str())
                .card("other", top[1 - i].as_str())
        } else {
            Msg::new(key!("backstage_tour_ask_keep")).card("card", id.as_str())
        };
        let yes = ctx::ask_yes(
            player_id,
            &Msg::new(key!("backstage_tour_ask_title")),
            &text,
        )?;
        if yes {
            keep.push(*id);
            continue;
        }
        // TODO(规则书)（剩余的翻入弃牌堆）: the peek can empty the draw pile, so `to_discard` fires
        // the base-rule refill and shuffles the flipped card straight back -- it never
        // stays in the discard (defer the flips).
        ctx::to_discard(player_id, id.as_str());
        ctx::gain(
            player_id,
            1000,
            &Msg::new(key!("backstage_tour_flip")).card("card", id.as_str()),
        )?;
    }
    if keep.len() == 2 {
        let refs: Vec<&str> = keep.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            player_id,
            &Msg::new(key!("backstage_tour_ask_title")),
            &Msg::new(key!("backstage_tour_ask_top")),
            &refs,
        )?;
        let top_i = pick.min(1);
        let under = keep[1 - top_i];
        let over = keep[top_i];
        ctx::add_to_fate(player_id, under.as_str(), ctx::Fate::DeckTop);
        ctx::add_to_fate(player_id, over.as_str(), ctx::Fate::DeckTop);
    } else {
        for id in keep {
            ctx::add_to_fate(player_id, id.as_str(), ctx::Fate::DeckTop);
        }
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
