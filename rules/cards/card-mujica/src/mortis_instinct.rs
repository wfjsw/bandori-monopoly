//! `Mujica:（睦/mortis）表演的本能` -- C# `CardMortisInstinct` (MatchHost.cs:5342-5435):
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（睦/mortis）表演的本能`）:
//! > （睦/mortis）表演的本能：
//! >  此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）（此卡复制[反击]卡时可在符合条件时打出）
//!
//! copy the last non-placed card another player played.

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{CardText};

pub const MORTIS_INSTINCT: CardDef = CardDef::new(
    "Mujica:（睦/mortis）表演的本能",
    &[On::Play("", None, play), On::Counteract(&[], "", Some(can_counteract), counteract).counter_reason(key!("mortis_instinct_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"表演的本能"#)
        .raw_name(r#"（睦/mortis）表演的本能"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）（此卡复制[反击]卡时可在符合条件时打出）"#))
        // sheet cell J2
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"若叶睦"#)]));

fn play(player_id: i32) -> card_sdk::Asked {
    run(player_id);
    Ok(())
}

fn can_counteract(_player: i32) -> bool {
    // TODO(规则书)[judgement]: 「（此卡复制[反击]卡时可在符合条件时打出）」 -- C#
    //   the clause under-specifies -- see the note above it
    // `CardMortisInstinct.CanCounteract` delegates to `Copy(player_id)?.CanCounteract(player_id, t)`.
    // Needs the play-history copy machinery (C# `H._playHistory` / `H.NewCard`)
    // so the copied card's counteraction window can be checked.
    false
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    run(player_id);
    Ok(())
}

fn run(player_id: i32) {
    // 规则书: 「此卡打出时效果为场上任意其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）」
    // TODO(规则书): the copy itself -- C# `CardMortisInstinct.Copy` walks
    // `H._playHistory` backwards for the latest `(player_id != me, !placed, id != me)`
    // entry, then `H.NewCard(id)` + nested `Play`/`Counteract` (a full `PlayCtx` with
    // `Extrene`/`Doubled`/`Tags` forwarding). The ABI has no play-history query
    // and no cross-card `PlayCtx` nesting (`ctx::play_card` runs a card's `play`
    // without the copied card's identity / counteraction mode).
    ctx::log(
        player_id,
        &Msg::new(key!("mortis_instinct_nothing")).player_id("who", player_id),
    );
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
