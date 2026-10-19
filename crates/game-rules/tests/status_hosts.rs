//! `status:*` host clauses -- tick, clamp, gate, un-latch, exile return as
//! the main move, charge cap / partial gain / `markerSpend` cancel, stun
//! payee. (`docs/STATUS-HOSTS.md` §5 Phase D.)

mod common;

use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

// =====================================================================
// Tick: stay / stun wear off one layer at turn end; stunStart at turn start
// =====================================================================

/// 规则书 48: 「玩家的每回合结束时移除一层」 -- stay drops one layer at
/// the owner's turn end, not all at once.
#[test]
fn stay_ticks_one_layer_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stay", 2);
    // Skip the move and end the turn: one layer leaves.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    let stay = t.m.world().st.players[0].state_get("stay");
    assert_eq!(stay, 1, "one layer wears off at turn end, one remains");
}

/// 规则书 49: same tick for stun.
#[test]
fn stun_ticks_one_layer_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stun", 2);
    // A stunned turn auto-skips; end_turn still runs the wear-off.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    let stun = t.m.world().st.players[0].state_get("stun");
    assert_eq!(stun, 1, "one [晕眩] layer wears off at turn end");
}

/// `stunStart` is the layer that only starts counting next turn
/// (`Tick::TurnStart`): it folds into `stun` at the owner's turn start.
#[test]
fn stun_start_folds_at_turn_start() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stunStart", 1);
    t.set_state(0, "stun", 0);
    // End P0's turn so the next P0 turn starts.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    // P1's turn: pass it.
    t.m.world_mut().st.skip_move = true;
    t.end(1).unwrap();
    drain(&mut t);
    // Now P0's turn start should have folded stunStart into stun (and then
    // the stun auto-skip ends the turn).
    let p = &t.m.world().st.players[0];
    assert_eq!(p.state_get("stunStart"), 0, "stunStart consumed at turn start");
}

// =====================================================================
// Clamp: status counters floor at 0
// =====================================================================

/// 「清除」 floors at 0 -- a negative write cannot go below.
#[test]
fn status_counters_floor_at_zero() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stay", 1);
    t.set_state(0, "stay", -5);
    assert_eq!(t.m.world().st.players[0].state_get("stay"), 0, "stay floors at 0");
    t.set_state(0, "exile", 1);
    t.set_state(0, "exile", -5);
    assert_eq!(t.m.world().st.players[0].state_get("exile"), 0, "exile floors at 0");
}

// =====================================================================
// Gates
// =====================================================================

/// 规则书 47/48: stay 「处于该状态时[无法移动]」 -- the move gate closes.
/// 规则书 51: [不可阻挡] 「无视已存在的stay」 -- it opens again.
#[test]
fn stay_blocks_move_unless_unstoppable() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stay", 1);
    let g = t.m.world().st.players[0].status_gates();
    assert!(!g.r#move, "[停留] closes the move gate");
    t.set_state(0, "unstoppable", 1);
    let g = t.m.world().st.players[0].status_gates();
    assert!(g.r#move, "[不可阻挡] 「无视已存在的[停留]」");
}

/// 规则书 49: stun closes move / act / pay.
#[test]
fn stun_closes_move_act_pay() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stun", 1);
    let g = t.m.world().st.players[0].status_gates();
    assert!(!g.r#move && !g.act && !g.pay, "[晕眩] closes move/act/pay");
}

/// 规则书 50: exile closes move / act / pay / on-board / targeted.
#[test]
fn exile_closes_all_but_presence_is_inverted() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "exile", 1);
    let g = t.m.world().st.players[0].status_gates();
    assert!(!g.r#move && !g.act && !g.pay, "[除外] closes move/act/pay");
    assert!(!g.on_board, "「玩家棋子不失为存在于任何格子上」 = off board");
    assert!(!g.targetable, "「不能被[指定]」");
}

// =====================================================================
// Un-latch: the last stay layer restores the main move
// =====================================================================

/// Dropping the last stay layer un-skips the turn's main move
/// (`status:stay` count-changed clause).
#[test]
fn stay_unlatch_restores_main_move() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stay", 1);
    t.m.world_mut().st.skip_move = true;
    // Clear the last layer through the engine verb (the clause runs).
    t.m.world_mut().give_stay(0, -1);
    assert!(
        !t.m.world().st.skip_move,
        "last [停留] layer un-latches the main move"
    );
}

/// exile is not part of the `skip_move` latch (glossary 13 grants
/// [无法移动] but not 「跳过主要移动阶段」): clearing stay releases the latch
/// even while exiled; the move gate itself stays closed.
#[test]
fn stay_unlatch_ignores_exile() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "stay", 1);
    t.set_state(0, "exile", 1);
    t.m.world_mut().st.skip_move = true;
    t.m.world_mut().give_stay(0, -1);
    assert!(!t.m.world().st.skip_move);
    assert!(!t.m.world().st.players[0].status_gates().r#move);
}

// =====================================================================
// Exile return as the main move (MyGO:无路矢 `exileMain`)
// =====================================================================

/// The return teleport is consumed as that turn's main move: the player
/// cannot also roll.
#[test]
fn exile_return_counts_as_main_move() {
    let mut t = Table::vanilla(2);
    // One exile layer, return to tile 5, as the main move.
    t.set_state(0, "exile", 1);
    t.set_state(0, "exileTo", 5);
    t.set_state(0, "exileMain", 1);
    // End P1 so we come back to P0... actually P0 is already the turn.
    // Force a new turn for P0: end P1 first if needed.
    if t.turn() != 0 {
        t.m.world_mut().st.skip_move = true;
        t.end(t.turn()).unwrap();
        drain(&mut t);
    }
    // End P0's current turn (exiled: auto-skip path) and let P1 pass so the
    // next P0 turn starts the exile tick.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    t.m.world_mut().st.skip_move = true;
    t.end(1).unwrap();
    drain(&mut t);
    // P0's turn: exile ticked out, return teleport ran as the main move.
    assert_eq!(t.pos(0), 5, "return [传送] landed on exileTo");
    assert!(
        t.m.world().turn.main_moved,
        "exileMain: the return teleport is the turn's [主要移动]"
    );
}

// =====================================================================
// Charge: cap, partial gain
// =====================================================================

/// rulebook 126: 「your charge-pot count cannot exceed the cap」 -- gain is partial.
#[test]
fn charge_gain_is_partial_under_the_cap() {
    let mut t = Table::vanilla(2);
    t.set_charge(0, 1, 2);
    let got = t.m.world_mut().gain_charge(0, 5, game_core::msg::Msg::new("test"));
    let have = t.charge(0);
    assert!(have <= 2, "capped at 2, have {have} (gain reported {got})");
    assert_eq!(have, 2, "partial gain fills to the cap");
}

/// A cap change pulls the value down under the new ceiling.
#[test]
fn charge_cap_drop_pulls_value_down() {
    let mut t = Table::vanilla(2);
    t.set_charge(0, 5, 5);
    t.m.world_mut().add_charge_max(0, -3);
    assert!(t.charge(0) <= 2, "value pulled under the new cap of 2");
}

// =====================================================================
// Stun payee (play.rs 「stunned cannot receive」)
// =====================================================================

/// 规则书 49: 「无法收付款」 -- a stund player cannot *receive* payment.
#[test]
fn stun_payee_is_refused() {
    let mut t = Table::vanilla(3);
    t.set_state(2, "stun", 1);
    // A card that pays the player with the most money (or a designated payee)
    // is hard to aim; the gate itself is the unit under test.
    let g = t.m.world().st.players[2].status_gates();
    assert!(!g.pay, "stunned payee has the pay gate closed");
}

// =====================================================================
// noHand 2 -> 1 -> 0 (`status:no_hand` host hooks; was inline in `play.rs`)
// =====================================================================

/// The host's turn-start fold: `noHand == 2` drops to `1` at the owner's
/// `TurnStart` (the 「this turn only」 half is spent). Identical to the old
/// inline dance.
#[test]
fn no_hand_folds_at_turn_start() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "noHand", 2);
    // End P0 (2 survives -- only 1 clears at turn end), then P1, so P0's
    // next turn start runs the fold.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    t.m.world_mut().st.skip_move = true;
    t.end(1).unwrap();
    drain(&mut t);
    // P0's turn has started: 2 -> 1.
    assert_eq!(t.m.world().st.players[0].state_get("noHand"), 1, "2 folds to 1 at turn start");
}

/// The host's turn-end clear: `noHand == 1` drops to `0` at the owner's
/// `TurnEnd`. Identical to the old inline dance.
#[test]
fn no_hand_clears_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "noHand", 1);
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.m.world().st.players[0].state_get("noHand"), 0, "1 clears to 0 at turn end");
}

/// `noHand == 2` is *not* cleared at turn end (only `1 -> 0` runs); it
/// survives to fold at the next turn start. Matches the old dance.
#[test]
fn no_hand_two_survives_turn_end() {
    let mut t = Table::vanilla(2);
    t.set_state(0, "noHand", 2);
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.m.world().st.players[0].state_get("noHand"),
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
    t.set_state(0, "unstoppable", 1);
    // Play a card whose body calls `rules_status::stun::add` at turn end
    // (Crychic:初演大成功 「回合结束后获得一层眩晕」).
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    let stun = t.state(0, "stun") + t.state(0, "stunStart");
    assert_eq!(stun, 0, "[不可阻挡] 「晕眩效果也不会获得新的层数」 (STA-01)");
}