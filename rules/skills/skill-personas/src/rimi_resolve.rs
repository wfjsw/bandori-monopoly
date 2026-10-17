//! `skill:牛込里美:里美的决心`
//!
//! 规则书（skill sheet, 牛込里美）:
//! > （1）你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]
//! > （初始1，上限3）
//! > （2）运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离
//! > 最远的地产商并[结算]且可选择盖房。
//!
//! （1） 「你的回合中[经过]…后在回合结束时」 -- the pass is remembered and the
//! pot pays at turn end, so the two halves are a latch and a payout. 「Live
//! House颜色格子」 is `is_live_house_for`, which sees a recolour too.
//!
//! （2） 「与你绝对距离最远的地产商」 is the 地产商 (`kind=agent`) tile at the
//! maximum absolute `dist` from where the player stands; 「并[结算]」 is the
//! teleport resolving, and 「可选择盖房」 is the build offer that follows.

use card_sdk::abi::{state_key, ActionKind};
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「你的回合中[经过]」 -- latched on the pass, paid at turn end.
const PASSED: &str = "skill.rimiResolve.passed";

pub const RIMI_RESOLVE: CardDef = CardDef::new(
    "skill:牛込里美:里美的决心",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(&[ActionKind::TurnStartBefore, ActionKind::DeckAtGameStart], "", None, declare_cap),
        // （1）「你的回合中[经过]任意Live House颜色格子」 -- `is_live_house_for`
        // is player-relative (sees a recolour too) and not in the condition
        // vocabulary, so it is the residual guard. `actor == owner` is the pre.
        On::Action(&[ActionKind::Pass], card_sdk::pre::MINE, Some(on_pass_live_house), on_pass),
        On::Action(&[ActionKind::TurnEnd], "actor == owner && slot('skill.rimiResolve.passed') != 0", None, at_turn_end),
    ],
)
    .silent(&[1, 2]);

/// 「初始1，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 3);
    Ok(())
}

/// Residual guard for （1） 「任意Live House颜色格子」 -- `is_live_house_for`
/// stays here (not yet in the condition vocabulary; it is player-relative and
/// sees a recolour too).
fn on_pass_live_house(player_id: i32) -> bool {
    ctx::is_live_house_for(player_id, ctx::trigger::tile())
}

/// （1）「你的回合中[经过]任意Live House颜色格子」 -- latch only. The Live
/// House check is the residual guard; 「你的回合中」 is a conditional effect
/// (the latch is armed only on this player's own turn).
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if ctx::turn_player() == player_id {
        state::set(player_id, PASSED, 1);
    }
    Ok(())
}

/// （1）「后在回合结束时获得1个[火罐]」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, PASSED, 0);
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("rimi_resolve_gain")))?;
    Ok(())
}

/// （2） 「可选择使用3个[火罐]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 3 {
        return Some(Msg::new(key!("rimi_resolve_no_fire")));
    }
    None
}

/// （2）「[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房」.
/// 「地产商」 is the agent tile class (`kind=agent`), not the buyable set --
/// 可购买格子 explicitly excludes 地产商 (rulebook 「[可购买格子]」).
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let at = ctx::player_pos(player_id);
    let mut best = 0;
    let mut far: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    for t in 0..ctx::tile_count() {
        if !ctx::is_agent(t) {
            continue;
        }
        let d = ctx::dist(at, t);
        if d > best {
            best = d;
            far.clear();
            far.push(t);
        } else if d == best {
            far.push(t);
        }
    }
    if far.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("rimi_resolve_title")),
        &Msg::new(key!("rimi_resolve_ask")),
        &far.iter()
            .map(|&t| Msg::new(key!("rimi_resolve_option")).tile("tile", t))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&to) = far.get(pick) else {
        return Ok(());
    };
    if !rules_status::fire::spend(player_id, 3, &Msg::new(key!("rimi_resolve_spend")))? {
        return Ok(());
    }
    // 「[传送]…并[结算]」 -- a teleport that settles.
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(to);
    plan::set_resolve(true);
    ctx::card_move(player_id);
    // 「且可选择盖房」.
    ctx::card_offer_build(player_id, &[to]);
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
