//! The status gates have two spellings: the `status:*` pseudo cards' CEL-only
//! `On::Gate` conditions (what the real ruleset answers) and
//! `MatchPlayer::status_gates` (the engine-side mirror `StubRules` runs). They
//! must agree on every layer combination. (`docs/STATUS-HOSTS.md`.)

mod common;

use common::*;
use game_core::engine::{CardRules, Gate};

#[test]
fn cel_gates_match_the_engine_mirror() {
    let rules = rules();
    let mut t = Table::vanilla(2);
    for stay in 0..3 {
        for stun in 0..2 {
            for stun_start in 0..2 {
                for exile in 0..2 {
                    for unstoppable in 0..2 {
                        t.set_state(0, "stay", stay);
                        t.set_state(0, "stun", stun);
                        t.set_state(0, "stunStart", stun_start);
                        t.set_state(0, "exile", exile);
                        t.set_state(0, "unstoppable", unstoppable);
                        let w = t.m.world();
                        let g = w.st.players[0].status_gates();
                        for (gate, want) in [
                            (Gate::Act, g.act),
                            (Gate::Pay, g.pay),
                            (Gate::Move, g.r#move),
                        ] {
                            let got = rules.gate_open(w, gate, 0);
                            assert_eq!(
                                got,
                                Some(want),
                                "{gate:?} with stay={stay} stun={stun} stunStart={stun_start} \
                                 exile={exile} unstoppable={unstoppable}"
                            );
                        }
                    }
                }
            }
        }
    }
}
