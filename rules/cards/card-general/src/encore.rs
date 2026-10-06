//! `通用:安可` -- C# `CardEncore` (MatchHost.cs:2388-2408): [反击] negate an
//! [异常移动效果] aimed at the user.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:安可`）:
//! > 安可：
//! > [手]：
//! > [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!

use card_sdk::abi::{AbKind, ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ENCORE: CardDef = CardDef::new(
    "通用:安可",
    &[On::CounterAct(&[ChainKind::Effect], can_react, react)],
);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「[使用者]即将因任何原因受到[异常移动效果]影响时」
    // C# `CardEncore.CanReact`: `t.Kind == "abnormal" && t.Target == seat`.
    trigger::kind() == ChainKind::Effect && trigger::target() == player_id
}

fn react(player_id: i32) -> card_sdk::Asked {
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
