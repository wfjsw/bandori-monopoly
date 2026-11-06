//! A card area (hand / draw pile / discard) stores card ids only.
//!
//! A rule entity that a body files there -- a def the ruleset marks
//! `CardDef::rule` (`tile:*`, `event:*`, `skill:*`, `mark:*`, `status:*`) --
//! is not a card: it is in no deck and cannot be played from hand, so keeping
//! the name deals back dead weight for the rest of the match (`black_suits`
//! dropping its borrowed `tile:circle` into seat 0's discard through
//! `holder.max(0)`).
//!
//! The engine **removes** such a row instead of filing it, and reports the
//! destination that asked for a pile (`World::note_non_card`): a Graveyard
//! landing only logs (`err.non_card_in_graveyard`), a hand or draw-pile landing
//! is a `debug_assert!` too, because every hand card must be playable.

mod common;

use common::Table;
use game_core::state::BOARD_OWNER;

fn board_rule(t: &Table) -> i32 {
    t.m.world()
        .field_instances(BOARD_OWNER)
        .into_iter()
        .next()
        .expect("a bound board rule")
        .0
}

/// Play `TEST:drop` (takes a board rule and files it to `dest`). A hand / deck
/// landing counts on the rule name and trips the assert before anything
/// commits, so the panic message is read instead of the event log.
fn file_board_rule(t: &mut Table, dest: i32) -> Result<(), String> {
    t.give(0, &["TEST:drop"]);
    let uid = board_rule(t);
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
    let msg = file_board_rule(&mut t, 1).expect_err("hand landing asserts");
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
    let msg = file_board_rule(&mut t, 4).expect_err("deck landing asserts");
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
    let mark = t.mark();
    file_board_rule(&mut t, 0).expect("a discard landing does not assert");
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
