//! `skill:Poppin' Party:星之鼓动`
//!
//! 规则书（band sheet, Poppin' Party）:
//! > （1）[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸，随后可使用2个星星贴纸
//! > 为此卡添加1个[奇迹水晶]，随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金
//! > （使用此卡[奇迹水晶]的效果为1回合1次）。（2）无法获取[CiRCLE奖励]（3）游戏开始
//! > 时所有Poppin' Party角色视为同时拥有"星之鼓动山丘"，可正常抵押"星之鼓动山丘"，且
//! > 非Poppin' Party角色对"星之鼓动山丘"的[结算]改为分摊支付给所有未抵押"星之鼓动
//! > 山丘"的Poppin' Party角色。"星之鼓动山丘"不可被抵押双倍支付购买，只有全部
//! > Poppin' Party角色破产后才可被正常购买。（4）不能通过[主要移动]的[结算]盖房。
//!
//! 「星星贴纸」 is a player counter; 「为此卡添加1个[奇迹水晶]」 is a band-card
//! crystal. （3） is the hill's ownership shared across the band.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:Poppin' Party:星之鼓动";
/// 「星星贴纸」.
const STICKER: &str = "星星贴纸";
/// The tiles （1） names, by number.
const SPOTS: [i32; 4] = [1, 16, 31, 46];
/// 「使用此卡[奇迹水晶]的效果为1回合1次」.
const USED: &str = "skill.poppin.used";

pub const POPPIN: CardDef = CardDef::new("skill:Poppin' Party:星之鼓动", &[
    On::Hook(&[HookKind::TurnStartBefore], mine, at_turn_start),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::SettleBefore], any, before_settle),
    On::Play(Some(can_crystal), to_crystal),
    On::Play(Some(can_cash), cash)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

fn at_turn_start(player_id: i32) {
    state::set(player_id, USED, 0);
    // （4）「不能通过[主要移动]的[结算]盖房」.
    state::set(player_id, state_key::NO_BUILD, 1);
}

/// （1）「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」.
fn on_pass(player_id: i32) {
    if !SPOTS.contains(&ctx::trigger::tile()) {
        return;
    }
    ctx::add_tok(player_id, STICKER, 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("poppin_sticker")).tile("tile", ctx::trigger::tile()));
}

/// （2）「无法获取[CiRCLE奖励]」, and （3）'s rent split.
fn before_settle(player_id: i32) {
    if ctx::trigger::player_id() == player_id {
        // （2） -- this player's own pass earns nothing.
        if ctx::is_circle(ctx::trigger::tile()) {
            plan::set_no_circle_reward(true);
        }
        // （4） -- no building off a main-move settle.
        if ctx::trigger::move_is_main() {
            plan::set_can_build(false);
        }
        return;
    }
    // （3）「非Poppin' Party角色对"星之鼓动山丘"的[结算]改为分摊支付给所有
    // 未抵押"星之鼓动山丘"的Poppin' Party角色」
    let hill = ctx::tile_named("星之鼓动山丘");
    let t = ctx::trigger::tile();
    if hill < 0 || t != hill {
        return;
    }
    if ctx::in_band(ctx::trigger::player_id(), "Poppin' Party") {
        return;
    }
    let mut payees: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    for p in ctx::others(player_id) {
        if ctx::in_band(p, "Poppin' Party") && !ctx::mortgaged_of(hill) {
            payees.push(p);
        }
    }
    if payees.is_empty() {
        return;
    }
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return;
    }
    let each = amount / (payees.len() as i32).max(1);
    ctx::trigger::set_pay_amount(0);
    let from = ctx::trigger::player_id();
    for p in payees {
        ctx::transfer(from, p, each, &Msg::new(key!("poppin_share")));
    }
    ctx::log(player_id, &Msg::new(key!("poppin_shared")).i("n", each as i64));
}

/// （1）「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」.
fn can_crystal(player_id: i32) -> Option<Msg> {
    if ctx::tok(player_id, STICKER) < 2 {
        return Some(Msg::new(key!("poppin_no_sticker")));
    }
    None
}

fn to_crystal(player_id: i32) {
    if ctx::tok(player_id, STICKER) < 2 {
        return;
    }
    ctx::add_tok(player_id, STICKER, -2, i32::MAX);
    ctx::add_card_crystals(player_id, ID, 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("poppin_crystal")));
}

/// （1）「随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金（…为1回合1次）」.
fn can_cash(player_id: i32) -> Option<Msg> {
    if state::get(player_id, USED) != 0 {
        return Some(Msg::new(key!("poppin_once")));
    }
    if ctx::card_crystals(player_id, ID) < 2 {
        return Some(Msg::new(key!("poppin_no_crystal")));
    }
    None
}

fn cash(player_id: i32) {
    if state::get(player_id, USED) != 0 || ctx::card_crystals(player_id, ID) < 2 {
        return;
    }
    ctx::add_card_crystals(player_id, ID, -2, i32::MAX);
    state::set(player_id, USED, 1);
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("poppin_title")),
        &Msg::new(key!("poppin_which")),
    ) {
        ctx::draw(player_id, 1);
    } else {
        ctx::gain(player_id, 2000, &Msg::new(key!("poppin_cash")));
    }
}

// （3）「所有Poppin' Party角色视为同时拥有"星之鼓动山丘"，可正常抵押"星之鼓动
// 山丘"」 and 「"星之鼓动山丘"不可被抵押双倍支付购买，只有全部Poppin' Party角色
// 破产后才可被正常购买」 -- shared ownership of one tile across every band member,
// with a mortgage each and a purchase lock.
// TODO(规则书)（3）: 「所有Poppin' Party角色视为同时拥有"星之鼓动山丘"」 -- a tile
//   has one owner in `st.owners`; co-ownership across every band member (and a
//   per-member mortgage of the same tile) has no form. The rent split above is
//   written against the single-owner shape.
// TODO(规则书)（3）: 「"星之鼓动山丘"不可被抵押双倍支付购买，只有全部Poppin' Party
//   角色破产后才可被正常购买」 -- a purchase lock keyed on *every* band member
//   being bankrupt. `ctx::player_out` is per player; the "all of them" roll-up is
//   a loop, but the "不可被抵押双倍支付购买" carve-out is a buy-gate the buy path
//   does not consult.