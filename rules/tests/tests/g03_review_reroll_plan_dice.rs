//! G03 review regression: a card-driven move reroll must roll the *plan's* dice
//! tables (`1d20+1d4` here, `2d20` there), not a bare 1d20.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

#[test]
fn zz_kanon_second_roll_uses_the_plan_dice() {
    let mut t = Table::new(&["松原花音", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.m.world_mut().st.players[0].state_set("skill.kanonLost.silent", 3);
    // First roll 1d20+1d4 = 8+2 = 10; second 1d20+1d4 = 7+1 = 8; |10-8| = 2.
    t.dice(&[8, 2, 7, 1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        2,
        "the second roll must carry the +1d4: {:?}",
        t.recent_keys(20)
    );
}

#[test]
fn zz_starry_reroll_uses_the_plan_dice() {
    // The main move is 2d20 (a plan rewrite); the rerolled cast must be 2d20
    // too, so it consumes two dice. A bare 1d20 reroll would take only the 7
    // and the kept new face would walk 7.
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "Mor:蝴蝶飞舞的星月夜");
    t.set_crystals(0, "Mor:蝴蝶飞舞的星月夜", 2);
    t.m.world_mut().turn.plan.base[0].dice.count = 2;
    drain(&mut t);
    t.set_pos(0, 0);
    // Main cast 2d20 = 1+1 = 2. The reroll is 2d20 = 7+1 = 8; the kept new face
    // walks 8.
    t.dice(&[1, 1, 7, 1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_some(), "a reroll choice: {}", t.dump_prompt());
    t.answer_one(1).unwrap(); // keep the new cast
    drain(&mut t);
    let keys = t.recent_keys(30);
    assert!(
        keys.iter().any(|k| k.contains("starry_night_rerolled")),
        "the reroll ran: {keys:?}"
    );
    assert_eq!(t.pos(0), 8, "the reroll must be 2d20 too: {keys:?}");
}
