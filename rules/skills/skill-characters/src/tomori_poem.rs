//! `skill:高松灯:诗超绊`（MyGO!!!!!）
//!
//! 规则书（skill sheet, 高松灯）:
//! > （1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）
//! > （2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
//! > [触发结算]前强制将该玩家[传送]至自己所在的格子，然后使其投掷1d20，若出目
//! > 小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下
//! > 的支付价格变为原本的一半。
//! > （3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在
//! > [触发结算]前将自己[传送]至那名玩家所在的格子。
//!
//! （2） and （3） are the same shape in opposite directions: someone's landing
//! is within five tiles of the other, and a pot buys a teleport onto the other's
//! square *before* the settle. The 「前后5格内」 window is `dist` <= 5.
//!
//! （2）'s follow-up -- the 1d20, the [停留] on 6 or less, and 「支付价格变为
//! 原本的一半」 -- rides on the forced landing. The half-price is a pay bend on
//! that settle, which is what `PayMul` is.

use card_sdk::abi::{state_key, AbKind, HookKind};
use card_sdk::ctx::{self, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// Settle this player owes at their own next turn end, from （2）.
const DUE: &str = "skill.tomoriPoem.due";

pub const TOMORI_POEM: CardDef = CardDef::new(
    "skill:高松灯:诗超绊",
    &[
        On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::Hook(&[HookKind::SettleBefore], |_| true, before_settle),
        On::Hook(&[HookKind::PayMul], half, on_pay),
        On::Hook(&[HookKind::TurnEnd], mine, at_turn_end),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「在此技能影响下的支付价格变为原本的一半」 -- only the payments of the
/// player this skill moved.
fn half(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, DUE) > 0
}

/// 「初始4，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 4);
    Ok(())
}

/// （1）「每次[经过]任意RiNG时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_ring(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("tomori_poem_gain")));
    Ok(())
}

/// （2） and （3） -- the landing is within five tiles of the other party, and a
/// pot buys a teleport onto their square before the settle.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    let landing = ctx::trigger::tile();
    if landing < 0 || mover == player_id {
        return Ok(());
    }
    // 「[移动终点]位于…前后5格内」
    if ctx::dist(landing, ctx::player_pos(player_id)) > 5 {
        return Ok(());
    }
    let (cost, theirs) = if mover == ctx::turn_player() {
        // （3） is this player's own landing near someone else.
        if ctx::trigger::player_id() != player_id {
            return Ok(());
        }
        (1, ctx::player_pos(player_id))
    } else {
        // （2） is someone else's landing near this player.
        (4, ctx::player_pos(player_id))
    };
    if state::get(player_id, state_key::FIRE) < cost {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_poem_title")),
        &Msg::new(key!("tomori_poem_ask")).i("n", cost as i64),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, cost, &Msg::new(key!("tomori_poem_spend"))) {
        return Ok(());
    }
    if !ctx::gate(mover, AbKind::Teleport) {
        return Ok(());
    }
    if theirs < 0 {
        return Ok(());
    }
    ctx::teleport_to(mover, theirs);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_poem_moved"))
            .player_id("who", mover)
            .tile("tile", theirs),
    );
    if cost == 4 {
        // （2）「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的
        // 下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半」.
        let n = ctx::roll(mover, 1, 20);
        if n <= 6 {
            ctx::give_stay(mover, 1);
            state::set(player_id, DUE, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("tomori_poem_stay")).i("n", n as i64),
            );
        }
    }
    Ok(())
}

/// 「在此技能影响下的支付价格变为原本的一半」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}

/// 「并在自己的下回合结束时[触发结算]」 -- the owed settle.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, DUE) == 0 {
        return Ok(());
    }
    state::set(player_id, DUE, 0);
    let at = ctx::player_pos(player_id);
    if at < 0 {
        return Ok(());
    }
    ctx::card_settle_at(player_id, at, true);
    Ok(())
}
