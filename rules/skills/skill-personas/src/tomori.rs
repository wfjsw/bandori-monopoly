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
//! that redirected settle; no extra `settle_tile_at` is needed (and adding one
//! would double-settle).
//!
//! （2）'s follow-up -- the 1d20, the [停留] on 6 or less, the settle owed at
//! the moved player's own next turn end, and 「支付价格变为原本的一半」 --
//! belongs to the **moved** player, not the owner. The move is 「其」/「自己」
//! throughout. The half-price is a pay bend, which is what `PaySplitMul` is.
//!
//! （2） is the **borrowed** clause: 内心的呐喊 (3) and 稍微努力了一下 (3) run it
//! through `ctx::invoke_entry(_paid)` on the declared entry id `"2"`, not through
//! the first Play (the skill has none). Everything the borrow must see -- the
//! mover, the abandoned landing, the skill's instance -- comes from the
//! invocation frame, and a borrower that already paid is visible through
//! `ctx::invoke_paid()`.

use bandori_std::ctx::{self, state, trigger};
use card_sdk::abi::{roll_source, ActionKind, FilterKind, RollPurpose};
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

/// Settle this player owes at their own next turn end, from （2）.
const OWED_SETTLE: &str = "skill.tomoriPoem.owedSettle";
/// Once per move: the press already ran on this landing (the settleBefore
/// redirect re-raises the window at the destination; do not ask again). The
/// borrowing card reads this key too, so a borrowed use is not followed by the
/// skill offering itself again.
const USED_THIS_MOVE: &str = "skill.tomoriPoem.usedThisMove";
/// 「在此技能影响下的支付价格变为原本的一半」 -- on the moved player, around
/// the forced settle and the owed settle only.
const HALF_PRICE: &str = "skill.tomoriPoem.halfPrice";
/// Set by a borrowing card while it runs. A borrowing controller holds a
/// variant persona, not this skill, so this card's `Filter` hook is not on
/// their field at all: the borrower carries the one bend and this filter must
/// stand down wherever both could answer.
const BORROWED: &str = "skill.tomoriPoem.borrowed";

pub const TOMORI: CardDef = CardDef::new(
    "skill:高松灯:诗超绊",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_DEED, None, on_pass)),
        // （2）/（3） applicability: a landing within five tiles of the other
        // party. `actor == owner` is （3） (own landing near someone else);
        // `actor != owner` is （2） (someone else's landing near us). Both arms
        // live in the residual guard -- the CEL `condition` only needs a real tile.
        // The pot-cost split and the fire balance stay residual (dynamic).
        // `dist` / `pos` are CEL natives (docs/GUARDS.md §4.2).
        On::Action(Action::SettleBefore(
            "tile.id >= 0",
            Some(before_settle_applies),
            before_settle,
        ))
        // 「你的（2）技能」 -- the declared id a borrowing card invokes. The
        // first Play is not one: this skill has no `On::Play` of its own.
        .id("2")
        .label("（2）"),
        // The half-price rides on the payer, whoever settled: the moved
        // player on the forced settle, the same player again on the owed one.
        // `is_placed` is not required -- the owed settle runs from a schedule,
        // after this instance may be gone.
        On::Filter(Filter::PaySplitMul("", Some(half_price_applies), on_pay)),
        // 「并在自己的下回合结束时[触发结算]」 -- a schedule, not a TurnEnd
        // hook: the moved player need not be this card's owner.
        On::Schedule("owed_settle", "", None, owed_settle),
        // The owed settle's payment happens after `owed_settle` returns (the
        // host request is deferred); drop the half-price latch when it is done.
        // Only a card-forced settle (`settle_redirect`) is that one.
        On::Action(Action::SettleAfter("", None, clear_owed_paying)),
        // Drop the once-per-move latch when the move is fully resolved (after
        // any settle), so the next move gets a fresh press.
        On::Action(Action::MoveResolved("", None, clear_used_this_move)),
    ],
).rule()
// 「（初始4，上限4）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 4),
    (bandori_std::fire::FIRE_CAP, 4),
]);

/// 「初始4，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]任意RiNG时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("tomori_gain")))?;
    Ok(())
}

/// Residual guard for [`before_settle`]: the landing is within five tiles of
/// the owner, the (2)/(3) arm split holds, and this move has not already been
/// pressed (the `settleBefore` redirect re-raises the window at the
/// destination).
///
/// The `dist` scan is live board geometry (docs/GUARDS.md §4.2); it stays out
/// of the CEL `condition` so the host and the IR fill cannot disagree on
/// `dist(tile.id, pos(owner))`.
///
/// The fire balance is **not** a gate: 不再迷茫 (2) 「可移除此卡上的一个
/// [奇迹水晶]以代替此次技能的火罐消耗」 lets a crystal stand in for the pots,
/// so a 0-fire owner with a crystal can still press. [`before_settle`]'s
/// `spend_charge` is what actually collects the cost (fire or the substitution).
fn before_settle_applies(player_id: i32) -> bool {
    let landing = ctx::trigger::position();
    if landing < 0 {
        return false;
    }
    let mover = ctx::trigger::player_id();
    if ctx::slot(mover, USED_THIS_MOVE) != 0 {
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

/// 「那名玩家所在的格子」 -- the other party within five tiles of the landing.
/// Nearest wins when several qualify (「前后五格内」 names one window).
/// Returns the **slot** they stand on (`teleport_to` writes a position); the
/// caller converts for the log.
// TODO(ABI): nearest-within-5 over `ctx::dist`; `ctx::nearest(Shortest)`
// answers the half, the range stays local.
fn nearest_other_position(player_id: i32) -> i32 {
    // Distance is a property of the physical slots, not of the tile
    // identities: on a swapped board the slot is what is walked.
    let landing = ctx::trigger::position();
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
    best
}

/// （2） and （3） -- the landing is within five tiles of the other party, and a
/// pot buys a teleport onto their square before the settle.
/// Applicability is the `condition` + [`before_settle_applies`].
///
/// 「并[触发结算]」: `teleport_to` inside `SettleBefore` is a bare position
/// write (`card_teleport`, `move_depth > 0`); `after_walk` then re-raises
/// `settleBefore` at the new tile and settles **once** there (Q7). The
/// abandoned landing never settles. Latch `USED_THIS_MOVE` so that redirect
/// does not open this press a second time.
///
/// A borrower (内心的呐喊 (3), 稍微努力了一下 (3)) has already asked and paid:
/// `ctx::invoke_paid()` is true and only the body runs.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    // （3） is this player's own landing near another (cost 1, teleport to
    // 「那名玩家」); （2） is someone else's landing near this player (cost 4,
    // teleport to *this* player's square). Same split as the guard.
    // `destination` is a board position (`teleport_to` writes the slot);
    // the log below names its tile.
    let (cost, destination) = if mover == player_id {
        (1, nearest_other_position(player_id))
    } else {
        (4, ctx::player_pos(player_id))
    };
    if destination < 0 {
        return Ok(());
    }
    // 「可以选择使用4个火罐」 -- the skill's own offer. A borrower already paid
    // its own (substituted) cost; collecting these four too would double-charge.
    if !ctx::invoke_paid() {
        if !ctx::ask_yes(
            player_id,
            &Msg::new(key!("tomori_title")),
            &Msg::new(key!("tomori_ask")).i("n", cost as i64),
        )? {
            return Ok(());
        }
        if !bandori_std::fire::spend_skill(player_id, cost, &Msg::new(key!("tomori_spend")))? {
            return Ok(());
        }
    }
    state::set(mover, USED_THIS_MOVE, 1);
    state::set(mover, HALF_PRICE, 1);
    // `teleport_to` runs the [传送] abnormal gate itself (`HostRequest::Teleport`).
    // A separate `ctx::gate` pre-check would declare the abnormal twice and the
    // second window can refuse after the first landed.
    ctx::teleport_to(mover, destination);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_moved"))
            .player_id("who", mover)
            .tile("tile", ctx::tile_at(destination)),
    );
    if mover != player_id {
        // （2）「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的
        // 下回合结束时[触发结算]」 -- 「其」 and 「自己」 are the moved player,
        // so the roll, the stay and the owed settle are all theirs. The follow-up
        // belongs to the (2) arm, not the cost: a borrowed (3) never gets here.
        let n = ctx::roll(mover, 1, 20, RollPurpose::Effect, roll_source::SKILL)?;
        if n <= 6 {
            rules_status::stay::add(mover, 1);
            state::set(mover, OWED_SETTLE, 1);
            ctx::at_next_turn_end(mover, "owed_settle");
            ctx::log(player_id, &Msg::new(key!("tomori_stay")).i("n", n as i64));
        }
    }
    Ok(())
}

/// Drop the once-per-move latch when the mover's move is done, and the
/// half-price latch with it -- unless an owed settle is still pending, which
/// pays later and must stay halved.
fn clear_used_this_move(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        state::set(mover, USED_THIS_MOVE, 0);
        if state::get(mover, OWED_SETTLE) == 0 {
            state::set(mover, HALF_PRICE, 0);
            state::set(mover, BORROWED, 0);
        }
    }
    Ok(())
}

/// 「在此技能影响下的支付价格变为原本的一半」 -- the guard's argument is the
/// card **owner**; the payment's payer is `trigger.player_id`, and the latch
/// rides on the payer. A borrowing card has already armed its own bend
/// (`BORROWED`); the two must not compose on one payment.
fn half_price_applies(_player_id: i32) -> bool {
    let payer = ctx::trigger::player_id();
    ctx::trigger::settle_tile() >= 0
        && state::get(payer, HALF_PRICE) > 0
        && state::get(payer, BORROWED) == 0
}

/// 「在此技能影响下的支付价格变为原本的一半」.
fn on_pay(_player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    (value + 1) / 2
}

/// 「并在自己的下回合结束时[触发结算]」 -- the owed settle, at the moved
/// player's own position, at the end of their next turn. Not a main move.
fn owed_settle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, OWED_SETTLE, 0);
    // A borrow's own bend is long gone; this is this skill's settle.
    state::set(player_id, BORROWED, 0);
    let at = ctx::player_pos(player_id);
    if at < 0 {
        state::set(player_id, HALF_PRICE, 0);
        return Ok(());
    }
    // `HALF_PRICE` is still armed from the forced settle (its `MoveResolved`
    // drop was skipped while owed); it must stay up for this payment, and the
    // deferred `settleAfter` drops it.
    ctx::settle_tile_at(player_id, ctx::tile_at(at), false);
    Ok(())
}

/// The owed settle finished paying: drop the latches. It is a **card-forced**
/// settle (`settle_redirect`); the mover's own settle is the walk's and must
/// keep the latch for the owed one. `settleAfter` fires for every settle.
fn clear_owed_paying(_player_id: i32) -> card_sdk::Asked {
    if !ctx::trigger::is_settle_redirect() {
        return Ok(());
    }
    let mover = ctx::trigger::player_id();
    if mover >= 0 && state::get(mover, HALF_PRICE) > 0 {
        state::set(mover, HALF_PRICE, 0);
        state::set(mover, BORROWED, 0);
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
pub fn half_price_applies_kani(player_id: i32) -> bool {
    half_price_applies(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn owed_settle_kani(player_id: i32) -> card_sdk::Asked {
    owed_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_owed_paying_kani(player_id: i32) -> card_sdk::Asked {
    clear_owed_paying(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_used_this_move_kani(player_id: i32) -> card_sdk::Asked {
    clear_used_this_move(player_id)
}
