//! A second `end` while 结束 is already up is a no-op (`play.rs::end_turn_cmd`).
//!
//! Regression: every duplicate used to re-run the end phase -- another
//! 「结束了回合」 line, another status layer worn off, another `turnEnd` round.

mod common;

use common::Table;
use game_core::net::NetMessage;

#[test]
fn repeated_end_during_settlement_logs_once() {
    let mut t = Table::vanilla(3);
    assert!(t.prompt().is_none());
    t.roll(0).unwrap();
    // Walk it to 结束, answering anything that opens.
    let mut g = 0;
    while t.m.state().step != 4 {
        if let Some(p) = t.prompt() {
            let _ = t.m.act(
                1,
                &NetMessage {
                    prompt: p.id,
                    value: p.fallback,
                    ..NetMessage::act("answer")
                },
            );
        }
        t.m.tick(0.05);
        g += 1;
        assert!(g < 900, "walk never settled: step {}", t.m.state().step);
    }
    // Two [晕眩] layers: the turn end wears exactly one off.
    {
        let p = &mut t.m.world_mut().st.players[0];
        p.state_set("stun", 2);
        p.state_set_expires("stun", Some(game_core::state::Tick::TurnEnd));
    }
    let mark = t.mark();
    // Several clients racing the same frame: the first ends the turn, the rest
    // arrive before any tick and must fold into it.
    let _ = t.m.act(1, &NetMessage::act("end"));
    for _ in 0..4 {
        assert!(
            t.m.act(1, &NetMessage::act("end")).is_ok(),
            "a duplicate end is accepted, not refused"
        );
    }
    t.m.tick(0.05);
    let keys = t.keys_since(mark);
    let n = keys.iter().filter(|k| *k == "log.end_turn").count();
    assert_eq!(n, 1, "one 结束 phase logs one 结束了回合: {keys:?}");
    assert_eq!(
        t.state(0, "stun"),
        1,
        "one turn end wears one layer, not one per end"
    );
}
