//! No log message may render an internal key as verbatim text.
//!
//! `Arg::Text` is for player-entered names only (`msg.rs`). Counter names,
//! state keys, mark kinds and trigger-kind wire names must ride as a nested
//! `Msg` whose key has i18n display text. This test walks a seeded game's
//! event tail (including `results` and nested args) and fails on a raw
//! `cards:` / `mark:` key, a lower-camel state key, or a known counter name
//! appearing as `Text`.

mod common;

use common::*;
use game_core::msg::{Arg, Msg};
use game_core::state::MatchEvent;

/// Internal-key shapes that must never appear as `Arg::Text`.
fn is_internal_key(s: &str) -> bool {
    if s.starts_with("cards:") || s.starts_with("mark:") || s.starts_with("skill.") {
        return true;
    }
    // Known on-card counter names.
    if matches!(s, "crystals" | "cp" | "cpField" | "skillBlock") {
        return true;
    }
    // Lower-camel engine state / prop keys (handLimit, startHand, skillState,
    // unstoppable, noHand, stunStart, exileTo, ...). Rulebook terms are
    // Chinese or bracketed ([停留]); card-declared counters are display names.
    if s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars().any(|c| c.is_ascii_uppercase())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return true;
    }
    false
}

fn walk_arg(arg: &Arg, where_: &str, bad: &mut Vec<String>) {
    match arg {
        Arg::Text(t) => {
            if is_internal_key(t) {
                bad.push(format!("{where_}: Text({t:?})"));
            }
        }
        Arg::Msg(m) => walk_msg(m, where_, bad),
        Arg::List(items) => {
            for (i, a) in items.iter().enumerate() {
                walk_arg(a, &format!("{where_}[{i}]"), bad);
            }
        }
        _ => {}
    }
}

fn walk_msg(m: &Msg, where_: &str, bad: &mut Vec<String>) {
    let w = format!("{where_} {}", m.key());
    for (name, arg) in m.a.iter() {
        walk_arg(arg, &format!("{w}.{name}"), bad);
    }
}

fn check_events(events: &[MatchEvent], label: &str) {
    let mut bad = Vec::new();
    for e in events {
        let w = format!("{label}#{} {}", e.id, e.r#type);
        walk_msg(&e.msg, &w, &mut bad);
        for (i, r) in e.results.iter().enumerate() {
            walk_msg(r, &format!("{w}.results[{i}]"), &mut bad);
        }
    }
    assert!(
        bad.is_empty(),
        "internal keys rendered as Text ({} lines):\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// A seeded game with skills and cards in play: every outcome / why line that
/// fired must name its subject via a typed arg, never a raw key.
#[test]
fn no_log_message_renders_an_internal_key_as_text() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    // Drive several shapes: a field hook, a money line, a roll, a turn.
    t.give_play(0, "TEST:tileHook").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.set_pos(0, 0);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.end(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.begin_turn(1);
    while t.prompt().is_some() {
        t.decline();
    }
    check_events(&t.events_since(mark), "fixture-game");
}

/// Shipped skill/card outcomes (counter writes, state writes, plan rewrites)
/// across a short real game: same contract.
#[test]
fn no_shipped_outcome_renders_an_internal_key_as_text() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.set_character_raw(0, "濑田薰");
    t.set_character_raw(1, "和奏瑞依");
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let mark = t.mark();
    // A few turns: hooks fire, caps re-arm, opt-ins decline.
    for _ in 0..3 {
        t.set_pos(0, 0);
        t.dice(&[3]);
        t.roll(0).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.end(0).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.begin_turn(1);
        while t.prompt().is_some() {
            t.decline();
        }
        t.set_pos(1, 0);
        t.dice(&[3]);
        t.roll(1).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.end(1).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.begin_turn(0);
        while t.prompt().is_some() {
            t.decline();
        }
    }
    check_events(&t.events_since(mark), "shipped-game");
}