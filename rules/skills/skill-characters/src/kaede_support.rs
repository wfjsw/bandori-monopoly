//! `skill:八幡海铃:熟练的支援贝斯手`
//!
//! 规则书（skill sheet, 八幡海铃）:
//! > （1）状态1：其他玩家可以在他们的回合开始时向你支付400资金指定一个属于你的格子，
//! > 使其立即对前后一格内的一名玩家进行一次半价[结算]，并获得相当于你获取数额的资金；
//! > 直到这两名玩家的下回合结束，若你仍在状态1，他们不会被该效果再次收款。每两个回合
//! > 没有玩家发动该技能，你在回合开始时获得1火罐（初始0，上限4），若达到上限，可在
//! > 你的回合开始时选择进入状态2。
//! > （2） 状态2： 进入状态2时你可消耗X个火罐，指定其他人的X个地契，每张地契向对应
//! > 玩家支付100+n*100资金（n为对应格子上的房屋数），直到状态2结束为止，你与对方均分
//! > 那些地契收取的资金；每通过此法消耗4个火罐，你抽1张卡。
//!
//! 状态1's offer is another player's, at *their* turn start -- so the hook runs
//! on every turn start and asks the turn's player. 「前后一格内的一名玩家」 is a
//! player standing on the named tile or one of its two neighbours; the
//! 「半价[结算]」 is `card_settle_at` with the plan's pay factor at 50.
//!
//! 「每两个回合没有玩家发动该技能」 is a shared rest counter: any press resets
//! it, and two quiet turns pay out one pot.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「每两个回合没有玩家发动该技能」 -- a band-wide rest counter.
const REST: &str = "skill.kaedeSupport.rest";
/// 「直到这两名玩家的下回合结束…他们不会被该效果再次收款」 -- per-payer latch,
/// keyed on the turn it expires.
const COOLDOWN: &str = "skill.kaedeSupport.cooldown.";
/// Tiles named in 状态2, as `"<tile>:<owner>"` markers this skill splits income on.
const SPLIT: &str = "skill.kaedeSupport.split.";

pub const KAEDE_SUPPORT: CardDef = CardDef::new("skill:八幡海铃:熟练的支援贝斯手", &[
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::TurnStartBefore], other_turn, offer_support),
    On::Hook(&[HookKind::TurnStartBefore], mine, at_turn_start),
    On::Hook(&[HookKind::TurnEnd], mine, at_turn_end),
    On::Play(Some(can_enter_two), enter_two)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other_turn(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// 「初始0，上限4」.
fn declare_cap(player_id: i32) {
    state::set_bounds(player_id, state_key::FIRE, 0, 4);
}

/// 状态1's offer, on the *other* player's turn start.
fn offer_support(player_id: i32) {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        return;
    }
    let other = ctx::trigger::player_id();
    // 「他们不会被该效果再次收款」 -- a latch the previous use set.
    let key = format!("{}{}", COOLDOWN, other);
    if state::get(player_id, &key) > ctx::turn_key() {
        return;
    }
    if !ctx::ask_yes(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_offer")).player_id("who", player_id).i("n", 400),
    ) {
        return;
    }
    if ctx::transfer(other, player_id, 400, &Msg::new(key!("kaede_support_paid"))) == 0 {
        return;
    }
    // 「指定一个属于你的格子」
    let mine_tiles = ctx::owned_tiles(player_id);
    if mine_tiles.is_empty() {
        return;
    }
    let tile = ctx::ask_tile(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_which")),
        &mine_tiles,
    );
    // 「使其立即对前后一格内的一名玩家进行一次半价[结算]」
    let mut near: Vec<i32> = Vec::new();
    for t in [tile, tile - 1, tile + 1] {
        if t < 0 || t >= ctx::tile_count() {
            continue;
        }
        for p in ctx::players_on(t, player_id) {
            if !near.contains(&p) {
                near.push(p);
            }
        }
    }
    if near.is_empty() {
        return;
    }
    let who = ctx::ask_player(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_who")),
        &near,
    );
    let before = ctx::money(player_id);
    plan::set_pay_factor(50);
    ctx::card_settle_at(who, tile, true);
    // 「并获得相当于你获取数额的资金」
    let got = (ctx::money(player_id) - before).max(0);
    if got > 0 {
        ctx::gain(player_id, got, &Msg::new(key!("kaede_support_gain")));
    }
    // 「直到这两名玩家的下回合结束…不会被该效果再次收款」
    let until = ctx::turn_key() + 2;
    state::set(player_id, &format!("{}{}", COOLDOWN, other), until);
    state::set(player_id, &format!("{}{}", COOLDOWN, who), until);
    // A press resets the rest counter.
    state::set(player_id, REST, 0);
    ctx::log(player_id, &Msg::new(key!("kaede_support_done")).tile("tile", tile).player_id("who", who));
}

/// 「每两个回合没有玩家发动该技能，你在回合开始时获得1火罐…若达到上限，可在你的
/// 回合开始时选择进入状态2」.
fn at_turn_start(player_id: i32) {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        return;
    }
    let n = state::get(player_id, REST) + 1;
    state::set(player_id, REST, n);
    if n >= 2 {
        state::set(player_id, REST, 0);
        ctx::gain_fire(player_id, 1, &Msg::new(key!("kaede_support_gain_fire")));
    }
    if state::get(player_id, state_key::FIRE) < state::max(player_id, state_key::FIRE) {
        return;
    }
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_enter")),
    ) {
        state::set(player_id, state_key::SKILL_STATE, 2);
        ctx::log(player_id, &Msg::new(key!("kaede_support_two")));
    }
}

fn at_turn_end(_player_id: i32) {}

/// 状态2（2）: 「进入状态2时你可消耗X个火罐，指定其他人的X个地契」.
fn can_enter_two(player_id: i32) -> Option<Msg> {
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Some(Msg::new(key!("kaede_support_not_two")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("kaede_support_no_fire")));
    }
    None
}

fn enter_two(player_id: i32) {
    // 「你可消耗X个火罐，指定其他人的X个地契」 -- X is however many pots the
    // player spends, one deed each.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_how_many")),
        1,
        state::get(player_id, state_key::FIRE),
    );
    if x <= 0 {
        return;
    }
    if !ctx::spend_fire(player_id, x, &Msg::new(key!("kaede_support_spend"))) {
        return;
    }
    for _ in 0..x {
        let theirs = other_deeds(player_id);
        if theirs.is_empty() {
            break;
        }
            let t = ctx::ask_tile(
            player_id,
            &Msg::new(key!("kaede_support_title")),
            &Msg::new(key!("kaede_support_deed")),
            &theirs,
        );
        let owner = ctx::tile_owner(t);
        if owner < 0 {
            continue;
        }
        // 「每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数）」
        let n = ctx::houses_of(t);
        ctx::transfer(player_id, owner, 100 + n * 100, &Msg::new(key!("kaede_support_buy_in")));
        // 「直到状态2结束为止，你与对方均分那些地契收取的资金」
        state::set(player_id, &format!("{}{}", SPLIT, t), owner);
    }
    // 「每通过此法消耗4个火罐，你抽1卡」
    let draws = x / 4;
    if draws > 0 {
        ctx::draw(player_id, draws);
        ctx::log(player_id, &Msg::new(key!("kaede_support_drew")).i("n", draws as i64));
    }
}

fn other_deeds(player_id: i32) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    for p in ctx::others(player_id) {
        out.extend(ctx::owned_tiles(p));
    }
    out
}

// 「你与对方均分那些地契收取的资金」 -- the split is a bend on the rent a named
// deed collects, which is the `payMul` window with `t.Pay.IsRent`.
// TODO(规则书): 「直到状态2结束为止，你与对方均分那些地契收取的资金」 -- the
//   clause under-specifies what 「均分」 divides: the rent figure before or after
//   any other bend, and what happens on an odd amount. Taken to be a clean split
//   of the settled rent with the remainder to the deed's owner. The split window
//   itself (`PayMul` on rent from a named tile) is written once 状态2's exit is
//   decided -- see the note below.
//
// TODO(规则书)[judgement]: 状态2's exit -- the sheet gives none (「直到状态2结束
//   为止」 with no clause that ends it). Contrast 三角初华's fire-pot cap and
//   若叶睦's hand-size rule. Without an exit, the split is permanent once named.