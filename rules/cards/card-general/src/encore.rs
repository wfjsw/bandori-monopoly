//! `通用:安可` -- C# `CardEncore` (MatchHost.cs:2388-2408): [反击] negate an
//!
//! 规则书（docs/rulebook/cards.json, id `通用:安可`）:
//! > 安可：
//! > [手]：
//! > [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!
//! [异常移动效果] aimed at the user.

use card_sdk::abi::{AbKind, ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ENCORE: CardDef = CardDef::new(
    "通用:安可",
    &[On::Counteract(
        &[ChainKind::Effect],
        None,
        counteract,
        "target == owner && effect.has(Abnormal)",
    )],
)
    .legacy(&[(0, legacy_can_counteract)]);

fn legacy_can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「[使用者]即将因任何原因受到[异常移动效果]影响时」
    // C# `CardEncore.CanCounteract`: `t.Kind == "abnormal" && t.Target == seat`.
    //
    // 「[异常移动效果]」 is the abnormal family (传送/停留/晕眩/除外/强制移动/
    // 强制停下/反方向 -- CROSS-TESTS 47–52), carried here as the chain's
    // `abnormal` effect entry. Not just any effect aimed at the user: the money
    // pipeline declares a `pay` effect on every [支付]/[获得] and must not open
    // this window (the [反击] window on those belongs to the pay-shaped counters).
    //
    // TODO(规则书)[反击]: 「因任何原因」 also covers an abnormal the user's *own*
    // card applies to them (cf. （兰）像往常一样's 「包括你的技能」), but the
    // engine's `abnormal` chain only opens when another player caused the effect
    // (`by != target`), and a `card_move` teleport never runs the abnormal gate
    // at all -- so a self-[传送] gets no window to answer.
    trigger::kind() == ChainKind::Effect
        && trigger::target() == player_id
        && ctx::effect::has(TriggerKind::Abnormal)
}

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
