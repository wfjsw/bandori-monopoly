//! `skill:羽泽鸫:伟大的平凡`
//!
//! 规则书（skill sheet, 羽泽鸫）:
//! > （1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）当你进行主要移动时，你可以失去一个[火罐]使本次移动反向，如果你这样做
//! > 了，你的下回合开始时，进行一次双倍掷骰的移动，向后移动经过CiRCLE时不获得
//! > CiRCLE奖励。
//!
//! （1） is the shared Afterglow rest counter (see [`super::ran_red`]).
//!
//! （2） has a present half and a deferred half. The present half is the reverse;
//! the deferred half -- 「下回合开始时，进行一次双倍掷骰的移动，向后移动经过
//! CiRCLE时不获得CiRCLE奖励」 -- is a *queued* move that runs at the next turn
//! start. A deferred movement is a value the keyed state can hold (「one extra
//! move at turn start」) and the engine reads; see the TODO below for the piece
//! that is not expressible yet.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

use super::ran_red::REST_TURNS;

/// 「下回合开始时，进行一次双倍掷骰的移动」 is armed here.
const OWED: &str = "skill.tsugumiPlain.owed";

pub const TSUGUMI_PLAIN: CardDef = CardDef::new(
    "skill:羽泽鸫:伟大的平凡",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::TurnEnd], "", Some(afterglow), tick),
        On::Hook(&[HookKind::RollPlan], "actor == owner && slot('skill.tsugumiPlain.owed') != 0", None, on_plan),
    ],
)
    .legacy(&[(3, legacy_mine)]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_band(player_id, "Afterglow")
}

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    // The owed move's 「不获得CiRCLE奖励」 clause ends with the turn (the
    // reward step consumes the prop if the move did pass CiRCLE).
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 0);
    let n = state::get(player_id, REST_TURNS) + 1;
    state::set(player_id, REST_TURNS, n);
    if n >= 3 {
        state::set(player_id, REST_TURNS, 0);
        ctx::gain_fire(player_id, 1, &Msg::new(key!("afterglow_rest_gain")))?;
    }
    Ok(())
}

/// 「当你进行主要移动时」 -- the (2) press is offered while the move is being
/// planned.
fn on_plan(player_id: i32) -> card_sdk::Asked {
    // 「进行一次双倍掷骰的移动，向后移动经过CiRCLE时不获得CiRCLE奖励」 --
    // the owed move's own shape. The CiRCLE veto is `prop::NO_REWARD` on this
    // instance (`docs/TILES.md`); `tick` disarms it at the turn end, when the
    // move is over.
    state::set(player_id, OWED, 0);
    plan::set_base_dice(2, 20, "伟大的平凡");
    plan::set_reverse(true);
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 1);
    ctx::at_turn_end(player_id);
    ctx::log(player_id, &Msg::new(key!("tsugumi_plain_owed")));
    Ok(())
}

fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("tsugumi_plain_no_fire")));
    }
    None
}

/// （2）「使本次移动反向」 + arm the deferred half.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("tsugumi_plain_spend")))? {
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    plan::set_reverse(true);
    // 「你的下回合开始时，进行一次双倍掷骰的移动」 -- armed for the next
    // `RollPlan` of this player's own turn.
    state::set(player_id, OWED, 1);
    ctx::log(player_id, &Msg::new(key!("tsugumi_plain_reverse")));
    Ok(())
}
