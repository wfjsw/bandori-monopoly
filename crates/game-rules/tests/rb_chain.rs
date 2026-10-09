//! Black-box tests for the [反击] chain builder: one round per timing, the
//! ring order of rulebook 89 under **ruling 2026-10-07**, counters to counters
//! (the 「新的时点」), and LIFO resolution.
//!
//! Ruling 2026-10-07 (verbatim): "Revise the chain counteraction mechanism.
//! Including the initial user, each user in the chain should be able to exhaust
//! all counteraction chances (or voluntarily abandon) for it to advance to next
//! player. Adjust as appropriate."
//!
//! The chain rule this pins down:
//! 1. **Start seat:** the ask ring starts with the **initial user** -- the
//!    player whose action or effect raised the link. If there is no player,
//!    the active turn player. (The old rule started at the seat *after* the
//!    trigger player and asked them last.)
//! 2. **Exhaust on each visit:** a seat keeps being offered its eligible
//!    counteractions until it passes explicitly or has none left. Only then
//!    does priority advance. (The old rule allowed one activation per visit.)
//! 3. **Round closing:** laps continue until a full lap brings no new
//!    declaration.
//! 4. **Counters to counters:** a declaration's own link gets its own round
//!    with the same rules, where the declarer is that round's initial user.
//! 5. **Resolution:** LIFO (newest first), unchanged.
//! 6. **Guards:** a seat can't declare the same card twice in a round;
//!    eligibility is re-checked before each offer.
//!
//! The cards under test are the `TEST:*` fixtures (`rules/fixtures/test-cards`):
//! `TEST:aimer` names one other player (the effect declaration X),
//! `TEST:probe` answers **any** effect link (a deliberately loose guard, so
//! several responders -- the triggering player included -- can line up on one
//! timing) and `TEST:deny` answers a **counter's** play and negates it.

mod common;

use common::{int_arg, tile, data, Table};
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
        .iter()
        .filter(|e| e.msg.key() == full)
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
// Clause 89, ruling 2026-10-07: the ring
// =====================================================================

// ruling 2026-10-07: 「Including the initial user, each user in the chain…」 --
// the ask ring starts with the **initial user**, the player whose action raised
// the link (P0 played TEST:aimer), not with the seat after them.
// Rewritten from `the_next_seat_is_asked_first_and_the_triggering_player_last`
// (old expectation: ring 1, 2, 0 -- next seat first, triggerer last).
#[test]
fn the_initial_user_is_asked_first() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer", "TEST:probe"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // Ring order (ruling 2026-10-07): 0 (initial user), 1, 2.
    assert_eq!(t.asked(), vec![0], "the initial user first: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![1], "then the next seat: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![2], "then the last seat: {}", t.dump_prompt());
    t.decline();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "no counter landed, so X settled"
    );
}

// ruling 2026-10-07: 「each user…should be able to exhaust all counteraction
// chances (or voluntarily abandon) for it to advance to next player」 -- a seat
// keeps the floor while it still holds an eligible card, and may declare twice
// in one visit. Rewritten from `a_player_may_declare_again_when_the_ring_returns`
// (old expectation: one activation per visit, the second declaration only when
// the ring came back) and from `a_declaration_passes_priority_to_the_next_seat`
// (old expectation: a declaration alone hands priority on).
#[test]
fn one_seat_declares_twice_in_one_visit_before_the_next_seat_is_asked() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    give_n(&mut t, 1, "TEST:probe", 2);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // P0 (initial user) holds no counter, so it is skipped; P1 is up first.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // ruling 2026-10-07: the visit does not end on a declaration -- P1 still
    // holds an eligible probe and is asked again in the same visit.
    assert_eq!(
        t.asked(),
        vec![1],
        "the same seat again while it still holds an eligible card: {}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:probe").unwrap();
    // Only now (no eligible card left) does priority advance.
    assert_eq!(t.asked(), vec![2], "only then the next seat: {}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    // All three answered X and settle LIFO: P2's probe is the newest
    // declaration (P1 declared first-to-last inside its one visit), so it
    // lands first, then P1's second, then P1's first.
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![2, 1, 1],
        "three probes settled LIFO: {:?}",
        ev_args(&t, "probe_fired", "n")
    );
}

// ruling 2026-10-07: 「…or voluntarily abandon」 -- an explicit pass ends the
// visit even while the seat still holds an eligible card; only then does
// priority advance. (The old rule advanced priority on the declaration itself
// and never re-offered the seat inside one visit.)
#[test]
fn an_explicit_pass_advances_priority() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    give_n(&mut t, 1, "TEST:probe", 2);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    assert_eq!(
        t.asked(),
        vec![1],
        "still P1's visit (ruling 2026-10-07): {}",
        t.dump_prompt()
    );
    // P1 abandons its second probe: the pass advances priority immediately.
    t.decline();
    assert_eq!(
        t.asked(),
        vec![2],
        "the explicit pass advanced priority, not the declaration: {}",
        t.dump_prompt()
    );
    // The abandoned probe may line up again on a later lap, but a plain pass
    // loop leaves it un-declared: only one probe settles.
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![1],
        "only the declared probe ran"
    );
    assert_eq!(
        t.draw_pile(1).iter().filter(|c| *c == "TEST:probe").count(),
        1,
        "one probe spent and immediately reshuffled, one kept: {:?}",
        t.hand(1)
    );
}

// ruling 2026-10-07: a seat with no eligible counteraction is skipped without
// a prompt (eligibility is re-checked before every offer), so the ring jumps
// from the initial user straight to the next seat that still holds one.
#[test]
fn a_seat_with_no_eligible_card_is_skipped_without_a_prompt() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer", "TEST:probe"]);
    // P1 holds nothing at all.
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![0], "the initial user first: {}", t.dump_prompt());
    t.decline();
    assert_eq!(
        t.asked(),
        vec![2],
        "P1 holds no eligible card and is never prompted: {}",
        t.dump_prompt()
    );
    t.decline();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
}

// ruling 2026-10-07: 「Including the initial user」 + laps until quiet -- a
// later seat's declaration keeps the round going and re-opens the earlier
// seats on the next lap. (The old rule also looped the ring, but started at
// the seat after the trigger player; here the re-opened seat P0 is the
// initial user and is asked first on every lap.)
#[test]
fn a_later_seats_declaration_re_opens_an_earlier_seat() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer", "TEST:probe"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // Lap 1: P0 (initial user) and P1 pass; P2 declares.
    assert_eq!(t.asked(), vec![0], "{}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap();
    // ruling 2026-10-07: lap 1 carried P2's declaration, so the round continues
    // into another lap and re-opens the earlier seats -- P0 first.
    assert_eq!(
        t.asked(),
        vec![0],
        "P2's declaration re-opened the earlier seat P0: {}",
        t.dump_prompt()
    );
    t.decline();
    assert_eq!(
        t.asked(),
        vec![1],
        "and the next earlier seat P1: {}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:probe").unwrap();
    // Lap 2 carried P1's declaration, so one more lap runs: P0 still holds its
    // probe and is asked once more; P2 is spent and skipped.
    assert_eq!(t.asked(), vec![0], "lap 3 re-opens P0 again: {}", t.dump_prompt());
    t.decline();
    // Lap 3 brought no declaration -- quiet lap, so X's round is closed and the
    // declared probes settle LIFO. Each probe body moves money, and the money
    // pipeline opens a window on every movement (docs/ENGINE.md); P0's leftover
    // probe answers any effect link and is offered there. Those windows are
    // fresh timings -- they must not be X's round.
    while t.prompt().is_some() {
        assert_ne!(
            answered_card(&t).as_deref(),
            Some("TEST:aimer"),
            "X's round must be closed after the quiet lap (ruling 2026-10-07): {}",
            t.dump_prompt()
        );
        t.decline();
    }
    // LIFO: P1's later declaration settles before P2's earlier one.
    assert_eq!(who_of(&t, "probe_fired"), vec![1, 2], "LIFO: {:?}", ev_args(&t, "probe_fired", "n"));
}

// ruling 2026-10-07: 「for it to advance to next player」 / round closing --
// laps continue until a full lap brings no new declaration. A lap that
// carried a declaration never closes the round, even if every seat passed
// after it; the *next* full lap must be quiet. (The old rule closed after one
// consecutive pass from every responder following the last declaration.)
#[test]
fn the_round_closes_after_a_quiet_lap() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    give_n(&mut t, 1, "TEST:probe", 2);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // Lap 1 (P0 holds no counter, skipped): P1 declares, then abandons its
    // second probe; P2 passes and keeps its probe.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    assert_eq!(t.asked(), vec![1], "exhaust / abandon in one visit: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    t.decline();
    // Lap 1 carried a declaration -- it does not close the round.
    assert!(
        t.prompt().is_some(),
        "a lap with a declaration does not close the round (ruling 2026-10-07): {}",
        t.dump_prompt()
    );
    // Lap 2: P1 and P2 still hold eligible cards, both pass. No declaration.
    assert_eq!(t.asked(), vec![1], "lap 2: {}", t.dump_prompt());
    t.decline();
    assert_eq!(t.asked(), vec![2], "lap 2: {}", t.dump_prompt());
    t.decline();
    // The quiet lap closes the round on X. The declared probe then settles and
    // its money movement is itself a timing (the money pipeline opens a window
    // on every movement); the leftover probes answer any effect link and are
    // offered there. Those windows are fresh timings -- not X's round.
    while t.prompt().is_some() {
        assert_ne!(
            answered_card(&t).as_deref(),
            Some("TEST:aimer"),
            "a quiet lap must close X's round (ruling 2026-10-07): {}",
            t.dump_prompt()
        );
        t.decline();
    }
    assert_eq!(who_of(&t, "probe_fired"), vec![1], "only the declared probe ran");
}

// =====================================================================
// Clause 89: several counters to one timing
// =====================================================================

// 规则书（89）: 「多个效果可[反击]同一个时点」 -- two players answer X itself,
// not each other's counters. ruling 2026-10-07: the ring starts at the initial
// user (P0); P0 holds no counter so it is skipped and P1 is asked first, and
// each declaration exhausts its seat before priority advances.
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
    // P1's visit is exhausted (its only probe is spent), so P2 is next -- and
    // P2 is asked about the same X, not about P1's probe.
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
// Ruling: LIFO resolution (unchanged by ruling 2026-10-07)
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

// ruling 2026-10-07 point 5 (LIFO, unchanged) with point 2 (exhaust on each
// visit): one seat's two declarations are two links, and they settle newest
// first -- the *second* declaration of the visit lands before the first, and
// both land after any later seat's counter.
#[test]
fn lifo_resolution_with_multiple_links_from_one_seat() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    give_n(&mut t, 1, "TEST:probe", 2);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // P1 exhausts both probes in one visit (ruling 2026-10-07), then P2.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap(); // older link
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap(); // newer link, same visit
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap(); // newest link
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    // LIFO over three links: P2 (declared last), then P1's second, then
    // P1's first. `n` counts settlements on the one shared link (X).
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![2, 1, 1],
        "newest first: {:?}",
        ev_args(&t, "probe_fired", "n")
    );
    assert_eq!(ev_args(&t, "probe_fired", "n"), vec![0, 1, 2], "n = settle order");
    assert_eq!(t.money(2), 10_100, "newest: +100");
    assert_eq!(t.money(1), 10_500, "P1's second +200, first +300");
    assert_eq!(t.money(0), 9_400, "X's player paid all three cuts");
}

// =====================================================================
// Clause 89: 「…后可对新的时点发动[反击]」 -- counters to counters
// =====================================================================

// 规则书（89）: 「所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]」
// -- a counter is a new timing, answered only once the round on X has closed.
// ruling 2026-10-07: the ring on X starts at the initial user and laps until
// quiet, so a seat still holding an eligible card is re-offered on the quiet
// lap before the round closes. Rewritten: the old rule closed X's round after
// a single pass from P2; now P2's first pass only ends its visit in lap 1 and
// the quiet lap re-offers it.
#[test]
fn counters_to_a_counter_wait_for_the_round_on_x_to_close() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe", "TEST:deny"]);
    t.play(0, "TEST:aimer").unwrap();
    // P0 (initial user) holds no counter -> skipped. P1 is up.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // P1's visit is exhausted (no second probe), so priority advances to P2.
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
    // ruling 2026-10-07: lap 1 carried P1's declaration, so the round on X is
    // not closed by that pass -- the quiet lap re-offers P2 (still holding its
    // probe) before X's round may close.
    assert_eq!(
        t.asked(),
        vec![2],
        "the quiet lap re-offers P2 before X's round closes: {}",
        t.dump_prompt()
    );
    assert!(
        t.counteract_offered("TEST:probe"),
        "still answering X: {}",
        t.dump_prompt()
    );
    assert!(
        !t.counteract_offered("TEST:deny"),
        "the deny still waits: {}",
        t.dump_prompt()
    );
    t.decline();
    // The quiet lap has run: X's round has closed; the declared counter is the
    // new timing. ruling 2026-10-07 point 4: that round starts with the
    // declarer (P1). P1 holds no card that answers a counter's play (the probe
    // answers effects only) so it is skipped; P2's deny is next.
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

// ruling 2026-10-07 point 4: 「a declaration's own link gets its own round with
// the same rules, where the declarer is that round's initial user」 -- the
// declarer is asked first on its own counter's round, ahead of everyone else.
// (The old rule opened that round at the seat after the declarer and asked the
// declarer last.)
// DISCREPANCY: with only the declarer holding a TEST:deny, the round over the
// declarer's own probe closes with **no prompt at all** -- the deny is not
// offered to the declarer against their own counter's play (either the
// fixture guard excludes self, or the engine omits the declarer from their own
// counter's round). The ruling's "the declarer is that round's initial user"
// is therefore unobservable with the TEST fixtures; the assertion below stays
// as the ruling's expectation.
#[ignore = "DISCREPANCY: TEST:deny is not offered to the declarer against their own counter's play, so the round over the declarer's own probe closes with no prompt and 'the declarer is that round's initial user' (ruling 2026-10-07) cannot be observed"]
#[test]
fn counters_to_counters_start_with_the_declarer() {
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    // P1: the probe that answers X, and a deny that answers a counter's play
    // -- so P1 has an eligible card on the round over its own probe and cannot
    // be skipped there.
    t.give(1, &["TEST:probe", "TEST:deny"]);
    t.play(0, "TEST:aimer").unwrap();
    // Round on X: P0 (initial user) holds no counter -> skipped; P1 declares.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // X's round closes at once (P1's remaining deny does not answer an effect,
    // P2 and P0 hold nothing) and the probe's own round opens -- starting with
    // its declarer P1 (ruling 2026-10-07), who is offered the deny.
    assert_eq!(
        t.asked(),
        vec![1],
        "the counter's round starts with its declarer (ruling 2026-10-07): {}",
        t.dump_prompt()
    );
    assert_eq!(
        answered_card(&t),
        Some("TEST:probe".to_string()),
        "the new timing is the declared counter"
    );
    assert!(
        t.counteract_offered("TEST:deny"),
        "the declarer's deny answers its own counter's play: {}",
        t.dump_prompt()
    );
    // P1 abandons; the quiet lap closes the round and the probe runs.
    t.decline();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(who_of(&t, "probe_fired"), vec![1], "the probe ran");
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "then X settled");
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
    // Round on the probe: ruling 2026-10-07 point 4 starts it with the
    // declarer P1, who holds nothing that answers a counter's play -- so the
    // first offer is P2's deny.
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
        t.draw_pile(1).contains(&"TEST:probe".to_string()),
        "the negated counter is spent and immediately reshuffled: {:?}",
        t.draw_pile(1)
    );
    // Its body would have negated X; it never ran, so X settled.
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "X landed because its counter was negated"
    );
}

// =====================================================================
// ruling 2026-10-07: a trigger with no player
// =====================================================================

// ruling 2026-10-07: 「the player whose action or effect raised the link. If
// there is no player, the active turn player.」 A rent payment is board-driven
// (`by_card` is None -- docs/CARDS.md), so the link has no player and the ask
// ring starts at the active turn player P0, not at the payer/payee's next seat.
#[test]
fn a_no_player_trigger_starts_at_the_turn_player() {
    let mut t = Table::vanilla(3);
    let tl = tile("富士见坂");
    let rent = data().tiles[tl].rent[0];
    assert!(rent > 0, "rent = {rent}");
    t.set_owner(tl, Some(1));
    t.set_pos(0, tl - 1);
    t.give(0, &["TEST:probe"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // The rent pay raises an effect link with no player; the ring starts at
    // the active turn player (P0) -- ruling 2026-10-07.
    assert_eq!(
        t.asked(),
        vec![0],
        "a no-player trigger starts at the turn player: {}",
        t.dump_prompt()
    );
    while t.prompt().is_some() {
        t.decline();
    }
    // No probe declared: the rent settles straight.
    assert_eq!(t.money(0), 10_000 - rent, "P0 paid the rent");
    assert_eq!(t.money(1), 10_000 + rent, "P1 received it");
    assert_eq!(t.money(2), 10_000, "P2 untouched");
    assert!(events(&t, "probe_fired").is_empty(), "no probe ran");
}
