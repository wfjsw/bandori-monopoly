//! `skill:高松灯:诗超绊`（MyGO!!!!!）
//!
//! 规则书（skill sheet, 高松灯）:
//! > （1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）
//! > （2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
//! > [触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]，然后使其投掷1d20，
//! > 若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响
//! > 下的支付价格变为原本的一半。
//! > （3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在
//! > [触发结算]前将自己[传送]至那名玩家所在的格子并[触发结算]。
//!
//! （2） and （3） are the same shape in opposite directions: someone's landing
//! is within five tiles of the other, and a pot buys a teleport onto the other's
//! square *before* the settle. The 「前后5格内」 window is `dist` <= 5. 「那名
//! 玩家」 in （3） is the nearest other player in that window.
//!
//! 「并[触发结算]」 (both arms): the teleport happens in the `SettleBefore`
//! window (行动阶段 14 「[触发结算]前」). A relocation there retargets the
//! pending settle -- `after_walk` re-raises `settleBefore` at the new tile and
//! then settles **once** at the destination, never at the abandoned landing
//! (`play.rs` Q7 / `SETTLE-STAGES.md` §2). So 「传送…并[触发结算]」 is exactly
//! that redirected settle; no extra `card_settle_at` is needed (and adding one
//! would double-settle).
//!
//! （2）'s follow-up -- the 1d20, the [停留] on 6 or less, and 「支付价格变为
//! 原本的一半」 -- rides on the forced landing. The half-price is a pay bend on
//! that settle, which is what `PaySplitMul` is.

use bandori_std::ctx::{self, state, trigger};
use card_sdk::abi::{roll_source, ActionKind, FilterKind, RollPurpose};
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

/// Settle this player owes at their own next turn end, from （2）.
const DUE: &str = "skill.tomoriPoem.due";
/// Once per move: the press already ran on this landing (the settleBefore
/// redirect re-raises the window at the destination; do not ask again).
const ACTED: &str = "skill.tomoriPoem.acted";

pub const TOMORI_POEM: CardDef = CardDef::new(
    "skill:高松灯:诗超绊",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(
            bandori_std::guards::OWN_DEED,
            None,
            on_pass)),
        // （2）/（3） applicability: a landing within five tiles of the other
        // party. `actor == owner` is （3） (own landing near someone else);
        // `actor != owner` is （2） (someone else's landing near us). Both arms
        // live in the residual guard -- the CEL `pre` only needs a real tile.
        // The pot-cost split and the fire balance stay residual (dynamic).
        // `dist` / `pos` are CEL natives (docs/GUARDS.md §4.2).
        On::Action(Action::SettleBefore(
            "tile.id >= 0",
            Some(before_settle_applies),
            before_settle)),
        On::Filter(Filter::PaySplitMul("", Some(half), on_pay)),
        On::Action(Action::TurnEnd(
            "actor == owner && slot('skill.tomoriPoem.due') != 0",
            None,
            at_turn_end)),
        // Drop the once-per-move latch when the move is fully resolved (after
        // any settle), so the next move gets a fresh press.
        On::Action(Action::MoveResolved("", None, clear_acted)),
    ],
)
// 「（初始4，上限4）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 4),
    (bandori_std::fire::FIRE_CAP, 4),
]);

/// 「在此技能影响下的支付价格变为原本的一半」 -- only the payments of the
/// player this skill moved.
fn half(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, DUE) > 0
}

/// 「初始4，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]任意RiNG时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("tomori_poem_gain")))?;
    Ok(())
}

/// Residual guard for [`before_settle`]: the landing is within five tiles of
/// the owner, the (2)/(3) arm split holds, and this move has not already been
/// pressed (the `settleBefore` redirect re-raises the window at the
/// destination).
///
/// The `dist` scan is live board geometry (docs/GUARDS.md §4.2); it stays out
/// of the CEL `pre` so the host and the IR fill cannot disagree on
/// `dist(tile.id, pos(owner))`.
///
/// The fire balance is **not** a gate: 不再迷茫 (2) 「可移除此卡上的一个
/// [奇迹水晶]以代替此次技能的火罐消耗」 lets a crystal stand in for the pots,
/// so a 0-fire owner with a crystal can still press. [`before_settle`]'s
/// `spend_charge` is what actually collects the cost (fire or the substitution).
fn before_settle_applies(player_id: i32) -> bool {
    let landing = ctx::trigger::tile();
    if landing < 0 {
        return false;
    }
    let mover = ctx::trigger::player_id();
    if ctx::slot(mover, ACTED) != 0 {
        return false;
    }
    // （2）「当其他玩家的[移动终点]位于您前后5格内时」 -- someone else's
    // landing within five of this player.
    if mover != player_id {
        return ctx::dist(landing, ctx::player_pos(player_id)) <= 5;
    }
    // （3）「当你的[移动终点]位于其他玩家的前后五格内时」 -- this player's
    // landing within five of some other player.
    (0..ctx::player_count()).any(|p| {
        p != player_id && !ctx::player_out(p) && ctx::dist(landing, ctx::player_pos(p)) <= 5
    })
}

/// （2） and （3） -- the landing is within five tiles of the other party, and a
/// pot buys a teleport onto their square before the settle.
/// Applicability is the `pre` + [`before_settle_applies`]; this side only
/// spends the pot and teleports.
///
/// 「并[触发结算]」: `teleport_to` inside `SettleBefore` is a bare position
/// write (`card_teleport`, `move_depth > 0`); `after_walk` then re-raises
/// `settleBefore` at the new tile and settles **once** there (Q7). The
/// abandoned landing never settles. Latch `ACTED` so that redirect does not
/// open this press a second time.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    // （3） is this player's own landing near another (cost 1, teleport to
    // 「那名玩家」); （2） is someone else's landing near this player (cost 4,
    // teleport to *this* player's square). Same split as the guard.
    let (cost, theirs) = if mover == player_id {
        let landing = ctx::trigger::tile();
        // 「那名玩家所在的格子」 -- the other party within five tiles. Nearest
        // wins when several qualify (「前后五格内」 names one window).
        let mut best = -1;
        let mut best_d = i32::MAX;
        for p in 0..ctx::player_count() {
            if p == player_id || ctx::player_out(p) {
                continue;
            }
            let d = ctx::dist(landing, ctx::player_pos(p));
            if d <= 5 && d < best_d {
                best_d = d;
                best = ctx::player_pos(p);
            }
        }
        (1, best)
    } else {
        (4, ctx::player_pos(player_id))
    };
    if theirs < 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_poem_title")),
        &Msg::new(key!("tomori_poem_ask")).i("n", cost as i64),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, cost, &Msg::new(key!("tomori_poem_spend")))? {
        return Ok(());
    }
    // `teleport_to` runs the [传送] abnormal gate itself (`HostRequest::Teleport`).
    // A separate `ctx::gate` pre-check would declare the abnormal twice and the
    // second window can refuse after the first landed.
    ctx::teleport_to(mover, theirs);
    state::set(mover, ACTED, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_poem_moved"))
            .player_id("who", mover)
            .tile("tile", theirs),
    );
    if cost == 4 {
        // （2）「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的
        // 下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半」.
        let n = ctx::roll(mover, 1, 20, RollPurpose::Effect, roll_source::SKILL)?;
        if n <= 6 {
            rules_status::stay::add(mover, 1);
            state::set(player_id, DUE, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("tomori_poem_stay")).i("n", n as i64),
            );
        }
    }
    Ok(())
}

/// Drop the once-per-move latch (`ACTED`) when the mover's move is done.
fn clear_acted(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        state::set(mover, ACTED, 0);
    }
    Ok(())
}

/// 「在此技能影响下的支付价格变为原本的一半」.
fn on_pay(_player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    (value + 1) / 2
}

/// 「并在自己的下回合结束时[触发结算]」 -- the owed settle.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, DUE, 0);
    let at = ctx::player_pos(player_id);
    if at < 0 {
        return Ok(());
    }
    ctx::card_settle_at(player_id, at, true);
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
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn before_settle_kani(player_id: i32) -> card_sdk::Asked {
    before_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32, value: i32) -> i32 {
    on_pay(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_acted_kani(player_id: i32) -> card_sdk::Asked {
    clear_acted(player_id)
}
