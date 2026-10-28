//! `RAS:R. I. O. T.` -- C# `CardRiot` (MatchHost.cs:9286-9320): [反击] everyone
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:R. I. O. T.`）:
//! > R. I. O. T.：
//! > [反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。
//!
//! discards their hand and redraws the same count; the counteractor draws 1 extra.

use alloc::vec::Vec;

use bandori_std::ctx::{self, CardPile};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, On};

pub const RIOT: CardDef = CardDef::new(
    "RAS:R. I. O. T.",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书[反击]: 「当你被其他人的卡效果影响时打出此卡」
        // The guest `effect::hits` (not the cond builtin, which reads effect
        // *targets* only) counts a forced payment's payer as hit too.
        "by >= 0 && by != owner",
        Some(can_counteract),
        counteract).counter_reason(key!("riot_when"))],
)
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"R. I. O. T."#)
        .raw_name(r#"R. I. O. T."#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。"#))
        // sheet cell H2
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

/// 规则书[反击]: 「当你被其他人的卡效果影响时」 -- the holder is the effect's
/// target **or** its payer (a forced payment is an effect on the payer too).
fn can_counteract(player_id: i32) -> bool {
    ctx::effect::hits(player_id)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡」
    // -- C# `H.DiscardFromHand` over every hand, then `H.DrawR(p, count)` for
    // each player still in the game and `H.DrawR(player, 1)` last.
    // Every hand goes to the discard first, then everyone redraws -- so nobody
    // redraws a card another player just discarded mid-pass.
    // 规则书: 「所有玩家」 -- a seat that has left the game draws nothing.
    let mut redraw: Vec<(i32, i32)> = Vec::new();
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) {
            continue;
        }
        let hand = ctx::cards_in(p, CardPile::Hand);
        for id in &hand {
            ctx::discard_from_hand(p, id.as_str());
        }
        redraw.push((p, hand.len() as i32));
    }
    for (p, n) in redraw {
        if n > 0 {
            ctx::draw(p, n)?;
        }
    }
    // 规则书: 「你额外抽1张卡」 -- a separate draw after every player's redraw.
    if !ctx::player_out(player_id) {
        ctx::draw(player_id, 1)?;
    }
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
