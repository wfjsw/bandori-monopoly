//! A card's effect activation is one `"card"` match event -- the client's card
//! flash -- and its outcome is logged as the card's own: 「<卡名> 的效果：
//! <what happened>」. Exactly one per real body run --
//!
//! * a field-card hook on a tile fires once when its guard admits the landing,
//!   and stays completely silent when the guard rejects;
//! * a counteraction whose own link an answer negated still shows the card,
//!   marked `negated` (its body does not run), logging 「<卡名> 的效果被无效」;
//! * a play a [反击] negated likewise shows, marked negated.
//!
//! Guard rejects and pre-scan skips run no body and so emit nothing. A fresh
//! activation (hand play, [反击], skill press) keeps its own 「打出」 line; an
//! already-in-play card's effect never says 「发动」.

mod common;

use common::*;
use game_core::msg::Arg;
use game_core::state::MatchEvent;

/// The `"card"` events naming `card` since `mark`, oldest first.
fn activations_since(t: &Table, mark: i32, card: &str) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == card)
        .collect()
}

/// The `log.card_effect` lines naming `card` since `mark` -- the
/// 「<卡名> 的效果：<what happened>」 attribution of an in-play card's effect.
fn effects_since(t: &Table, mark: i32, card: &str) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| {
            e.msg.key() == "log.card_effect"
                && e.msg.a.get("card") == Some(&Arg::Card(card.to_string()))
        })
        .collect()
}

/// Skip every open prompt (counteraction windows included).
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// `TEST:tileHook` is placed on its player's field; its `settle` hook admits
/// only that player's own land. The holder landing is one activation; another
/// player landing is a guard reject and must not mention the card at all.
#[test]
fn field_card_hook_on_a_tile_activates_once_and_stays_silent_when_the_guard_rejects() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, "TEST:tileHook").unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, "TEST:tileHook"),
        "the play places the card: {}",
        t.dump_prompt()
    );

    // The holder lands somewhere: the settle hook's guard admits.
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let fired = activations_since(&t, mark, "TEST:tileHook");
    assert_eq!(
        fired.len(),
        1,
        "exactly one activation when the hook fires: {}",
        t.recent_keys(20).join(", ")
    );
    assert_eq!(fired[0].kind, "hook", "a settle hook is a hook activation");
    assert!(!fired[0].negated, "a hook that ran is not negated");
    assert_eq!(fired[0].player_id, 0, "the card's owner bars the flash");
    assert_eq!(fired[0].value, 3, "the tile it fired on rides the event");
    // An already-in-play card's effect never says 「发动」: its flash carries no
    // line of its own -- the outcome is logged as 「<卡名> 的效果：<what>」.
    assert_eq!(fired[0].msg.key(), "", "the flash adds no generic line");
    let effects = effects_since(&t, mark, "TEST:tileHook");
    assert_eq!(
        effects.len(),
        1,
        "the outcome names the card as its source: {}",
        t.recent_keys(20).join(", ")
    );

    // Another player lands: `trigger::player_id() != holder` rejects, so the
    // body never runs -- and nothing about the card reaches the UI.
    let mark = t.mark();
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, 0);
    t.dice(&[4]);
    t.roll(1).unwrap();
    drain(&mut t);
    let rejected = activations_since(&t, mark, "TEST:tileHook");
    assert!(
        rejected.is_empty(),
        "a guard reject must not flash: {:?}",
        rejected
    );
    assert!(
        effects_since(&t, mark, "TEST:tileHook").is_empty(),
        "a guard reject logs nothing about the card: {}",
        t.recent_keys(20).join(", ")
    );
}

/// `TEST:probe` answers an effect; `TEST:deny` answers that counter's own play
/// and negates it. The probe's activation is the negated one -- the card still
/// shows, marked 无效, while its body does not run.
#[test]
fn negated_counteraction_shows_negated() {
    let mut t = Table::vanilla(3);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.set_hand(2, &[]);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:deny"]);
    let mark = t.mark();
    t.play(0, "TEST:aimer").unwrap();
    // P0 is the initial user and holds no counter -> skipped; P1's probe is up.
    assert!(t.counteract_offered("TEST:probe"), "{}", t.dump_prompt());
    t.counteract(1, "TEST:probe").unwrap();
    // The round on X has to close (its quiet lap) before the probe's own round
    // opens: pass on everything until the deny window is the one offered.
    for _ in 0..12 {
        if t.prompt().is_none() || t.counteract_offered("TEST:deny") {
            break;
        }
        t.decline();
    }
    assert!(
        t.counteract_offered("TEST:deny"),
        "the deny answers the probe's play: {}",
        t.dump_prompt()
    );
    t.counteract(2, "TEST:deny").unwrap();
    drain(&mut t);

    let probe = activations_since(&t, mark, "TEST:probe");
    assert_eq!(
        probe.len(),
        1,
        "one activation line for the negated counter: {}",
        t.recent_keys(30).join(", ")
    );
    assert!(probe[0].negated, "a negated activation is marked so");
    assert_eq!(probe[0].kind, "counter");
    assert_eq!(probe[0].player_id, 1, "the counter's declarer owns it");
    assert_eq!(probe[0].msg.key(), "log.card_negated");

    // The deny itself ran: an activation, not negated.
    let deny = activations_since(&t, mark, "TEST:deny");
    assert_eq!(deny.len(), 1, "the deny activates: {}", t.recent_keys(30).join(", "));
    assert!(!deny[0].negated);
    assert_eq!(deny[0].kind, "counter");

    // The root play was not negated -- it activated normally.
    let aimer = activations_since(&t, mark, "TEST:aimer");
    assert_eq!(aimer.len(), 1, "the play activates: {}", t.recent_keys(30).join(", "));
    assert!(!aimer[0].negated);
    assert_eq!(aimer[0].kind, "play");
}

/// A [反击] that negates a play (`TEST:denyPlay` answers the root `card` link):
/// the played card still flashes, marked negated, while its body does not run.
#[test]
fn negated_play_shows_negated() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:denyPlay"]);
    let mark = t.mark();
    t.play(0, "TEST:aimer").unwrap();
    assert!(
        t.counteract_offered("TEST:denyPlay"),
        "the denyPlay answers the root play: {}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:denyPlay").unwrap();
    drain(&mut t);

    let aimer = activations_since(&t, mark, "TEST:aimer");
    assert_eq!(
        aimer.len(),
        1,
        "one activation line for the negated play: {}",
        t.recent_keys(30).join(", ")
    );
    assert!(aimer[0].negated, "the play is marked negated");
    assert_eq!(aimer[0].kind, "play");
    assert_eq!(aimer[0].player_id, 0);
    assert_eq!(aimer[0].msg.key(), "log.card_negated");

    // The counter's body ran -- it is the one that negated.
    let deny = activations_since(&t, mark, "TEST:denyPlay");
    assert_eq!(deny.len(), 1, "{}", t.recent_keys(30).join(", "));
    assert!(!deny[0].negated);
    assert_eq!(deny[0].kind, "counter");
}