//! `AG:（绯玛丽）如果并非没问题` -- C# `CardHimariPlusOne` (MatchHost.cs:1792-1816).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（绯玛丽）如果并非没问题`）:
//! > （绯玛丽）如果并非没问题：【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1
//!
//! Counteraction: when one of your own rolls is under 6, it goes up by 1.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const HIMARI_PLUS_ONE: CardDef = CardDef::new(
    "AG:（绯玛丽）如果并非没问题",
    // G4: kind (Roll|MoveRoll) is the category; `mine` + the roll threshold
    // are the condition. Residual guard deleted.
    &[On::Counteract(
        &[ChainKind::Roll, ChainKind::MoveRoll],
        None,
        counteract,
        "actor == owner && move.roll != null && move.roll < 6",
    )],
)
    .legacy(&[(0, legacy_can_counteract)]);

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    if !matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll) {
        return false;
    }
    if trigger::player_id() != player_id {
        return false;
    }
    matches!(trigger::move_roll(), Some(r) if r < 6)
}

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
