//! Sayo's card modifies the resolved main move at the MoveBefore window.
mod common;
use common::*;
use game_core::engine::MoveKind;
use game_core::msg::Arg;

const SKILL: &str = "skill:冰川纱夜:踏上荆棘之路的觉悟";
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
fn card_and_six_pots_are_alternatives_in_one_window() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.title.key(), "ask.counteract.title");
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(5)));
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(6)));
    assert_eq!(p.options.len(), 3, "card, six pots, skip");
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_some());
    assert_eq!(t.pos(0), 1);
    t.counteract(0, CARD).unwrap();
    assert!(t.expect_prompt().title.key().ends_with("sayo_play_ask_title"));
    t.answer(0, 1).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.m.world().turn.main_steps, 7);
    assert_eq!(t.fire(0), 10, "using the card costs no pots");
    assert!(t.prompt().is_none(), "no subsequent paid offer: {}", t.dump_prompt());
    assert!(!t.hand(0).iter().any(|c| c == CARD));
}

#[test]
fn choosing_six_pots_keeps_the_card_and_the_skill_in_place() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.dice(&[5]);
    t.roll(0).unwrap();
    let source = t.option(SKILL).unwrap();
    t.answer(0, source).unwrap();
    assert!(t.expect_prompt().title.key().ends_with("sayo_thorns_title"));
    assert_eq!(t.pos(0), 1);
    t.answer(0, 2).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.fire(0), 4);
    assert_eq!(t.hand(0), vec![CARD]);
    assert!(t.m.world().st.players[0].field.iter().any(|f| f.card == SKILL));
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    t.set_fire(0, 10, 10);
    t.m.world_mut().turn.main_moved = false;
    t.m.world_mut().turn.plan = Default::default();
    t.m.world_mut().st.step = game_core::state::stage::OPS;
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none(), "paid use also prevents a subsequent card use");
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn hand_play_restrictions_do_not_suppress_the_paid_skill() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.m.world_mut().st.players[0].state_set(game_core::state::key::NO_HAND, 1);
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(t.option(CARD).is_none());
    let source = t.option(SKILL).unwrap();
    t.answer(0, source).unwrap();
    t.answer(0, 1).unwrap();
    assert_eq!(t.fire(0), 4);
    assert_eq!(t.pos(0), 7);
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn insufficient_pots_offer_only_the_card_and_skip() {
    let mut t = table();
    t.set_fire(0, 5, 10);
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert_eq!(t.expect_prompt().options.len(), 2);
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_none());
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.fire(0), 5);
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn duplicate_cards_cannot_be_declared_in_the_same_shared_window() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.set_hand(0, &[CARD, CARD]);
    declare(&mut t);
    // Source selection must lead directly to the distance choice, never a
    // second card/paid-source declaration before the first body settles.
    t.answer(0, 1).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.hand(0), vec![CARD]);
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
}

#[test]
fn card_consumes_the_same_once_per_turn_use_as_the_paid_skill() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    declare(&mut t);
    t.answer(0, 0).unwrap();
    t.set_hand(0, &[CARD]);
    t.m.world_mut().turn.main_moved = false;
    t.m.world_mut().turn.plan = Default::default();
    t.m.world_mut().st.step = game_core::state::stage::OPS;
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none(), "neither alternative can be used twice: {}", t.dump_prompt());
    assert_eq!(t.fire(0), 10);
    assert_eq!(t.hand(0), vec![CARD]);
    t.begin_turn(0);
    t.set_pos(0, 1);
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_some());
}

#[test]
fn declining_the_shared_window_keeps_both_sources() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.dice(&[5]);
    t.roll(0).unwrap();
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.fire(0), 10);
    assert_eq!(t.hand(0), vec![CARD]);
    assert!(t.prompt().is_none(), "no second skill popup: {}", t.dump_prompt());
}

#[test]
fn choosing_the_card_preserves_its_counteraction_response_window() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.set_hand(1, &["TEST:deny"]);
    t.dice(&[5]);
    t.roll(0).unwrap();
    t.counteract(0, CARD).unwrap();
    assert!(t.counteract_offered("TEST:deny"), "{}", t.dump_prompt());
    t.counteract(1, "TEST:deny").unwrap();
    assert_eq!(t.pos(0), 6, "a negated card must not extend the walk");
    assert_eq!(t.fire(0), 10, "no fallback paid-skill activation");
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert!(t.m.world().st.players[0].field.iter().any(|f| f.card == SKILL));
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
