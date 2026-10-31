//! A second `end` for a turn whose end is already up is a no-op -- whether it
//! lands on 结束 or on the same seat's extra turn that already opened.
//!
//! Clients send it twice: the accepting seat's own duplicate when its view
//! lags the state, and every other seat's client racing the same frame. The
//! record keeps those commands, so a replay used to re-run the whole end phase
//! per duplicate -- one more 「结束了回合」 line each, another status layer
//! worn off, another `turnEnd` round of hooks.

mod common;

use game_core::engine::{Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;

fn member(id: i32) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot: true,
        mentality: BotMentality::Standard,
        ..Default::default()
    }
}

fn match_4() -> Match {
    let members: Vec<RoomMember> = (1..=4).map(member).collect();
    let mut m = Match::new(
        common::data_arc(),
        std::sync::Arc::new(StubRules),
        &members,
        1,
        // Solo is the practice table; the hand-built 4-seat board must not
        // trip the book's 3-10 player bound.
        game_core::MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

/// Run to seat 0's 运营 and return that seat's member id. The tick comes
/// before the drive so the check catches 运营 before the bot seat rolls out of
/// it.
fn seat_zero_turn(m: &mut Match) -> i32 {
    let mut guard = 0;
    loop {
        common::cross_mulligan(m);
        m.tick(0.05);
        let st = m.state();
        if st.phase == "play" && st.step == 2 && st.turn == 0 && !st.busy {
            return st.players[0].member;
        }
        common::drive(m);
        guard += 1;
        assert!(guard < 20_000, "never reached 运营: {:?}", m.state().step);
    }
}

/// Roll and hold until 结束 (`stage::END`) is up -- without driving it, so the
/// test spends the first end itself.
fn to_settlement(m: &mut Match, me: i32) {
    let _ = m.act(me, &NetMessage::act("roll"));
    let mut g = 0;
    while m.state().step != 4 {
        m.tick(0.05);
        if m.state().step != 4 {
            common::drive(m);
        }
        g += 1;
        assert!(g < 400, "walk never settled: step {}", m.state().step);
    }
}

fn logged_ends(m: &Match, mark: i32) -> usize {
    m.events_since(mark)
        .into_iter()
        .filter(|e| e.msg.key() == "log.end_turn")
        .count()
}

#[test]
fn repeated_end_during_settlement_logs_once() {
    let mut m = match_4();
    let me = seat_zero_turn(&mut m);
    let mark = m.world().recent.back().map_or(0, |e| e.id);
    to_settlement(&mut m, me);
    // The first end is the real one; the rest are duplicates that arrive
    // before the next tick (the replay applies back-to-back commands with no
    // tick between them). A duplicate is accepted, not refused.
    assert!(m.act(me, &NetMessage::act("end")).is_ok());
    for _ in 0..3 {
        assert!(
            m.act(me, &NetMessage::act("end")).is_ok(),
            "a duplicate end is accepted, not refused"
        );
    }
    m.tick(0.05);
    assert_eq!(logged_ends(&m, mark), 1, "one 结束 phase, one 结束了回合");
}

#[test]
fn duplicate_end_on_the_extra_turn_does_not_end_it() {
    let mut m = match_4();
    let me = seat_zero_turn(&mut m);
    // Seat 0 has an extra turn queued behind this one.
    m.world_mut().extra_turns.push(0);
    let mark = m.world().recent.back().map_or(0, |e| e.id);
    to_settlement(&mut m, me);
    assert!(m.act(me, &NetMessage::act("end")).is_ok());
    // The queued turn keeps the seat and opens 开始 on the next tick: a late
    // duplicate lands there and must fold instead of ending the extra turn.
    let mut g = 0;
    while m.state().step != 1 {
        m.tick(0.05);
        g += 1;
        assert!(g < 40, "extra turn never opened: step {}", m.state().step);
    }
    assert_eq!(m.state().turn, 0);
    assert!(
        m.act(me, &NetMessage::act("end")).is_ok(),
        "a duplicate end on the extra turn is accepted"
    );
    for _ in 0..30 {
        m.tick(0.05);
    }
    assert_eq!(m.state().turn, 0, "the extra turn is still seat 0's");
    assert_eq!(m.state().step, 2, "and it reached 运营, not 结束");
    assert_eq!(logged_ends(&m, mark), 1, "only the first end logged");
}
