//! G03 review regression: a blocked turn whose stay is lifted mid-turn must
//! not settle twice (once on the walk, once at end_turn).

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

#[test]
fn zz_blocked_then_lifted_settles_once() {
    let mut t = Table::new(&["户山香澄", "lock"]);
    t.strip_skills();
    t.clean();
    let target = tile("羽丘女子学院"); // idx 10, rent 220
    t.set_owner(target, Some(1));
    t.set_pos(0, target - 3); // 江户川公园, roll 3 lands on target
    t.set_state(0, "stay", 1);
    t.begin_turn(0);
    drain(&mut t);
    assert!(t.m.world().st.skip_move, "turn start was blocked");
    assert_eq!(
        t.m.world().blocked_settle_answer(0),
        game_core::engine::SettleAnswer::Settle,
        "blocked turn start stamps the withheld settle"
    );
    // Lift the stay mid-turn (hitoshizuku's status clear shape).
    t.m.world_mut()
        .apply_layer(0, bandori_std::state_key::STAY, -1);
    t.m.world_mut().clear_move_skip(0);
    t.dice(&[3, 0]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), target, "the main move ran");
    let after_walk = t.money(0);
    assert_eq!(after_walk, 10_000 - 220, "the walk settled the rent");
    t.end(0).unwrap();
    drain(&mut t);
    let after_end = t.money(0);
    assert_eq!(
        after_end, after_walk,
        "end_turn must not settle the destination tile again"
    );
}
