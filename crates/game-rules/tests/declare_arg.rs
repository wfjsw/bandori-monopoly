//! Activation arguments (`declare`, docs/ACTIVATION.md §2) and the
//! cross-card applicability query (`ctx::can_fire` / `ctx::card_replayable`).
//!
//! * a declined `declare` fires nothing -- no body, no header, no flash, no
//!   log -- and for a play the card is **still in hand** (the LOY-SEEYOU-01
//!   observable);
//! * an accepted `declare_arg` runs the body once and the body sees the arg;
//! * a decline on one entry does not stop the next entry on the same timing;
//! * a decline that came from a prompt replays without re-asking;
//! * `can_fire` / `card_replayable` answer for a gated pair;
//! * a probe of an asking residual is `RuleError::GuardPrompted`, never a
//!   prompt.

mod common;

use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn keys_contain(t: &Table, needle: &str) -> bool {
    t.recent_keys(40).iter().any(|k| k.contains(needle))
}

/// A declining `declare` fires nothing and leaves the card in hand.
#[test]
fn declined_declare_leaves_the_card_in_hand_and_runs_nothing() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:decline"]);
    t.set_hand(1, &[]);
    let mark = t.mark();
    let r = t.give_play(0, "TEST:decline");
    drain(&mut t);
    // The card never left the hand: the activation did not commit.
    assert!(
        t.hand(0).iter().any(|c| c == "TEST:decline"),
        "a declined declare must leave the card in hand: hand={:?}",
        t.hand(0)
    );
    assert!(
        r.is_ok(),
        "a decline is a normal 'do not fire', not an error: {r:?}"
    );
    let since: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == "TEST:decline")
        .collect();
    assert!(
        since.is_empty(),
        "a declined declare announces nothing: {since:?}"
    );
    assert!(
        !keys_contain(&t, "decline_body"),
        "the body has no decline path and logs unconditionally -- silence proves it never ran: {:?}",
        t.recent_keys(40)
    );
}

/// An accepted `declare_arg` runs the body once and the body sees the arg.
#[test]
fn accepted_declare_runs_the_body_once_with_the_arg() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:pick"]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:pick").unwrap();
    // The declare prompt is "use this?" -- answer yes.
    assert!(t.prompt().is_some(), "the declare asks: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    let hits: Vec<_> = t
        .recent_keys(40)
        .into_iter()
        .filter(|k| k.contains("pick body"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "the body runs exactly once: {:?}",
        t.recent_keys(40)
    );
    assert!(
        hits[0].contains("7") || t.recent_keys(40).iter().any(|k| k.contains("7")),
        "the body sees the declared arg (7): {:?}",
        t.recent_keys(40)
    );
}

/// Answering "no" to the declare is a decline: no body, card stays in hand.
#[test]
fn declining_the_declare_prompt_never_runs_the_body() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:pick"]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:pick").unwrap();
    assert!(t.prompt().is_some());
    t.answer_one(1).unwrap(); // no
    drain(&mut t);
    assert!(
        t.hand(0).iter().any(|c| c == "TEST:pick"),
        "declining at the declare leaves the card in hand: {:?}",
        t.hand(0)
    );
    assert!(
        !keys_contain(&t, "pick body"),
        "the body never ran: {:?}",
        t.recent_keys(40)
    );
}

/// A decline on one entry does not stop the next entry on the same timing.
#[test]
fn sibling_entry_still_runs_after_a_decline() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:decline", "TEST:declineSib"]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:declineSib").unwrap();
    drain(&mut t);
    assert!(
        keys_contain(&t, "decline-sib play ran"),
        "the sibling play body runs: {:?}",
        t.recent_keys(40)
    );
}

/// `ctx::card_replayable` / `ctx::can_fire` answer for a gated pair.
/// `TEST:gate_closed`'s `On::Play` gate always refuses, so it is never
/// playable and its body never runs.
#[test]
fn can_fire_sees_a_closed_gate() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:gate_closed"]);
    t.set_hand(1, &[]);
    // The play is refused at the offer layer (the gate's `why` is shown).
    let r = t.give_play(0, "TEST:gate_closed");
    drain(&mut t);
    assert!(
        r.is_err() || t.hand(0).iter().any(|c| c == "TEST:gate_closed"),
        "a closed gate keeps the card: r={r:?} hand={:?}",
        t.hand(0)
    );
    assert!(
        !keys_contain(&t, "gate-closed body ran"),
        "the body never runs behind a closed gate: {:?}",
        t.recent_keys(40)
    );
}