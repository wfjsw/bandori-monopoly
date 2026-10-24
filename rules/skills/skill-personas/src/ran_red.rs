//! `skill:美竹兰:叛逆的红挑染`
//!
//! 规则书（skill sheet, 美竹兰）:
//! > （1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]
//! > CiRCLE时不获得CiRCLE奖励。
//!
//! （1） is shared by every Afterglow persona: 「每四回合没有使用Afterglow角色
//! 的（2）技能获得一个[火罐]」 -- a pot every three turns in which *nobody* in
//! the band pressed their (2). It is a band-wide counter, not this player's, so
//! the latch lives on the band and every Afterglow skill reads it.
//!
//! （2） 「投掷移动步数前」 is `DiceRollPlan`, the moment the dice table is being
//! built. 「选择本次移动向后」 is the plan's reverse; 「[经过]CiRCLE时不获得
//! CiRCLE奖励」 is the plan's no-circle-reward flag.

use bandori_std::ctx::{self, plan, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The band-wide 「每四回合没有使用…（2）技能」 counter. Shared by every
/// Afterglow skill: a press anywhere in the band resets it.
pub const REST_TURNS: &str = "skill.afterglow.restTurns";

pub const RAN_RED: CardDef = CardDef::new(
    "skill:美竹兰:叛逆的红挑染",
    &[
        On::Play("", Some(can_use), use_skill).label("（2）"),
        On::Action(Action::TurnStartBefore("", None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)),
        On::Action(Action::TurnEnd("", Some(afterglow), tick)),
    ],
)
.silent(&[1, 2])
// 「（初始1，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 1),
    (bandori_std::fire::FIRE_CAP, 1),
]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_group(player_id, "Afterglow")
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    // （2）'s 「向后移动[经过]CiRCLE时不获得CiRCLE奖励」 ends with the turn (the
    // reward step consumes the prop if the move did pass CiRCLE).
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 0);
    rest_tick(player_id)
}

/// （1）「每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]」, run at each
/// Afterglow owner's turn end. Every outcome is logged: the count, the pot
/// gained (the engine's own gain line), or the engine's cap-refused line.
pub fn rest_tick(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, REST_TURNS) + 1;
    if n < 4 {
        state::set(player_id, REST_TURNS, n);
        ctx::log(
            player_id,
            &Msg::new(key!("afterglow_rest_count")).i("n", n as i64),
        );
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("afterglow_rest_gain")))?;
    Ok(())
}

/// （2） 「可使用一个[火罐]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 1 {
        return Some(Msg::new(key!("ran_red_no_fire")));
    }
    None
}

/// （2）「选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("ran_red_spend")))? {
        return Ok(());
    }
    // A press of any Afterglow (2) resets the band's rest counter.
    state::set(player_id, REST_TURNS, 0);
    plan::set_reverse(true);
    // （2）「向后移动[经过]CiRCLE时不获得CiRCLE奖励」 -- `prop::NO_REWARD` on this
    // instance (`docs/TILES.md`); `tick` disarms it at the turn end.
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 1);
    ctx::log(player_id, &Msg::new(key!("ran_red_reverse")));
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn tick_kani(player_id: i32) -> card_sdk::Asked {
    tick(player_id)
}
