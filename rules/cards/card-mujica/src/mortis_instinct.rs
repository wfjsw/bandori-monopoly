//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（睦/mortis）表演的本能`）:
//! > （睦/mortis）表演的本能：
//! >  此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）（此卡复制[反击]卡时可在符合条件时打出）
//!
//! copy the last non-placed card another player played. The engine resolves
//! the copy: the entry's empty list means every kind, `copies_previous` makes
//! the matcher walk the play record and dispatch **the copied card's own
//! entry**, and a **play** stamps `trigger::link_copied` with the source so
//! the play half copies too (design G06 §2.15).

use card_sdk::ctx::trigger;
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const MORTIS_INSTINCT: CardDef = CardDef::new(
    "Mujica:（睦/mortis）表演的本能",
    &[
        On::Play("", None, play),
        // 规则书: 「（此卡复制[反击]卡时可在符合条件时打出）」 -- the copied card's
        // own kind decides when it is offered; `Card` is the playable link the
        // copy is declared under.
        // Empty links = every chain kind; the copied card's own entry decides
        // whether it may be played in the window (its contract included).
        On::Counteract(&[], "", Some(can_counteract), counteract)
            .copies_previous()
            .counter_reason(key!("mortis_instinct_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"表演的本能"#)
        .raw_name(r#"（睦/mortis）表演的本能"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）（此卡复制[反击]卡时可在符合条件时打出）"#))
        // sheet cell J2
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"若叶睦"#)]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「此卡打出时效果为…上一张卡」 -- the link names the source.
    if let Some(src) = trigger::link_copied() {
        ctx::play_card(&src, player_id)?;
    } else {
        // Nothing on the play record to copy: name the empty resolution.
        ctx::log(
            player_id,
            &Msg::new(key!("mortis_instinct_nothing")).player_id("who", player_id),
        );
    }
    Ok(())
}

fn can_counteract(_player: i32) -> bool {
    // The matcher resolves the copied card's own entry (with its contract);
    // no card of its own admits or refuses here.
    true
}

fn counteract(_player_id: i32) -> card_sdk::Asked {
    // The copy resolves to the source card's entry and body, so this body
    // runs only when the matcher kept the entry itself.
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

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
