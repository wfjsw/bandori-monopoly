//! A second `end` while 结束 is already up is a no-op.
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

#[test]
fn repeated_end_during_settlement_logs_once() {
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
    // Run to seat 0's 运营.
    let mut guard = 0;
    loop {
        common::cross_mulligan(&mut m);
        common::drive(&mut m);
        let st = m.state();
        if st.phase == "play" && st.step == 2 && st.turn == 0 && !st.busy {
            break;
        }
        m.tick(0.05);
        guard += 1;
        assert!(guard < 20_000, "never reached 运营: {:?}", m.state().step);
    }
    let mark = m.world().recent.back().map_or(0, |e| e.id);
    // Roll and let the walk settle into 结束 (the stage holds after the walk;
    // the driven seat may spend the first end there).
    let _ = m.act(1, &NetMessage::act("roll"));
    let mut g = 0;
    while m.state().step != 4 {
        m.tick(0.05);
        common::drive(&mut m);
        g += 1;
        assert!(g < 400, "walk never settled: step {}", m.state().step);
    }
    // Duplicates that arrive before the next tick (the replay applies
    // back-to-back commands with no tick between them). A second end is
    // accepted as a no-op, never refused.
    for _ in 0..3 {
        assert!(
            m.act(1, &NetMessage::act("end")).is_ok(),
            "a duplicate end is accepted, not refused"
        );
    }
    m.tick(0.05);
    let keys: Vec<String> = m
        .events_since(mark)
        .into_iter()
        .map(|e| e.msg.key().to_string())
        .collect();
    let n = keys.iter().filter(|k| *k == "log.end_turn").count();
    assert_eq!(n, 1, "one 结束 phase logs one 结束了回合: {keys:?}");
}
