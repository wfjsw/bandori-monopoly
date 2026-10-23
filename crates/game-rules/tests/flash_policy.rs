//! Declarable flash policy (ABI v55) + once-per-flash identity (user, 2026-10-11).
//!
//! * `Flash::Always` flashes a self-only entry; `Flash::Never` suppresses a
//!   cross-player one; `ctx::no_flash` wins over Always/Auto; `ctx::flash`
//!   wins over Auto.
//! * `ctx::flash_card` posts a parent-linked `"flash"` event before later
//!   effects; `ctx::flash_card_keyed` with the same key flashes once.
//! * A body re-run after a prompt must not flash the same identity twice:
//!   (a) flash then ask, answered yes -> one flash, posted before the prompt;
//!   (b) answered no -> same, one flash;
//!   (c) two prompts, one flash per ordinal, none doubled;
//!   (d) `flash_card_keyed` same key twice -> once;
//!   (e) a thrown-away drive posts none;
//!   (f) nested drives are independent;
//!   (h) event stream deterministic across two runs.

mod common;

use common::*;
use game_core::state::MatchEvent;

/// Proactive `"flash"` cues (`ctx::flash` / `flash_card` / `flash_tile`) since
/// `mark`. Oldest first. This is the once-per-flash unit.
fn flashes_since(t: &Table, mark: i32) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "flash")
        .collect()
}

/// The activation's own card-flash decision (`"card"` event with `flash`).
fn card_flashes_since(t: &Table, mark: i32) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.flash && !e.negated && !e.card.is_empty())
        .collect()
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn play(t: &mut Table, card: &str) {
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, card).unwrap();
}

/// Play `card` and return the event mark taken just before the play.
fn play_marked(t: &mut Table, card: &str) -> i32 {
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    let mark = t.mark();
    t.give_play(0, card).unwrap();
    mark
}

// -- item 5: declarative policy + precedence ---------------------------------

/// `Flash::Always` flashes an owner-self-only entry (gain to self).
#[test]
fn always_flashes_a_self_only_entry() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashAlways");
    drain(&mut t);
    let f = card_flashes_since(&t, mark);
    assert!(
        f.iter().any(|e| e.card == "TEST:flashAlways"),
        "Always must flash a self-only gain: {}",
        t.recent_keys(20).join(", ")
    );
}

/// `Flash::Never` suppresses a cross-player pay's flash; the log entry remains.
#[test]
fn never_suppresses_a_cross_player_one() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashNever");
    drain(&mut t);
    let activated: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == "TEST:flashNever")
        .collect();
    assert!(
        !activated.is_empty(),
        "Never keeps the log entry: {}",
        t.recent_keys(20).join(", ")
    );
    assert!(
        activated.iter().all(|e| !e.flash),
        "Never suppresses the flash: {activated:?}"
    );
    assert!(
        flashes_since(&t, mark).is_empty(),
        "no flash event at all: {}",
        t.recent_keys(20).join(", ")
    );
}

/// Owner-self-only + `Flash::Auto` (the default): log entry, no flash.
#[test]
fn auto_self_only_does_not_flash() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashAutoSelf");
    drain(&mut t);
    let activated: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == "TEST:flashAutoSelf")
        .collect();
    assert!(!activated.is_empty(), "the entry still announces");
    assert!(
        activated.iter().all(|e| !e.flash),
        "Auto self-only: no flash: {activated:?}"
    );
    assert!(
        card_flashes_since(&t, mark).is_empty(),
        "and no card-flash: {}",
        t.recent_keys(20).join(", ")
    );
}

/// `ctx::no_flash` wins over a cross-player Auto flash.
#[test]
fn no_flash_suppresses_an_auto_flash() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashSuppress");
    drain(&mut t);
    assert!(
        flashes_since(&t, mark).is_empty(),
        "ctx::no_flash suppresses: {}",
        t.recent_keys(20).join(", ")
    );
}

/// `ctx::flash_card` posts a parent-linked `"flash"` event **before** later
/// body lines (code order).
#[test]
fn flash_card_posts_a_parent_linked_event_before_later_effects() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashCardEarly");
    drain(&mut t);
    let evs = t.events_since(mark);
    let header = evs
        .iter()
        .find(|e| e.r#type == "card" && e.card == "TEST:flashCardEarly")
        .expect("activation header");
    let flash_ev = evs
        .iter()
        .find(|e| e.r#type == "flash")
        .expect("proactive flash event");
    assert_eq!(
        flash_ev.parent, header.id,
        "the flash is parent-linked under the activation"
    );
    let after = evs
        .iter()
        .find(|e| e.msg.key().contains("flash_card_after"))
        .expect("later body line");
    assert!(
        flash_ev.id < after.id,
        "the flash lands before the later effect (code order): {}",
        t.recent_keys(20).join(", ")
    );
}

/// Precedence table: Never > no_flash > flash/Always > Auto.
#[test]
fn precedence_table() {
    // Never beats a cross-player Auto.
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashNever");
    drain(&mut t);
    assert!(flashes_since(&t, mark).is_empty(), "Never > Auto");

    // Always beats Auto self-only.
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashAlways");
    drain(&mut t);
    assert!(
        !card_flashes_since(&t, mark).is_empty(),
        "Always > Auto: {}",
        t.recent_keys(20).join(", ")
    );

    // no_flash beats Auto cross-player.
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashSuppress");
    drain(&mut t);
    assert!(flashes_since(&t, mark).is_empty(), "no_flash > Auto");
}

// -- item 6: once-per-flash --------------------------------------------------

/// (a) `ctx::flash()` then `ask_yes`, answered **yes**: exactly ONE flash,
/// posted before the prompt.
#[test]
fn flash_then_ask_yes_flashes_once_before_the_prompt() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashThenAsk");
    // The flash (and its header) must already be on the stream at the prompt.
    let before = t.expect_prompt();
    assert!(
        before.kind == "choice" || before.kind == "yesno",
        "the body paused at ask_yes: {before:?}"
    );
    let at_prompt = t.events_since(mark);
    let f: Vec<_> = at_prompt.iter().filter(|e| e.r#type == "flash").collect();
    assert_eq!(
        f.len(),
        1,
        "exactly one flash posted before the prompt: {}",
        t.recent_keys(20).join(", ")
    );
    // Answer yes and drain: still one flash (the re-run must not re-post).
    t.answer_one(0).unwrap();
    drain(&mut t);
    let f2 = flashes_since(&t, mark);
    assert_eq!(
        f2.len(),
        1,
        "the re-run does not flash again: {}",
        t.recent_keys(20).join(", ")
    );
}

/// (b) answered **no**: same, one flash.
#[test]
fn flash_then_ask_no_flashes_once() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashThenAsk");
    t.answer_one(1).unwrap();
    drain(&mut t);
    let f = flashes_since(&t, mark);
    assert_eq!(
        f.len(),
        1,
        "answered no: still one flash: {}",
        t.recent_keys(20).join(", ")
    );
}

/// (c) two prompts in a row, one flash per distinct ordinal, none doubled.
#[test]
fn two_prompts_one_flash_per_ordinal() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashTwoAsks");
    // First prompt: one flash so far.
    t.answer_one(0).unwrap();
    // Second prompt: two flashes so far (one per ordinal).
    if t.prompt().is_some() {
        t.answer_one(0).unwrap();
    }
    drain(&mut t);
    let f = flashes_since(&t, mark);
    assert_eq!(
        f.len(),
        2,
        "one flash per ordinal, none doubled: {}",
        t.recent_keys(20).join(", ")
    );
}

/// (d) `flash_card_keyed` with the same key twice in one drive flashes once.
#[test]
fn flash_card_keyed_same_key_flashes_once() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashKeyedTwice");
    drain(&mut t);
    let f: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "flash")
        .collect();
    assert_eq!(
        f.len(),
        1,
        "the same key flashes once: {}",
        t.recent_keys(20).join(", ")
    );
}

/// (e) a drive thrown away (guard reject / discarded probe) posts none.
/// `TEST:pre_reject`'s pre-rejects its hook; a play of it never reaches a
/// body, so no flash appears.
#[test]
fn a_thrown_away_drive_posts_none() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    // A guard-rejecting hook: the body never runs.
    t.place_raw(0, "TEST:pre_reject");
    let mark = t.mark();
    // Land so the hook would run if it were admitted.
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let ran: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == "TEST:pre_reject" && e.flash)
        .collect();
    assert!(
        ran.is_empty(),
        "a rejected drive posts no flash: {}",
        t.recent_keys(20).join(", ")
    );
}

/// (f) a nested drive's flashes are independent of the outer's.
#[test]
fn nested_drive_flashes_are_independent() {
    let mut t = Table::vanilla(2);
    let mark = play_marked(&mut t, "TEST:flashOuter");
    drain(&mut t);
    let f: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "flash")
        .collect();
    assert_eq!(
        f.len(),
        2,
        "outer + inner each flash once: {}",
        t.recent_keys(20).join(", ")
    );
    let keys: Vec<_> = f.iter().map(|e| e.msg.key().to_string()).collect();
    assert!(
        keys.iter().any(|k| k.contains("flash_outer_cue"))
            && keys.iter().any(|k| k.contains("flash_inner_cue")),
        "both cues posted: {keys:?}"
    );
}

/// (h) `--state` unchanged: the event stream is deterministic across two runs.
/// Two identical drives from the same board produce identical flash ids and
/// keys in the same order.
#[test]
fn event_stream_deterministic_across_two_runs() {
    let run = || {
        let mut t = Table::vanilla(2);
        let mark = play_marked(&mut t, "TEST:flashKeyedTwice");
        drain(&mut t);
        let f: Vec<_> = t
            .events_since(mark)
            .into_iter()
            .filter(|e| e.r#type == "flash" || (e.r#type == "card" && e.flash))
            .map(|e| (e.r#type.clone(), e.msg.key().to_string()))
            .collect();
        f
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "the flash stream is deterministic across two runs");
    assert!(!a.is_empty(), "and it actually contains flashes");
}
