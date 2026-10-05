//! `R:选择自己的舞台` -- C# `CardOwnStage` (MatchHost.cs:10710-10745): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:选择自己的舞台`）:
//! > 选择自己的舞台：
//! > [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算
//!
//! on an abnormal move (not [除外]): choose whether this move (or the end of a
//! movement-locked turn) settles.

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger, AbKind};
use card_sdk::{key, CardDef, On, Msg};

pub const OWN_STAGE: CardDef = CardDef::new("R:选择自己的舞台", &[
    On::CounterAct(&[ChainKind::Effect], can_react, react),
]);

/// 规则书[反击]: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」
fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「受到[除外]以外的异常移动效果影响时」 -- C# `t.Kind == "abnormal" &&
    // t.Target == player && t.Ab != null && t.Ab.Kind != "exile"`.
    if trigger::kind() != ChainKind::Effect || trigger::target() != player_id {
        return false;
    }
    // 规则书[反击]: 「[除外]以外」 -- C# `t.Ab != null && t.Ab.Kind != "exile"`
    // (`trigger::abnormal_kind()` is `t.Ab.Kind`).
    trigger::abnormal_kind().is_some_and(|k| k != AbKind::Exile)
}

fn react(player_id: i32) {
    // 规则书[反击]: 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("own_stage_ask_title")),
        &Msg::new(key!("own_stage_ask_text")),
    );
    let why = if yes {
        Msg::new(key!("own_stage_settle")).player_id("who", player_id)
    } else {
        Msg::new(key!("own_stage_no_settle")).player_id("who", player_id)
    };
    ctx::log(player_id, &why);
    // 规则书[反击]: 「（或无法移动的回合结束时）」 -- C# `H.SetV(i, "stageEnd", yes ? 1 : 2)`
    // when the abnormal is stay/stun (C# `CardOwnStage.React` branches on
    // `c.Trigger.Ab.Kind`).
    match trigger::abnormal_kind() {
        Some(AbKind::Stay) | Some(AbKind::Stun) => {
            ctx::set_slot(player_id, "stageEnd", if yes { 1 } else { 2 });
        }
        // 规则书[反击]: 「选择自己的本次移动…是否触发结算」 -- C# `H._resolveOverride[i] =
        // (yes ? 1 : 0)` for every other abnormal.
        _ => {
            ctx::set_slot(player_id, "resolveOverride", if yes { 1 } else { 0 });
        }
    }
}
