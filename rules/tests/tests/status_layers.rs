//! Status-layer behaviour over the generic engine services -- tick, clamp,
//! gate, un-latch, exile return as the main move, charge cap, stun payee,
//! noHand dance. Assertions name the frozen wire keys through
//! `bandori_std::state_key`; the engine keeps no named mirror.
//!
//! Wear-off and flooring are **declared by the writer** (`state_set_expires`
//! / `state_set_flags`), so the helpers here stamp them the way
//! `bandori_std::status` grant verbs do.

mod common;

use bandori_std::state_key;
use common::*;
use game_core::engine::Gate;
use game_core::state::{state_flag, Tick};

/// Layer write the way `bandori_std::status` grants it: NEVER_NEGATIVE +
/// ENGINE_OWNED, and the tick that peels one layer.
fn grant_layer(t: &mut Table, who: usize, key: &str, n: i32, tick: Tick) {
    let p = who as i32;
    let w = t.m.world_mut();
    w.apply_layer(p, key, n);
    w.state_set_flags(p, key, state_flag::NEVER_NEGATIVE | state_flag::ENGINE_OWNED);
    w.state_set_expires(p, key, Some(tick));
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// `rules.gate_open` for seat `who` (the ruleset CEL is the whole answer).
fn gate(t: &Table, gate: Gate, who: usize) -> bool {
    t.m.world().gate_open(rules().as_ref(), gate, who)
}

// =====================================================================
// Clamp: status counters floor at 0 (NEVER_NEGATIVE, declared by the writer)
// =====================================================================

/// 「清除」 floors at 0 -- a negative write cannot go below once the writer
/// declared `NEVER_NEGATIVE`.
#[test]
fn stay_floors_at_zero() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STAY, 1, Tick::TurnEnd);
    t.set_state(0, state_key::STAY, -5);
    assert_eq!(
        t.state(0, state_key::STAY),
        0,
        "[停留] floors at 0"
    );
}

#[test]
fn exile_floors_at_zero() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE, -5);
    assert_eq!(
        t.state(0, state_key::EXILE),
        0,
        "[除外] floors at 0"
    );
}

// =====================================================================
// Tick: stay / stun wear off one layer at turn end; stunStart at turn start
// =====================================================================

/// 规则书 48: 「玩家的每回合结束时移除一层」 -- stay drops one layer at
/// the owner's turn end, not all at once.
#[test]
fn stay_ticks_one_layer_at_turn_end() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STAY, 2, Tick::TurnEnd);
    pass(&mut t, 0);
    assert_eq!(
        t.state(0, state_key::STAY),
        1,
        "one layer wears off at turn end, one remains"
    );
}

/// 规则书 49: same tick for stun.
#[test]
fn stun_ticks_one_layer_at_turn_end() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STUN, 2, Tick::TurnEnd);
    pass(&mut t, 0);
    assert_eq!(
        t.state(0, state_key::STUN),
        1,
        "one [晕眩] layer wears off at turn end"
    );
}

/// `stunStart` folds into `stun` at the owner's turn start.
#[test]
fn stun_start_folds_at_turn_start() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STUN_START, 1, Tick::TurnStart);
    pass(&mut t, 0);
    pass(&mut t, 1);
    // P0's next turn start: the pending layer folds into `stun`.
    assert_eq!(
        t.state(0, state_key::STUN_START),
        0,
        "stunStart consumed at turn start"
    );
}

// =====================================================================
// Un-latch: the last stay layer restores the main move
// =====================================================================

/// Dropping the last stay layer un-skips the turn's main move
/// (`status:stay` count-changed clause, via `clear_move_skip`).
#[test]
fn stay_unlatch_restores_main_move() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STAY, 1, Tick::TurnEnd);
    t.m.world_mut().st.skip_move = true;
    // Clear the last layer and release the latch (the stay host's
    // count-changed clause).
    t.m.world_mut().apply_layer(0, state_key::STAY, -1);
    t.m.world_mut().clear_move_skip(0);
    assert!(
        !t.m.world().st.skip_move,
        "last [停留] layer un-latches the main move"
    );
}

/// exile is not part of the `skip_move` latch: clearing stay releases the
/// latch even while exiled; the move gate itself stays closed.
#[test]
fn stay_unlatch_ignores_exile() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::STAY, 1, Tick::TurnEnd);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.m.world_mut().st.skip_move = true;
    t.m.world_mut().apply_layer(0, state_key::STAY, -1);
    t.m.world_mut().clear_move_skip(0);
    assert!(!t.m.world().st.skip_move);
    assert!(
        !gate(&t, Gate::Move, 0),
        "[除外] keeps the move gate closed"
    );
}

// =====================================================================
// Gates
// =====================================================================

/// 规则书 47/48: stay 「处于该状态时[无法移动]」 -- the move gate closes.
/// 规则书 51: [不可阻挡] 「无视已存在的stay」 -- it opens again.
#[test]
fn stay_blocks_move_unless_unstoppable() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::STAY, 1);
    assert!(!gate(&t, Gate::Move, 0), "[停留] closes the move gate");
    t.set_state(0, state_key::UNSTOPPABLE, 1);
    assert!(
        gate(&t, Gate::Move, 0),
        "[不可阻挡] 「无视已存在的[停留]」"
    );
}

/// 规则书 49: stun closes move / act / pay.
#[test]
fn stun_closes_move_act_pay() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::STUN, 1);
    assert!(
        !gate(&t, Gate::Move, 0) && !gate(&t, Gate::Act, 0) && !gate(&t, Gate::Pay, 0),
        "[晕眩] closes move/act/pay"
    );
}

/// 规则书 50: exile closes move / act / pay / on-board / targeted.
#[test]
fn exile_closes_all_but_presence_is_inverted() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::EXILE, 1);
    assert!(
        !gate(&t, Gate::Move, 0) && !gate(&t, Gate::Act, 0) && !gate(&t, Gate::Pay, 0),
        "[除外] closes move/act/pay"
    );
    assert!(
        !gate(&t, Gate::OnBoard, 0),
        "「玩家棋子不失为存在于任何格子上」 = off board"
    );
    assert!(!gate(&t, Gate::Targetable, 0), "「不能被[指定]」");
}

// =====================================================================
// Charge: cap, partial gain
// =====================================================================

/// rulebook 126: 「your charge-pot count cannot exceed the cap」 -- gain is partial.
#[test]
fn charge_gain_is_partial_under_the_cap() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 1, 2);
    let got = t.m.world_mut().gain_marker(0, "charge", 5);
    let have = t.charge(0);
    assert!(have <= 2, "capped at 2, have {have} (gain reported {got})");
    assert_eq!(have, 2, "partial gain fills to the cap");
}

/// A cap change pulls the value down under the new ceiling.
#[test]
fn charge_cap_drop_pulls_value_down() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 5, 5);
    t.m.world_mut().add_marker_max(0, "charge", -3);
    assert!(t.charge(0) <= 2, "value pulled under the new cap of 2");
}

// =====================================================================
// Stun payee (play.rs 「stunned cannot receive」)
// =====================================================================

/// 规则书 49: 「无法收付款」 -- a stund player cannot *receive* payment.
#[test]
fn stun_payee_is_refused() {
    let mut t = Table::vanilla(3);
    t.set_state(2, state_key::STUN, 1);
    // A card that pays the player with the most money (or a designated payee)
    // is hard to aim; the gate itself is the unit under test.
    assert!(
        !gate(&t, Gate::Pay, 2),
        "stunned payee has the pay gate closed"
    );
}

// =====================================================================
// Exile return as the main move (MyGO:无路矢 `exileMain`)
// =====================================================================

/// The return teleport is consumed as that turn's main move: the player
/// cannot also roll.
#[test]
fn exile_return_counts_as_main_move() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE_TO, 5);
    t.set_state(0, state_key::EXILE_MAIN, 1);
    // Get to P0's next turn so the exile tick runs the return.
    if t.turn() != 0 {
        let cur = t.turn();
        pass(&mut t, cur);
    }
    pass(&mut t, 0);
    pass(&mut t, 1);
    // P0's turn: exile ticked out, return teleport ran as the main move.
    assert_eq!(t.pos(0), 5, "return [传送] landed on exileTo");
    assert!(
        t.m.world().turn.main_moved,
        "exileMain: the return teleport is the turn's [主要移动]"
    );
}

// =====================================================================
// noHand 2 -> 1 -> 0 (`status:no_hand` host hooks)
// =====================================================================

/// The host's turn-start fold: `noHand == 2` drops to `1` at the owner's
/// `TurnStart`.
#[test]
fn no_hand_folds_at_turn_start() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::NO_HAND, 2);
    pass(&mut t, 0);
    pass(&mut t, 1);
    assert_eq!(
        t.state(0, state_key::NO_HAND),
        1,
        "2 folds to 1 at turn start"
    );
}

/// The host's turn-end clear: `noHand == 1` drops to `0` at the owner's
/// `TurnEnd`.
#[test]
fn no_hand_clears_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::NO_HAND, 1);
    pass(&mut t, 0);
    assert_eq!(
        t.state(0, state_key::NO_HAND),
        0,
        "1 clears to 0 at turn end"
    );
}

/// `noHand == 2` is *not* cleared at turn end (only `1 -> 0` runs); it
/// survives to fold at the next turn start.
#[test]
fn no_hand_two_survives_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::NO_HAND, 2);
    pass(&mut t, 0);
    assert_eq!(
        t.state(0, state_key::NO_HAND),
        2,
        "2 is not the clear target; it waits for the next turn start"
    );
}

// =====================================================================
// STA-01 (stun half): [不可阻挡] 「晕眩效果也不会获得新的层数」
// =====================================================================

/// A positive stun grant while [不可阻挡] is up is refused by the host verb
/// (`status::stun::add`), so no layer lands. Existing layers still tick off.
#[test]
fn unstoppable_refuses_new_stun_layers() {
    let mut t = Table::vanilla(2);
    t.set_state(0, state_key::UNSTOPPABLE, 1);
    // Play a card whose body calls `rules_status::stun::add` at turn end
    // (Crychic:初演大成功 「回合结束后获得一层眩晕」).
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    pass(&mut t, 0);
    let stun = t.state(0, state_key::STUN) + t.state(0, state_key::STUN_START);
    assert_eq!(stun, 0, "[不可阻挡] 「晕眩效果也不会获得新的层数」 (STA-01)");
}