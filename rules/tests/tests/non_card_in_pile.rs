//! A card area (hand / draw pile / discard) stores ids that can live there.
//!
//! Two kinds of rule entity exist, and the book separates them:
//!
//! * A **character skill** (`skill:<persona>:<skill>`), tile / event / mark /
//!   status rule is text on some other card. The engine built the instance from
//!   nothing, so it is removed with nothing in its place and the destination
//!   that asked for a pile only reports (`err.non_card_in_graveyard`, or a
//!   `debug_assert!` for a hand / draw pile landing).
//! * A **group skill card** (`skill:<group>:<skill>`) is a card (「团技能卡」;
//!   海铃 (3) says 「置入弃牌堆」), so it is filed like any card and not
//!   reported.
//!
//! `TEST:drop` takes the instance at `TEST.drop.uid` to `TEST.drop.dest`.

mod common;

use common::Table;
use game_core::state::BOARD_OWNER;

/// A placed group skill card (the fixture roster carries the real bands).
const GROUP_SKILL: &str = "skill:Poppin' Party:星之鼓动";

fn board_rule(t: &Table) -> i32 {
    t.m.world()
        .field_instances(BOARD_OWNER)
        .into_iter()
        .next()
        .expect("a bound board rule")
        .0
}

/// Play `TEST:drop` on `uid` with `dest`. A hand / draw-pile landing of a
/// dropped rule counts on the assert before anything commits, so the panic
/// message is read instead of the event log.
fn file(t: &mut Table, uid: i32, dest: i32) -> Result<(), String> {
    t.give(0, &["TEST:drop"]);
    t.set_state(0, "TEST.drop.uid", uid);
    t.set_state(0, "TEST.drop.dest", dest);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.play(0, "TEST:drop").unwrap()
    }))
    .map_err(|p| {
        p.downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

#[test]
fn a_board_rule_filed_to_a_hand_is_removed_and_reported() {
    let mut t = Table::vanilla(2);
    let uid = board_rule(&t);
    let msg = file(&mut t, uid, 1).expect_err("hand landing asserts");
    assert!(msg.contains("err.non_card_in_hand"), "reported: {msg}");
    assert!(
        !t.hand(0).iter().any(|c| c.starts_with("tile:")),
        "kept a rule in the hand: {:?}",
        t.hand(0)
    );
}

#[test]
fn a_board_rule_filed_to_a_draw_pile_is_removed_and_reported() {
    let mut t = Table::vanilla(2);
    let uid = board_rule(&t);
    let msg = file(&mut t, uid, 4).expect_err("deck landing asserts");
    assert!(msg.contains("err.non_card_in_deck"), "reported: {msg}");
    let deck = t.m.world().hidden[0].deck.clone();
    assert!(
        !deck.iter().any(|c| c.starts_with("tile:")),
        "kept a rule in the draw pile: {deck:?}"
    );
}

/// A Graveyard landing is the ordinary removal destination: the row is gone
/// either way, so it only reports (no assert) and files nothing.
#[test]
fn a_board_rule_filed_to_a_discard_only_reports() {
    let mut t = Table::vanilla(2);
    let uid = board_rule(&t);
    let mark = t.mark();
    file(&mut t, uid, 0).expect("a discard landing does not assert");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "err.non_card_in_graveyard"),
        "reported: {keys:?}"
    );
    assert!(
        !t.discard(0).iter().any(|c| c.starts_with("tile:")),
        "kept a rule in the discard: {:?}",
        t.discard(0)
    );
}

/// A group skill is a card: the discard keeps it and there is no report.
#[test]
fn a_group_skill_card_filed_to_a_discard_is_kept() {
    let mut t = Table::vanilla(2);
    // A non-empty draw pile, so the write is not refilled into the deck at once.
    t.set_draw(0, &["TEST:drop"]);
    let uid = t.place_raw(0, GROUP_SKILL);
    let mark = t.mark();
    file(&mut t, uid, 0).expect("a group skill card does not assert");
    assert!(
        t.discard(0).iter().any(|c| c == GROUP_SKILL),
        "dropped a group skill card: {:?}",
        t.discard(0)
    );
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k == "err.non_card_in_graveyard"),
        "reported a kept card: {keys:?}"
    );
}

/// The same card landing in a hand is kept there (no dead-weight assert).
#[test]
fn a_group_skill_card_filed_to_a_hand_is_kept() {
    let mut t = Table::vanilla(2);
    let uid = t.place_raw(0, GROUP_SKILL);
    file(&mut t, uid, 1).expect("a group skill card does not assert");
    assert!(
        t.hand(0).iter().any(|c| c == GROUP_SKILL),
        "dropped a group skill card: {:?}",
        t.hand(0)
    );
}

/// A group skill card in hand is not dead weight: playing it places it (the
/// instance is the card), so it does not stay a hand entry or go to the pile.
#[test]
fn a_group_skill_card_in_hand_is_played_onto_the_field() {
    let mut t = Table::vanilla(2);
    // A skill with no `On::Play` at all -- the `normal` gate must not refuse it.
    const PASSIVE: &str = "skill:Hello, Happy World!:传播笑容";
    t.set_draw(0, &["TEST:drop"]);
    t.give_play(0, PASSIVE)
        .expect("a skill card is playable from hand");
    assert!(t.on_field(0, PASSIVE), "the played skill was not placed");
    assert!(!t.hand(0).iter().any(|c| c == PASSIVE), "still in hand");
    assert!(
        !t.discard(0).iter().any(|c| c == PASSIVE),
        "wrongly discarded"
    );
}

/// The injection primitives treat a group skill card as a card: the hand and
/// deck writes log the rule entity but keep the name, and `to_discard` files
/// it instead of reporting a drop.
#[test]
fn injected_group_skill_cards_are_kept_and_logged() {
    let mut t = Table::vanilla(2);
    // A non-empty draw pile, so the discard write is not refilled at once.
    t.set_draw(0, &["TEST:drop"]);
    let mark = t.mark();
    t.m.world_mut().add_to_hand(0, GROUP_SKILL);
    assert!(
        t.hand(0).iter().any(|c| c == GROUP_SKILL),
        "hand injection dropped the card"
    );
    // `to_discard` answers "the deck refilled", not success: a non-empty deck
    // makes it `false` even though the card was filed.
    t.m.world_mut().to_discard(0, GROUP_SKILL);
    assert!(
        !t.hand(0).iter().any(|c| c == GROUP_SKILL),
        "to_discard left the hand entry"
    );
    assert!(
        t.discard(0).iter().any(|c| c == GROUP_SKILL),
        "to_discard dropped a card: {:?}",
        t.discard(0)
    );
    t.m.world_mut().add_to_deck(0, GROUP_SKILL, false);
    assert!(
        t.m.world().hidden[0].deck.iter().any(|c| c == GROUP_SKILL),
        "deck injection dropped the card"
    );
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.rule_to_hand"),
        "hand injection not logged: {keys:?}"
    );
    assert!(
        keys.iter().any(|k| k == "log.rule_to_deck"),
        "deck injection not logged: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.starts_with("err.non_card")),
        "a kept card was reported: {keys:?}"
    );
}
