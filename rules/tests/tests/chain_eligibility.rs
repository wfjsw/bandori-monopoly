//! Counteraction eligibility cuts the book names (out / off-board).
//!
//! The engine-side half of the bot-window suite lives in
//! `crates/game-rules/tests/chain_bot.rs` (out / bankrupt). This file holds the
//! off-board half: the engine keeps no named mirror of the layer keys, so the
//! cut is exercised here where `bandori_std::state_key` may name the frozen
//! wire key. "Is a bot" is not a rulebook reason -- an off-board seat is never
//! asked, whatever drives it.

mod common;

use bandori_std::state_key;
use common::*;
use game_core::state::{BotMentality, MatchEvent};

/// Fixture events with this name, oldest first (`key!("…")` in
/// `rules/fixtures/test-cards`).
fn events(t: &Table, name: &str) -> Vec<MatchEvent> {
    let full = format!("cards:fixture-test-cards.{name}");
    t.st()
        .events
        .iter()
        .filter(|e| log_key(e) == full)
        .cloned()
        .collect()
}

/// One integer argument per fixture event, oldest first.
fn ev_args(t: &Table, name: &str, arg: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| int_arg(&e.msg, arg).unwrap_or(-1))
        .collect()
}

/// The `who` of each `probe_fired` -- the resolution order.
fn who_of(t: &Table, name: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| match e.msg.a.get("who") {
            Some(game_core::msg::Arg::PlayerId(v)) => *v as i64,
            Some(game_core::msg::Arg::I(v)) | Some(game_core::msg::Arg::N(v)) => *v,
            other => panic!("who is not a player arg: {other:?}"),
        })
        .collect()
}

/// An off-board bot holds an eligible counter and is never asked. The live
/// counter at seat 1 is, so the window still opens.
///
/// Seat layout: `TEST:aimer` names `others().first()` = seat 1, so the
/// off-board seat sits at 2 and the live counter at 1 -- the effect link is
/// raised against a live player and the window opens normally.
#[test]
fn off_board_seats_never_get_an_offer() {
    let mut t = Table::vanilla(3);
    t.make_bot(2, BotMentality::Standard);
    t.set_state(2, state_key::EXILE, 1);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    let mut asked_ever = Vec::new();
    while t.prompt().is_some() {
        let a = t.asked();
        assert!(
            !a.contains(&2),
            "the off-board seat was offered: {:?} {}",
            a,
            t.dump_prompt()
        );
        asked_ever.extend(a);
        t.decline();
    }
    assert!(!asked_ever.is_empty(), "the window never opened at all");
    assert_eq!(
        who_of(&t, "probe_fired"),
        Vec::<i64>::new(),
        "nobody declared"
    );
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "X named its live target"
    );
}
