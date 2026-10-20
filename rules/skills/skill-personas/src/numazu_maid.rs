//! `skill:鳰原令王那:梦幻可爱♪女仆`
//!
//! 规则书（skill sheet, 鳰原令王那）:
//! > （1）游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]
//! > （2）你拥有格子上的房屋总数增加时可选择失去1PAREO标记（初始1，上限3），
//! > 所有非自己的玩家分摊支付你"格子的房屋造价×格子上的房屋数"四分之一的资金
//! > （所有你拥有的格子中取最高值）
//!
//! （2）'s figure is 「格子的房屋造价×格子上的房屋数」 -- the build cost of one
//! house times how many stand there -- and 「所有你拥有的格子中取最高值」 picks
//! whichever of the player's deeds gives the largest such product before the
//! quarter is taken. 「分摊支付」 is the split-pay the vocabulary already has.

use card_sdk::abi::ActionKind;
use bandori_std::ctx;
use card_sdk::{Action, CardDef, key, Msg, On};
use skill_groups::fans;

const PAREO: &str = "PAREO标记";

pub const NUMAZU_MAID: CardDef = CardDef::new(
    "skill:鳰原令王那:梦幻可爱♪女仆",
    &[
        On::Action(Action::DeckAtGameStart("", "", None, at_start)),
        // （2）「所有你拥有的格子中取最高值」 -- the offer exists only when some
        // deed carries a positive `build_cost × houses`. Derived scan, residual.
        On::Action(Action::HouseAdded("actor == owner && tok('PAREO标记') >= 1", "", Some(has_house_product), on_built)),
        // （2）'s offer is also a press, so a card can run it out of turn
        // (pareo_far's 「视为你的房屋总数增加」 -- C# `SkillPareo -> Offer()`).
        // On::Play's gate is `Option<Msg>` (WhyNot), not a bool residual.
        On::Play("tok('PAREO标记') >= 1", "", Some(offer_gate), offer),
    ],
);

/// （1）「游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]」,
/// and （2）'s 「初始1」 -- the PAREO mark starts at 1 (cap 3).
fn at_start(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「失去1PAREO标记（初始1，上限3）」 -- the mark exists from
    // match start, one deep. Written before the (1) loop so the early return
    // there cannot skip it.
    ctx::add_tok(player_id, PAREO, 1, 3)?;
    for p in 0..ctx::player_count() {
        if p != player_id && !ctx::player_out(p) && ctx::in_group(p, "Pastel✽Palettes") {
            let _ = fans::grant(player_id, 1, 0)?;
            return Ok(());
        }
    }
    Ok(())
}

/// （2）「你拥有格子上的房屋总数增加时可选择失去1PAREO标记」 -- `houseAdded`
/// is the moment a house commits.
fn on_built(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}

/// （2）'s offer body: lose 1 PAREO mark, the others split-pay a quarter of the
/// largest `build_cost × houses` among the player's deeds. Reached two ways --
/// the `houseAdded` hook above, and the press entry (`On::Play`) a card runs
/// for 「视为你的房屋总数增加」 (pareo_far, C# `SkillPareo -> Offer()`).
/// Largest `build_cost × houses` over the player's deeds -- 「所有你拥有的格子
/// 中取最高值」.
fn best_house_product(player_id: i32) -> i32 {
    let mut best = 0;
    for t in ctx::owned_tiles(player_id) {
        let houses = ctx::houses_of(t);
        if houses <= 0 {
            continue;
        }
        let v = ctx::build_cost(t) * houses;
        if v > best {
            best = v;
        }
    }
    best
}

/// Residual guard for [`on_built`]: a positive house product exists.
fn has_house_product(player_id: i32) -> bool {
    best_house_product(player_id) > 0
}

/// `On::Play` gate (WhyNot shape): same predicate, generic `err.play_pre`
/// (the old body-top was a silent return -- docs/GUARDS.md §4.4).
fn offer_gate(player_id: i32) -> Option<Msg> {
    if has_house_product(player_id) {
        None
    } else {
        Some(Msg::new(key!("err.play_pre")))
    }
}

/// Applicability is `tok('PAREO标记') >= 1` + `has_house_product`.
fn offer(player_id: i32) -> card_sdk::Asked {
    // 「所有你拥有的格子中取最高值」 -- the largest `build_cost × houses` over
    // the player's own deeds, which is 「格子的房屋造价×格子上的房屋数」.
    let best = best_house_product(player_id);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("numazu_maid_title")),
        &Msg::new(key!("numazu_maid_ask")).i("n", best as i64),
    )? {
        return Ok(());
    }
    ctx::add_tok(player_id, PAREO, -1, i32::MAX)?;
    // 「所有非自己的玩家分摊支付你…四分之一的资金」.
    let pot = best / 4;
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    // `PIPELINE-AUDIT` Q2: the command-wide pre-split stage shapes `pot`
    // before it divides (the 「分摊前」 figure).
    let why = Msg::new(key!("numazu_maid_why"));
    let Some(shaped) = ctx::pay_total(others[0], player_id, pot, &why)? else {
        return Ok(());
    };
    let per = (shaped + others.len() as i32 - 1) / others.len() as i32;
    let share = ((per + 9) / 10) * 10;
    for p in others {
        ctx::pay_leg(p, player_id, share, &why)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("numazu_maid_done")).i("n", share as i64),
    );
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_built_kani(player_id: i32) -> card_sdk::Asked {
    on_built(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn offer_kani(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}
