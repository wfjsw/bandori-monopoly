//! Black-box tests for the [反击] chain builder: one round per timing, the
//! ring order of rulebook 89, counters to counters (the 「新的时点」), and LIFO
//! resolution.
//!
//! The cards under test are the `TEST:*` fixtures (`rules/fixtures/test-cards`):
//! `TEST:aimer` names one other player (the effect declaration X),
//! `TEST:probe` answers **any** effect link (a deliberately loose guard, so
//! several responders -- the triggering player included -- can line up on one
//! timing) and `TEST:deny` answers a **counter's** play and negates it.

mod common;

use common::{int_arg, Table};
use game_core::msg::Arg;
use game_core::state::MatchEvent;

/// Fixture log key (`key!("…")` in `rules/fixtures/test-cards`).
fn ev(name: &str) -> String {
    format!("cards:fixture-test-cards.{name}")
}

/// Fixture events with this name, oldest first.
fn events(t: &Table, name: &str) -> Vec<MatchEvent> {
    let full = ev(name);
    t.st()
        .events
        .into_iter()
        .filter(|e| e.msg.key() == full)
        .collect()
}

/// One integer argument per fixture event, oldest first.
fn ev_args(t: &Table, name: &str, arg: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| int_arg(&e.msg, arg).unwrap_or(-1))
        .collect()
}

/// The `who` of each `probe_fired` / `deny_fired` -- the resolution order.
fn who_of(t: &Table, name: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| match e.msg.a.get("who") {
            Some(Arg::PlayerId(v)) => *v as i64,
            Some(Arg::I(v)) | Some(Arg::N(v)) => *v,
            other => panic!("who is not a player arg: {other:?}"),
        })
        .collect()
}

/// The card the open [反击] window's detail line names -- the timing it answers.
fn answered_card(t: &Table) -> Option<String> {
    let p = t.prompt()?;
    match p.text.a.get("detail") {
        Some(Arg::Msg(m)) => match m.a.get("card") {
            Some(Arg::Card(id)) => Some(id.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Give `who` `n` copies of `card`.
fn give_n(t: &mut Table, who: usize, card: &str, n: usize) {
    let cards: Vec<&str> = vec![card; n];
    t.give(who, &cards);
}

// =====================================================================
// Clause 89: the ring
// =====================================================================

// 规则书（89）: 「从行动顺序上在触发[反击]时点的玩家的**下一位**玩家开始依次决定」
// -- the next seat is asked first and the triggering player last.
#[test]
fn the_next_seat_is_asked_first_and_the_triggering_player_last() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer", "TEST:probe"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // Ring order after P0 (who triggered X): 1, 2, 0.
    assert_eq!(t.asked(), vec![1], "first: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![2], "second: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![0], "the triggerer last: {}", t.dump_prompt());
    t.decline();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "no counter landed, so X settled"
    );
}

// 规则书（89）: 「依次决定是否使用[反击]效果」 -- a declaration hands priority
// on to the next responder, not back to the player the timing belongs to.
#[test]
fn a_declaration_passes_priority_to_the_next_seat() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer", "TEST:probe"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // Next seat (2), not the starter's seat (0) and not 1 again.
    assert_eq!(
        t.asked(),
        vec![2],
        "priority advanced to the next responder: {}",
        t.dump_prompt()
    );
}

// 规则书（89）: 「依次决定」 -- a player may declare again when the ring comes
// back to them, and never twice in one visit.
#[test]
fn a_player_may_declare_again_when_the_ring_returns() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    give_n(&mut t, 1, "TEST:probe", 2);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // First visit: one declaration only -- the window is a single pick.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // Not a second declaration in the same visit: the ring moves on.
    assert_eq!(t.asked(), vec![2], "one per visit: {}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap();
    // The ring came back around to P1, who still holds a probe.
    assert_eq!(t.asked(), vec![1], "ring returned: {}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    // All three answered X and settle LIFO: P1's *second* probe is the newest
    // declaration, so it lands first, then P2, then P1's first.
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![1, 2, 1],
        "three probes settled LIFO: {:?}",
        ev_args(&t, "probe_fired", "n")
    );
}

// =====================================================================
// Clause 89: several counters to one timing
// =====================================================================

// 规则书（89）: 「多个效果可[反击]同一个时点」 -- two players answer X itself,
// not each other's counters.
#[test]
fn two_players_answer_the_same_effect_not_each_other() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    assert_eq!(
        answered_card(&t),
        Some("TEST:aimer".to_string()),
        "the window answers X"
    );
    t.counteract(1, "TEST:probe").unwrap();
    // P2 is asked about the same X -- not about P1's probe.
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    assert_eq!(
        answered_card(&t),
        Some("TEST:aimer".to_string()),
        "the second window answers X too, not the first counter"
    );
    t.counteract(2, "TEST:probe").unwrap();
    // Both bodies ran against X: the answered link's `seq` is 0 (X) for both,
    // and the settlement counter (`n`) counted up on one shared link.
    assert_eq!(ev_args(&t, "probe_fired", "seq"), vec![0, 0], "both on X");
    assert_eq!(
        ev_args(&t, "probe_fired", "n"),
        vec![0, 1],
        "one link, two settlements"
    );
}

// =====================================================================
// Ruling: LIFO resolution
// =====================================================================

// Counters to one timing resolve newest first, and each still settles before
// the timing it answers -- readable in the money (the newest probe takes the
// smallest cut) and in the log order.
#[test]
fn counters_resolve_newest_first() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    t.counteract(1, "TEST:probe").unwrap(); // older
    t.counteract(2, "TEST:probe").unwrap(); // newer
    // Newest first: P2 saw n=0 and took 100; P1 saw n=1 and took 200.
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![2, 1],
        "newest first: {:?}",
        events(&t, "probe_fired")
            .iter()
            .map(|e| int_arg(&e.msg, "n"))
            .collect::<Vec<_>>()
    );
    assert_eq!(t.money(2), 10_100, "the newer counter settled first (+100)");
    assert_eq!(t.money(1), 10_200, "the older counter settled second (+200)");
    assert_eq!(t.money(0), 9_700, "X's player paid both cuts");
    // Both settled before X: the effect still landed.
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1]);
}

// =====================================================================
// Clause 89: 「…后可对新的时点发动[反击]」 -- counters to counters
// =====================================================================

// 规则书（89）: 「所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]」
// -- a counter is a new timing, answered only once the round on X has closed.
#[test]
fn counters_to_a_counter_wait_for_the_round_on_x_to_close() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe", "TEST:deny"]);
    t.play(0, "TEST:aimer").unwrap();
    t.counteract(1, "TEST:probe").unwrap();
    // Still inside the round on X: P2 may answer X (the probe), and the deny --
    // which answers a *counter* -- is not on offer yet.
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    assert!(
        t.counteract_offered("TEST:probe"),
        "still answering X: {}",
        t.dump_prompt()
    );
    assert!(
        !t.counteract_offered("TEST:deny"),
        "a counter's own counter waits for the round on X to close: {}",
        t.dump_prompt()
    );
    t.decline();
    // The round on X has closed; the declared counter is the new timing.
    assert_eq!(t.asked(), vec![2], "the new timing: {}", t.dump_prompt());
    assert!(
        t.counteract_offered("TEST:deny"),
        "now the counter's play can be answered: {}",
        t.dump_prompt()
    );
    assert!(
        !t.counteract_offered("TEST:probe"),
        "the probe answers effects, not a counter's play: {}",
        t.dump_prompt()
    );
    assert_eq!(
        answered_card(&t),
        Some("TEST:probe".to_string()),
        "the new timing is the declared counter"
    );
}

// A counter whose activation one of its own answers negated does not run its
// body -- and the timing it would have answered settles untouched.
#[test]
fn a_negated_counters_body_does_not_run() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:deny"]);
    t.play(0, "TEST:aimer").unwrap();
    t.counteract(1, "TEST:probe").unwrap();
    // Round on X closes (P2's deny does not answer an effect; P0 has nothing).
    // Round on the probe: P2 negates it.
    assert!(t.counteract_offered("TEST:deny"), "{}", t.dump_prompt());
    t.counteract(2, "TEST:deny").unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(who_of(&t, "deny_fired"), vec![2], "the deny ran");
    assert!(
        events(&t, "probe_fired").is_empty(),
        "the negated counter's body did not run"
    );
    // The declaration still stands: the card was played and is spent.
    assert!(
        t.discard(1).contains(&"TEST:probe".to_string()),
        "the negated counter is still spent: {:?}",
        t.discard(1)
    );
    // Its body would have negated X; it never ran, so X settled.
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "X landed because its counter was negated"
    );
}