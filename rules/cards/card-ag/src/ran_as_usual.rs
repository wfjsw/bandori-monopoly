//! `AG:(兰) 像往常一样` -- C# `CardRanAsUsual` (MatchHost.cs:1095-1160):
//! arm a turn-end undo of abnormal movement (reaction or hand play).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:(兰) 像往常一样`）:
//! > (兰) 像往常一样：
//! >
//! > （1）此卡可以当反击使用
//! >
//! > （2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const RAN_AS_USUAL: CardDef = CardDef {
    id: "AG:(兰) 像往常一样",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书（1）: 「此卡可以当反击使用」 -- C# reacts on an `abnormal` trigger
    // aimed at the seat.
    if trigger::kind() != TriggerKind::Abnormal {
        return false;
    }
    if trigger::target() != seat {
        return false;
    }
    // C# `!H.V(seat, "asUsualTurn").Equals(H.TurnKey)` -- once per turn.
    ctx::slot(seat, "asUsualTurn") != ctx::turn_key()
}

fn play(seat: i32) {
    // 规则书（1）: 「此卡可以当反击使用」 -- the C# also plays it from hand (`Play`).
    arm(seat);
}

fn react(seat: i32) {
    arm(seat);
}

fn arm(seat: i32) {
    // C# `H.SetV(i, "asUsualTurn", H.TurnKey)` -- latch for the once-per-turn gate.
    ctx::set_slot(seat, "asUsualTurn", ctx::turn_key());
    // 规则书（2）: 「受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- the C# `Arm` only logs and queues `Undo` on `H._turnCtx.AtEnd`.
    ctx::log(seat, &Msg::new(key!("ran_as_usual_armed")).seat("who", seat));
    // TODO(规则书)（2）: 「回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- needs the Fx.TurnEnd / turn-end queue hook (C# `H._turnCtx.AtEnd.Add(Undo)`),
    // the per-turn start snapshot (C# `H._turnSnap[i]`: pos / stay / stun / exile)
    // and the per-turn abnormal-move counter (C# `H._abnormalTurn[i]`), plus the
    // movement-plan reverse flag (`H._turnCtx.Plan.Reverse`) for the skill case.
    // Until then the seat keeps whatever the abnormal move did.
}