//! G05: an offer raised at the plan pass.
//!
//! The plan pass is a **filter** kind (`DiceRollPlan`, engine 67) that also
//! carries an action side (the dual `ActionKind::DiceRollPlan`), so a placed
//! card can ask there before a die is thrown. The ask is the entry's
//! `declare_arg`, which means:
//!
//! * the action runs at a filter kind, before the ladder's filter fold and
//!   before the cast;
//! * a decline runs no body and leaves no log line and no card flash;
//! * an accepted offer claims the move (`plan::claim`), and a second copy's
//!   guard reads the claim to stand itself down without asking;
//! * the recorded answer replays instead of re-asking, so the body runs once.
//!
//! `TEST:planOffer` accepts (and pins three steps), `TEST:planDecline`
//! declines from its collector with no prompt at all.

mod common;

use common::*;

/// Play `card` from `who`'s hand; the card places itself.
fn place(t: &mut Table, who: usize, card: &str) {
    t.give_play(who, card).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
}

fn plan_steps(t: &Table) -> Option<i32> {
    t.m.world().turn.plan.steps
}

fn saw(t: &Table, mark: i32, needle: &str) -> bool {
    t.keys_since(mark).iter().any(|k| k.contains(needle))
}

/// Is the open prompt the plan offer (rather than the settle's buy, which the
/// walk raises after it)?
fn plan_asked(t: &Table) -> bool {
    t.prompt()
        .is_some_and(|p| p.title.key().contains("plan_offer_title"))
}

/// The action side of the dual kind runs at the plan pass.
#[test]
fn action_at_a_filter_kind_runs() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    place(&mut t, 0, "TEST:planOffer");
    let mark = t.mark();
    t.roll(0).unwrap();
    assert!(
        t.prompt().is_some(),
        "the offer asks at the plan pass: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap(); // yes
    while t.prompt().is_some() {
        t.answer_one(0).unwrap();
    }
    assert!(
        saw(&t, mark, "plan_offer_body"),
        "the accepted body ran: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(
        plan_steps(&t),
        Some(3),
        "the body's `plan::set_steps` shapes this move"
    );
}

/// The offer is raised before the dice: nothing has been cast when it asks.
#[test]
fn action_runs_before_the_filter_fold() {
    let mut t = Table::vanilla(2);
    place(&mut t, 0, "TEST:planOffer");
    let mark = t.mark();
    t.roll(0).unwrap();
    assert!(t.prompt().is_some(), "{}", t.dump_prompt());
    assert!(
        !saw(&t, mark, "log.dice") && !saw(&t, mark, "dice"),
        "the offer precedes the cast: {:?}",
        t.keys_since(mark)
    );
}

/// A collector that declines runs nothing: no body log and no plan write.
#[test]
fn decline_writes_nothing() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    place(&mut t, 0, "TEST:planDecline");
    let mark = t.mark();
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    assert!(
        !saw(&t, mark, "plan_decline_body"),
        "the declining collector's body never runs: {:?}",
        t.keys_since(mark)
    );
    assert_ne!(
        plan_steps(&t),
        Some(5),
        "the body's `set_steps(5)` never wrote"
    );
}

/// …and it says nothing and flashes nothing: the only trace of the offer is
/// the prompt that was answered.
#[test]
fn decline_logs_nothing() {
    let mut t = Table::vanilla(2);
    place(&mut t, 0, "TEST:planDecline");
    let mark = t.mark();
    t.roll(0).unwrap();
    assert!(
        !plan_asked(&t),
        "the declining collector asks nothing: {}",
        t.dump_prompt()
    );
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k.contains("plan_decline")),
        "a declined declare logs nothing of its own: {keys:?}"
    );
}

/// No `"card"` event for the instance at the decline -- a decline is not an
/// activation, so nothing flashes.
#[test]
fn decline_does_not_flash() {
    let mut t = Table::vanilla(2);
    place(&mut t, 0, "TEST:planDecline");
    let mark = t.mark();
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    let flashed: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == "TEST:planDecline")
        .collect();
    assert!(flashed.is_empty(), "no flash for a decline: {flashed:?}");
}

/// The recorded answer replays: one yes runs the body exactly once.
#[test]
fn hook_declare_runs_the_body_once_with_the_arg() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    place(&mut t, 0, "TEST:planOffer");
    let mark = t.mark();
    t.roll(0).unwrap();
    t.answer_one(0).unwrap();
    while t.prompt().is_some() {
        t.answer_one(0).unwrap();
    }
    let hits = t
        .keys_since(mark)
        .into_iter()
        .filter(|k| k.contains("plan_offer_body"))
        .count();
    assert_eq!(hits, 1, "the replayed drive runs the body once: {:?}", t.keys_since(mark));
    // …and the recorded answer carries the arg the collector declared.
    let saw_arg = t.m.world().recent.iter().any(|e| {
        e.msg.key().contains("plan_offer_body")
            && matches!(e.msg.args.get("arg"), Some(game_core::msg::Arg::Integer(7)))
    });
    assert!(
        saw_arg,
        "the body sees the declared arg: {:?}",
        t.keys_since(mark)
    );
}

/// A card that takes the move stands the second copy down: one prompt, one
/// body.
#[test]
fn second_copy_not_offered_after_a_claim() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    place(&mut t, 0, "TEST:planOffer");
    place(&mut t, 0, "TEST:planOffer");
    let mark = t.mark();
    t.roll(0).unwrap();
    t.answer_one(0).unwrap(); // yes on the first copy
    assert!(
        !plan_asked(&t),
        "the claimed move skips the second copy: {}",
        t.dump_prompt()
    );
    let hits = t
        .keys_since(mark)
        .into_iter()
        .filter(|k| k.contains("plan_offer_body"))
        .count();
    assert_eq!(hits, 1, "only the first copy acted: {:?}", t.keys_since(mark));
}

/// The claim is per move: it is cleared with the move that raised it.
#[test]
fn claim_is_cleared_after_the_move() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    place(&mut t, 0, "TEST:planOffer");
    t.roll(0).unwrap();
    t.answer_one(0).unwrap(); // yes
    while t.prompt().is_some() {
        t.decline();
    }
    // The card was placed, not discarded -- it survives its own offer.
    assert!(
        t.on_field(0, "TEST:planOffer"),
        "the offer leaves the card placed: {:?}",
        t.field_ids(0)
    );
    assert!(
        !t.m.world().turn.plan.claimed,
        "the claim dies with the move: keys={:?}",
        t.recent_keys(15)
    );
}
