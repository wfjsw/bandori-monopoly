//! `skill:山吹沙绫:焕然一新的天空中`
//!
//! 规则书（skill sheet, 山吹沙绫）:
//! > （1）其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名
//! > 玩家获得200资金且自己获得1个saaya标记和1个[火罐]（初始3，上限5）；拥有saaya
//! > 标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动
//! > 到下一个可购买格子）
//! > （2）[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。
//!
//! （1） has two halves and they trade on the same mark: a pay of at least 1,000
//! from someone else buys a mark plus a pot (and 200 for them); holding a mark
//! shaves 1d10 off your next move roll. 「计算后小等于0则移动到下一个可购买
//! 格子」 is the floor: a shave that empties the face sends the mover to the
//! next buyable tile instead.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const MARK: &str = "saaya标记";

pub const SAAYA_SKY: CardDef = CardDef::new("skill:山吹沙绫:焕然一新的天空中", &[
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::PayAfter], other, on_pay_after),
    On::Hook(&[HookKind::RollAfter], mine, on_roll),
    On::Hook(&[HookKind::PayChoose], mine, on_pay)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// 「初始3，上限5」.
fn declare_cap(player_id: i32) {
    state::set_bounds(player_id, state_key::FIRE, 0, 5);
}

/// （1）「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名
/// 玩家获得200资金且自己获得1个saaya标记和1个[火罐]」.
fn on_pay_after(player_id: i32) {
    if ctx::tok(player_id, MARK) > 0 {
        return;
    }
    let amount = ctx::trigger::value();
    if amount < 1000 {
        return;
    }
    let payer = ctx::trigger::player_id();
    if payer < 0 || payer == player_id {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("saaya_sky_title")),
        &Msg::new(key!("saaya_sky_ask")).player_id("who", payer),
    ) {
        return;
    }
    ctx::add_tok(player_id, MARK, 1, 1);
    ctx::gain_fire(player_id, 1, &Msg::new(key!("saaya_sky_gain")));
    if !ctx::player_out(payer) {
        ctx::gain(payer, 200, &Msg::new(key!("saaya_sky_payee")));
    }
}

/// （1）「拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10
/// （计算后小等于0则移动到下一个可购买格子）」.
fn on_roll(player_id: i32) {
    if ctx::tok(player_id, MARK) < 1 {
        return;
    }
    ctx::add_tok(player_id, MARK, -1, 1);
    let cut = ctx::roll(player_id, 1, 10).max(0);
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    let after = before - cut;
    if after <= 0 {
        // 「计算后小等于0则移动到下一个可购买格子」 -- the face is empty, so the
        // move goes to the next buyable tile instead.
        let at = ctx::player_pos(player_id);
        let n = ctx::tile_count();
        let mut to = -1;
        for k in 1..n {
            let t = (at + k).rem_euclid(n);
            if ctx::is_buyable(t) {
                to = t;
                break;
            }
        }
        if to >= 0 {
            ctx::plan::set_teleport_to(to);
            ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
            ctx::trigger::set_move_roll(0);
        }
        ctx::log(player_id, &Msg::new(key!("saaya_sky_floor")).tile("tile", to));
        return;
    }
    ctx::trigger::set_move_roll(after);
    ctx::log(player_id, &Msg::new(key!("saaya_sky_cut")).i("n", cut as i64).i("total", after as i64));
}

/// （2）「[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）」.
fn on_pay(player_id: i32) {
    let amount = ctx::trigger::value();
    if amount <= 0 || state::get(player_id, state_key::FIRE) < 5 {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("saaya_sky_cut_title")),
        &Msg::new(key!("saaya_sky_cut_ask")).i("n", amount as i64),
    ) {
        return;
    }
    if ctx::spend_fire(player_id, 5, &Msg::new(key!("saaya_sky_spend"))) {
        ctx::trigger::set_pay_amount((amount - 5000).max(0));
    }
}