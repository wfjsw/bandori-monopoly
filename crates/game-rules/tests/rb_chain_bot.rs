//! Black-box tests for [反击] windows at **bot** seats (`can_counteract_now`).
//!
//! The rulebook's eligibility is out / [除外] (plus `CannotPlay`: stunned,
//! no-hand). "Is a bot" is not a rulebook reason, so every mentality is offered
//! exactly like a human and answers through the engine's AI fill
//! (`Cx::fill_ai`): standard by its `CounterParams` propensity (default
//! `DEFAULT_COUNTERACT_PROPENSITY_MILLI` = 600‰, user ruling 2026-10-08 --
//! "bots must be able to counteract"), chaos by `CHAOS_COUNTER_CHANCE`. An
//! Advanced seat is held (`ai` off) for an external driver -- the offer is an
//! ordinary prompt and the engine's deadline answers it with the fallback (the
//! skip), so a window never stalls. Out / exiled seats still never see an
//! offer.
//!
//! Chain semantics (ruling 2026-10-07) are unchanged for humans -- see
//! `rb_chain.rs`. Fixtures: `TEST:aimer` (effect X), `TEST:probe` (answers any
//! effect link), from `rules/fixtures/test-cards`.

mod common;

use common::{characters, data, int_arg, Table};
use game_core::msg::Arg;
use game_core::state::{BotMentality, MatchEvent};
use game_core::strategy::{
    CounterParams, StrategyBook, StrategyEntry, StrategyParams, PARAMS_VERSION, POLICY_STANDARD,
    STRATEGY_BOOK_VERSION,
};
use game_core::MatchMode;
use std::collections::BTreeMap;

/// Fixture log key (`key!("…")` in `rules/fixtures/test-cards`).
fn ev(name: &str) -> String {
    format!("cards:fixture-test-cards.{name}")
}

/// The outcome line a log event speaks as: itself, or the `what` inside an
/// in-play card's attribution (「<卡名> 的效果：<what>」, `log.card_effect`).
fn unadorned(msg: &game_core::msg::Msg) -> &game_core::msg::Msg {
    if msg.key() == "log.card_effect" {
        if let Some(Arg::Msg(m)) = msg.a.get("what") {
            return m;
        }
    }
    msg
}

/// Fixture events with this name, oldest first.
fn events(t: &Table, name: &str) -> Vec<MatchEvent> {
    let full = ev(name);
    t.st()
        .events
        .iter()
        .filter(|e| unadorned(&e.msg).key() == full)
        .cloned()
        .collect()
}

/// One integer argument per fixture event, oldest first.
fn ev_args(t: &Table, name: &str, arg: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| int_arg(unadorned(&e.msg), arg).unwrap_or(-1))
        .collect()
}

/// The `who` of each `probe_fired` -- the resolution order.
fn who_of(t: &Table, name: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| match unadorned(&e.msg).a.get("who") {
            Some(Arg::PlayerId(v)) => *v as i64,
            Some(Arg::I(v)) | Some(Arg::N(v)) => *v,
            other => panic!("who is not a player arg: {other:?}"),
        })
        .collect()
}

/// A strategy book that only names `who`'s [反击] propensity for `card`.
fn book_declaring(who: &str, card: &str, propensity_milli: i32) -> StrategyBook {
    let mut params = StrategyParams::default();
    params.counteract.insert(
        card.into(),
        CounterParams {
            propensity_milli,
            by_kind: BTreeMap::new(),
        },
    );
    let mut b = StrategyBook::default();
    b.version = STRATEGY_BOOK_VERSION;
    b.policy = POLICY_STANDARD.into();
    b.params_version = PARAMS_VERSION;
    b.generated_at = "rb_chain_bot".into();
    b.me.push(StrategyEntry {
        me: who.into(),
        band: String::new(),
        opponent_bands: Vec::new(),
        deck: Vec::new(),
        params,
    });
    b
}

/// The first `n` characters, as [`Table::vanilla`] seats them.
fn vanilla_book(n: usize, book: StrategyBook) -> Table {
    let names = characters();
    let chars: Vec<&str> = names.iter().take(n).map(String::as_str).collect();
    let mut t = Table::with_book(&chars, book, MatchMode::Solo);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t
}

/// Same as [`vanilla_book`], but on the online (Casual) clock so a held seat's
/// prompt deadline actually runs out -- Solo never forces a local answer.
fn vanilla_casual(n: usize) -> Table {
    let names = characters();
    let chars: Vec<&str> = names.iter().take(n).map(String::as_str).collect();
    let mut t = Table::with_data(data(), &chars, common::SEED, MatchMode::Casual);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t
}

/// [`Table::vanilla`] under a given match seed (the propensity draws ride the
/// world RNG, so a rate test needs a spread of seeds).
fn vanilla_seed(n: usize, seed: u64) -> Table {
    let names = characters();
    let chars: Vec<&str> = names.iter().take(n).map(String::as_str).collect();
    let mut t = Table::with_seed(&chars, seed);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t
}

// =====================================================================
// Bot seats are offered and answered
// =====================================================================

// The C# carry-over ("out / AI / exiled players never open a window") dropped
// the AI half: a bot is a player, so the window opens and the seat answers
// through `fill_ai`.
//
// **User ruling 2026-10-08: "bots must be able to counteract"** -- the old
// standard default (propensity 0, never declare) is gone. The default is
// `DEFAULT_COUNTERACT_PROPENSITY_MILLI` (600‰ per offered card, drawn from the
// match RNG); a strategy book entry at 0 is how a card is held back. The
// window must open and be answered either way.
#[test]
fn a_standard_bot_declares_at_the_default_propensity() {
    let (mut offered, mut declared) = (0usize, 0usize);
    for seed in 1..=12 {
        let mut t = vanilla_seed(2, seed);
        t.make_bot(1, BotMentality::Standard);
        t.give(0, &["TEST:aimer"]);
        t.give(1, &["TEST:probe"]);
        t.play(0, "TEST:aimer").unwrap();
        // P0 (initial user) holds no counter after the play, so it is skipped;
        // the bot seat is up -- the window is not skipped for it.
        assert_eq!(t.asked(), vec![1], "seed {seed}: {}", t.dump_prompt());
        assert!(
            t.counteract_offered("TEST:probe"),
            "seed {seed}: {}",
            t.dump_prompt()
        );
        offered += 1;
        t.tick_until("the bot answers", |t| t.prompt().is_none());
        if !who_of(&t, "probe_fired").is_empty() {
            declared += 1;
        }
        assert_eq!(
            ev_args(&t, "aimer_done", "got"),
            vec![1],
            "seed {seed}: X settled"
        );
    }
    assert!(
        declared > 0,
        "the default propensity never declared in {offered} offers (ruling 2026-10-08: bots must be able to counteract)"
    );
    assert!(
        declared < offered,
        "the default declared every one of {offered} offers"
    );
    let rate = declared as f64 / offered as f64;
    assert!(
        (0.3..=0.9).contains(&rate),
        "default counter rate {rate:.2} ({declared}/{offered}) is nowhere near 0.6"
    );
}

// The book can hold a card back (explicit `propensity_milli: 0`) -- and the
// ring walk around a held-back bot is the old skip flow, deterministic.
#[test]
fn a_strategy_book_can_hold_a_counter_back() {
    let names = characters();
    let bot_char = names[1].clone();
    let mut t = vanilla_book(3, book_declaring(&bot_char, "TEST:probe", 0));
    t.make_bot(1, BotMentality::Standard);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // P0 (initial user) holds no counter after the play, so it is skipped;
    // the bot seat is up first -- the window is not skipped for it.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    assert!(t.counteract_offered("TEST:probe"), "{}", t.dump_prompt());
    // The engine's bot schedule answers it (the book's 0 = the skip).
    t.tick_until("the bot answers", |t| t.asked() != vec![1]);
    assert_eq!(t.asked(), vec![2], "priority advanced: {}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap();
    // Lap 1 carried P2's declaration, so the ring re-opens the earlier seats:
    // P0 holds nothing, the bot is offered once more (it still holds its probe).
    assert_eq!(t.asked(), vec![1], "second lap re-offers the bot: {}", t.dump_prompt());
    t.tick_until("the bot answers lap 2", |t| t.asked() != vec![1]);
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    // Only P2's probe settled -- the held-back bot took the skip both times.
    assert_eq!(who_of(&t, "probe_fired"), vec![2], "the bot never declared");
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "X settled after the counters"
    );
}

// Standard's `CounterParams` propensity is what declares -- an in-memory
// strategy book entry at 1000 forces a declaration every offer (the default
// 600‰ is the rate test above). `fill_ai`'s `counteract_pick` reads exactly
// this.
#[test]
fn a_standard_bot_declares_on_its_counteract_propensity() {
    let names = characters();
    let bot_char = names[1].clone();
    let mut t = vanilla_book(3, book_declaring(&bot_char, "TEST:probe", 1_000));
    t.make_bot(1, BotMentality::Standard);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.tick_until("the bot declares", |t| t.asked() != vec![1]);
    // The declaration left the hand and became a counter to X.
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![1],
        "the bot's probe answered X"
    );
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "X still settled");
}

// The bot's counter resolves in the chain alongside a human's, under the
// ruling's LIFO order: the later declaration (P2's) settles first.
#[test]
fn a_bot_counteraction_resolves_lifo_alongside_a_human() {
    let names = characters();
    let bot_char = names[1].clone();
    let mut t = vanilla_book(3, book_declaring(&bot_char, "TEST:probe", 1_000));
    t.make_bot(1, BotMentality::Standard);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    // Ring (ruling 2026-10-07): P0 skipped (no counter), then the bot, then P2.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    t.tick_until("the bot declares", |t| t.asked() != vec![1]);
    assert_eq!(t.asked(), vec![2], "{}", t.dump_prompt());
    t.counteract(2, "TEST:probe").unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    // Both answered X; LIFO settles the newest declaration (P2) first.
    assert_eq!(
        who_of(&t, "probe_fired"),
        vec![2, 1],
        "human's counter first, then the bot's: {:?}",
        ev_args(&t, "probe_fired", "n")
    );
    assert_eq!(ev_args(&t, "probe_fired", "seq"), vec![0, 0], "both on X");
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "X settled last");
}

// =====================================================================
// Held (Advanced) seats: the offer is a prompt; the deadline's fallback
// =====================================================================

// An Advanced seat is held (`ai` off) for bot-service / the browser worker.
// The [反击] offer is an ordinary prompt -- it must appear and wait on that
// seat -- and the online clock's fallback (the skip) answers it when nothing
// else does, so a window never stalls. Solo would wait forever by design.
#[test]
fn an_advanced_seat_gets_the_offer_and_the_fallback_answers_it() {
    let mut t = vanilla_casual(3);
    t.make_bot(1, BotMentality::Advanced);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    assert_eq!(t.asked(), vec![1], "the held seat gets the offer: {}", t.dump_prompt());
    assert!(t.counteract_offered("TEST:probe"), "{}", t.dump_prompt());
    // It is a live prompt, unanswered, while the deadline has not run out.
    t.m.tick(0.25);
    assert_eq!(t.asked(), vec![1], "still waiting: {}", t.dump_prompt());
    // Casual deadline: the prompt's 12 s + `REMOTE_GRACE` (1.5 s), then the
    // fallback -- the skip -- is applied.
    t.tick_until("the fallback answers", |t| t.prompt().is_none());
    // The skip: the probe stayed in hand, nothing declared, X settled.
    assert_eq!(who_of(&t, "probe_fired"), Vec::<i64>::new(), "no declaration");
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "X settled");
    let hand = &t.m.world().hidden[1].hand;
    assert!(
        hand.iter().any(|c| c == "TEST:probe"),
        "the fallback kept the card in hand: {hand:?}"
    );
}

// =====================================================================
// Rulebook exclusions
// =====================================================================

// Out / [除外] are the book's only eligibility cuts (plus `CannotPlay`).
// They are skipped per visit whatever the seat is driven by -- a bot seat
// included. "Is a bot" is not a reason; these two are.
//
// Seat layout: `TEST:aimer` names `others().first()` = seat 1, so the dead
// seat sits at 2 and the live counter at 1 -- the effect link is raised
// against a live player and the window opens normally.
#[test]
fn out_and_exiled_seats_never_get_an_offer() {
    // Exiled bot holds an eligible counter and is never asked.
    let mut t = Table::vanilla(3);
    t.make_bot(2, BotMentality::Standard);
    t.set_state(2, "exile", 1);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    let mut asked_ever = Vec::new();
    while t.prompt().is_some() {
        let a = t.asked();
        assert!(
            !a.contains(&2),
            "the exiled seat was offered: {:?} {}",
            a,
            t.dump_prompt()
        );
        asked_ever.extend(a);
        t.decline();
    }
    assert!(!asked_ever.is_empty(), "the window never opened at all");
    assert_eq!(who_of(&t, "probe_fired"), Vec::<i64>::new(), "nobody declared");
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "X named its live target");

    // Out (bankrupt) human: same cut.
    let mut t = Table::vanilla(3);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.m.world_mut().st.players[2].bankrupt = true;
    t.play(0, "TEST:aimer").unwrap();
    let mut asked_ever = Vec::new();
    while t.prompt().is_some() {
        let a = t.asked();
        assert!(
            !a.contains(&2),
            "the out seat was offered: {:?} {}",
            a,
            t.dump_prompt()
        );
        asked_ever.extend(a);
        t.decline();
    }
    assert!(!asked_ever.is_empty(), "the window never opened at all");
    assert_eq!(who_of(&t, "probe_fired"), Vec::<i64>::new(), "nobody declared");
    assert_eq!(ev_args(&t, "aimer_done", "got"), vec![1], "X named its live target");
}