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
        .filter(|k| k.contains("pick_body"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "the body runs exactly once: {:?}",
        t.recent_keys(40)
    );
    // The body logs `pick_body` with `{{arg}}`; the declared arg is 7.
    let saw_arg = t.m.world().recent.iter().any(|e| {
        e.msg.key().contains("pick_body")
            && matches!(e.msg.a.get("arg"), Some(game_core::msg::Arg::I(7)))
    });
    assert!(
        saw_arg,
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
        !keys_contain(&t, "pick_body"),
        "the body never ran: {:?}",
        t.recent_keys(40)
    );
}

/// A decline on one card does not stop another card's activation on the same
/// timing, and a card whose declare declines still leaves its sibling entry
/// free to run.
#[test]
fn sibling_entry_still_runs_after_a_decline() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:decline", "TEST:decline_sib"]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:decline").unwrap();
    drain(&mut t);
    assert!(
        t.hand(0).iter().any(|c| c == "TEST:decline"),
        "the declined card stays in hand: {:?}",
        t.hand(0)
    );
    t.give_play(0, "TEST:decline_sib").unwrap();
    drain(&mut t);
    assert!(
        keys_contain(&t, "decline_sib_play"),
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

/// An eligibility probe of a residual guard that would prompt refuses with
/// `RuleError::GuardPrompted` and folds to "not eligible" -- the card is never
/// offered in the counteraction window, and no mid-scan prompt appears.
#[test]
fn probe_of_an_asking_guard_never_prompts() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:echo"]);
    t.set_hand(1, &["TEST:asking_guard"]);
    // Player 0 plays, which raises `card` and opens a counteraction window.
    t.give_play(0, "TEST:echo").unwrap();
    drain(&mut t);
    // The asking-guard card must not be offered: the probe refused it.
    assert!(
        !t.counteract_offered("TEST:asking_guard"),
        "an asking residual is folded to not-eligible: {}",
        t.dump_prompt()
    );
}

/// Declining the declare of a **counteraction** means no counter is declared
/// at all: the card stays in hand, the body never runs, and the answered
/// timing resolves as if the card had not been offered. `OP_DECLARE` runs in
/// `finish_offer`, before `declare_one` commits / `build_round`'s `hand.remove`.
#[test]
fn counteract_decline_declares_no_counter() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:echo"]);
    t.set_hand(1, &["TEST:pick_counter"]);
    let mark = t.mark();
    // Player 0 plays; the `card` link opens a counteraction window. Do not
    // `drain` before the offer assert -- that would decline the window away.
    t.give_play(0, "TEST:echo").unwrap();
    assert!(
        t.counteract_offered("TEST:pick_counter"),
        "the counter is offered: {}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:pick_counter").unwrap();
    // `OP_DECLARE` asks "Counter with this?" -- decline.
    assert!(t.prompt().is_some(), "the declare asks: {}", t.dump_prompt());
    t.answer_one(1).unwrap(); // no
    drain(&mut t);

    assert!(
        t.hand(1).iter().any(|c| c == "TEST:pick_counter"),
        "a declined declare never removes the card from hand: {:?}",
        t.hand(1)
    );
    assert!(
        !keys_contain(&t, "pick_c_body"),
        "the body has no decline path and logs unconditionally -- silence proves it never ran: {:?}",
        t.recent_keys(40)
    );
    let counters: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.card == "TEST:pick_counter")
        .collect();
    assert!(
        counters.is_empty(),
        "no counter declaration happened at all: {counters:?}"
    );
}

/// A decline that came from a prompt **records that prompt's answer**. The
/// multi-pass drive re-runs the activation from scratch with the answer log
/// attached, so if the "no" were dropped the re-run would ask again. Assert
/// there is no second prompt and the same absence of body/declaration
/// (docs/ACTIVATION.md §2 "Bots and the answer log").
#[test]
fn prompted_decline_replays_without_re_asking() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:echo"]);
    t.set_hand(1, &["TEST:pick_counter"]);
    t.give_play(0, "TEST:echo").unwrap();
    // No `drain` before the offer assert: that would decline the window away.
    assert!(t.counteract_offered("TEST:pick_counter"), "{}", t.dump_prompt());
    t.counteract(1, "TEST:pick_counter").unwrap();
    assert!(t.prompt().is_some(), "the declare asks once: {}", t.dump_prompt());
    t.answer_one(1).unwrap(); // no -- and the drive re-runs with this answer

    // The re-run consumed the recorded "no" from the log. A second prompt here
    // would mean the answer was dropped and `OP_DECLARE` re-asked.
    assert!(
        t.prompt().is_none(),
        "a prompted decline must not re-ask on the multi-pass re-run: {}",
        t.dump_prompt()
    );
    // …and it stays quiet: nothing else is pending on this activation.
    drain(&mut t);
    assert!(
        t.prompt().is_none(),
        "no follow-on prompt from the declined activation: {}",
        t.dump_prompt()
    );
    assert!(
        t.hand(1).iter().any(|c| c == "TEST:pick_counter"),
        "still in hand after the replayed decline: {:?}",
        t.hand(1)
    );
    assert!(
        !keys_contain(&t, "pick_c_body"),
        "the body never ran across the replay: {:?}",
        t.recent_keys(40)
    );
}

/// A [反击]'s `OP_DECLARE` collector sees the **trigger it is declaring
/// against**, not a default one. `TEST:trigger_seen`'s collector accepts only
/// when `trigger::kind()` is `Card`; a `Run` bound to `Trigger::default()`
/// (kind `None`) would decline and the body would never run. `detour` needs
/// this the moment its pick filters options on the roll
/// (docs/ACTIVATION.md §2).
#[test]
fn counteract_declare_sees_the_trigger() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["TEST:echo"]);
    t.set_hand(1, &["TEST:trigger_seen"]);
    // Player 0 plays; the `card` link opens a counteraction window.
    t.give_play(0, "TEST:echo").unwrap();
    assert!(
        t.counteract_offered("TEST:trigger_seen"),
        "the counter is offered: {}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:trigger_seen").unwrap();
    // The collector is prompt-free and gates on the trigger kind, so there is
    // nothing to answer: declaring is the whole interaction.
    drain(&mut t);
    assert!(
        keys_contain(&t, "trigger_seen_body"),
        "the collector saw trigger::kind() == Card and admitted the entry -- \
         a default trigger would have declined and left this silent: {:?}",
        t.recent_keys(40)
    );
}