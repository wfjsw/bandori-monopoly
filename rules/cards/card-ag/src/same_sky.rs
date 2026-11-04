//!
//! 规则书（docs/rulebook/cards.json, id `AG:朝同一片天空迈进`）:
//! > 朝同一片天空迈进：
//! > [反击]抽出此卡时立刻打出，如果你手牌数至少为3，获得手牌数*600的资金，如果你的手牌数小于3，抽一张卡（开局时抽到此卡洗回）
//!
//! [反击] auto-plays when drawn: money by hand size, else draw.

use bandori_std::ctx::{self, trigger};
use bandori_std::zones::ZoneChangeCause;
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const SAME_SKY: CardDef = CardDef::new(
    "AG:朝同一片天空迈进",
    &[
        On::Action(Action::Drawn("trigger_card == card.id", None, counteract).counter_reason(key!("same_sky_when"))),
        // 规则书: 「（开局时抽到此卡洗回）」 -- the deal itself, while the copy is in
        // the opening hand.
        On::Action(Action::OpeningHandDealt("", None, return_at_opening)),
        // 规则书: 「换牌后，若此卡在你的手牌中，则将此卡打出」 -- the mulligan
        // redraw is an uncounted `handChanged` with cause `Mulligan`.
        On::Action(Action::HandChanged("", None, after_mulligan)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"朝同一片天空迈进"#)
        .raw_name(r#"朝同一片天空迈进"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击]抽出此卡时立刻打出，如果你手牌数至少为3，获得手牌数*600的资金，如果你的手牌数小于3，抽一张卡（开局时抽到此卡洗回）"#))
        // sheet cell C11
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

const ID: &str = "AG:朝同一片天空迈进";

/// 规则书: 「（开局时抽到此卡洗回）」 -- the dealt copy goes back into the deck at
/// the deal, before the mulligan asks.
fn return_at_opening(player_id: i32) -> card_sdk::Asked {
    // Pull every copy out of the opening hand and shuffle it back into the deck.
    let mut returned = 0;
    while ctx::take_card(player_id, ID, ctx::CardPile::Hand) {
        ctx::add_to_dest(player_id, ID, ctx::Dest::DeckRandom, false);
        returned += 1;
    }
    if returned > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("same_sky_returned")).player_id("who", player_id),
        );
    }
    Ok(())
}

/// 规则书: 「换牌后，若此卡在你的手牌中，则将此卡打出」 -- only the mulligan
/// redraw (cause `Mulligan`); the copy still in hand is played for real, through
/// the play pipeline (`card` window, `cardPlayed`).
fn after_mulligan(player_id: i32) -> card_sdk::Asked {
    let is_mulligan = trigger::zone().is_some_and(|z| z.cause == ZoneChangeCause::Mulligan);
    if !is_mulligan {
        return Ok(());
    }
    if !ctx::cards_in(player_id, ctx::CardPile::Hand)
        .iter()
        .any(|c| c == ID)
    {
        return Ok(());
    }
    // 规则书: 「则将此卡打出」.
    ctx::play_hand_card(player_id, ID);
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `trigger_card == card.id` owns "this card was drawn".
    // 规则书[反击]: 「抽出此卡时立刻打出」 -- the drawn copy is played, one
    // hand -> discard move. (Taking it and then calling `to_discard` would also
    // strip a *second* held copy, because `to_discard` removes a matching hand
    // card before pushing; `discard_from_hand` is the single-move op.)
    if !ctx::discard_from_hand(player_id, ID) {
        return Ok(());
    }
    // 手牌数 after the play.
    let n = ctx::hand_size(player_id);
    if n >= 3 {
        // 规则书[反击]: 「如果你手牌数至少为3，获得手牌数*600的资金」
        ctx::gain(
            player_id,
            n * 600,
            &Msg::new(key!("same_sky_reason")).i("n", n as i64),
        )?;
    } else {
        // 规则书[反击]: 「如果你的手牌数小于3，抽一张卡」
        ctx::draw(player_id, 1)?;
    }
    // This is the [反击] declaration half (its own counteract pipeline); the
    // mulligan half above plays for real through `play_hand_card`.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn return_at_opening_kani(player_id: i32) -> card_sdk::Asked {
    return_at_opening(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_mulligan_kani(player_id: i32) -> card_sdk::Asked {
    after_mulligan(player_id)
}
