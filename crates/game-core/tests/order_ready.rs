//! The 1d20 turn-order phase ends on every seat's ack, not on a fixed wait.
//!
//! G-FLOW-13 rolls the order at match construction. The phase used to burn a
//! flat 4s no matter who was ready; it now leaves as soon as each seat's
//! client has the rolled order on screen. `ORDER_WAIT` stays the cap for a
//! seat that never acks (closed / hidden tab). The acks are `Input::OrderReady`
//! in the record, so these tests also pin the replay shape.

use std::sync::Arc;

use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
mod common;
use common::data_arc as data;
use common::Pump;

fn members(n: usize, bots: usize) -> Vec<RoomMember> {
    (1..=n as i32)
        .map(|id| RoomMember {
            id,
            player: format!("P{id}"),
            bot: id as usize > n - bots,
            ..Default::default()
        })
        .collect()
}

fn new_match(n: usize, bots: usize, mode: MatchMode) -> Match {
    Match::new(
        data(),
        Arc::new(StubRules),
        &members(n, bots),
        7,
        mode,
        ScoreWeights::default(),
    )
}

fn phase(m: &Match) -> String {
    m.state().phase.clone()
}

/// Tick one 0.1s quantum, panicking if the phase was already past `order`.
fn tick_order(m: &mut Match) {
    assert_eq!(phase(m), "order", "phase left the order beat");
    m.pump_tick(0.1);
}

#[test]
fn every_client_ack_ends_the_order_phase() {
    let mut m = new_match(3, 0, MatchMode::Casual);
    assert_eq!(phase(&m), "order");
    // The phase does not spin on its own while acks are missing.
    for _ in 0..10 {
        tick_order(&mut m);
    }
    assert_eq!(phase(&m), "order");
    let ids: Vec<i32> = m.state().players.iter().map(|p| p.member).collect();
    for id in ids {
        assert!(m.order_ready(id), "ack from member {id} registers");
    }
    // The settle beat, then the pick.
    for _ in 0..10 {
        if phase(&m) == "pick" {
            break;
        }
        tick_order(&mut m);
    }
    assert_eq!(phase(&m), "pick");
}

/// Tick through the settle beat until the phase moves.
fn leave_order(m: &mut Match) {
    for _ in 0..10 {
        if phase(m) != "order" {
            return;
        }
        m.pump_tick(0.1);
    }
    panic!("order phase never ended");
}

#[test]
fn bots_do_not_need_an_ack() {
    let mut m = new_match(3, 2, MatchMode::Casual);
    let human = m
        .state()
        .players
        .iter()
        .find(|p| !p.bot)
        .map(|p| p.member)
        .unwrap();
    assert!(m.order_ready(human));
    leave_order(&mut m);
    assert_eq!(phase(&m), "pick");
}

#[test]
fn a_missing_ack_waits_out_the_cap() {
    let mut m = new_match(2, 0, MatchMode::Casual);
    let ids: Vec<i32> = m.state().players.iter().map(|p| p.member).collect();
    assert!(m.order_ready(ids[0]));
    for _ in 0..38 {
        tick_order(&mut m);
    }
    assert_eq!(
        phase(&m),
        "order",
        "one acking client is not the whole table"
    );
    // Past the 4s cap the phase leaves regardless.
    leave_order(&mut m);
    assert_eq!(phase(&m), "pick");
}

#[test]
fn ranked_order_leaves_to_the_ban_walk() {
    let mut m = new_match(5, 0, MatchMode::Ranked);
    let ids: Vec<i32> = m.state().players.iter().map(|p| p.member).collect();
    for id in ids {
        m.order_ready(id);
    }
    leave_order(&mut m);
    assert_eq!(phase(&m), "ban");
    assert_eq!(m.state().turn, 4, "ban starts from the last seat");
}

#[test]
fn acks_outside_the_order_phase_and_from_non_seats_are_ignored() {
    let mut m = new_match(2, 0, MatchMode::Casual);
    assert!(!m.order_ready(99), "no such member");
    let ids: Vec<i32> = m.state().players.iter().map(|p| p.member).collect();
    for id in &ids {
        assert!(m.order_ready(*id));
    }
    leave_order(&mut m);
    assert_eq!(phase(&m), "pick");
    for id in ids {
        assert!(!m.order_ready(id), "phase already moved on");
    }
}

#[test]
fn a_save_keeps_the_acks_it_already_has() {
    let mut m = new_match(2, 0, MatchMode::Casual);
    let ids: Vec<i32> = m.state().players.iter().map(|p| p.member).collect();
    assert!(m.order_ready(ids[0]));
    let blob = m.save();
    let mut back = Match::restore(data(), Arc::new(StubRules), &blob).unwrap();
    assert!(
        !back.order_ready(ids[0]),
        "the restored seat is already acked"
    );
    assert!(back.order_ready(ids[1]));
    leave_order(&mut back);
    assert_eq!(phase(&back), "pick");
}
