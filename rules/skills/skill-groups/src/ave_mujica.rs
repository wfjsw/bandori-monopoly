//! `skill:Ave Mujica:假面之下的真实`
//!
//! 规则书（band sheet, Ave Mujica）:
//! > 你拥有"状态1"和"状态2"两种状态，初始处于状态1。处于状态2的第三回合开始时为
//! > 此卡添加一个奇迹水晶。你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己
//! > 结算，对应的其他玩家收支不变），处于状态2时可消耗一个奇迹水晶使你的一次[支付]
//! > 金额减半。若你的技能拥有火罐，进入状态2时将火罐补充至上限且每回合结束时失去1
//! > 火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐。
//!
//! This skill owns the 「状态1 / 状态2」 axis every Ave Mujica character skill
//! and the two CRYCHIC ones reference: `skillState` is 1 or 2, and the rules
//! below are what being in 2 *does*. A character skill that says 「进入状态2」
//! writes that value and this skill's hooks pick it up.
//!
//! 「从[收取]与[支付]的资金改为1.5倍（只对自己结算，对应的其他玩家收支不变）」
//! is a bend on *this* player's side of a transfer only -- the other party's
//! figure is untouched. That is a per-party bend, which `PaySplitMul` carries as
//! the amount seen from one side; the mul stage folds every writer as a factor
//! (MONEY-01), and [`after_bend`] claws the one-sided half back.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

/// Turns spent in 状态2. 「处于状态2的第三回合开始时」 reads this.
const IN_TWO: &str = "skill.aveMujica.inTwo";
/// `skillState` as of the last check, so 「进入状态2」 can be seen as a transition.
const WAS: &str = "skill.aveMujica.was";
/// Printed figure of the payment this bend is inflating, for the PayAfter
/// one-sided fixup (「对应的其他玩家收支不变」).
const ORIG: &str = "skill.aveMujica.orig";

pub const AVE_MUJICA: CardDef = CardDef::new(
    "skill:Ave Mujica:假面之下的真实",
    &[
        On::Play("", Some(can_halve), halve),
        On::Action(Action::TurnStartBefore(
            "slot('skillState') != 0",
            None,
            at_turn_start,
        )),
        // Latch the composed figure for the PayAfter one-sided fixup. Rides
        // `PaySplitChooseBefore`, the last pre-round stage after every mul:
        // with the mul stage folding as a product (MONEY-01) no single mul body
        // can see the composed figure, so the clawback latches it here; see
        // `latch_orig`.
        On::Action(Action::PaySplitChooseBefore(
            "slot('skillState') == 2 && card.placed",
            None,
            latch_orig,
        )),
        On::Filter(Filter::PaySplitMul(
            "slot('skillState') == 2 && card.placed",
            None,
            bend,
        )),
        // 「只对自己结算，对应的其他玩家收支不变」 -- PaySplitMul rewrites one shared
        // figure, so the counterpart's side is restored here (inverse of this
        // body's own factor, see `after_bend`).
        On::Action(Action::PayAfter(
            "slot('skillState') == 2 && card.placed",
            None,
            after_bend,
        )),
        On::Action(Action::TurnEnd(
            "actor == owner && slot('skillState') == 2",
            None,
            at_turn_end,
        )),
    ],
)
.rule();

/// 「你在状态2时…」 -- the bends only apply in 状态2.
fn in_two(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, state_key::SKILL_STATE) == 2
}

/// 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶」, and the pot refill on
/// entering 状态2.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // 「初始处于状态1」 -- restated so a fresh match starts in 1.
    if state::get(player_id, state_key::SKILL_STATE) == 0 {
        state::set(player_id, state_key::SKILL_STATE, 1);
        state::set(player_id, IN_TWO, 0);
        state::set(player_id, WAS, 1);
        return Ok(());
    }
    on_state(player_id)?;
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Ok(());
    }
    let n = state::get(player_id, IN_TWO) + 1;
    state::set(player_id, IN_TWO, n);
    // 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶」 -- a *band-card*
    // crystal: this rule's own instance, the same pool `group_crystals` reads.
    if n == 3 {
        ctx::add_crystals(1, i32::MAX)?;
        ctx::log(player_id, &Msg::new(key!("ave_mujica_crystal")));
    }
    Ok(())
}

/// 「进入状态2时将火罐补充至上限」 -- the character skills drive the entry, so
/// this card watches `skillState` for the 1 → 2 edge and refills on it. Runs at
/// the turn start and the turn end, which brackets every place a character skill
/// writes the value.
///
/// MARKERS.md §10(1) pin (implemented as recommended): on entering 状态2 the
/// **band refill runs first**, then the face's 「进入状态2时」 clauses (so
/// `skill:八幡海铃:熟练的支援贝斯手`'s 「进入状态2时你可消耗X个火罐」 spends from
/// the refilled pool). The face entry hooks run after this band-skill hook in
/// the same `TurnStartBefore` pass (band skills are placed before character
/// skills; `on_state` is this card's turn-start body).
fn on_state(player_id: i32) -> card_sdk::Asked {
    let now = state::get(player_id, state_key::SKILL_STATE);
    let was = state::get(player_id, WAS);
    state::set(player_id, WAS, now);
    if now != 2 || was == 2 {
        return Ok(());
    }
    // 「若你的技能拥有火罐，进入状态2时将火罐补充至上限」 -- a named regime
    // guest over the owner's pool (`fire::regime`), never a raw write.
    if bandori_std::fire::max(player_id) <= 0 {
        return Ok(());
    }
    let got =
        bandori_std::fire::regime::fill_to_cap(player_id, &Msg::new(key!("ave_mujica_refill")))?;
    if got > 0 {
        ctx::log(player_id, &Msg::new(key!("ave_mujica_refill")));
    }
    Ok(())
}

/// Latch the **composed figure** for [`after_bend`] -- the amount the payment
/// will settle at before the one money round. Rides
/// `ActionKind::PaySplitChooseBefore`, which follows the whole mul phase, so
/// every other writer's factor (and every add) is already composed into
/// `value`; the clawback has to undo only *this* card's 1.5×.
fn latch_orig(player_id: i32) -> card_sdk::Asked {
    let value = ctx::trigger::value();
    if value <= 0 {
        return Ok(());
    }
    // 「只对自己结算，对应的其他玩家收支不变」 -- only the side that is this
    // player moves.
    let from = ctx::trigger::player_id();
    let to = ctx::trigger::target();
    let mine = from == player_id || to == player_id;
    if !mine {
        return Ok(());
    }
    state::set(player_id, ORIG, value);
    Ok(())
}

/// 「你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算）」 -- the figure
/// seen from this player's side. Pure (ABI v78 Filter).
fn bend(player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    // 「只对自己结算，对应的其他玩家收支不变」 -- only the side that is this
    // player moves.
    let from = ctx::trigger::player_id();
    let to = ctx::trigger::target();
    let mine = from == player_id || to == player_id;
    if !mine {
        return value;
    }
    // Live sheet 1.5倍 (rulebook-doc 「翻倍」 is superseded). Integer fold
    // rounds the .5 up, matching OPEN-取整's exact transform.
    (value * 3 + 1) / 2
}

/// The counterpart's figure stays the amount without this 1.5× (`PaySplitMul`
/// rewrites one shared transfer). Claw the one-sided delta back to / from the
/// bank so only this player's side moved.
///
/// `moved` is the settled figure -- the latched amount times every factor,
/// already through the one money round. The counterpart's counterfactual is
/// the latched amount undoing **this** card's factor, `latched / 1.5` (rounded
/// up, the same convention the bend rounds by) through that same round. Every
/// other factor therefore survives on the counterpart's side, and the delta is
/// carried by the bank.
fn after_bend(player_id: i32) -> card_sdk::Asked {
    let latched = state::get(player_id, ORIG);
    if latched <= 0 {
        return Ok(());
    }
    state::set(player_id, ORIG, 0);
    let moved = ctx::trigger::value();
    if moved <= 0 {
        return Ok(());
    }
    let from = ctx::trigger::player_id();
    let to = ctx::trigger::target();
    // What the counterpart would have moved without this body: `latched`
    // undoing the body's 1.5× (ceil, like the bend), then the one money round
    // the payment would have taken anyway (`play.rs` ceil10).
    let base = ((latched * 2 + 2) / 3 + 9) / 10 * 10;
    let delta = moved - base;
    if delta == 0 {
        return Ok(());
    }
    // The counterpart moves `base` either way: a later factor can leave the
    // composed figure *below* the counterfactual (the bend is one factor of a
    // product), and then the counterpart has moved less than it should and
    // gets the difference back. Either direction is bank-carried.
    if from == player_id && to >= 0 {
        // I paid `moved`; the counterpart should net `base`.
        if delta > 0 {
            ctx::pay(to, delta, &Msg::new(key!("ave_mujica_one_sided")))?;
        } else {
            ctx::gain(to, -delta, &Msg::new(key!("ave_mujica_one_sided")))?;
        }
    } else if to == player_id && from >= 0 {
        // I received `moved`; the counterpart should have paid `base`.
        if delta > 0 {
            ctx::gain(from, delta, &Msg::new(key!("ave_mujica_one_sided")))?;
        } else {
            ctx::pay(from, -delta, &Msg::new(key!("ave_mujica_one_sided")))?;
        }
    }
    Ok(())
}

/// 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半」 -- the press is the
/// halve, so it rides the pay window rather than standing alone.
fn can_halve(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    // 规则书（`PPP:Returns` [持续]（3））: 「使用因此卡获得的团卡的主动效果时需要
    // 支付1星星贴纸」 -- the sticker is a cost of activation on the borrowed copy.
    if let Some(reason) = crate::copy_sticker_gate(player_id) {
        return Some(reason);
    }
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Some(Msg::new(key!("ave_mujica_not_two")));
    }
    if crate::cover_crystals(player_id) < 1 {
        return Some(Msg::new(key!("ave_mujica_no_crystal")));
    }
    None
}

fn halve(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id)?;
    // 规则书: 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」
    // -- 「此卡」 is the placed `Sumimi:#L11`, drained first.
    crate::spend_crystals(player_id, 1)?;
    ctx::log(player_id, &Msg::new(key!("ave_mujica_halved")));
    Ok(())
}

/// 「每回合结束时失去1火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去
/// 所有剩余火罐」. Named regime guests (`fire::regime`), never a raw write.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    on_state(player_id)?;
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Ok(());
    }
    // 「每回合结束时失去1火罐」 -- only when the character skill grants pots.
    if bandori_std::fire::max(player_id) <= 0 {
        return Ok(());
    }
    bandori_std::fire::regime::drain(player_id, 1, &Msg::new(key!("ave_mujica_burn")))?;
    // 「回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐」
    if bandori_std::fire::regime::count(player_id) == 0 {
        state::set(player_id, state_key::SKILL_STATE, 1);
        state::set(player_id, IN_TWO, 0);
        bandori_std::fire::regime::burn_all(player_id, &Msg::new(key!("ave_mujica_exit")))?;
        ctx::log(player_id, &Msg::new(key!("ave_mujica_left")));
    }
    Ok(())
}

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"Ave Mujica"#)
    .short_name(r#"Mujica"#)
    .color(r#"#881144"#)
    .skill(r#"假面之下的真实"#)
    .text(r#"你拥有“状态1”和“状态2”两种状态，初始处于状态1。处于状态2的第三回合开始时为此卡添加一个奇迹水晶。你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算，对应的其他玩家收支不变），处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。若你的技能拥有火罐，进入状态2时将火罐补充至上限且每回合结束时失去1火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐。"#)
    .logo("");

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn halve_kani(player_id: i32) -> card_sdk::Asked {
    halve(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn latch_orig_kani(player_id: i32) -> card_sdk::Asked {
    latch_orig(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn bend_kani(player_id: i32, value: i32) -> i32 {
    bend(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_bend_kani(player_id: i32) -> card_sdk::Asked {
    after_bend(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
