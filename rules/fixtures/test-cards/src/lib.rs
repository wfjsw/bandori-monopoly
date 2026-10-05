#![cfg_attr(target_arch = "wasm32", no_std)]

//! Test-only cards for the host's cross-module tests.

use card_sdk::{ctx, key, CardDef, Msg};

/// Plays another card (in another module) in the middle of its own effect.
const RELAY: CardDef = CardDef { id: "TEST:relay", play: Some(relay), can_react: None, react: None, why_not: None };
/// Plays itself forever; the host must stop it at the depth limit.
const RECURSE: CardDef = CardDef { id: "TEST:recurse", play: Some(recurse), can_react: None, react: None, why_not: None };

fn relay(seat: i32) {
    ctx::log(seat, &Msg::new(key!("relay_start")));
    ctx::play_card("HHW:（育美）", seat);
    ctx::log(seat, &Msg::new(key!("relay_end")));
}

fn recurse(seat: i32) {
    ctx::play_card("TEST:recurse", seat);
}

card_sdk::bandori_ruleset!(&[RELAY, RECURSE]);
