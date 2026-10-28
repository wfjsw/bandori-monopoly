//! `AG:（绯玛丽）如果并非没问题` -- C# `CardHimariPlusOne` (MatchHost.cs:1792-1816).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（绯玛丽）如果并非没问题`）:
//! > （绯玛丽）如果并非没问题：【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1
//!
//! Counteraction: when one of your own rolls is under 6, it goes up by 1.
//!
//! Reviewed 10/23/2026

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const HIMARI_PLUS_ONE: CardDef = CardDef::new(
    "AG:（绯玛丽）如果并非没问题",
    // G4: kind is the category (DiceRollBefore | DiceRollResult); actor ==
    // owner + a face under 6 is the condition. Residual guard deleted.
    &[On::Counteract(
        // DiceRollBefore = a card-driven roll (the face rides `value`);
        // DiceRollResult = the move roll.
        &[ChainKind::DiceRollBefore, ChainKind::DiceRollResult],
        // 规则书: 「【反击】当你的一次掷骰小于6时」
        // TODO(规则书)（当你的一次掷骰小于6时）: 「一次掷骰」 covers any of the user's
        //   rolls -- `roll_direct` raises no [反击] window, so a direct roll can
        //   never be raised here.
        "actor == owner && ((move.roll != null && move.roll < 6) || (move.roll == null && value >= 0 && value < 6))",
        None,
        counteract).counter_reason(key!("himari_plus_one_when"))],
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
    // 规则书: 「使结果+1」 -- the face rides `value` on a card-driven roll.
    let before = trigger::move_roll().unwrap_or_else(|| trigger::value().max(0));
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
