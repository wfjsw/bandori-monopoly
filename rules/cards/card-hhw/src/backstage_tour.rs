//! `HHW:出发！后台之旅！` -- C# `CardBackstageTour` (MatchHost.cs:3718-3779): look
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:出发！后台之旅！`）:
//! > 出发！后台之旅！：
//! >  看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金
//!
//! at the top two deck cards, keep 0-2 in order, discard the rest for 1,000 each.

use alloc::vec::Vec;

use alloc::string::String;
use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, Msg, On};

pub const BACKSTAGE_TOUR: CardDef =
    CardDef::new("HHW:出发！后台之旅！", &[On::Play("", Some(cant_play), play)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardBackstageTour.WhyNot` refuses the play when the draw pile and the
    // discard pile are both empty ("抽卡区没有卡").
    if ctx::deck_count(player_id) + ctx::discard_size(player_id) == 0 {
        return Some(Msg::new(key!("x_no_deck_cards")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // C# `if (h.draw.Count < 2 && h.discard.Count > 0) { h.draw.InsertRange(0,
    // h.discard); h.discard.Clear(); }` -- when the draw pile is too thin, the
    // discard comes back underneath it (the C# log calls this 洗回).
    if ctx::deck_count(player_id) < 2 && ctx::discard_size(player_id) > 0 {
        let discard = ctx::cards_in(player_id, CardPile::Discard);
        for id in &discard {
            ctx::take_card(player_id, CardPile::Discard, id);
            ctx::add_to_deck_at(player_id, id, ctx::DeckPos::Bottom);
        }
        ctx::log(
            player_id,
            &Msg::new(key!("backstage_tour_reshuffled")).player_id("who", player_id),
        );
        // C# also fires `H.Each((Fx f) => f.Reshuffled(i))` -- that persistent
        // Fx hook is still missing (same as `card-roselia`'s 曲奇时间).
    }
    // 规则书: 「看牌堆顶2张牌」 -- C# `h.draw.Skip(Count - 2).Reverse()` = the top
    // two of the draw pile, top card first (`cards_in(Deck)` is top-first).
    let top: Vec<String> = ctx::cards_in(player_id, CardPile::Deck)
        .into_iter()
        .take(2)
        .collect();
    if top.is_empty() {
        return Ok(());
    }
    // C# removes the peeked cards from the draw pile before the keep/discard
    // prompts, then puts the keepers back on top.
    for id in &top {
        ctx::take_card(player_id, CardPile::Deck, id);
    }
    // 规则书: 「选择0~2张以任意顺序放回」 -- C# `H.AskYes(..., "放回", "翻入弃卡区")`
    // per card (yes = keep on top of the draw pile).
    let mut keep: Vec<String> = Vec::new();
    for id in &top {
        let yes = ctx::ask_yes(
            player_id,
            &Msg::new(key!("backstage_tour_ask_title")),
            &Msg::new(key!("backstage_tour_ask_keep")).card("card", id),
        )?;
        if yes {
            keep.push(id.clone());
            continue;
        }
        // 规则书: 「剩余的翻入弃牌堆，每翻入一张获得1000资金」 -- C#
        // `H.ToDiscard(i, id)` + `H.GainR(i, 1000, CardName)`.
        ctx::to_discard(player_id, id);
        ctx::gain(
            player_id,
            1000,
            &Msg::new(key!("backstage_tour_flip")).card("card", id),
        )?;
    }
    // 规则书: 「以任意顺序放回」 -- when both are kept, C# `H.AskCard(..., "哪一张
    // 放在最上面？")` picks which one is the new top (`h.draw.Add(under);
    // h.draw.Add(top)`).
    if keep.len() == 2 {
        let refs: Vec<&str> = keep.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            player_id,
            &Msg::new(key!("backstage_tour_ask_title")),
            &Msg::new(key!("backstage_tour_ask_top")),
            &refs,
        )?;
        let top_i = pick.min(1);
        let under = keep[1 - top_i].clone();
        let over = keep[top_i].clone();
        ctx::add_to_deck_at(player_id, &under, ctx::DeckPos::Top);
        ctx::add_to_deck_at(player_id, &over, ctx::DeckPos::Top);
    } else {
        for id in keep {
            ctx::add_to_deck_at(player_id, &id, ctx::DeckPos::Top);
        }
    }
    Ok(())
}
