//! `CRYCHIC:主唱太拼命了` -- C# `CardVocalTooHard` (MatchHost.cs:3120-3145): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:主唱太拼命了`）:
//! > 主唱太拼命了：
//! > [反击] 一次性向其他玩家支付至少5000资金时，免除此次支付。
//!
//! that cancels one 5,000+ payment to another player.

use card_sdk::abi::ChainKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const VOCAL_TOO_HARD: CardDef = CardDef::new(
    "CRYCHIC:主唱太拼命了",
    &[On::Counteract(
        &[ChainKind::Effect],
        "actor == owner && effect.has(Pay) && target >= 0 && value >= 5000",
        None,
        counteract,
    )],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"主唱太拼命了"#)
        .raw_name(r#"主唱太拼命了"#)
        .tags(&[r#"反击"#])
        .text(r#"[反击] 一次性向其他玩家支付至少5000资金时，免除此次支付。"#)
        .cell(r#"M10"#)
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「免除此次支付」
    trigger::set_pay_amount(0);
    ctx::log(
        player_id,
        &Msg::new(key!("vocal_too_hard_note")).player_id("who", player_id),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
