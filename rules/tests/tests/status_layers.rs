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
    w.state_set_flags(
        p,
        key,
        state_flag::NEVER_NEGATIVE | state_flag::ENGINE_OWNED,
    );
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
    assert_eq!(t.state(0, state_key::STAY), 0, "[停留] floors at 0");
}

#[test]
fn exile_floors_at_zero() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE, -5);
    assert_eq!(t.state(0, state_key::EXILE), 0, "[除外] floors at 0");
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
        "last [停留] layer un-latches the is_main_move move"
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
    assert!(gate(&t, Gate::Move, 0), "[不可阻挡] 「无视已存在的[停留]」");
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
/// The kernel (`bandori_std::fire`) polices this; the engine is a dumb store.
#[test]
fn charge_gain_is_partial_under_the_cap() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 1, 2);
    let got = t.gain_fire(0, 5);
    let have = t.charge(0);
    assert!(have <= 2, "capped at 2, have {have} (gain reported {got})");
    assert_eq!(have, 2, "partial gain fills to the cap");
}

/// A cap change pulls the value down under the new ceiling (kernel pull-under).
#[test]
fn charge_cap_drop_pulls_value_down() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 5, 5);
    t.adjust_fire_cap(0, -3);
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

/// The return teleport is consumed as that turn's main move -- the player
/// cannot also roll -- and, as a main-move landing, it settles: the owned
/// landing tile offers its build. (C-Q21 still leaves the exact shape open;
/// the ordinary main-move landing settle is what lands.)
#[test]
fn exile_return_counts_as_main_move() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE_TO, 5);
    t.set_state(0, state_key::EXILE_MAIN, 1);
    // Own the return tile so the settle's own build offer is observable.
    t.own(0, &[5]);
    // Get to P0's next turn so the exile tick runs the return.
    if t.turn() != 0 {
        let cur = t.turn();
        pass(&mut t, cur);
    }
    pass(&mut t, 0);
    t.m.world_mut().st.skip_move = true;
    t.end(1).unwrap();
    // P0's turn start stopped on the settle's own build offer: the exile
    // ticked out and the return teleport ran as the main move, landing on the
    // owned deed with an ordinary landing settle.
    assert_eq!(t.pos(0), 5, "return [传送] landed on exileTo");
    assert!(
        t.m.world().turn.main_moved,
        "exileMain: the return teleport is the turn's [主要移动]"
    );
    assert!(
        t.dump_prompt().contains("ask.build"),
        "the return landing settled and offered the build: {}",
        t.dump_prompt()
    );
    drain(&mut t);
}

/// The return settle pays the landed deed's rent -- the whole point of
/// 「视为当回合的主要移动」: the return is a landing like the roll it
/// replaces, not a free hop.
#[test]
fn review_return_teleport_settles() {
    let mut t = Table::vanilla(2);
    t.own(1, &[5]);
    t.set_houses(5, 1); // rent table at one house: 280
    let before = t.money(0);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE_TO, 5);
    if t.turn() != 0 {
        let cur = t.turn();
        pass(&mut t, cur);
    }
    pass(&mut t, 0);
    pass(&mut t, 1);
    assert_eq!(t.pos(0), 5, "returned onto the housed deed");
    assert!(
        t.money(0) < before,
        "the return settle charged rent: {} -> {}",
        before,
        t.money(0)
    );
}

/// The return to an unowned deed opens the settle's own buy offer.
#[test]
fn review_return_teleport_offers_buy() {
    let mut t = Table::vanilla(2);
    grant_layer(&mut t, 0, state_key::EXILE, 1, Tick::TurnStart);
    t.set_state(0, state_key::EXILE_TO, 5);
    if t.turn() != 0 {
        let cur = t.turn();
        pass(&mut t, cur);
    }
    pass(&mut t, 0);
    t.m.world_mut().st.skip_move = true;
    t.end(1).unwrap();
    assert_eq!(t.pos(0), 5, "returned onto the unowned deed");
    assert!(
        t.dump_prompt().contains("ask.buy"),
        "the return landing offered the buy: {}",
        t.dump_prompt()
    );
    drain(&mut t);
}

// =====================================================================
// noHand 2 -> 1 -> 0 (`event:飞鸟山之战`'s own clause)
// =====================================================================

/// Advance until it is `who`'s 运营 stage (setup only -- before observe).
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == 2 {
            return;
        }
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Put `event` on top of the event deck and draw it with `who` on CiRCLE 咖啡厅
/// (「抽取一张手卡，然后抽取一个事件卡」), then decline everything. `faces` are
/// the dice after the landing face. The *next* seat is the one the veto lands on.
fn draw_event_quiet(t: &mut Table, who: usize, event: &str, faces: &[i32]) {
    // Keep the rest of the deck so the draw does not empty it.
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    let mut all = vec![1];
    all.extend_from_slice(faces);
    t.dice(&all);
    let _ = t.roll(who);
    drain(t);
}

/// End turns until it is `who`'s turn (the event's walk / a stun skip may
/// already have moved the clock on).
fn pass_until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// The event grants `noHand = 2` and stays in play; the holder's turn start
/// folds it to 1 (「下个」 half spent) without filing the event.
#[test]
fn no_hand_folds_at_turn_start() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    assert!(t.event_in_play("飞鸟山之战"), "the veto keeps the event in play");
    pass_until_turn(&mut t, 2);
    assert_eq!(
        t.state(2, state_key::NO_HAND),
        1,
        "2 folds to 1 at turn start"
    );
    assert!(
        t.event_in_play("飞鸟山之战"),
        "the event still has the clear to do"
    );
}

/// The holder's turn end clears 1 -> 0 and the event files itself into the
/// discard (its clause is spent).
#[test]
fn no_hand_clears_at_turn_end() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    pass_until_turn(&mut t, 2); // their turn start: 2 -> 1
    pass(&mut t, 2); // their turn end: 1 -> 0
    assert_eq!(
        t.state(2, state_key::NO_HAND),
        0,
        "1 clears to 0 at turn end"
    );
    assert!(
        !t.event_in_play("飞鸟山之战"),
        "the spent event left the board"
    );
    assert_eq!(
        t.event_discard(),
        vec!["飞鸟山之战".to_string()],
        "filed into the event discard"
    );
}

/// `noHand == 2` is *not* cleared at turn end (only `1 -> 0` runs); it survives
/// to fold at the next turn start, so the event must stay too.
#[test]
fn no_hand_two_survives_turn_end() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    pass_until_turn(&mut t, 2); // 2 -> 1 at their turn start
    t.set_state(2, state_key::NO_HAND, 2); // a second grant mid-turn
    pass(&mut t, 2);
    assert_eq!(
        t.state(2, state_key::NO_HAND),
        2,
        "2 is not the clear target; it waits for the next turn start"
    );
    assert!(
        t.event_in_play("飞鸟山之战"),
        "the re-armed veto keeps the event in play"
    );
}

/// A turn that exile auto-skips still spends the veto: the deadline passes
/// with that turn's end, so the event is not stretched by the skip. Driven by
/// the skip's own log line, so the assertion is "right after the skip" -- under
/// a `TurnStart` fold the value would still be 2 there.
#[test]
fn no_hand_spends_on_an_exile_skipped_turn() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    // Two layers: their turn start peels one and still auto-skips.
    grant_layer(&mut t, 2, state_key::EXILE, 2, Tick::TurnStart);
    let mut skipped = false;
    for _ in 0..6 {
        let cur = t.turn();
        pass(&mut t, cur);
        if t.recent_keys(60).iter().any(|k| k == "log.exiled_skip") {
            skipped = true;
            break;
        }
    }
    assert!(skipped, "the turn was exile-skipped: {:?}", t.recent_keys(80));
    assert_eq!(
        t.state(2, state_key::NO_HAND),
        0,
        "the skipped turn spent the veto"
    );
    assert!(
        !t.event_in_play("飞鸟山之战"),
        "the event lapsed with the skipped turn"
    );
}

/// A holder who is knocked out owes no turn end any more: the owed veto is
/// dropped with them and the event files instead of sitting in play forever.
#[test]
fn no_hand_files_the_event_when_the_holder_is_bankrupted() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    assert_eq!(t.state(2, state_key::NO_HAND), 2, "seat 2 owes the veto");
    // Bankrupt the holder on a five-house 购物中心 before their next turn.
    // The owner is the drawer (seat 1): the stunned seat 0 would close the
    // pay gate on both sides and no rent would ever land.
    t.set_money(2, 0);
    let mall = tile("购物中心");
    t.own(1, &[mall]);
    t.set_houses(mall, 5);
    t.begin_turn(2);
    t.set_pos(2, mall - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    assert!(t.p(2).bankrupt, "{:?}", t.recent_keys(20));
    assert_eq!(
        t.state(2, state_key::NO_HAND),
        0,
        "the out seat's veto is dropped"
    );
    assert!(
        !t.event_in_play("飞鸟山之战"),
        "the event files with the holder"
    );
}

/// Two holders over one instance: the event must survive the first clear and
/// file only when the last holder is clear. Order-agnostic -- the draw's walk
/// may already have moved the turn on.
#[test]
fn no_hand_stays_until_every_holder_is_clear() {
    let mut t = Table::vanilla(3);
    draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
    // Both seats owe (the shape a re-activation leaves: one shared instance).
    t.set_state(2, state_key::NO_HAND, 2);
    t.set_state(0, state_key::NO_HAND, 2);
    // Drop the event's 2 `stunStart` on seat 0: a stun-skipped turn ends inside
    // the same `pass` that ends seat 2's, leaving no observable point between
    // the two clears.
    t.set_state(0, state_key::STUN_START, 0);
    let mut clears = 0;
    let mut survived_first = false;
    for _ in 0..10 {
        let cur = t.turn();
        pass(&mut t, cur);
        let n = [0usize, 2]
            .iter()
            .filter(|p| t.state(**p, state_key::NO_HAND) == 0)
            .count();
        if n > clears {
            clears = n;
            if clears == 1 {
                survived_first = t.event_in_play("飞鸟山之战");
            }
        }
        if clears == 2 {
            break;
        }
    }
    assert_eq!(clears, 2, "both holders spent their veto");
    assert!(
        survived_first,
        "the first clear must not file the event: {:?}",
        t.recent_keys(60)
    );
    assert!(
        !t.event_in_play("飞鸟山之战"),
        "the last holder's clear files it"
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
    assert_eq!(
        stun, 0,
        "[不可阻挡] 「晕眩效果也不会获得新的层数」 (STA-01)"
    );
}
