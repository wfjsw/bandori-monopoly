//! `skill:高松灯（CRYCHIC）:跌跌撞撞...`
//!
//! 规则书（skill sheet, 高松灯（CRYCHIC））:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）你回合内的任意时刻，可消耗1火罐清除自身一层[停留]
//! > （3）当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算。
//!
//! （2） 「你回合内的任意时刻」 is the whole turn, not one phase -- so the press
//! is open whenever it is this player's turn, which is what the act gate already
//! checks.
//!
//! （3） has two halves on one condition: a [停留] now, and a settle at turn end.
//! The settle is the `settleAtEnd` counter the engine already honours.

use card_sdk::abi::{state_key, HookKind};
use ruleset_core::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

pub const TOMORI_CRYCHIC: CardDef = CardDef::new(
    "skill:高松灯（CRYCHIC）:跌跌撞撞...",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], ruleset_core::guards::OWN_START, None, on_pass),
        On::Hook(&[HookKind::RollAfter], card_sdk::pre::MINE, None, on_roll),
        // The body-top `player_pos < 0` early-out is the condition now: a
        // player not standing on a square has nothing to settle.
        On::AtEnd("owner.pos >= 0", None, settle_now),
    ],
)
    .silent(&[1]);

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("tomori_crychic_gain")))?;
    Ok(())
}

/// （3）「当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let face = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    if face > 6 {
        return Ok(());
    }
    rules_status::stay::add(player_id, 1);
    // 「并在回合结束时触发结算」 -- a scheduled turn-end rule op
    // (`docs/TILES.md`), not an engine counter.
    ctx::before_turn_end(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_crychic_stay")).i("n", face as i64),
    );
    Ok(())
}

/// （2） 「可消耗1火罐清除自身一层[停留]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 1 {
        return Some(Msg::new(key!("tomori_crychic_no_fire")));
    }
    if ctx::state::get(player_id, state_key::STAY) < 1 {
        return Some(Msg::new(key!("tomori_crychic_no_stay")));
    }
    None
}

/// （2）「消耗1火罐清除自身一层[停留]」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("tomori_crychic_spend")))? {
        return Ok(());
    }
    ctx::state::add(player_id, state_key::STAY, -1);
    ctx::log(player_id, &Msg::new(key!("tomori_crychic_cleared")));
    Ok(())
}

/// （3）「并在回合结束时触发结算」 -- `H.SettleAt` on the square the player is
/// standing on (`docs/TILES.md`: a scheduling clause, not an engine counter).
/// Standing on a square is the entry's condition (`owner.pos >= 0`).
fn settle_now(player_id: i32) -> card_sdk::Asked {
    let at = ctx::player_pos(player_id);
    ctx::card_settle_at(player_id, at, false);
    Ok(())
}

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
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_kani(player_id: i32) -> card_sdk::Asked {
    on_roll(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_now_kani(player_id: i32) -> card_sdk::Asked {
    settle_now(player_id)
}
