//!
//! 规则书（docs/rulebook/cards.json, id `PP:再次闪耀`）:
//! > 再次闪耀：
//! > [特]：
//! > 此卡不受除拥有此卡的玩家以外的玩家的效果影响。
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//!
//! > （1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。
//!
//! > （2）[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。
//!
//! while short on cash the owner may record a colour and take a rescue.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};
use skill_groups::fans;

pub const SHINE_AGAIN: CardDef = CardDef::new(
    "PP:再次闪耀",
    &[
        // 规则书[手]: 「将此卡放置在[使用者]的[场地]」 -- the hand play. The
        // [共鸣] branch (2) is offered at the owner's turn start (see its entry
        // below), not as a second `Play` entry (its `is_placed` gate was blocking
        // this play).
        On::Play("", None, shine_again),
        // 规则书[持续]（1）: 「[拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡
        // 未记录且[拥有者]拥有同色地契的颜色」 -- the offer exists only while
        // short on cash and with an unrecorded owned colour to name. The money
        // shortfall is CEL; the colour scan is residual.
        //
        // Rides `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position.
        // The condition's `money(owner) < value` still gates on the figure in flight.
        On::Action(Action::PaySplitChooseBefore("actor == owner && card.placed && value > 0 && money(owner) < value", Some(has_unrecorded_color), pay_choose).counter_reason(key!("shine_again_when"))),
        // TODO(规则书)（2）: 规则书[持续]（2）: 「[共鸣]记录一个此卡未记录的颜色」 -- offered at the owner's turn
        // start (the text gives no narrower timing).
        On::Action(Action::TurnStart("actor == owner && card.placed", None, offer_resonance)),
    ],
)
// 规则书[特]: 「此卡不受除拥有此卡的玩家以外的玩家的效果影响」 -- a
// placement-stamped immunity flavour (the engine stamps the placed instance).
.props(&[(card_sdk::abi::prop::IMMUNE_EXCEPT_OWNER, 1)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"再次闪耀"#)
        .raw_name(r#"再次闪耀"#)
        .tags(&[r#"手"#, r#"反击"#, r#"持续"#, r#"特"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"此卡不受除拥有此卡的玩家以外的玩家的效果影响。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。"#),
                    it(r#"[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。"#),
                ]),
            ]),
        ]))
        // sheet cell D2
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

const PROP_COLORS: &str = "shine_again_colors";

fn shine_again(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "PP:再次闪耀",
        &Msg::new(key!("shine_again_note")),
    );
    ctx::set_prop(PROP_COLORS, 0);
    // 「不受除拥有此卡的玩家以外的…影响」 rides the `immuneExceptOwner` prop
    // declared on the card: the engine stamps the placed instance, so the
    // owner's own effects still land.
    Ok(())
}

fn unrecorded_owned_colors(player_id: i32) -> Vec<i32> {
    let recorded = ctx::prop(PROP_COLORS);
    let mut out: Vec<i32> = Vec::new();
    for t in ctx::owned_tiles(player_id) {
        if !ctx::is_buyable(t) {
            continue;
        }
        let g = ctx::tile_group(t);
        if g < 0 || out.contains(&g) {
            continue;
        }
        if recorded & (1 << g) != 0 {
            continue;
        }
        out.push(g);
    }
    out
}

/// Every colour the board carries that this card has not recorded -- the (2)
/// pool, which is not restricted to colours the owner holds a deed in.
fn unrecorded_colors() -> Vec<i32> {
    let recorded = ctx::prop(PROP_COLORS);
    let mut out: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        if !ctx::is_buyable(t) {
            continue;
        }
        let g = ctx::tile_group(t);
        if g < 0 || g >= 31 || out.contains(&g) {
            continue;
        }
        if recorded & (1 << g) != 0 {
            continue;
        }
        out.push(g);
    }
    out
}

/// Residual guard for [`pay_choose`]: an unrecorded owned colour exists.
/// Derived scan (owned groups minus the recorded bitset).
fn has_unrecorded_color(player_id: i32) -> bool {
    !unrecorded_owned_colors(player_id).is_empty()
}

/// Applicability is `money(owner) < value` + `has_unrecorded_color`.
fn pay_choose(player_id: i32) -> card_sdk::Asked {
    let colors = unrecorded_owned_colors(player_id);
    // 规则书[持续]（1）: 「记录一个此卡未记录且[拥有者]拥有同色地契的颜色」
    let mut options: Vec<Msg> = Vec::new();
    for &g in &colors {
        options.push(Msg::new(key!("shine_again_color")).i("g", g as i64));
    }
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("shine_again_title")),
        &Msg::new(key!("shine_again_ask")).i("n", trigger::value() as i64),
        &options,
    )?;
    let g = colors[pick.min(colors.len() - 1)];
    ctx::set_prop(PROP_COLORS, ctx::prop(PROP_COLORS) | (1 << g));
    // TODO(规则书)（[拥有者]拥有的1个同色地契的购买价格）: the book does not say which deed when several
    // in the colour match -- we take the dearest.
    let mut best = 0;
    for t in ctx::owned_tiles(player_id) {
        if ctx::is_buyable(t) && ctx::tile_group(t) == g {
            let p = ctx::tile_price(t);
            if p > best {
                best = p;
            }
        }
    }
    // Dict writes (`ctx::state`) do not open a `markerGain` [反击] window, so this
    // grant is safe nested in the pay pipeline.
    let _ = fans::grant(player_id, 1, 0)?;
    // 规则书[持续]（1）: 「[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金」
    // -- a printed 获得, so it belongs in the ordinary Money pipeline, but that is
    // deferred: `gain_raw` here. Re-entry is not the reason (the (1) condition is
    // `actor == owner`, a bank gain's actor is -1, and `is_money_hook_kind`
    // guards the rest).
    // TODO(ABI)（[拥有者][获得]“…÷5的资金”） ctx::gain: inside a
    //   `PaySplitChooseBefore` body the gain parks a Pay host request and the
    //   body is not re-entered past it -- everything after `ctx::gain`
    //   (the recorded colour and the fan here) is dropped, so the gain must
    //   stay raw until the pay-choose drive resumes its body.
    let money = best / 5;
    if money > 0 {
        ctx::gain_raw(
            player_id,
            money,
            &Msg::new(key!("shine_again_reason")).i("n", money as i64),
        )?;
    }
    Ok(())
}

/// [持续]（2）「[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]」 --
/// a press: the same resonance cost every other [共鸣] card uses.
fn can_record(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return Some(Msg::new(key!("shine_again_not_placed")));
    }
    // 规则书[持续]（2）: 「记录一个此卡未记录的颜色」 -- any colour, not only an
    // owned one.
    if unrecorded_colors().is_empty() {
        return Some(Msg::new(key!("shine_again_no_colour")));
    }
    None
}

/// Offer the [共鸣] record at the owner's turn start (see the TODO on the hook).
/// 规则书[持续]（2）: 「[共鸣]记录一个此卡未记录的颜色」
fn offer_resonance(player_id: i32) -> card_sdk::Asked {
    if can_record(player_id).is_some() {
        return Ok(());
    }
    record(player_id)
}

/// 规则书[持续]（2）: 「[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]」
fn record(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    // The pool first: the 共鸣 is the cost of the record, so it must not be paid
    // when there is nothing left to record.
    // 规则书[持续]（2）: 「记录一个此卡未记录的颜色」 -- any unrecorded colour.
    let pool = unrecorded_colors();
    if pool.is_empty() {
        return Ok(());
    }
    if !crate::resonance::try_resonance(player_id)? {
        return Ok(());
    }
    let opts: Vec<Msg> = pool
        .iter()
        .map(|&g| Msg::new(key!("shine_again_colour")).i("n", g as i64))
        .collect();
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("shine_again_title")),
        &Msg::new(key!("shine_again_which")),
        &opts,
    )?;
    let Some(&g) = pool.get(pick) else {
        return Ok(());
    };
    ctx::set_prop(PROP_COLORS, ctx::prop(PROP_COLORS) | (1 << g));
    // 规则书[持续]（2）: 「[拥有者]获得1个正面[P✽P粉丝]」
    let _ = fans::grant(player_id, 1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("shine_again_recorded")).i("n", g as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn shine_again_kani(player_id: i32) -> card_sdk::Asked {
    shine_again(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_choose_kani(player_id: i32) -> card_sdk::Asked {
    pay_choose(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn offer_resonance_kani(player_id: i32) -> card_sdk::Asked {
    offer_resonance(player_id)
}
