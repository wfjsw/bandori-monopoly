//! `AG:(兰) 像往常一样` -- C# `CardRanAsUsual` (MatchHost.cs:1095-1160):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:(兰) 像往常一样`）:
//! > (兰) 像往常一样：
//!
//! > （1）此卡可以当反击使用
//! >  
//! > （2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。
//!
//! arm a turn-end undo of abnormal movement (counteraction or hand play).

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const RAN_AS_USUAL: CardDef = CardDef::new(
    "AG:(兰) 像往常一样",
    &[
        On::Play("", None, play),
        // G4: kind (Effect) is the category. The condition carries the whole
        // guard body: target rel + `effect.has(Abnormal)` + the once-per-turn
        // slot. Residual guard deleted.
        // TODO(规则书)（2）: 「（包括你的技能）」 also covers an abnormal the user's
        //   *own* skill applies to them, but the engine's `abnormal` chain only opens
        //   when another player caused the effect (`by != target`) -- a self-[传送]
        //   gets no window to answer (same gap as 通用:安可's 「因任何原因」).
        // TODO(规则书)（1）: the C# adds a once-per-turn gate
        //   (`!H.V(player_id, "asUsualTurn").Equals(H.TurnKey)`) that the passage
        //   does not name -- 「此卡可以当反击使用」 has no per-turn limit. Left in
        //   place (it narrows, never broadens) until the book rules on it.
        On::Counteract(
            &[ChainKind::Effect],
            "target == owner && effect.has(Abnormal) && slot('asUsualTurn') != turn_key",
            None,
            counteract,
        ),
        On::AtEnd(at_end),
    ],
)
    .legacy(&[(1, legacy_can_counteract)]);

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    if trigger::kind() != ChainKind::Effect {
        return false;
    }
    if trigger::target() != player_id {
        return false;
    }
    if !ctx::effect::has(TriggerKind::Abnormal) {
        return false;
    }
    ctx::slot(player_id, "asUsualTurn") != ctx::turn_key()
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「此卡可以当反击使用」 -- the C# also plays it from hand (`Play`).
    arm(player_id);
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    arm(player_id);
    Ok(())
}

fn arm(player_id: i32) {
    // C# `H.SetV(i, "asUsualTurn", H.TurnKey)` -- latch for the once-per-turn gate.
    ctx::set_slot(player_id, "asUsualTurn", ctx::turn_key());
    // 规则书（2）: 「受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- the C# `Arm` logs and queues `Undo` on `H._turnCtx.AtEnd`.
    ctx::log(
        player_id,
        &Msg::new(key!("ran_as_usual_armed")).player_id("who", player_id),
    );
    // C# `H._turnCtx.AtEnd.Add(() => Undo(i))` -- the turn-end queue hook
    // (`ctx::before_turn_end` + `On::AtEnd`). The card need not be in play.
    ctx::before_turn_end(player_id);
}

/// C# `CardRanAsUsual.Undo`, queued on `H._turnCtx.AtEnd`. The scheduling half
/// is live; the restore body is held (see the TODO in `arm`).
fn at_end(player_id: i32) -> card_sdk::Asked {
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // `ctx::abnormal_count(player_id)` (C# `H._abnormalTurn[i]`) is the no-op
    // gate: nothing abnormal landed, nothing to undo.
    if ctx::abnormal_count(player_id) <= 0 {
        return Ok(());
    }
    // 规则书（2）: 「回到起始地点并取消所有受到的效果（不进行任何结算）」 --
    // restore the four things the turn-start snapshot recorded (C#
    // `H._turnSnap[i]`). The teleport is a plain position write, so nothing
    // settles on the way back.
    // TODO(规则书)（2）: the C# no-op gate also fires on the `Plan.Reverse` skill
    //   case, which a counter-only gate misses -- a reverse *skill* move is an
    //   abnormal move even though `_abnormalTurn` never counted it.
    let (pos, stay, stun, exile) = ctx::turn_snap(player_id);
    if pos >= 0 {
        ctx::teleport_to(player_id, pos);
    }
    ctx::state::set(player_id, card_sdk::abi::state_key::STAY, stay);
    ctx::state::set(player_id, card_sdk::abi::state_key::STUN, stun);
    ctx::state::set(player_id, card_sdk::abi::state_key::EXILE, exile);
    ctx::log(
        player_id,
        &Msg::new(key!("ran_as_usual_undo"))
            .player_id("who", player_id)
            .tile("tile", pos),
    );
    Ok(())
}
