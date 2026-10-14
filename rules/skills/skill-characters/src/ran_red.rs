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
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The band-wide 「每三回合没有使用…（2）技能」 counter. Shared by every
/// Afterglow skill: a press anywhere in the band resets it.
pub const REST_TURNS: &str = "skill.afterglow.restTurns";

pub const RAN_RED: CardDef = CardDef::new(
    "skill:美竹兰:叛逆的红挑染",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::TurnEnd], "", Some(afterglow), tick),
    ],
).silent(&[1]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_band(player_id, "Afterglow")
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    // （2）'s 「向后移动[经过]CiRCLE时不获得CiRCLE奖励」 ends with the turn (the
    // reward step consumes the prop if the move did pass CiRCLE).
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 0);
    rest_tick(player_id)
}

/// （1）「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]」, run at each
/// Afterglow owner's turn end. Every outcome is logged: the count, the pot
/// gained (the engine's own gain line), or a gain the cap refused.
pub fn rest_tick(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, REST_TURNS) + 1;
    if n < 3 {
        state::set(player_id, REST_TURNS, n);
        ctx::log(player_id, &Msg::new(key!("afterglow_rest_count")).i("n", n as i64));
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    if rules_status::fire::get(player_id) >= rules_status::fire::max(player_id) {
        ctx::log(
            player_id,
            &Msg::new(key!("afterglow_rest_capped")).i("max", rules_status::fire::max(player_id) as i64),
        );
        return Ok(());
    }
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("afterglow_rest_gain")))?;
    Ok(())
}

/// （2） 「可使用一个[火罐]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("ran_red_no_fire")));
    }
    None
}

/// （2）「选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("ran_red_spend")))? {
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

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::CharacterDef =
ruleset_core::roster::CharacterDef::new(r#"美竹兰"#)
    .display(r#"美竹兰"#)
    .band(r#"Afterglow"#)
    .color(r#"#EE0022"#)
    .skill(r#"叛逆的红挑染"#)
    .text(r#"（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励。"#)
    .cn_id(r#"006"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"像往常一样"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

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
