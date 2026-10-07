//! `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？`
//!
//! 规则书（skill sheet, 丰川祥子（CRYCHIC））:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）
//! > （2）其他人的主要移动结束时，[触发结算]前若在你的前后5格内，可选择向你支付
//! > 300资金并使自己的移动终点沿最短路径向你靠近1格。
//! > （3）你的主要移动结束时，[触发结算]前若前后五格内有别的玩家，你可消耗1火罐，
//! > 选择其中一名玩家并使自己的移动终点沿最短路径向其靠近1格。
//! > （4）若你的乐队技能为"Ave Mujica"，你距离其他玩家5格以内时处于状态2，但只有
//! > 洗牌时可以为你的乐队技能卡上添加奇迹水晶。
//!
//! （2） and （3） are the same shape in opposite directions: at the settle, if
//! someone is within five tiles, the landing may be nudged one tile toward
//! them. 「沿最短路径」 is whichever direction shortens `dist`. The move-end
//! moment is `settleBefore` -- the settle has not run yet.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

pub const SAKI_CRYCHIC: CardDef = CardDef::new(
    "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？",
    &[
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::Hook(&[HookKind::SettleBefore], any, before_settle),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始1，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("saki_crychic_gain")));
    Ok(())
}

/// （2） / （3） -- the landing is within five tiles of the other party.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    let landing = ctx::trigger::tile();
    if landing < 0 || !ctx::trigger::move_is_main() {
        return Ok(());
    }
    let at = ctx::player_pos(player_id);
    if ctx::dist(landing, at) > 5 {
        return Ok(());
    }
    if mover == player_id {
        // （3） 「你可消耗1火罐，选择其中一名玩家并使自己的移动终点沿最短路径
        // 向其靠近1格」.
        if state::get(player_id, state_key::FIRE) < 1 {
            return Ok(());
        }
        if !ctx::ask_yes(
            player_id,
            &Msg::new(key!("saki_crychic_title")),
            &Msg::new(key!("saki_crychic_ask_self")),
        )? {
            return Ok(());
        }
        if !ctx::spend_fire(player_id, 1, &Msg::new(key!("saki_crychic_spend"))) {
            return Ok(());
        }
        nudge(player_id, at);
    } else {
        // （2） 「可选择向你支付300资金并使自己的移动终点沿最短路径向你靠近1格」.
        if !ctx::ask_yes(
            mover,
            &Msg::new(key!("saki_crychic_title")),
            &Msg::new(key!("saki_crychic_ask_other")).player_id("who", player_id),
        )? {
            return Ok(());
        }
        if ctx::transfer(mover, player_id, 300, &Msg::new(key!("saki_crychic_pay")))? <= 0 {
            return Ok(());
        }
        nudge(mover, at);
    }
    Ok(())
}

/// 「使自己的移动终点沿最短路径向…靠近1格」 -- one step toward `goal`.
fn nudge(who: i32, goal: i32) {
    let landing = ctx::trigger::tile();
    if landing < 0 {
        return;
    }
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let fwd = (landing + 1) % n;
    let back = (landing - 1).rem_euclid(n);
    let toward = if ctx::dist(fwd, goal) <= ctx::dist(back, goal) {
        fwd
    } else {
        back
    };
    ctx::plan::set_teleport_to(toward);
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::log(
        who,
        &Msg::new(key!("saki_crychic_nudged")).tile("tile", toward),
    );
}

// （4）「若你的乐队技能为"Ave Mujica"，你距离其他玩家5格以内时处于状态2，但只有
// 洗牌时可以为你的乐队技能卡上添加奇迹水晶」 -- the two-state mechanic the band
// skill owns, and a carve-out on crystal gains the engine does not tag.
// TODO(规则书)[judgement]: （4） -- the clause under-specifies -- the state axis is
//   shared with the Ave Mujica band skill, and 「只有洗牌时可以…添加奇迹水晶」
//   carves a class of gains the engine does not tag.
