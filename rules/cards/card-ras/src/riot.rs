//! `RAS:R. I. O. T.` -- C# `CardRiot` (MatchHost.cs:9286-9320): [反击] everyone
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:R. I. O. T.`）:
//! > R. I. O. T.： 
//! > [反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。
//!
//! discards their hand and redraws the same count.

use alloc::vec::Vec;

use card_sdk::abi::ChainKind;
use bandori_std::ctx::{self, CardPile};
use card_sdk::{CardDef, On};

pub const RIOT: CardDef = CardDef::new(
    "RAS:R. I. O. T.",
    &[On::Counteract(
        &[ChainKind::Effect],
        "by >= 0 && by != owner && effect.hits(owner)",
        r#"你被其他人的卡效果影响"#,
        None,
        counteract,
    )],
)
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"R. I. O. T."#)
        .raw_name(r#"R. I. O. T."#)
        .tags(&[r#"反击"#])
        .text(r#"[反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。"#)
        .cell(r#"H2"#)
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡」
    // -- C# `H.DiscardFromHand` over every hand, then `H.DrawR(p, count + (p ==
    // player ? 1 : 0))` for each player_id still in the game.
    // Every hand goes to the discard first, then everyone redraws -- so nobody
    // redraws a card another player just discarded mid-pass.
    let mut redraw: Vec<(i32, i32)> = Vec::new();
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) {
            continue;
        }
        let hand = ctx::cards_in(p, CardPile::Hand);
        for id in &hand {
            ctx::discard_from_hand(p, id);
        }
        redraw.push((p, hand.len() as i32 + (p == player_id) as i32));
    }
    for (p, n) in redraw {
        if n > 0 {
            ctx::draw(p, n)?;
        }
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
