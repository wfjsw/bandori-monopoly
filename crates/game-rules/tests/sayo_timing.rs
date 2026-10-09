//! Sayo's optional extension belongs after the resolved distance, before walking.
mod common;
use common::*;
use game_core::engine::MoveKind;
use game_core::msg::Arg;

const SKILL: &str = "skill:冰川纱夜:踏上荆棘之路的觉悟";

fn sayo() -> Table {
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 10, 10);
    t.set_pos(0, 1);
    t.set_pos(1, 30);
    t
}

fn offer(t: &mut Table) {
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert!(
        p.title.key().ends_with("sayo_thorns_title"),
        "{}",
        t.dump_prompt()
    );
    assert_eq!(t.pos(0), 1, "the choice must precede the first step");
    assert_eq!(t.fire(0), 10, "opening the choice must not spend pots");
    assert_eq!(
        p.fallback, 0,
        "timeout/default must skip the optional skill"
    );
    assert_eq!(p.options.len(), 3);
}

#[test]
fn offer_shows_resolved_distance_and_each_landing_before_walking() {
    let mut t = sayo();
    offer(&mut t);
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(5)));
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(6)));
    assert_eq!(p.options[1].a.get("tile"), Some(&Arg::Tile(7)));
    assert_eq!(p.options[2].a.get("tile"), Some(&Arg::Tile(8)));
}

#[test]
fn choosing_one_or_two_extends_this_move_and_spends_six_once() {
    for add in [1, 2] {
        let mut t = sayo();
        offer(&mut t);
        t.answer(0, add).unwrap();
        assert_eq!(t.pos(0), 6 + add as usize);
        assert_eq!(t.fire(0), 4);
        assert_eq!(t.m.world().turn.main_steps, 5 + add);
        assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    }
}

#[test]
fn skipping_keeps_original_distance_and_fire() {
    let mut t = sayo();
    offer(&mut t);
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.fire(0), 10);
}

#[test]
fn manual_skill_cannot_be_used_before_the_roll() {
    let mut t = sayo();
    assert!(t.skill(0, SKILL).is_err());
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none());
    offer(&mut t);
}

#[test]
fn insufficient_fire_or_teleport_does_not_offer_or_spend() {
    for teleport in [false, true] {
        let mut t = sayo();
        if teleport {
            t.m.world_mut().turn.plan.kind = MoveKind::Teleport;
        } else {
            t.set_fire(0, 5, 10);
        }
        let fire = t.fire(0);
        t.dice(&[5]);
        t.roll(0).unwrap();
        assert!(t.prompt().is_none(), "{}", t.dump_prompt());
        assert_eq!(t.fire(0), fire);
    }
}

#[test]
fn forced_side_move_does_not_offer_the_main_move_skill() {
    let mut t = sayo();
    t.set_pos(0, 10);
    t.m.world_mut().turn.plan.forced = true;
    t.give_play(0, "TEST:mover").unwrap();
    while let Some(p) = t.prompt() {
        assert!(!p.title.key().ends_with("sayo_thorns_title"));
        t.decline();
    }
    assert_eq!(t.fire(0), 10);
    assert!(!t.m.world().turn.main_moved);
}

#[test]
fn final_distance_override_and_existing_extra_steps_are_preserved() {
    let mut t = sayo();
    t.m.world_mut().hidden[0].next_steps = Some(7);
    t.m.world_mut().turn.plan.extra_steps = 2;
    offer(&mut t);
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(9)));
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(10)));
    t.answer(0, 2).unwrap();
    assert_eq!(t.pos(0), 12, "7 determined + 2 existing + 2 Sayo");
    assert_eq!(t.fire(0), 4);
}

#[test]
fn reverse_move_previews_and_extends_in_the_same_direction() {
    let mut t = sayo();
    t.set_pos(0, 20);
    t.m.world_mut().turn.plan.reverse = true;
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(15)));
    assert_eq!(p.options[2].a.get("tile"), Some(&Arg::Tile(13)));
    t.answer(0, 2).unwrap();
    assert_eq!(t.pos(0), 13);
    assert_eq!(t.fire(0), 4);
}

#[test]
fn use_is_once_per_turn_and_resets_on_the_next_turn() {
    let mut t = sayo();
    offer(&mut t);
    t.answer(0, 1).unwrap();
    t.set_fire(0, 10, 10);
    // Even if another effect allowed another main move in the same turn,
    // replenished pots must not allow a second use.
    t.m.world_mut().turn.main_moved = false;
    t.m.world_mut().turn.plan = Default::default();
    t.m.world_mut().st.step = game_core::state::stage::OPS;
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(t.fire(0), 10);
    t.begin_turn(0);
    t.set_pos(0, 1);
    offer(&mut t);
}
