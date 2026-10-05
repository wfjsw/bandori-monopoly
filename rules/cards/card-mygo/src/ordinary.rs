//! `MyGO:普通与理所当然` -- C# `CardOrdinary` (MatchHost.cs:7087-7113): [反击]
//! after an abnormal-move effect, copy the step count of your last non-teleport
//! main move onto the next one.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:普通与理所当然`）:
//! > 普通与理所当然：
//! >  [反击] 受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const ORDINARY: CardDef = CardDef {
    id: "MyGO:普通与理所当然",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「受到异常移动效果影响后可打出」 -- C# `t.Kind == "abnormal"
    // && t.Target == seat`.
    if trigger::kind() != TriggerKind::Abnormal || trigger::target() != seat {
        return false;
    }
    // C# also requires `H.V(seat, "lastWalk") > 0` -- there must be a previous
    // non-teleport main move whose length can be copied.
    // TODO(ABI): the engine never writes the `lastWalk` slot (C# `NoteWalk`:
    // `SetV(m.Seat, "lastWalk", m.Total + 1)` after each non-teleport main walk),
    // so this guard stays false until it does.
    ctx::slot(seat, "lastWalk") > 0
}

fn react(seat: i32) {
    // 规则书[反击]: 「使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数」
    // C# `NoteWalk` stores `lastWalk = m.Total + 1`; the C# `React` reads back
    // `H.V(seat, "lastWalk") - 1` as the step count.
    let steps = ctx::slot(seat, "lastWalk") - 1;
    ctx::log(seat, &Msg::new(key!("ordinary_log")).i("n", steps as i64));
    // TODO(规则书): 「使你下一次主要移动的格数变为...」 -- needs the NextStepsFx
    // movement override (C# `H.ExtraOf<NextStepsFx>(c.Seat).Steps = steps` over
    // `MoveBefore`) so the next main move uses `steps`; the vocabulary has no
    // move-plan attachment.
}