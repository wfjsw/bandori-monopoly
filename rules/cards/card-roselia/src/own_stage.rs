//! `R:选择自己的舞台` -- C# `CardOwnStage` (MatchHost.cs:10710-10745): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:选择自己的舞台`）:
//! > 选择自己的舞台： 
//! > [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算
//!
//! on an abnormal move (not [除外]): choose whether this move (or the end of a
//! movement-locked turn) settles.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger, AbKind};
use card_sdk::{key, CardDef, Msg, On};

pub const OWN_STAGE: CardDef = CardDef::new(
    "R:选择自己的舞台",
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner",
        Some(can_counteract),
        counteract,
    )],
)
    .legacy(&[(0, legacy_can_counteract)]);

/// The [异常移动效果] this chain link declared, as its `AbKind`.
///
/// `trigger::abnormal_kind()` reads `t.Ab.Kind` on the settlement-hook kinds
/// (`abnormal` / `abnormalGuard`); inside the `effect` [反击] window the kind
/// is `Effect` and the abnormal rides the link's effect list instead -- each
/// entry's `value` is its `AbKind`.
fn abnormal_kind() -> Option<AbKind> {
    (0..ctx::effect::count()).find_map(|i| {
        (ctx::effect::kind(i) == TriggerKind::Abnormal)
            .then(|| AbKind::from_i32(ctx::effect::value(i)))
            .flatten()
    })
}

/// 规则书[反击]: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」
fn can_counteract(_player_id: i32) -> bool {
    // G4: the actor rel is the condition (`target == owner`); 「[除外]以外」 is
    // a derived chain lookup (GUARDS.md §6) and stays here.
    abnormal_kind().is_some_and(|k| k != AbKind::Exile)
}

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    if trigger::kind() != ChainKind::Effect || trigger::target() != player_id {
        return false;
    }
    abnormal_kind().is_some_and(|k| k != AbKind::Exile)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("own_stage_ask_title")),
        &Msg::new(key!("own_stage_ask_text")),
    )?;
    let why = if yes {
        Msg::new(key!("own_stage_settle")).player_id("who", player_id)
    } else {
        Msg::new(key!("own_stage_no_settle")).player_id("who", player_id)
    };
    ctx::log(player_id, &why);
    // 规则书[反击]: 「（或无法移动的回合结束时）」 -- C# `H.SetV(i, "stageEnd", yes ? 1 : 2)`
    // when the abnormal is stay/stun (C# `CardOwnStage.Counteract` branches on
    // `c.Trigger.Ab.Kind`).
    match abnormal_kind() {
        Some(AbKind::Stay) | Some(AbKind::Stun) => {
            ctx::set_slot(player_id, "stageEnd", if yes { 1 } else { 2 });
        }
        // 规则书[反击]: 「选择自己的本次移动…是否触发结算」 -- C# `H._resolveOverride[i] =
        // (yes ? 1 : 0)` for every other abnormal.
        _ => {
            ctx::set_slot(player_id, "resolveOverride", if yes { 1 } else { 0 });
        }
    }
    Ok(())
}
