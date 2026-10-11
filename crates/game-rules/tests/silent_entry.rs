//! Silent-entry presentation (`CardDef::silent`): a bookkeeping entry runs its
//! body but posts no activation flash and no 「效果适用」 header.
//!
//! * a silent entry produces no `"card"` event and no header;
//! * a non-silent sibling entry on the same card still flashes;
//! * a nested activation from a silent body still announces normally;
//! * the debug guard (silent + visible effect) fires in a test fixture.

mod common;

use common::*;
use game_core::state::MatchEvent;
use game_rules::take_silent_visible;

/// The `"card"` events naming `card` since `mark`, oldest first.
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

/// Place `card` on player 0's field and land so its settle hooks fire.
fn place_and_land(t: &mut Table, card: &str) -> i32 {
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, card).unwrap();
    drain(t);
    assert!(t.on_field(0, card), "the play places the card: {}", t.dump_prompt());
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(t);
    mark
}

/// A silent bookkeeping hook posts nothing; its non-silent sibling on the same
/// settle still flashes exactly once.
#[test]
fn silent_entry_produces_no_card_event_and_the_sibling_still_flashes() {
    let mut t = Table::vanilla(2);
    let mark = place_and_land(&mut t, "TEST:silentPair");
    let fired = activations_since(&t, mark, "TEST:silentPair");
    assert_eq!(
        fired.len(),
        1,
        "only the flashing sibling announces: {}",
        t.recent_keys(30).join(", ")
    );
    assert_eq!(
        fired[0].msg.key(),
        "log.effect_applied",
        "the sibling's header is a real 「效果适用」"
    );
    // The silent entry's bookkeeping write still happened (body ran).
    // It leaves no stream trail; the sibling's body line is under the header.
    let all: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.parent == fired[0].id)
        .collect();
    let flash_lines: Vec<_> = all
        .iter()
        .filter(|e| e.msg.key().contains("silent_pair_flash"))
        .collect();
    assert_eq!(
        flash_lines.len(),
        1,
        "the sibling's body line is under its header: {all:?}"
    );
}

/// A silent body that triggers another card's hook through the engine: the
/// triggered hook still announces normally (the flag hides only the silent
/// entry's own flash).
///
/// `TEST:silentPay`'s silent settle body gains money; that runs the money
/// pipeline's `payTotalAdd` stage, which fires `TEST:totalCut`'s hook.
#[test]
fn nested_activation_from_a_silent_body_still_announces() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.place_raw(0, "TEST:totalCut");
    t.give_play(0, "TEST:silentPay").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    // The silent outer body posts nothing.
    assert!(
        activations_since(&t, mark, "TEST:silentPay").is_empty(),
        "the silent outer body must not flash: {}",
        t.recent_keys(30).join(", ")
    );
    // The hook the gain triggered still announces.
    let nested = activations_since(&t, mark, "TEST:totalCut");
    assert_eq!(
        nested.len(),
        1,
        "the triggered hook still flashes: {}",
        t.recent_keys(30).join(", ")
    );
    assert_eq!(nested[0].kind, "hook", "a field hook is a hook activation");
}

/// A silent body that pays: no flash, but the debug guard records the
/// violation (`CardDef::silent` must not hide a real effect).
#[test]
fn silent_plus_visible_effect_fires_the_debug_guard() {
    let _ = take_silent_visible();
    let mut t = Table::vanilla(2);
    let mark = place_and_land(&mut t, "TEST:silentPay");
    assert!(
        activations_since(&t, mark, "TEST:silentPay").is_empty(),
        "the silent body must not flash even when it pays: {}",
        t.recent_keys(30).join(", ")
    );
    // The money still moved -- the flag hides the flash, not the effect.
    assert!(
        t.events_since(mark)
            .iter()
            .any(|e| e.r#type == "gain" || e.msg.key().contains("gain") || e.msg.key().contains("pay")),
        "the pay is still in the stream: {}",
        t.recent_keys(30).join(", ")
    );
    let violations = take_silent_visible();
    assert!(
        violations.iter().any(|(card, what)| card == "TEST:silentPay" && what.contains("pay")),
        "the guard must record the violation: {violations:?}"
    );
}

/// The manifest flag rides both build paths: the fixture's entry 1 is silent,
/// entry 2 is not.
#[test]
fn manifest_carries_the_silent_flag() {
    let r = rules();
    let idx = r.ruleset().card("TEST:silentPair").expect("silentPair loaded");
    let info = &r.ruleset().cards()[idx as usize];
    assert!(!info.on[0].silent, "the Play entry announces");
    assert!(info.on[1].silent, "the bookkeeping hook is silent");
    assert!(!info.on[2].silent, "the sibling hook announces");
}

// Shipped-card silent-entry checks live in `rules/tests/tests/silent_shipped.rs`
// (the game suite). This file is engine-universal (TEST:* fixtures only).
