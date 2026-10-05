//! `AG:(兰) 像往常一样` -- C# `CardRanAsUsual` (MatchHost.cs:1095-1160):
//! arm a turn-end undo of abnormal movement (reaction or hand play).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:(兰) 像往常一样`）:
//! > (兰) 像往常一样：
//! >
//! > （1）此卡可以当反击使用
//! >
//! > （2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。
//! >

use card_sdk::abi::ChainKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const RAN_AS_USUAL: CardDef = CardDef::new("AG:(兰) 像往常一样", &[
    On::Play(play),
    On::React(&[ChainKind::Effect], can_react, react),
    On::AtEnd(at_end),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书（1）: 「此卡可以当反击使用」 -- C# reacts on an `abnormal` trigger
    // aimed at the player.
    if trigger::kind() != ChainKind::Effect {
        return false;
    }
    if trigger::target() != player_id {
        return false;
    }
    // C# `!H.V(player_id, "asUsualTurn").Equals(H.TurnKey)` -- once per turn.
    ctx::slot(player_id, "asUsualTurn") != ctx::turn_key()
}

fn play(player_id: i32) {
    // 规则书（1）: 「此卡可以当反击使用」 -- the C# also plays it from hand (`Play`).
    arm(player_id);
}

fn react(player_id: i32) {
    arm(player_id);
}

fn arm(player_id: i32) {
    // C# `H.SetV(i, "asUsualTurn", H.TurnKey)` -- latch for the once-per-turn gate.
    ctx::set_slot(player_id, "asUsualTurn", ctx::turn_key());
    // 规则书（2）: 「受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- the C# `Arm` logs and queues `Undo` on `H._turnCtx.AtEnd`.
    ctx::log(player_id, &Msg::new(key!("ran_as_usual_armed")).player_id("who", player_id));
    // C# `H._turnCtx.AtEnd.Add(() => Undo(i))` -- the turn-end queue hook
    // (`ctx::before_turn_end` + `On::AtEnd`). The card need not be in play.
    ctx::before_turn_end(player_id);
    // TODO(规则书)（2）: 「回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- the turn-end queue hook is wired above, and the no-abnormal gate is now
    // `ctx::abnormal_count(player_id)` (C# `H._abnormalTurn[i]`), but the `Undo` body
    // still needs the per-turn start snapshot (C# `H._turnSnap[i]`: pos / stay /
    // stun / exile) and the movement-plan reverse flag (C# `H._turnCtx.Plan.Reverse`,
    // the skill case the counter alone would miss). Until then the player keeps
    // whatever the abnormal move did.
}

/// C# `CardRanAsUsual.Undo`, queued on `H._turnCtx.AtEnd`. The scheduling half
/// is live; the restore body is held (see the TODO in `arm`).
fn at_end(player_id: i32) {
    if ctx::player_out(player_id) {
        return;
    }
    // `ctx::abnormal_count(player_id)` (C# `H._abnormalTurn[i]`) is on the surface
    // now, but the turn-start snapshot and `Plan.Reverse` are not -- and the C#
    // no-op gate also fires on the Plan.Reverse skill case a counter-only gate
    // would miss -- so there is nothing faithful to restore to.
    let _ = player_id;
}
