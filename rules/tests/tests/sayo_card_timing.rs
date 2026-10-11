//! Sayo chooses distance, then its payment, before declaring either source.
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

fn roll(t: &mut Table) {
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.key(), "ask.counteract.move_extension");
    assert_eq!(p.options.len(), 3);
    assert_eq!(p.options[0].a.get("n"), Some(&Arg::I(2)));
    assert_eq!(p.options[1].a.get("n"), Some(&Arg::I(1)));
    assert_eq!(p.options[2].key(), "ask.counteract.skip");
    assert_eq!(p.fallback, 2);
    assert_eq!(t.pos(0), 1);
}

fn distance(t: &mut Table, extra: i32) {
    t.answer(0, 2 - extra).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.key(), "ask.counteract.move_extension_payment");
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(extra as i64)));
    assert_eq!(p.options.last().unwrap().key(), "ask.counteract.back");
    assert_eq!(p.fallback as usize, p.options.len() - 1);
    assert_eq!(t.pos(0), 1);
    assert!(t.hand(0).iter().any(|id| id == CARD), "not declared yet");
}

#[test]
fn card_uses_final_override_and_shows_landings_before_walking() {
    let mut t = table();
    t.m.world_mut().hidden[0].next_steps = Some(7);
    roll(&mut t);
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(7)));
    assert_eq!(p.text.a.get("tile"), Some(&Arg::Tile(8)));
    assert_eq!(p.options[0].a.get("tile"), Some(&Arg::Tile(10)));
    distance(&mut t, 2);
    assert_eq!(t.expect_prompt().text.a.get("tile"), Some(&Arg::Tile(10)));
    t.counteract(0, CARD).unwrap();
    assert_eq!(t.pos(0), 10);
    assert_eq!(t.m.world().turn.main_steps, 9);
    assert_eq!(t.fire(0), 0);
}

#[test]
fn either_extension_applies_after_card_payment_without_a_third_prompt() {
    for extra in [1, 2] {
        let mut t = table();
        roll(&mut t);
        distance(&mut t, extra);
        t.counteract(0, CARD).unwrap();
        assert_eq!(t.pos(0), 6 + extra as usize);
        assert_eq!(t.m.world().turn.main_steps, 5 + extra);
        assert_eq!(t.fire(0), 0);
        assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    }
}

#[test]
fn card_and_six_pots_are_alternatives_after_the_distance_choice() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    distance(&mut t, 2);
    assert_eq!(t.expect_prompt().options.len(), 3, "card, six pots, back");
    assert_eq!(t.option(CARD), Some(0));
    assert_eq!(t.option(SKILL), Some(1));
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_some());
    assert_eq!(t.fire(0), 10);
    t.counteract(0, CARD).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none(), "no subsequent paid offer");
    assert!(!t.hand(0).iter().any(|c| c == CARD));
}

#[test]
fn choosing_six_pots_keeps_the_card_and_the_skill_in_place() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    distance(&mut t, 2);
    t.counteract(0, SKILL).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.fire(0), 4);
    assert_eq!(t.hand(0), vec![CARD]);
    assert!(t.on_field(0, SKILL));
    assert!(t.prompt().is_none());
    t.set_fire(0, 10, 10);
    t.m.world_mut().turn.main_moved = false;
    t.m.world_mut().turn.plan = Default::default();
    t.m.world_mut().st.step = game_core::state::stage::OPS;
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        t.prompt().is_none(),
        "paid use also prevents another card use"
    );
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn back_reopens_distance_without_spending_then_allows_a_different_choice() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    distance(&mut t, 2);
    for _ in 0..3 {
        t.decline(); // back
        assert_eq!(
            t.expect_prompt().text.key(),
            "ask.counteract.move_extension"
        );
        assert_eq!(t.hand(0), vec![CARD]);
        assert_eq!(t.fire(0), 10);
        assert_eq!(t.pos(0), 1);
        assert_eq!(t.state(0, "skill.sayoThorns.used"), 0);
        distance(&mut t, 1);
    }
    t.counteract(0, SKILL).unwrap();
    assert_eq!(t.pos(0), 7, "the latest +1 wins, earlier +2 is not applied");
    assert_eq!(t.fire(0), 4);
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn back_then_pass_keeps_both_sources_and_original_distance() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    distance(&mut t, 2);
    t.decline();
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.hand(0), vec![CARD]);
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none());
}

#[test]
fn back_then_card_payment_uses_only_the_new_distance() {
    let mut t = table();
    roll(&mut t);
    distance(&mut t, 1);
    t.decline();
    distance(&mut t, 2);
    t.counteract(0, CARD).unwrap();
    assert_eq!(t.pos(0), 8);
    assert!(!t.hand(0).iter().any(|id| id == CARD));
    assert!(t.prompt().is_none());
}

#[test]
fn hand_play_restrictions_do_not_suppress_the_paid_skill() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.m.world_mut().st.players[0].state_set(game_core::state::key::NO_HAND, 1);
    roll(&mut t);
    distance(&mut t, 1);
    assert!(t.option(CARD).is_none());
    t.counteract(0, SKILL).unwrap();
    assert_eq!(t.fire(0), 4);
    assert_eq!(t.pos(0), 7);
    assert_eq!(t.hand(0), vec![CARD]);
}

#[test]
fn insufficient_pots_offer_only_the_card_and_back() {
    let mut t = table();
    t.set_fire(0, 5, 10);
    roll(&mut t);
    distance(&mut t, 2);
    assert_eq!(t.expect_prompt().options.len(), 2);
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_none());
    t.decline();
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
    roll(&mut t);
    distance(&mut t, 2);
    t.counteract(0, CARD).unwrap();
    assert_eq!(t.pos(0), 8);
    assert_eq!(t.hand(0), vec![CARD]);
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none());
}

#[test]
fn card_consumes_the_shared_once_per_turn_use_and_next_turn_resets_it() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    distance(&mut t, 1);
    t.counteract(0, CARD).unwrap();
    t.set_hand(0, &[CARD]);
    t.m.world_mut().turn.main_moved = false;
    t.m.world_mut().turn.plan = Default::default();
    t.m.world_mut().st.step = game_core::state::stage::OPS;
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none());
    assert_eq!(t.fire(0), 10);
    assert_eq!(t.hand(0), vec![CARD]);
    t.begin_turn(0);
    t.set_pos(0, 1);
    roll(&mut t);
    distance(&mut t, 2);
    assert!(t.option(CARD).is_some());
    assert!(t.option(SKILL).is_some());
}

#[test]
fn declining_the_initial_window_keeps_both_sources() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    roll(&mut t);
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.fire(0), 10);
    assert_eq!(t.hand(0), vec![CARD]);
    assert!(t.prompt().is_none());
}

#[test]
fn card_payment_preserves_its_counteraction_response_window() {
    let mut t = table();
    t.set_fire(0, 10, 10);
    t.set_hand(1, &["TEST:deny"]);
    roll(&mut t);
    distance(&mut t, 2);
    t.counteract(0, CARD).unwrap();
    assert!(t.counteract_offered("TEST:deny"));
    t.counteract(1, "TEST:deny").unwrap();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.fire(0), 10);
    assert!(t.prompt().is_none());
    assert!(t.on_field(0, SKILL));
}

#[test]
fn reverse_move_and_existing_extra_steps_are_preserved() {
    let mut t = table();
    t.set_pos(0, 20);
    t.m.world_mut().turn.plan.reverse = true;
    t.m.world_mut().turn.plan.extra_steps = 2;
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.a.get("n"), Some(&Arg::I(7)));
    assert_eq!(p.options[0].a.get("tile"), Some(&Arg::Tile(11)));
    t.answer(0, 0).unwrap();
    t.counteract(0, CARD).unwrap();
    assert_eq!(t.pos(0), 11);
}

#[test]
fn teleport_and_forced_side_move_do_not_offer_the_card() {
    let mut t = table();
    t.m.world_mut().turn.plan.kind = MoveKind::Teleport;
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none());
    let mut t = table();
    t.m.world_mut().turn.plan.forced = true;
    t.give_play(0, "TEST:mover").unwrap();
    while t.prompt().is_some() {
        assert_ne!(
            t.expect_prompt().text.key(),
            "ask.counteract.move_extension"
        );
        t.decline();
    }
    assert!(t.hand(0).iter().any(|c| c == CARD));
}

#[test]
fn card_cannot_be_a_pre_roll_play() {
    let mut t = table();
    assert!(t.play(0, CARD).is_err());
    roll(&mut t);
    t.decline();
    assert_eq!(t.pos(0), 6);
    assert_eq!(t.hand(0), vec![CARD]);
}
