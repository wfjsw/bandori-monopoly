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

use bandori_std::ctx::{self, state};
use card_sdk::abi::Dice;
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PlanValue};

const MARK: &str = "saaya标记";
/// The roll whose mark was spent; the settled arm floors only that one.
const FLOOR: &str = "skill.saayaSky.floor";

pub const SAAYA: CardDef = CardDef::new(
    "skill:山吹沙绫:焕然一新的天空中",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        // （1）'s 「一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时」 is the
        // hook's applicability, not its effect (the ask is the effect).
        On::Action(Action::PayAfter("", Some(pay_after_applies), on_pay_after)),
        // （1）'s 「拥有saaya标记时」 -- the mark is the applicability. The cut
        // die is a term of the movement roll itself (plan `extra`), so a
        // re-roll of that roll (`Mor:蝴蝶飞舞的星月夜`) throws it again; the
        // After arm below only spends the mark and checks the floor.
        On::Filter(Filter::DiceRollPlan(
            card_sdk::condition::MINE,
            Some(roll_needs_mark),
            on_plan,
        )),
        On::Action(Action::DiceRollAfter(
            card_sdk::condition::MINE,
            Some(roll_needs_mark),
            on_roll,
        )),
        // 「计算后小等于0…」 must also cover a re-cast (which never raises
        // After) and must decide on the face the player actually kept, so it
        // runs at `diceRollSettled` -- once per transaction, after the pick.
        On::Action(Action::DiceRollSettled(
            card_sdk::condition::MINE,
            Some(is_floored_roll),
            on_result,
        )),
        // （2）'s 「可使用5个[火罐]」 -- the amount and the pot are the
        // applicability; spending the pot is the cost, kept in the body.
        // Interactive (ask_yes + fire spend + amount rewrite). Rides
        // `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position.
        On::Action(Action::PaySplitChooseBefore(
            card_sdk::condition::MINE,
            Some(pay_cut_applies),
            on_pay,
        )),
    ],
)
.rule()
// 「（初始3，上限5）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 3),
    (bandori_std::fire::FIRE_CAP, 5),
]);

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// （1）'s applicability: another player spent/paid at least 1000 and this
/// player holds no saaya mark yet.
fn pay_after_applies(player_id: i32) -> bool {
    other(player_id)
        && ctx::held_count(player_id, MARK) == 0
        && ctx::trigger::value() >= 1000
        && ctx::trigger::player_id() >= 0
}

/// （1）'s applicability for the dice cut: the mark is there to spend.
fn roll_needs_mark(player_id: i32) -> bool {
    ctx::held_count(player_id, MARK) >= 1
}

/// （2）'s applicability: a non-zero money change and the 5-pot price.
fn pay_cut_applies(player_id: i32) -> bool {
    ctx::trigger::value() > 0 && bandori_std::fire::get(player_id) >= 5
}

/// 「初始3，上限5」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名
/// 玩家获得200资金且自己获得1个saaya标记和1个[火罐]」. Applicability is
/// [`pay_after_applies`]; the ask is the effect.
fn on_pay_after(player_id: i32) -> card_sdk::Asked {
    let payer = ctx::trigger::player_id();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("saaya_title")),
        &Msg::new(key!("saaya_ask")).player_id("who", payer),
    )? {
        return Ok(());
    }
    ctx::add_held_by_name(player_id, MARK, 1, 1)?;
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("saaya_gain")))?;
    if !ctx::player_out(payer) {
        ctx::gain(payer, 200, &Msg::new(key!("saaya_payee")))?;
    }
    Ok(())
}

/// 「此次投掷结果减1d10」 -- the cut is a minus term of the movement roll,
/// part of it (and of any re-roll of it), not a value subtracted afterwards.
fn on_plan(_player_id: i32, mut v: PlanValue) -> PlanValue {
    v.extra = Some(Dice {
        count: -1,
        sides: 10,
    });
    v
}

/// （1）「拥有saaya标记时投掷移动骰时失去1个saaya标记」. The face already
/// carries the plan's d10; this arm only spends the mark and pins the floor to
/// this roll.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    ctx::add_held_by_name(player_id, MARK, -1, 1)?;
    state::set(player_id, FLOOR, ctx::trigger::roll_id());
    Ok(())
}

/// Is this the roll whose mark was spent (set by [`on_roll`])?
fn is_floored_roll(player_id: i32) -> bool {
    let id = ctx::trigger::roll_id();
    id >= 0 && id == state::get(player_id, FLOOR)
}

/// 「计算后小等于0则移动到下一个可购买格子」 -- on the face that settles.
fn on_result(player_id: i32) -> card_sdk::Asked {
    let after = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
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
        ctx::log(player_id, &Msg::new(key!("saaya_floor")).tile("tile", to));
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("saaya_cut")).i("total", after as i64),
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
        &Msg::new(key!("saaya_cut_title")),
        &Msg::new(key!("saaya_cut_ask")).i("n", amount as i64),
    )? {
        return Ok(());
    }
    if bandori_std::fire::spend_skill(player_id, 5, &Msg::new(key!("saaya_spend")))? {
        ctx::trigger::set_pay_amount((amount - 5000).max(0));
    }
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_after_kani(player_id: i32) -> card_sdk::Asked {
    on_pay_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_kani(player_id: i32) -> card_sdk::Asked {
    on_roll(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32) -> card_sdk::Asked {
    on_pay(player_id)
}
