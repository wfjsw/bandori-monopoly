//! `通用:安可` -- C# `CardEncore` (MatchHost.cs:2388-2408): [反击] negate an
//!
//! 规则书（docs/rulebook/cards.json, id `通用:安可`）:
//! > 安可：
//! > [手]：
//! > [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!
//! [异常移动效果] aimed at the user.

use card_sdk::abi::{AbKind, ChainKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ENCORE: CardDef = CardDef::new(
    "通用:安可",
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner && effect.has(Abnormal)", Some(key!("encore_when")),
        None,
        counteract,
    )],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"安可"#)
        .raw_name(r#"安可"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(r#"[手]：
[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。"#)
        .cell(r#"A13"#)
        .fields(&[("group", r#"通用"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「无效此次[异常移动效果]和其导致的所有效果」
    // C# `c.Trigger.Cancelled = true` then 「安可：这次…无效」.
    trigger::negate_effect();
    ctx::log(
        player_id,
        &Msg::new(key!("encore_negate")).player_id("who", player_id),
    );
    // Narrowed: the C# log names the abnormal (`AbName(c.Trigger.Ab?.Kind)`).
    // `trigger::abnormal_kind()` now carries `t.Ab.Kind` (v25), but `Msg` has
    // no localized AbKind-name argument, so the log keeps the rulebook's
    // generic [异常移动效果] wording rather than hardcoding the C# display names.
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
