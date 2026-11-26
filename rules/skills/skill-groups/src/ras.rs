//! `skill:RAISE A SUILEN:UNSTOPPABLE`
//!
//! 规则书（band sheet, RAISE A SUILEN）:
//! > （1）每回合可选择只获得第一次受到的[异常移动效果]。（2）因该技能以外的效果在
//! > livehouse格子[触发结算]时，若你的下回合开始时仍在那个格子上，你的下一次主要
//! > 移动可变为传送至你拥有的一个livehouse格子。
//!
//! （1） is a per-turn claim: once the first abnormal movement effect has landed
//! this turn, the rest are dropped. The abnormal gate is where that is decided.
//!
//! （2） latches a livehouse settle, then at the next turn start offers the
//! teleport if the player is still standing there.

use bandori_std::ctx::{self, plan, state};
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Filter, GateValue, Msg, On};

/// 「每回合可选择只获得第一次受到的[异常移动效果]」 -- 1 once one has landed.
const TAKEN: &str = "skill.ras.taken";
/// The livehouse tile the settle latched.
const ON_HOUSE: &str = "skill.ras.onHouse";
/// The teleport option has come due: the player stood on the remembered tile
/// at this turn's start, so the 运营-phase press may use it.
const ARMED: &str = "skill.ras.armed";

pub const RAS: CardDef = CardDef::new(
    "skill:RAISE A SUILEN:UNSTOPPABLE",
    &[
        // Turn-start bookkeeping: clear the per-turn latch and drop a stale
        // landing memory. Silent -- the player never sees this as a clause.
        On::Action(Action::TurnStartBefore(
            card_sdk::condition::MINE,
            None,
            at_turn_start_reset,
        ))
        .silent(),
        // （1）「每回合可选择只获得第一次受到的[异常移动效果]」 -- the drop of the
        // second+ hit belongs on `AbnormalGuard` (the declaration gate that can
        // refuse the effect); the `Abnormal` outcome hook is too late to cancel.
        // 「受到的」 is `target == owner` (the recipient), not `actor == owner`.
        // Pure decision (ABI v74): the one-shot latch is written by
        // [`mark_taken`] when the allowed effect actually lands.
        On::Filter(Filter::AbnormalGuard("target == owner", None, on_abnormal)),
        // One-shot latch for （1）: `Abnormal` reports what actually landed
        // (受到的), self-applied included. Paired with the pure gate above.
        On::Action(Action::Abnormal("target == owner", None, mark_taken)).silent(),
        On::Action(Action::Settle(card_sdk::condition::MINE, None, on_settle)).silent(),
        // （2）「自己的下回合开始时，如果自己仍在此卡[自动]（2）记录的格子上：可
        // 在运营阶段选择立刻进入移动阶段并将自己的[主要移动]改为[传送]…」 -- the
        // turn start arms it (`AT_START`), the press is this 主动 used in 运营.
        // The engine already refuses an `On::Play` outside the operations phase.
        // Destination presence is the residual guard.
        On::Play("", Some(cant_use), use_teleport),
    ],
)
.rule();

/// Turn-start bookkeeping: always clear the per-turn latch; a remembered
/// landing that is still underfoot arms the 运营-phase press, a stale one is
/// dropped. `ARMED` is what makes the press 下回合-only (a settle later this
/// turn does not arm it until the next turn start).
fn at_turn_start_reset(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, TAKEN, 0);
    let t = state::get(player_id, ON_HOUSE);
    if t >= 0 && ctx::tile_at(ctx::player_pos(player_id)) == t {
        state::set(player_id, ARMED, 1);
    } else {
        state::set(player_id, ARMED, 0);
        state::set(player_id, ON_HOUSE, -1);
    }
    Ok(())
}

/// Owned Live Houses the （2） teleport can land on. Residual-only (per-player
/// colour via `is_live_house_for`, outside the CEL schema -- docs/GUARDS.md §4.2).
fn owned_livehouses(player_id: i32) -> alloc::vec::Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&u| ctx::is_live_house_for(player_id, u))
        .collect()
}

/// Residual guard for the （2） press: armed at this turn's start, still on the
/// remembered livehouse, and owning at least one Live House to teleport to.
/// Derived scan (docs/GUARDS.md §4.2).
///
/// 规则书（band sheet, RAISE A SUILEN）:
/// > （1）自己的下回合开始时，如果自己仍在此卡[自动]（2）记录的格子上：可在运营
/// > 阶段选择立刻进入移动阶段并将自己的[主要移动]改为[传送]至自己拥有的1个Live
/// > House格子。
///
/// The press is 「可…」 -- it exists only when such a destination exists. A
/// player who owns no Live House has nothing the offer can name, so the entry
/// must not activate (no flash, no log).
fn still_on_remembered(player_id: i32) -> bool {
    let t = state::get(player_id, ON_HOUSE);
    state::get(player_id, ARMED) != 0
        && t >= 0
        && ctx::tile_at(ctx::player_pos(player_id)) == t
        && owned_livehouses(player_id).first().is_some()
}

/// [`On::Play`]'s residual gate: the press is blocked while the clause's
/// precondition does not hold (the reason names it).
fn cant_use(player_id: i32) -> Option<Msg> {
    if still_on_remembered(player_id) {
        None
    } else {
        Some(Msg::new(key!("ras_not_ready")))
    }
}

/// （1）「立刻进入移动阶段并将自己的[主要移动]改为[传送]至自己拥有的1个Live
/// House格子」 -- a press, so an [`On::Play`]. The walk runs now, which is
/// 「立刻进入移动阶段」; the destination is the first owned Live House.
fn use_teleport(player_id: i32) -> card_sdk::Asked {
    let Some(&to) = owned_livehouses(player_id).first() else {
        // Guard already admitted a destination; keep the accept path total.
        return Ok(());
    };
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(ctx::position_of_tile(to));
    plan::set_settles(true);
    state::set(player_id, ON_HOUSE, -1);
    state::set(player_id, ARMED, 0);
    ctx::run_planned_move(player_id);
    ctx::log(player_id, &Msg::new(key!("ras_moved")).tile("tile", to));
    Ok(())
}

/// （1）「每回合可选择只获得第一次受到的[异常移动效果]」 -- pure gate decision.
/// Allow the first hit of the turn, drop the rest. The one-shot latch is
/// written by [`mark_taken`] on the `Abnormal` landing (Filter bodies must be
/// pure; `state::set` is refuse_if_filter'd).
/// TODO(规则书): 「可选择」 -- the claim is automatic (no decline window for
///   the first hit). Same discrepancy as `m09_ras_band_first_abnormal_only`.
fn on_abnormal(player_id: i32, mut g: GateValue) -> GateValue {
    // Allow (leave the host's answer) on the first hit; deny the rest.
    if state::get(player_id, TAKEN) != 0 {
        g.allowed = Some(false);
    }
    g
}

/// One-shot latch for （1）: the allowed abnormal actually landed (「受到的」).
/// Rides `Abnormal` (what got through the gate and settled), not the gate
/// itself -- a pure FilterFlag cannot write.
fn mark_taken(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, TAKEN, 1);
    Ok(())
}

/// （2）「因该技能以外的效果在livehouse格子[触发结算]时」.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::by_card().is_some() {
        // 「因该技能以外的效果」 -- a card-driven settle still counts; what is
        // excluded is this skill's own teleport landing.
        return Ok(());
    }
    let t = ctx::trigger::tile();
    if t < 0 || !ctx::is_live_house_for(player_id, t) {
        return Ok(());
    }
    state::set(player_id, ON_HOUSE, t);
    Ok(())
}

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"RAISE A SUILEN"#)
    .short_name(r#"RAS"#)
    .color(r#"#22CCCC"#)
    .skill(r#"UNSTOPPABLE"#)
    .text(r#"[自动]：
（1）每回合，自己可选择只受到第一次受到的[异常移动效果]。
（2）自己因本技能以外的效果在Live House格子[结算]时：记录该格子，供此卡的[主动]（1）效果检查。
[主动]：
（1）自己的下回合开始时，如果自己仍在此卡[自动]（2）记录的格子上：可在运营阶段选择立刻进入移动阶段并将自己的[主要移动]改为[传送]至自己拥有的1个Live House格子。"#)
    .logo(r#"018"#);

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_reset_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start_reset(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_abnormal_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    on_abnormal(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn mark_taken_kani(player_id: i32) -> card_sdk::Asked {
    mark_taken(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_settle_kani(player_id: i32) -> card_sdk::Asked {
    on_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn use_teleport_kani(player_id: i32) -> card_sdk::Asked {
    use_teleport(player_id)
}
