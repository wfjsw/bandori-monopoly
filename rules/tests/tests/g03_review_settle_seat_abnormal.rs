//! G03 review regression: `blocked_settle_answer` must reach the seat an
//! abnormal effect hit when the counter fires during *another* player's turn.

mod common;
use common::*;

const OWN_STAGE: &str = "R:选择自己的舞台";
const RAIN: &str = "通用:雨啊，快点来吧";

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn fresh(chars: &[&str]) -> Table {
    let mut t = Table::new(chars);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// B answers Skip while A's turn runs; at B's own blocked turn end the settle
/// should be suppressed.
#[test]
fn probe_cross_turn_skip_lost() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(1, &[OWN_STAGE]);
    t.own(0, &[10]);
    t.set_pos(1, 10);
    t.give(0, &[RAIN]);
    drain(&mut t);
    t.dice(&[2, 2]);
    t.play(0, RAIN).unwrap();
    t.answer(0, 0).unwrap(); // the extra target
    assert!(t.counteract_offered(OWN_STAGE), "{}", t.dump_prompt());
    t.counteract(1, OWN_STAGE).unwrap();
    t.answer_one(1).unwrap(); // no
    drain(&mut t);
    assert_eq!(t.state(1, "stay"), 1, "the [停留] landed on B");
    let m1 = t.money(1);
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.turn(), 1, "B's turn opened");
    t.end(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(1),
        m1,
        "B answered Skip; the blocked turn end must not settle (pos {})",
        t.pos(1)
    );
}

/// B answers Settle while A's turn runs; the answer belongs to B's blocked
/// turn, so A's own end must not settle A a second time, and B's yes must
/// still fire at B's blocked turn end.
#[test]
fn probe_cross_turn_yes_waits_for_bs_turn() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(1, &[OWN_STAGE]);
    t.own(1, &[10]);
    t.own(0, &[11]);
    t.set_pos(0, 3);
    t.set_pos(1, 11);
    t.give(0, &[RAIN]);
    drain(&mut t);
    t.dice(&[2, 2]);
    t.play(0, RAIN).unwrap();
    t.answer(0, 0).unwrap();
    assert!(t.counteract_offered(OWN_STAGE), "{}", t.dump_prompt());
    t.counteract(1, OWN_STAGE).unwrap();
    t.answer_one(0).unwrap(); // yes
    drain(&mut t);
    // Drop the user's own [停留] (state and latch) so A still walks and settles
    // once normally at its landing.
    t.m.world_mut().st.players[0].state_set("stay", 0);
    t.m.world_mut().clear_move_skip(0);
    t.dice(&[7]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "A landed on 10");
    let after_move = t.money(0);
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        after_move,
        "A is not blocked at its own end; the answer waits for B ({} -> {})",
        after_move,
        t.money(0)
    );
    assert_eq!(t.turn(), 1, "B's turn opened");
    let before_b = t.money(1);
    t.end(1).unwrap();
    drain(&mut t);
    assert!(
        t.money(1) < before_b,
        "B's yes settles B at its own blocked turn end ({} -> {})",
        before_b,
        t.money(1)
    );
}
