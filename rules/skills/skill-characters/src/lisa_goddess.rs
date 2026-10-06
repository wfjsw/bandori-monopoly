//! `skill:今井莉莎:慈爱女神`
//!
//! 规则书（skill sheet, 今井莉莎）:
//! > （1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）
//! > （2）移动阶段前，你可以消耗一个火罐，将此次移动变为传送到距离你正向距离最近的
//! > 角色所在的格子，触发结算。本次传送触发结算前，你可以额外消耗一个火罐或使与你
//! > 在同一个格子上的另一个角色获得一个可超过上限的临时火罐，若你这样做，此次传送
//! > 不触发任何结算。
//! > （3）你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐需要支付你600
//! > 资金（被冲榜类效果被动消耗无需支付）。
//!
//! （1） is `Pass` onto CiRCLE.
//!
//! （2） is a teleport to the nearest character *ahead*, and then an optional
//! second cost that cancels the settle -- 「若你这样做，此次传送不触发任何结算」.
//! Both halves are one press; the second is offered while the move is still
//! being planned.
//!
//! （3） is the temporary pot: a counter on another player that expires and whose
//! *use* pays this player 600. That is an attachment with a cost attached to its
//! use, which is a shape the keyed state can hold (a value plus an expiry) but
//! whose 「使用…需要支付你600资金」 needs the spend path to know the pot's
//! provenance. See the TODO below.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

pub const LISA_GODDESS: CardDef = CardDef::new(
    "skill:今井莉莎:慈爱女神",
    &[
        On::Play(Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
        On::Hook(&[HookKind::Pass], mine, on_pass),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("lisa_goddess_gain")));
    Ok(())
}

/// （2） 「你可以消耗一个火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("lisa_goddess_no_fire")));
    }
    None
}

/// （2）「将此次移动变为传送到距离你正向距离最近的角色所在的格子，触发结算」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「距离你正向距离最近的角色」 -- the nearest character ahead.
    let at = ctx::player_pos(player_id);
    let n = ctx::tile_count();
    let mut best = i32::MAX;
    let mut to = -1;
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) {
            continue;
        }
        let d = ctx::tile_forward(at, ctx::player_pos(p));
        if d > 0 && d < best {
            best = d;
            to = ctx::player_pos(p);
        }
    }
    if to < 0 {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("lisa_goddess_spend"))) {
        return Ok(());
    }
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(to);
    plan::set_resolve(true);
    ctx::log(
        player_id,
        &Msg::new(key!("lisa_goddess_moved")).tile("tile", to),
    );
    // 「本次传送触发结算前，你可以额外消耗一个火罐或使与你在同一个格子上的另一个
    // 角色获得一个可超过上限的临时火罐，若你这样做，此次传送不触发任何结算」 --
    // offered here, before the move runs.
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    let here: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p) && ctx::player_pos(p) == to)
        .collect();
    if here.is_empty() {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("lisa_goddess_cancel_title")),
        &Msg::new(key!("lisa_goddess_cancel_ask")),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("lisa_goddess_spend"))) {
        return Ok(());
    }
    // 「若你这样做，此次传送不触发任何结算」
    plan::set_resolve(false);
    // 「使与你在同一个格子上的另一个角色获得一个可超过上限的临时火罐」
    let who = here[0];
    state::add(who, "tempFire", 1);
    state::set_max(who, "tempFire", i32::MAX);
    // TODO(规则书)[judgement]: （3）「你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐
    //   需要支付你600资金」 -- the clause under-specifies -- the spend path has to
    //   know a pot's *provenance* to charge the provider, and 「被冲榜类效果被动
    //   消耗无需支付」 carves out a class of spends the engine does not tag.
    ctx::log(
        player_id,
        &Msg::new(key!("lisa_goddess_temp")).player_id("who", who),
    );
    Ok(())
}
