//! `MyGO:普通与理所当然` -- C# `CardOrdinary` (MatchHost.cs:7087-7113): [反击]
//! after an abnormal-move effect, copy the step count of your last non-teleport
//! main move onto the next one.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:普通与理所当然`）:
//! > 普通与理所当然：
//! >  [反击] 受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const ORDINARY: CardDef = CardDef::new("MyGO:普通与理所当然", &[
    On::React(&[ChainKind::Effect], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「受到异常移动效果影响后可打出」 -- C# `t.Kind == "abnormal"
    // && t.Target == player`.
    if trigger::kind() != ChainKind::Effect || trigger::target() != player_id {
        return false;
    }
    // C# also requires `H.V(seat, "lastWalk") > 0` -- there must be a previous
    // non-teleport main move whose length can be copied. The engine writes the
    // slot (`NoteWalk`: `SetV(player_id, "lastWalk", steps + 1)` after each
    // non-teleport main walk; 0 means "none").
    ctx::slot(player_id, "lastWalk") > 0
}

fn react(player_id: i32) {
    // 规则书[反击]: 「使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的
    // 移动格数」 -- C# `React` reads `H.V(seat, "lastWalk") - 1` (the `NoteWalk`
    // count) and installs it as `H.ExtraOf<NextStepsFx>(c.Seat).Steps = steps`,
    // which `MoveBefore` applies to the next main move. `ctx::set_next_steps` is
    // that NextStepsFx: the engine takes the stored count when planning the move.
    let steps = ctx::slot(player_id, "lastWalk") - 1;
    ctx::set_next_steps(player_id, steps);
    ctx::log(player_id, &Msg::new(key!("ordinary_log")).i("n", steps as i64));
}