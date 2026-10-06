//! `skill:纯田真奈:甜甜圈爱好者`（Sumimi）
//!
//! 规则书（skill sheet, 纯田真奈）:
//! > （1）每次[经过]CiRCLE时获得1个[火罐]（初始1，上限2）
//! > （2）经过"购物中心"，"便利店"，"快餐店"，"羽泽咖啡厅"，"山吹面包房"时，在
//! > 触发结算前可选择消耗一个火罐并向对应格子进行一次支付（可向抵押格子正常支付其
//! > 原本收费，若为无主格子则为消耗对应收费资金），之后，直到你的下个回合结束，你的
//! > 资金消耗减半。
//!
//! （2） 「向对应格子进行一次支付」 is a voluntary pay to the tile's owner (or
//! the bank, when it is unowned) *before* the settle -- a pre-emptive charge
//! that buys the half-price window. 「直到你的下个回合结束，你的资金消耗减半」
//! is a pay bend with a `TurnEnd` expiry, which is what the keyed state carries.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The five tiles the clause names.
const SHOPS: [&str; 5] = ["购物中心", "便利店", "快餐店", "羽泽咖啡厅", "山吹面包房"];
/// 「你的资金消耗减半」 -- armed until the next turn end.
const HALF: &str = "skill.manaDonut.half";

pub const MANA_DONUT: CardDef = CardDef::new(
    "skill:纯田真奈:甜甜圈爱好者",
    &[
        On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::Hook(&[HookKind::PayChoose], half, on_pay),
        On::Hook(&[HookKind::TurnEnd], mine, at_turn_end),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「你的资金消耗减半」 -- only this player's own spends.
fn half(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, HALF) > 0
}

/// 「初始1，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if ctx::is_circle(t) {
        ctx::gain_fire(player_id, 1, &Msg::new(key!("mana_donut_gain")));
        return Ok(());
    }
    // （2）「经过"购物中心"，…时，在触发结算前可选择消耗一个火罐并向对应格子
    // 进行一次支付」.
    if !SHOPS.iter().any(|&n| t == ctx::tile_named(n)) {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    let price = ctx::rent_of(t).max(ctx::buy_price(t));
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("mana_donut_title")),
        &Msg::new(key!("mana_donut_ask"))
            .tile("tile", t)
            .i("n", price as i64),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("mana_donut_spend"))) {
        return Ok(());
    }
    // 「可向抵押格子正常支付其原本收费，若为无主格子则为消耗对应收费资金」 --
    // the pay goes to the owner when there is one, and simply leaves otherwise.
    let owner = ctx::tile_owner(t);
    if owner >= 0 && owner != player_id {
        ctx::transfer(player_id, owner, price, &Msg::new(key!("mana_donut_pay")))?;
    } else {
        ctx::pay(player_id, price, &Msg::new(key!("mana_donut_pay")))?;
    }
    // 「之后，直到你的下个回合结束，你的资金消耗减半」
    state::set(player_id, HALF, 1);
    state::set_expires(player_id, HALF, card_sdk::ctx::state::TURN_END);
    ctx::log(player_id, &Msg::new(key!("mana_donut_half")));
    Ok(())
}

/// 「你的资金消耗减半」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    ctx::trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}

fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, HALF, 0);
    Ok(())
}
