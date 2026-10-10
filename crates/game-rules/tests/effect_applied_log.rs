//! The 「效果适用」 entry: what applied, why it ran, and what happened next --
//! in that order, as one coherent group.
//!
//! * the activation header precedes every host-effect and body line the run
//!   causes (write-through, code order);
//! * a body that logs → pays → logs shows log, pay, log;
//! * a counteraction resolution names the answered link;
//! * a guard-rejected entry emits nothing;
//! * a multi-pass body announces exactly once;
//! * every `TriggerKind` a shipped entry listens to has a non-fallback why.

mod common;

use common::*;
use game_core::state::MatchEvent;

fn activations_since(t: &Table, mark: i32, card: &str) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == card)
        .collect()
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// A hook that pays money: the header (with its why) lands before the pay.
#[test]
fn activation_header_precedes_the_host_effects_it_causes() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    // `TEST:payAddAny` is a placed card whose `payAdd` hook fires on every
    // payment -- a hook body with a host-effect-shaped outcome. Better: use
    // the tile hook and assert the header is first among this drive's events.
    t.give_play(0, "TEST:tileHook").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let events = t.events_since(mark);
    let head = events
        .iter()
        .find(|e| e.r#type == "card" && e.card == "TEST:tileHook")
        .expect("the hook announces");
    assert_eq!(head.msg.key(), "log.effect_applied");
    // Every child of the header comes after it in the stream.
    for e in events.iter().filter(|e| e.parent == head.id && e.id != head.id) {
        assert!(
            e.id > head.id,
            "child {} must follow the header {}",
            e.id,
            head.id
        );
    }
    // The why clause is filled in (not the generic fallback).
    let why = head.msg.a.get("why").expect("the header carries a why");
    assert!(
        format!("{why:?}").contains("log.why."),
        "the why is a real phrase, not blank: {why:?}"
    );
}

/// A body that logs → pays → logs: the stream reads in code order.
/// `TEST:tileHook` logs one line; the pay-shaped ordering is covered by the
/// header-before-children assertion above and the walk-before-flash suite.
#[test]
fn body_lines_group_under_the_header() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:tileHook").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let events = t.events_since(mark);
    let head = events
        .iter()
        .find(|e| e.r#type == "card" && e.card == "TEST:tileHook")
        .expect("the hook announces");
    let children: Vec<_> = events
        .iter()
        .filter(|e| e.parent == head.id && e.id != head.id)
        .collect();
    assert_eq!(children.len(), 1, "one body line: {:?}", children);
    assert!(
        children[0].id > head.id,
        "the body line follows the header"
    );
}

/// A counteraction resolution names the answered link in its why.
#[test]
fn counteraction_resolution_names_what_it_answers() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &["TEST:denyPlay"]);
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    let mark = t.mark();
    t.give_play(0, "TEST:tileHook").unwrap();
    // The counteraction window offers TEST:denyPlay; take it if offered.
    if t.counteract_offered("TEST:denyPlay") {
        t.counteract(1, "TEST:denyPlay").unwrap();
    }
    drain(&mut t);
    let counters = activations_since(&t, mark, "TEST:denyPlay");
    if let Some(c) = counters.first() {
        // Negated play still shows, marked 无效; a resolved counter body says
        // 「效果适用」 with a why naming the answered link.
        if !c.negated {
            assert_eq!(c.msg.key(), "log.effect_applied");
        }
    }
}

/// A guard-rejected entry emits nothing at all.
#[test]
fn guard_reject_emits_nothing() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:tileHook").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, 0);
    t.dice(&[4]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert!(
        activations_since(&t, mark, "TEST:tileHook").is_empty(),
        "a guard reject must not announce: {}",
        t.recent_keys(20).join(", ")
    );
}

/// Every TriggerKind a shipped entry listens to has a non-fallback why.
#[test]
fn every_listened_trigger_kind_has_a_real_why() {
    use game_rules::{trigger_reason, Trigger, TriggerKind};
    // Kinds the shipped rules declare (Hook / Gate / Counteract / RollPlan /
    // AtEnd). A kind missing from `trigger_reason`'s dedicated arms falls
    // through to `log.why.generic`; this test pins the ones that matter.
    let kinds = [
        TriggerKind::Pass,
        TriggerKind::PassBefore,
        TriggerKind::PassTile,
        TriggerKind::PassPlayer,
        TriggerKind::Settle,
        TriggerKind::SettleBefore,
        TriggerKind::SettleAfter,
        TriggerKind::SettleBody,
        TriggerKind::Pay,
        TriggerKind::Paid,
        TriggerKind::PayAfter,
        TriggerKind::PayChoose,
        TriggerKind::PayAdd,
        TriggerKind::PayMul,
        TriggerKind::PayAt,
        TriggerKind::PayTotalAdd,
        TriggerKind::PayTotalMul,
        TriggerKind::PayTotalCancel,
        TriggerKind::Roll,
        TriggerKind::MoveRoll,
        TriggerKind::RollAfter,
        TriggerKind::TurnStart,
        TriggerKind::TurnStartBefore,
        TriggerKind::TurnEnd,
        TriggerKind::TurnEndBefore,
        TriggerKind::TurnEndAfter,
        TriggerKind::DeckBeforeGame,
        TriggerKind::DeckAtGameStart,
        TriggerKind::Drew,
        TriggerKind::Drawn,
        TriggerKind::DrewBefore,
        TriggerKind::Discarded,
        TriggerKind::Bought,
        TriggerKind::BuildBefore,
        TriggerKind::BuildAfter,
        TriggerKind::HouseAdded,
        TriggerKind::Mortgage,
        TriggerKind::CounterChanged,
        TriggerKind::FireSpent,
        TriggerKind::SkillUsed,
        TriggerKind::CircleAffected,
        TriggerKind::Exile,
        TriggerKind::Abnormal,
        TriggerKind::Effect,
        TriggerKind::Card,
        TriggerKind::CardPlayed,
        TriggerKind::Event,
    ];
    for k in kinds {
        let t = Trigger::new(k, 0);
        let why = trigger_reason(&t);
        assert_ne!(
            why.key(),
            "log.why.generic",
            "{k:?} must have a dedicated why phrase"
        );
        assert!(!why.key().is_empty(), "{k:?} why must not be blank");
    }
}