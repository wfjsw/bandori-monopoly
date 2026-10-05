#![cfg_attr(target_arch = "wasm32", no_std)]

//! Test-only cards for the host's cross-module tests.

use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::abi::TriggerKind;
use card_sdk::{key, CardDef, Msg, On};

/// Plays another card (in another module) in the middle of its own effect.
const RELAY: CardDef = CardDef::new("TEST:relay", &[On::Play(relay)]);
/// Plays itself forever; the host must stop it at the depth limit.
const RECURSE: CardDef = CardDef::new("TEST:recurse", &[On::Play(recurse)]);

/// Logs from both `play` and `react`, with no kind guard: the host must run its
/// `react` exactly once per play (at its own `card` trigger), not again at
/// `cardAfter` / `cardPlayed`.
const ECHO: CardDef = CardDef::new("TEST:echo", &[On::Play(echo_play), On::React(&[TriggerKind::Card], never, echo_react)]);
/// Lists its player's hand through `cards_in` (the host->guest list) and logs
/// the count next to `hand_size`, so a test can check the two agree.
const LISTER: CardDef = CardDef::new("TEST:lister", &[On::Play(lister)]);
/// Stuns the first other player through the [abnormal] gate, then logs how many
/// abnormal effects reached that player this turn.
const STUNNER: CardDef = CardDef::new("TEST:stunner", &[On::Play(stunner)]);
/// Placed on the first other player's field; guards that player against every
/// abnormal effect (C# `IAbnormalGuard`).
const GUARD: CardDef = CardDef::new("TEST:guard", &[On::Play(guard_play), On::Hook(&[TriggerKind::AbnormalGuard], guard)]);

/// Targets the first other player (C# `H.Target`) and logs what it got and that
/// player's `_targeted` counter.
const AIMER: CardDef = CardDef::new("TEST:aimer", &[On::Play(aimer)]);
/// Placed on the first other player's field; makes that player immune to other
/// players' effects (C# `ImmuneAll`).
const SHIELD: CardDef = CardDef::new("TEST:shield", &[On::Play(shield_play), On::Hook(&[TriggerKind::ImmuneAll], shield)]);

fn aimer(player_id: i32) {
    let Some(&target) = ctx::others(player_id).first() else { return };
    let got = ctx::target(target).unwrap_or(-1);
    ctx::log(player_id, &Msg::new(key!("aimer_done")).i("got", got as i64).i("count", ctx::targeted_count(target) as i64));
}

fn shield_play(player_id: i32) {
    let Some(&target) = ctx::others(player_id).first() else { return };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:shield", &Msg::new(key!("shield_note")));
}

fn shield(player_id: i32) {
    if trigger::player_id() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("shield_held")));
    }
}

/// Shapes a 3-step walk via the plan ops and runs it immediately with
/// `card_move` (C# `H.CardMove`): the run pauses, the engine walks, the effect
/// resumes. Logs where the player ended up.
const MOVER: CardDef = CardDef::new("TEST:mover", &[On::Play(mover)]);

fn mover(player_id: i32) {
    ctx::plan::set_steps(3);
    ctx::log(player_id, &Msg::new(key!("mover_planned")));
    ctx::card_move(player_id);
    ctx::log(player_id, &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64));
}

fn never(_player: i32) -> bool {
    false
}

fn echo_play(player_id: i32) {
    ctx::log(player_id, &Msg::new(key!("echo_play")));
}

fn echo_react(player_id: i32) {
    ctx::log(player_id, &Msg::new(key!("echo_react")));
}

fn lister(player_id: i32) {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    ctx::log(player_id, &Msg::new(key!("lister_count")).i("n", hand.len() as i64).i("size", ctx::hand_size(player_id) as i64));
    // Round-trip a real id: take the first card out of the hand and put it on
    // the discard pile, then confirm the discard listing shows it.
    if let Some(first) = hand.first() {
        if ctx::take_card(player_id, CardPile::Hand, first) {
            ctx::to_discard(player_id, first);
            let found = ctx::cards_in(player_id, CardPile::Discard).iter().any(|c| c == first);
            ctx::log(player_id, &Msg::new(key!("lister_moved")).i("found", found as i64));
        }
    }
}

fn stunner(player_id: i32) {
    let Some(&target) = ctx::others(player_id).first() else { return };
    ctx::give_stun(target, 1);
    ctx::log(player_id, &Msg::new(key!("stunner_done")).i("count", ctx::abnormal_count(target) as i64));
}

fn guard_play(player_id: i32) {
    let Some(&target) = ctx::others(player_id).first() else { return };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:guard", &Msg::new(key!("guard_note")));
}

fn guard(player_id: i32) {
    if trigger::target() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("guard_blocked")));
    }
}

fn relay(player_id: i32) {
    ctx::log(player_id, &Msg::new(key!("relay_start")));
    ctx::play_card("HHW:（育美）", player_id);
    ctx::log(player_id, &Msg::new(key!("relay_end")));
}

fn recurse(player_id: i32) {
    ctx::play_card("TEST:recurse", player_id);
}

card_sdk::bandori_ruleset!(&[RELAY, RECURSE, ECHO, LISTER, STUNNER, GUARD, AIMER, SHIELD, MOVER]);
