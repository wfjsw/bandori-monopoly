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

pub const SAAYA_SKY: CardDef = CardDef::new(
    "skill:山吹沙绫:焕然一新的天空中",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        // （1）'s 「一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时」 is the
        // hook's applicability, not its effect (the ask is the effect).
        On::Hook(&[HookKind::PayAfter], "", Some(pay_after_applies), on_pay_after),
        // （1）'s 「拥有saaya标记时」 -- the mark is the applicability.
        On::Hook(&[HookKind::RollAfter], card_sdk::pre::MINE, Some(roll_needs_mark), on_roll),
        // （2）'s 「可使用5个[火罐]」 -- the amount and the pot are the
        // applicability; spending the pot is the cost, kept in the body.
        On::Hook(&[HookKind::PayChoose], card_sdk::pre::MINE, Some(pay_cut_applies), on_pay),
    ],
)
    .legacy(&[(2, legacy_mine), (3, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// （1）'s applicability: another player spent/paid at least 1000 and this
/// player holds no saaya mark yet.
fn pay_after_applies(player_id: i32) -> bool {
    other(player_id)
        && ctx::tok(player_id, MARK) == 0
        && ctx::trigger::value() >= 1000
        && ctx::trigger::player_id() >= 0
}

/// （1）'s applicability for the dice cut: the mark is there to spend.
fn roll_needs_mark(player_id: i32) -> bool {
    ctx::tok(player_id, MARK) >= 1
}

/// （2）'s applicability: a non-zero money change and the 5-pot price.
fn pay_cut_applies(player_id: i32) -> bool {
    ctx::trigger::value() > 0 && state::get(player_id, state_key::FIRE) >= 5
}

/// 「初始3，上限5」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 3, 5);
    Ok(())
}

/// （1）「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名
/// 玩家获得200资金且自己获得1个saaya标记和1个[火罐]」. Applicability is
/// [`pay_after_applies`]; the ask is the effect.
fn on_pay_after(player_id: i32) -> card_sdk::Asked {
    let payer = ctx::trigger::player_id();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("saaya_sky_title")),
        &Msg::new(key!("saaya_sky_ask")).player_id("who", payer),
    )? {
        return Ok(());
    }
    ctx::add_tok(player_id, MARK, 1, 1)?;
    ctx::gain_fire(player_id, 1, &Msg::new(key!("saaya_sky_gain")))?;
    if !ctx::player_out(payer) {
        ctx::gain(payer, 200, &Msg::new(key!("saaya_sky_payee")))?;
    }
    Ok(())
}

/// （1）「拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10
/// （计算后小等于0则移动到下一个可购买格子）」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, MARK, -1, 1)?;
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
        ctx::log(
            player_id,
            &Msg::new(key!("saaya_sky_floor")).tile("tile", to),
        );
        return Ok(());
    }
    ctx::trigger::set_move_roll(after);
    ctx::log(
        player_id,
        &Msg::new(key!("saaya_sky_cut"))
            .i("n", cut as i64)
            .i("total", after as i64),
    );
    Ok(())
}

/// （2）「[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）」.
/// The amount and the pot are [`pay_cut_applies`]'s job; spending the pot is
/// the cost and stays here (resolution-time: the pot may be spent between the
/// gate and this body).
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("saaya_sky_cut_title")),
        &Msg::new(key!("saaya_sky_cut_ask")).i("n", amount as i64),
    )? {
        return Ok(());
    }
    if ctx::spend_fire(player_id, 5, &Msg::new(key!("saaya_sky_spend")))? {
        ctx::trigger::set_pay_amount((amount - 5000).max(0));
    }
    Ok(())
}
