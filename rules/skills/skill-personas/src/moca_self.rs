//! `skill:青叶摩卡:我行我素`
//!
//! 规则书（skill sheet, 青叶摩卡）:
//! > （1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子
//! > （不触发结算且该传送本身不引发其他被动效果（如玩家重叠））
//!
//! （1） is the shared Afterglow rest counter (see [`super::ran_red`]).
//!
//! （2） is a counteraction to *someone else's* act -- a skill press or a hand play --
//! and the teleport it buys is inert: 「不触发结算」 and 「不引发其他被动效果
//! （如玩家重叠）」. Both are the plan's own flags (`resolve: false`) plus the
//! fact that a plain position write raises nothing.
//!
//! Sheet 2026-10-17 drops the old 「该效果对每个玩家的被动技能和卡存在2次使用cd」
//! per-source cooldown; the offer is now unbounded apart from the pot.

use card_sdk::abi::ActionKind;
use bandori_std::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

use super::ran_red::REST_TURNS;

pub const MOCA_SELF: CardDef = CardDef::new(
    "skill:青叶摩卡:我行我素",
    &[
        On::Action(&[ActionKind::TurnStartBefore, ActionKind::DeckAtGameStart], "", "", None, declare_cap),
        On::Action(&[ActionKind::TurnEnd], "", "", Some(afterglow), tick),
        // （2）「当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]…」 -- the
        // offer exists only for another seat, with a pot to spend. Fire is CEL.
        On::Action(&[ActionKind::CardPlayed], "charge(owner) >= 1", "", Some(other_seat), on_card),
        On::Action(&[bandori_std::kind::SkillUsed], "charge(owner) >= 1", "", Some(other_seat), on_skill),
    ],
).silent(&[0])
    // 「（初始1，上限1）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 1), (bandori_std::fire::FIRE_CAP, 1)]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_group(player_id, "Afterglow")
}

/// Residual guard for the （2） offer: the actor is another seat.
fn other_seat(player_id: i32) -> bool {
    let src = ctx::trigger::player_id();
    src >= 0 && src != player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    super::ran_red::rest_tick(player_id)
}

/// （2） 「当他人…打出手牌时」.
fn on_card(player_id: i32) -> card_sdk::Asked {
    offer(player_id, ctx::trigger::player_id())?;
    Ok(())
}

/// （2） 「当他人使用角色技能时」.
fn on_skill(player_id: i32) -> card_sdk::Asked {
    offer(player_id, ctx::trigger::player_id())?;
    Ok(())
}

/// （2） 「你可以消耗一个[火罐]传送到其所在格子」.
/// Applicability is `charge(owner) >= 1` + `other_seat`.
fn offer(player_id: i32, src: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("moca_self_title")),
        &Msg::new(key!("moca_self_ask")).player_id("who", src),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("moca_self_spend")))? {
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    // 「不触发结算且该传送本身不引发其他被动效果」 -- a plain position write:
    // no move plan, so no pass/arrive/settle and no player-overlap bookkeeping.
    let to = ctx::player_pos(src);
    if to >= 0 {
        ctx::teleport_to(player_id, to);
        ctx::log(
            player_id,
            &Msg::new(key!("moca_self_moved")).tile("tile", to),
        );
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn tick_kani(player_id: i32) -> card_sdk::Asked {
    tick(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_card_kani(player_id: i32) -> card_sdk::Asked {
    on_card(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_skill_kani(player_id: i32) -> card_sdk::Asked {
    on_skill(player_id)
}
