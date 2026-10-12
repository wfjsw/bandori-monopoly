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
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「每回合可选择只获得第一次受到的[异常移动效果]」 -- 1 once one has landed.
const TAKEN: &str = "skill.ras.taken";
/// The livehouse tile the settle latched.
const ON_HOUSE: &str = "skill.ras.onHouse";

pub const RAS: CardDef = CardDef::new(
    "skill:RAISE A SUILEN:UNSTOPPABLE",
    &[
        // Turn-start bookkeeping: clear the per-turn latch and drop a stale
        // landing memory. Silent -- the player never sees this as a clause.
        On::Hook(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start_reset),
        On::Hook(&[HookKind::Abnormal], card_sdk::pre::MINE, None, on_abnormal),
        On::Hook(&[HookKind::Settle], card_sdk::pre::MINE, None, on_settle),
        // （2）「若你的下回合开始时仍在那个格子上」 -- the teleport offer.
        // The remembered-tile scan is a derived read (docs/GUARDS.md §4.2).
        On::Hook(
            &[HookKind::TurnStartBefore],
            card_sdk::pre::MINE,
            Some(still_on_remembered),
            at_turn_start_offer,
        ),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine), (2, legacy_mine)]).silent(&[0, 2]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// Turn-start bookkeeping: always clear the per-turn latch; drop a stale
/// landing memory when the player has moved off it.
fn at_turn_start_reset(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, TAKEN, 0);
    let t = state::get(player_id, ON_HOUSE);
    if t >= 0 && ctx::player_pos(player_id) != t {
        state::set(player_id, ON_HOUSE, -1);
    }
    Ok(())
}

/// Owned Live Houses the （2） teleport can land on. Residual-only (per-player
/// colour via `is_live_house_for`, outside the CEL schema -- docs/GUARDS.md §4.2).
fn owned_livehouses(player_id: i32) -> alloc::vec::Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&u| ctx::is_live_house_for(player_id, u))
        .collect()
}

/// Residual guard for the （2） teleport offer: still standing on the
/// remembered livehouse, and owning at least one Live House to teleport to.
/// Derived scan (docs/GUARDS.md §4.2).
///
/// 规则书（band sheet, RAISE A SUILEN）:
/// > （2）因该技能以外的效果在livehouse格子[触发结算]时，若你的下回合开始时
/// > 仍在那个格子上，你的下一次主要移动可变为传送至你拥有的一个livehouse格子。
///
/// The offer is 「可变为传送至你拥有的一个livehouse格子」 -- it exists only when
/// such a destination exists. A player who owns no Live House has nothing the
/// offer can name, so the entry must not activate (no prompt, no flash, no log).
fn still_on_remembered(player_id: i32) -> bool {
    let t = state::get(player_id, ON_HOUSE);
    t >= 0
        && ctx::player_pos(player_id) == t
        && owned_livehouses(player_id).first().is_some()
}

/// （2）「若你的下回合开始时仍在那个格子上」 -- the teleport offer.
/// Destination presence is the residual guard's job; this body only picks one.
fn at_turn_start_offer(player_id: i32) -> card_sdk::Asked {
    let t = state::get(player_id, ON_HOUSE);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("ras_title")),
        &Msg::new(key!("ras_teleport")).tile("tile", t),
    )? {
        return Ok(());
    }
    // 「你的下一次主要移动可变为传送至你拥有的一个livehouse格子」
    let Some(&to) = owned_livehouses(player_id).first() else {
        // Guard already admitted a destination; keep the accept path total.
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
