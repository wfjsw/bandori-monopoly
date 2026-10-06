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
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

pub const TOMORI_CRYCHIC: CardDef = CardDef::new("skill:高松灯（CRYCHIC）:跌跌撞撞...", &[
    On::Play(Some(can_use), use_skill),
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::RollAfter], mine, on_roll),
]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 1);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("tomori_crychic_gain")));
    Ok(())
}

/// （3）「当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let face = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    if face > 6 {
        return Ok(());
    }
    ctx::give_stay(player_id, 1);
    // 「并在回合结束时触发结算」 -- the engine's `settleAtEnd` counter.
    ctx::inc_slot(player_id, "settleAtEnd", 1);
    ctx::log(player_id, &Msg::new(key!("tomori_crychic_stay")).i("n", face as i64));
    Ok(())
}

/// （2） 「可消耗1火罐清除自身一层[停留]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("tomori_crychic_no_fire")));
    }
    if ctx::state::get(player_id, state_key::STAY) < 1 {
        return Some(Msg::new(key!("tomori_crychic_no_stay")));
    }
    None
}

/// （2）「消耗1火罐清除自身一层[停留]」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("tomori_crychic_spend"))) {
        return Ok(());
    }
    ctx::state::add(player_id, state_key::STAY, -1);
    ctx::log(player_id, &Msg::new(key!("tomori_crychic_cleared")));
    Ok(())
}