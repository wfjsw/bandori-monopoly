//! `skill:RAISE A SUILEN:UNSTOPPABLE`
//!
//! 规则书（band sheet, RAISE A SUILEN）:
//! > （1）每回合可选择只获得第一次受到的[异常移动效果]。（2）因该技能以外的效果在
//! > livehouse格子[触发结算]时，若你的下回合开始时仍在那个格子上，你的下一次主要
//! > 移动可变为传送至你拥有的一个livehouse格子。
//!
//! （1） is a per-turn claim: once the first abnormal movement effect has landed
//! this turn, the rest are dropped. The abnormal gate is where that is decided.
//!
//! （2） latches a livehouse settle, then at the next turn start offers the
//! teleport if the player is still standing there.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「每回合可选择只获得第一次受到的[异常移动效果]」 -- 1 once one has landed.
const TAKEN: &str = "skill.ras.taken";
/// The livehouse tile the settle latched.
const ON_HOUSE: &str = "skill.ras.onHouse";

pub const RAS: CardDef = CardDef::new(
    "skill:RAISE A SUILEN:UNSTOPPABLE",
    &[
        On::Hook(&[HookKind::TurnStartBefore], None, at_turn_start, card_sdk::pre::MINE),
        On::Hook(&[HookKind::Abnormal], None, on_abnormal, card_sdk::pre::MINE),
        On::Hook(&[HookKind::Settle], None, on_settle, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, TAKEN, 0);
    // （2）「若你的下回合开始时仍在那个格子上」
    let t = state::get(player_id, ON_HOUSE);
    if t < 0 || ctx::player_pos(player_id) != t {
        state::set(player_id, ON_HOUSE, -1);
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("ras_title")),
        &Msg::new(key!("ras_teleport")).tile("tile", t),
    )? {
        return Ok(());
    }
    // 「你的下一次主要移动可变为传送至你拥有的一个livehouse格子」
    let houses: alloc::vec::Vec<i32> = ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&u| ctx::is_live_house_for(player_id, u))
        .collect();
    let Some(&to) = houses.first() else {
        return Ok(());
    };
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(to);
    plan::set_resolve(true);
    state::set(player_id, ON_HOUSE, -1);
    ctx::log(player_id, &Msg::new(key!("ras_moved")).tile("tile", to));
    Ok(())
}

/// （1）「每回合可选择只获得第一次受到的[异常移动效果]」 -- once one has landed
/// this turn, the rest are dropped.
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, TAKEN) == 0 {
        state::set(player_id, TAKEN, 1);
        return Ok(());
    }
    ctx::trigger::set_cancelled();
    Ok(())
}

/// （2）「因该技能以外的效果在livehouse格子[触发结算]时」.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::by_card().is_some() {
        // 「因该技能以外的效果」 -- a card-driven settle still counts; what is
        // excluded is this skill's own teleport landing.
        return Ok(());
    }
    let t = ctx::trigger::tile();
    if t < 0 || !ctx::is_live_house_for(player_id, t) {
        return Ok(());
    }
    state::set(player_id, ON_HOUSE, t);
    Ok(())
}
