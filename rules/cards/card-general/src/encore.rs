//! `通用:安可` -- C# `CardEncore` (MatchHost.cs:2388-2408): [反击] negate an
//! [异常移动效果] aimed at the user.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:安可`）:
//! > 安可：
//! > [手]：
//! > [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const ENCORE: CardDef = CardDef {
    id: "通用:安可",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「[使用者]即将因任何原因受到[异常移动效果]影响时」
    // C# `CardEncore.CanReact`: `t.Kind == "abnormal" && t.Target == seat`.
    trigger::kind() == TriggerKind::Abnormal && trigger::target() == seat
}

fn react(seat: i32) {
    // 规则书[反击]: 「无效此次[异常移动效果]和其导致的所有效果」
    // C# `c.Trigger.Cancelled = true` then 「安可：这次…无效」.
    ctx::log(seat, &Msg::new(key!("encore_negate")).seat("who", seat));
    // TODO(ABI): 「无效此次[异常移动效果]和其导致的所有效果」 -- needs a
    // trigger-cancel hook (C# `c.Trigger.Cancelled = true`) so the abnormal and
    // everything it leads to are dropped. The C# log also names the abnormal
    // (`AbName(c.Trigger.Ab?.Kind)`); the trigger carries no `Ab`.
}