//! Sayo's card modifies the resolved main move at the MoveBefore window.
mod common;
use common::*;
use game_core::engine::MoveKind;
use game_core::msg::Arg;

const CARD: &str = "R:（纱夜）弹奏弹奏弹奏，继续弹奏";

fn table() -> Table {
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 0, 10);
    t.set_pos(0, 1);
    t.set_pos(1, 30);
    t.set_hand(0, &[CARD]);
    t
}

fn declare(t: &mut Table) {
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(CARD), "{}", t.dump_prompt());
    assert_eq!(t.pos(0), 1, "the main move has not started");
    t.counteract(0, CARD).unwrap();
    assert!(
        t.expect_prompt()
            .title
            .key()
            .ends_with("sayo_play_ask_title"),
        "{}",
        t.dump_prompt()
    );
    assert_eq!(
        t.pos(0),
        1,
        "choosing the extension still precedes movement"
    );
}

#[test]
fn card_uses_final_override_and_shows_landings_before_walking() {
    let mut t = table();
    t.m.world_mut().hidden[0].next_steps = Some(7);
    declare(&mut t);
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(7)));
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(8)));
    assert_eq!(p.options[1].a.get("tile"), Some(&Arg::Tile(10)));
    t.answer(0, 1).unwrap();
    assert_eq!(t.pos(0), 10);
    assert_eq!(t.m.world().turn.main_steps, 9);
    assert_eq!(t.fire(0), 0, "the card's extension remains free");
}

#[test]
fn either_extension_applies_to_this_move() {
    for option in [0, 1] {
        let mut t = table();
        declare(&mut t);
        t.answer(0, option).unwrap();
        assert_eq!(t.pos(0), 7 + option as usize);
        assert_eq!(t.m.world().turn.main_steps, 6 + option);
        assert_eq!(t.fire(0), 0);
    }
}

#[test]
fn card_and_skill_add_to_each_other_without_an_extra_paid_offer() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    declare(&mut t);
    t.answer(0, 1).unwrap();
    let p = t.expect_prompt();
    assert!(
        p.title.key().ends_with("sayo_thorns_title"),
        "{}",
        t.dump_prompt()
    );
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(7)));
    assert_eq!(t.fire(0), 10);
    assert_eq!(t.pos(0), 1);
    t.answer(0, 1).unwrap();
    assert_eq!(t.pos(0), 9);
    assert_eq!(t.m.world().turn.main_steps, 8);
    assert_eq!(t.fire(0), 4, "only the separately chosen skill spends pots");
}

#[test]
fn reverse_move_and_existing_extra_steps_are_preserved() {
    let mut t = table();
    t.set_pos(0, 20);
    t.m.world_mut().turn.plan.reverse = true;
    t.m.world_mut().turn.plan.extra_steps = 2;
    t.dice(&[5]);
    t.roll(0).unwrap();
    t.counteract(0, CARD).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(7)));
    assert_eq!(p.options[1].a.get("tile"), Some(&Arg::Tile(11)));
    t.answer(0, 1).unwrap();
    assert_eq!(t.pos(0), 11);
}

#[test]
fn teleport_and_forced_side_move_do_not_offer_the_card() {
    let mut t = table();
    t.m.world_mut().turn.plan.kind = MoveKind::Teleport;
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(!t.counteract_offered(CARD));
    let mut t = table();
    t.m.world_mut().turn.plan.forced = true;
    t.give_play(0, "TEST:mover").unwrap();
    while t.prompt().is_some() {
        assert!(!t.counteract_offered(CARD));
        t.decline();
    }
    assert!(t.hand(0).iter().any(|c| c == CARD));
}

#[test]
fn card_can_be_declined_and_is_not_a_pre_roll_play() {
    let mut t = table();
    assert!(t.play(0, CARD).is_err());
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(CARD));
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert!(t.hand(0).iter().any(|c| c == CARD));
}
