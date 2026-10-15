//! `AG:Y.O.L.O` -- C# `CardYolo` (MatchHost.cs:956-987).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:Y.O.L.O`）:
//! > Y.O.L.O： 
//! >  你的任意掷骰结算前打出此卡，使结果增加1d4结果的数字
//!
//! Sheet 2026-10-06 新卡组卡 C3: 「你的**任意**掷骰结算前」 (was 「你的掷骰
//! 结算前」) -- any of the user's rolls, not only the main move roll.

use card_sdk::abi::ChainKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const YOLO: CardDef = CardDef::new(
    "AG:Y.O.L.O",
    // G4: kind (Roll|MoveRoll) is the category; `mine` + the settled-roll
    // clause are the condition. Residual guard deleted.
    &[On::Counteract(
        &[ChainKind::Roll, ChainKind::MoveRoll],
        "actor == owner && (move.roll != null || value >= 0)",
        None,
        counteract,
    )],
)
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Y.O.L.O"#)
        .raw_name(r#"Y.O.L.O"#)
        .tags(&[])
        .text(r#"你的任意掷骰结算前打出此卡，使结果增加1d4结果的数字"#)
        .cell(r#"C3"#)
        .fields(&[("band", r#"Afterglow"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let before = trigger::move_roll().unwrap_or_else(|| trigger::value().max(0));
    let n = ctx::roll(player_id, 1, 4);
    trigger::set_move_roll(before + n);
    ctx::log(
        player_id,
        &Msg::new(key!("yolo_boost"))
            .player_id("who", trigger::player_id())
            .i("n", n as i64)
            .i("total", (before + n) as i64),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
