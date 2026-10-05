//! `skill:Hello, Happy World!:传播笑容`
//!
//! 规则书（band sheet, Hello, Happy World!）:
//! > （1）向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家并使此卡获得
//! > 一个奇迹水晶。（2）被记录的玩家的格子产生收款时，移除此卡的一个奇迹水晶，记录
//! > 该次收款的价格（可累加）。（3）你进行盖房时，可从盖房花费中减去所记录的数字，
//! > 然后将记录数字归零。
//!
//! 「记录该玩家」 is one named player; 「记录该次收款的价格（可累加）」 is a running
//! total this card holds. （3） spends that total against a build.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:Hello, Happy World!:传播笑容";
/// The recorded player, or -1.
const WHO: &str = "skill.hhw.who";
/// 「记录该次收款的价格（可累加）」.
const SAVED: &str = "skill.hhw.saved";

pub const HHW: CardDef = CardDef::new("skill:Hello, Happy World!:传播笑容", &[
    On::Hook(&[HookKind::PayChoose], mine, on_pay_choose),
    On::Hook(&[HookKind::PayAfter], mine, after_pay),
    On::Hook(&[HookKind::BuildBefore], mine, before_build)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「向他人的格子付款时可选择支付双倍价格」.
fn on_pay_choose(player_id: i32) {
    let to = ctx::trigger::target();
    if to < 0 || to == player_id {
        return;
    }
    if !ctx::trigger::pay_is_rent() {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hhw_title")),
        &Msg::new(key!("hhw_double")).player_id("who", to),
    ) {
        return;
    }
    let amount = ctx::trigger::value();
    ctx::trigger::set_pay_amount(amount * 2);
    // 「记录该玩家并使此卡获得一个奇迹水晶」
    state::set(player_id, WHO, to);
    ctx::add_card_crystals(player_id, ID, 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("hhw_recorded")).player_id("who", to));
}

/// （2）「被记录的玩家的格子产生收款时，移除此卡的一个奇迹水晶，记录该次收款的
/// 价格（可累加）」.
fn after_pay(player_id: i32) {
    let who = state::get(player_id, WHO);
    if who < 0 {
        return;
    }
    // The recorded player collected a rent.
    if ctx::trigger::player_id() != who {
        return;
    }
    if !ctx::trigger::pay_is_rent() {
        return;
    }
    if ctx::card_crystals(player_id, ID) < 1 {
        return;
    }
    ctx::add_card_crystals(player_id, ID, -1, i32::MAX);
    let v = ctx::trigger::value();
    state::set(player_id, SAVED, state::get(player_id, SAVED) + v);
    ctx::log(player_id, &Msg::new(key!("hhw_saved")).i("n", v as i64));
}

/// （3）「你进行盖房时，可从盖房花费中减去所记录的数字，然后将记录数字归零」.
fn before_build(player_id: i32) {
    let saved = state::get(player_id, SAVED);
    if saved <= 0 {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hhw_title")),
        &Msg::new(key!("hhw_spend")).i("n", saved as i64),
    ) {
        return;
    }
    ctx::set_build_discount(saved, 1);
    state::set(player_id, SAVED, 0);
    ctx::log(player_id, &Msg::new(key!("hhw_built")).i("n", saved as i64));
}