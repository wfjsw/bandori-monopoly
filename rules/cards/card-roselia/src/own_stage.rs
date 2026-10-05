//! `R:选择自己的舞台` -- C# `CardOwnStage` (MatchHost.cs:10710-10745): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:选择自己的舞台`）:
//! > 选择自己的舞台：
//! > [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算
//!
//! on an abnormal move (not [除外]): choose whether this move (or the end of a
//! movement-locked turn) settles.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const OWN_STAGE: CardDef = CardDef {
    id: "R:选择自己的舞台",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书[反击]: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」
fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「受到[除外]以外的异常移动效果影响时」 -- C# `t.Kind == "abnormal" &&
    // t.Target == seat && t.Ab != null && t.Ab.Kind != "exile"`.
    if trigger::kind() != TriggerKind::Abnormal || trigger::target() != seat {
        return false;
    }
    // TODO(规则书)[反击]: 「[除外]以外」 -- the abnormal trigger carries no `t.Ab.Kind`,
    // so [除外] cannot be filtered out yet (C# `t.Ab.Kind != "exile"`).
    true
}

fn react(seat: i32) {
    // 规则书[反击]: 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
    let yes = ctx::ask_yes(
        seat,
        &Msg::new(key!("own_stage_ask_title")),
        &Msg::new(key!("own_stage_ask_text")),
    );
    let why = if yes {
        Msg::new(key!("own_stage_settle")).seat("who", seat)
    } else {
        Msg::new(key!("own_stage_no_settle")).seat("who", seat)
    };
    ctx::log(seat, &why);
    // 规则书[反击]: 「（或无法移动的回合结束时）」 -- C# `H.SetV(i, "stageEnd", yes ? 1 : 2)`
    // when the abnormal is stay/stun.
    ctx::set_slot(seat, "stageEnd", if yes { 1 } else { 2 });
    // 规则书[反击]: 「选择自己的本次移动…是否触发结算」 -- C# `H._resolveOverride[i] =
    // (yes ? 1 : 0)` for every other abnormal.
    ctx::set_slot(seat, "resolveOverride", if yes { 1 } else { 0 });
    // TODO(规则书)[反击]: the two slots are written for both branches because the
    // abnormal trigger carries no `t.Ab.Kind` to tell stay/stun from the rest (C#
    // `CardOwnStage.React`); the engine must honour `stageEnd` / `resolveOverride`.
}