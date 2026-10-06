//! `skill:二叶筑紫:交给班长吧`
//!
//! 规则书（skill sheet, 二叶筑紫）:
//! > （1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，
//! > 发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量，若为抽卡效果则记录
//! > 2200。单轮结算（比如登上武道馆的结算）中此效果最多触发一次
//! > （2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作。
//! > 以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得。此效果对"抽牌"
//! > 动作发动时必须翻面记录数字为2200及以上的标记。
//! > （3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得
//! > 移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。
//!
//! 「X（占位）标记」 is a two-faced mark: face-up when sent, face-down once
//! claimed in （2）. A mark has a `count`, not a face, so the face is a second
//! mark kind and 「翻面」 moves one from the up kind to the down kind -- the
//! same composition the P✽P fan flips use.
//!
//! 「记录获得因此效果获得资金的数量」 rides beside the mark: a slot per
//! recipient holding the recorded figure. 「若为抽卡效果则记录2200」 is the
//! text's own constant for a draw.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const UP: &str = "X标记(正)";
const DOWN: &str = "X标记(反)";
/// Recorded figure per recipient, keyed `skill.tsugushi.rec:<player>`.
fn rec_key(p: i32) -> alloc::string::String {
    alloc::format!("skill.tsugushi.rec:{p}")
}
/// 「每次发动（3）技能时单个标记的收益减200，最低单标记收益200」.
const RATE: &str = "skill.tsugushi.rate";

pub const TSUGUSHI_MONITOR: CardDef = CardDef::new("skill:二叶筑紫:交给班长吧", &[
    On::Play(Some(can_cash), cash),
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare),
    On::Hook(&[HookKind::PayChoose], mine, on_gain),
    On::Hook(&[HookKind::PayChoose], other, on_theirs),
]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// The per-lap rate starts at 800 and drops by 200 per (3), floored at 200.
fn declare(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, RATE) == 0 {
        state::set(player_id, RATE, 800);
    }
    let _ = state_key::SKILL_STATE;
    Ok(())
}

/// （1）「当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，
/// 发送给对方一个X（占位）标记并记录…数量」.
fn on_gain(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tsugushi_title")),
        &Msg::new(key!("tsugushi_ask")),
    )? {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("tsugushi_title")),
        &Msg::new(key!("tsugushi_who")),
        &others
            .iter()
            .map(|&p| Msg::new(key!("tsugushi_option")).player_id("who", p))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&who) = others.get(pick) else { return Ok(()); };
    // 「可以改为指定…一个角色进行一次该动作」 -- the figure moves to them.
    ctx::trigger::set_pay_amount(0);
    ctx::gain(who, amount, &Msg::new(key!("tsugushi_forwarded")).i("n", amount as i64));
    // 「发送给对方一个X（占位）标记并记录…数量」
    ctx::add_tok(who, UP, 1, i32::MAX);
    state::set(player_id, &rec_key(who), amount);
    ctx::log(player_id, &Msg::new(key!("tsugushi_sent")).player_id("who", who).i("n", amount as i64));
    Ok(())
}

/// （2）「当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作」.
fn on_theirs(player_id: i32) -> card_sdk::Asked {
    let src = ctx::trigger::player_id();
    if src < 0 || src == player_id {
        return Ok(());
    }
    if ctx::tok(src, UP) < 1 {
        return Ok(());
    }
    let recorded = state::get(player_id, &rec_key(src));
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // 「此效果对"抽牌"动作发动时必须翻面记录数字为2200及以上的标记」
    if recorded < 2200 && ctx::slot(player_id, "skill.tsugushi.isDraw") != 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tsugushi_title")),
        &Msg::new(key!("tsugushi_take")).player_id("who", src),
    )? {
        return Ok(());
    }
    // 「将对方的X标记翻面」
    ctx::add_tok(src, UP, -1, i32::MAX);
    ctx::add_tok(src, DOWN, 1, i32::MAX);
    // 「以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得」
    let cap = recorded + 300;
    let mine = amount.min(cap);
    let theirs = amount - mine;
    ctx::trigger::set_pay_amount(0);
    ctx::gain(player_id, mine, &Msg::new(key!("tsugushi_taken")).i("n", mine as i64));
    if theirs > 0 {
        ctx::gain(src, theirs, &Msg::new(key!("tsugushi_excess")).i("n", theirs as i64));
    }
    Ok(())
}

/// （3） 「当场上除你之外的玩家都拥有一枚翻面后的X标记时」.
fn can_cash(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    let all = (0..ctx::player_count()).filter(|&p| p != player_id && !ctx::player_out(p)).all(|p| ctx::tok(p, DOWN) >= 1);
    if !all {
        return Some(Msg::new(key!("tsugushi_not_ready")));
    }
    None
}

/// （3）「移除所有X标记，你获得移除标记数量*800的资金（每次发动（3）技能时单个标记
/// 的收益减200，最低单标记收益200）」.
fn cash(player_id: i32) -> card_sdk::Asked {
    let mut n = 0;
    for p in 0..ctx::player_count() {
        let both = ctx::tok(p, UP) + ctx::tok(p, DOWN);
        if both > 0 {
            ctx::add_tok(p, UP, -ctx::tok(p, UP), i32::MAX);
            ctx::add_tok(p, DOWN, -ctx::tok(p, DOWN), i32::MAX);
            n += both;
        }
    }
    let rate = state::get(player_id, RATE).max(200);
    ctx::gain(player_id, n * rate, &Msg::new(key!("tsugushi_cash")).i("n", (n * rate) as i64));
    // 「每次发动（3）技能时单个标记的收益减200，最低单标记收益200」
    state::set(player_id, RATE, (rate - 200).max(200));
    ctx::log(player_id, &Msg::new(key!("tsugushi_done")));
    Ok(())
}