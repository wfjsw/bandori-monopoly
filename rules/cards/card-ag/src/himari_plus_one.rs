//! `AG:（绯玛丽）如果并非没问题` -- C# `CardHimariPlusOne` (MatchHost.cs:1792-1816).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（绯玛丽）如果并非没问题`）:
//! > （绯玛丽）如果并非没问题：【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1
//!
//! Counteraction: when one of your own rolls is under 6, it goes up by 1.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const HIMARI_PLUS_ONE: CardDef = CardDef::new(
    "AG:（绯玛丽）如果并非没问题",
    // G4: kind (Roll|DiceRollValue) is the category; `mine` + the roll threshold
    // are the condition. Residual guard deleted.
    &[On::Counteract(
        &[ChainKind::DiceRollBefore, ChainKind::DiceRollResult],
        "actor == owner && move.roll != null && move.roll < 6",
        Some(key!("himari_plus_one_when")),
        None,
        counteract,
    )],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"如果并非没问题"#)
        .raw_name(r#"（绯玛丽）如果并非没问题"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1"#))
        // sheet cell C15
        .fields(&[("group", r#"Afterglow"#), ("owner", r#"上原绯玛丽"#)]),
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使结果+1」
    let Some(before) = trigger::move_roll() else {
        return Ok(());
    };
    trigger::set_move_roll(before + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("himari_plus_one"))
            .player_id("who", player_id)
            .card("card", "AG:（绯玛丽）如果并非没问题")
            .i("total", (before + 1) as i64),
    );
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
