//! `skill:美竹兰:叛逆的红挑染`
//!
//! 规则书（skill sheet, 美竹兰）:
//! > （1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]
//! > CiRCLE时不获得CiRCLE奖励。
//!
//! （1） is shared by every Afterglow character: 「每三回合没有使用Afterglow角色
//! 的（2）技能获得一个[火罐]」 -- a pot every three turns in which *nobody* in
//! the band pressed their (2). It is a band-wide counter, not this player's, so
//! the latch lives on the band and every Afterglow skill reads it.
//!
//! （2） 「投掷移动步数前」 is `RollPlan`, the moment the dice table is being
//! built. 「选择本次移动向后」 is the plan's reverse; 「[经过]CiRCLE时不获得
//! CiRCLE奖励」 is the plan's no-circle-reward flag.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The band-wide 「每三回合没有使用…（2）技能」 counter. Shared by every
/// Afterglow skill: a press anywhere in the band resets it.
pub const REST_TURNS: &str = "skill.afterglow.restTurns";

pub const RAN_RED: CardDef = CardDef::new("skill:美竹兰:叛逆的红挑染", &[
    On::Play(Some(can_use), use_skill),
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::TurnEnd], afterglow, tick),
]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_band(player_id, "Afterglow")
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) {
    state::set_bounds(player_id, state_key::FIRE, 0, 1);
}

/// （1）「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) {
    let n = state::get(player_id, REST_TURNS) + 1;
    state::set(player_id, REST_TURNS, n);
    if n >= 3 {
        state::set(player_id, REST_TURNS, 0);
        ctx::gain_fire(player_id, 1, &Msg::new(key!("afterglow_rest_gain")));
    }
}

/// （2） 「可使用一个[火罐]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("ran_red_no_fire")));
    }
    None
}

/// （2）「选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励」.
fn use_skill(player_id: i32) {
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("ran_red_spend"))) {
        return;
    }
    // A press of any Afterglow (2) resets the band's rest counter.
    state::set(player_id, REST_TURNS, 0);
    plan::set_reverse(true);
    plan::set_no_circle_reward(true);
    ctx::log(player_id, &Msg::new(key!("ran_red_reverse")));
}