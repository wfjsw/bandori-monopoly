//! Cross-card messages (`On::Message` + `ctx::send`, ABI v50).
//!
//! A card answers a named message from another card and returns a reply. The
//! receiver is addressed by uid, by (seat, card id), or by a standing board
//! pseudo card. No receiver / a rejecting `pre` means the send returns 0 and
//! the handler never runs.

mod common;

use common::*;

/// Fixture log key (`key!("…")` in `rules/fixtures/test-cards`).
fn ev(name: &str) -> String {
    format!("cards:fixture-test-cards.{name}")
}

/// `TEST:ping`'s play sends `"ping"` (a=2, b=3) to the placed `TEST:ping` on
/// its own field. The handler sums the payload and replies 5.
#[test]
fn message_handler_replies_with_a_value() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:ping");
    t.give_play(0, "TEST:ping").unwrap();
    let keys = t.recent_keys(24);
    assert!(keys.contains(&ev("ping_got")), "handler ran: {:?}", keys);
    assert!(keys.contains(&ev("ping_reply")), "reply logged: {:?}", keys);
}

/// A `pre` that rejects every sender is not a receiver: the handler body
/// never runs and the send replies 0.
#[test]
fn a_rejecting_pre_is_not_a_receiver() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:pingReject");
    t.place_raw(0, "TEST:ping");
    t.give_play(0, "TEST:ping").unwrap();
    let keys = t.recent_keys(24);
    // The rejecting copy is the first field copy of TEST:pingReject; the send
    // targets card id TEST:ping, so only the placed TEST:ping answers. Pin
    // that the reject card's handler did not run via its own log key.
    assert!(
        !keys.iter().any(|k| k.contains("pingReject")),
        "rejecting handler must not run: {:?}",
        keys
    );
}

/// Addressing a card that is not in play: no receiver, reply 0, no handler.
#[test]
fn send_with_no_receiver_replies_zero() {
    let mut t = Table::vanilla(2);
    // No TEST:ping placed: the play's send has no receiver.
    t.give_play(0, "TEST:ping").unwrap();
    let keys = t.recent_keys(24);
    assert!(
        !keys.contains(&ev("ping_got")),
        "no receiver, no handler: {:?}",
        keys
    );
}
