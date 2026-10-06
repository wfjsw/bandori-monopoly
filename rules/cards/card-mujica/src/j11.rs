//! `Mujica:#J11` -- C# `CardJ11` (MatchHost.cs:5876-5928): a crystal-decay card
//! you may discard to stun yourself and your 1-tile neighbours.
//!
//! Not in `docs/rulebook/cards.json`; translated from the C# class.
//! C# `CardJ11 : DecayCard` (MatchHost.cs:5876-5928): placed with 2 miracle
//! crystals; on pay-choose / targeted, may discard to 1 [stun] self and the
//! players within 1 tile (and cancel the payment when stunned).
//!

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mujica:#J11";

pub const J11: CardDef = CardDef::new(
    "Mujica:#J11",
    &[
        On::Play(None, play),
        // `DecayCard.TurnEnd` (the crystal tick) + `CardJ11.PayChoose` +
        // `CardJ11.Targeted` -- all field hooks, not [反击]s.
        On::Hook(
            &[HookKind::TurnEnd, HookKind::PayChoose, HookKind::Targeted],
            |_| true,
            react,
        ),
        On::Hook(
            &[HookKind::CrystalsChanged],
            crystals_changed_guard,
            on_crystals_changed,
        ),
    ],
);

fn play(player_id: i32) -> card_sdk::Asked {
    // C# `H.PlaceFromPlay(c, -1, -1, 2)` -- place with 2 miracle crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("j11_note")));
    ctx::set_crystals(2);
    ctx::log(
        player_id,
        &Msg::new(key!("j11_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardJ11.Fire` / `CardJ11.Ask`'s body -- discard the card to [stun] its
/// owner and every player within 1 tile.
fn fire(player_id: i32) {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::give_stun(player_id, 1);
    let here = ctx::player_pos(player_id);
    for p in ctx::others(player_id) {
        // C# `H.Within(Seat, 1)` (includeSame: true): `H.Dist(seat.pos, p.pos) <= 1`.
        if ctx::dist(here, ctx::player_pos(p)) <= 1 {
            ctx::give_stun(p, 1);
        }
    }
    ctx::log(
        player_id,
        &Msg::new(key!("j11_fired")).player_id("who", player_id),
    );
}

/// Field hooks: `DecayCard.TurnEnd` (the crystal tick), `CardJ11.PayChoose`,
/// and `CardJ11.Targeted`. Runs through the Fx hook dispatch, so these are
/// field effects, not [反击]s.
fn react(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    match trigger::kind() {
        // `DecayCard.TurnEnd` (C# MatchHost.cs:1874-1897) -- burn one miracle
        // crystal at the card's own player's turn end (`turn != DecayOn` -> skip,
        // `DecayOn => Player`), and discard the card when it runs out.
        TriggerKind::TurnEnd => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            // C# `AddCrystals(-1, ...)`; the `if (Crystals <= 0) Unplace(discard)`
            // half is [`on_crystals_changed`] (`Empty` is a no-op for `CardJ11`).
            ctx::decay();
        }
        // C# `CardJ11.PayChoose` (MatchHost.cs:5895-5901): about to pay money
        // -> may discard; the stun then cancels the payment (C# `p.cancel =
        // true` when `Me.Stunned`).
        TriggerKind::PayChoose => {
            if trigger::player_id() != player_id || trigger::value() <= 0 {
                return Ok(());
            }
            if !ctx::ask_yes(
                player_id,
                &Msg::new(key!("j11_title")),
                &Msg::new(key!("j11_ask_pay")).n("money", trigger::value() as i64),
            )? {
                return Ok(());
            }
            fire(player_id);
            trigger::set_pay_amount(0);
        }
        // C# `CardJ11.Targeted` (MatchHost.cs:5903-5910): another player's card
        // targets the owner -> may discard. `t.Target != Seat || t.ByCard ==
        // Player || !placed` -> skip.
        TriggerKind::Targeted => {
            if trigger::target() != player_id
                || trigger::by_card().is_some_and(|by| by == player_id)
            {
                return Ok(());
            }
            if !ctx::ask_yes(
                player_id,
                &Msg::new(key!("j11_title")),
                &Msg::new(key!("j11_ask_targeted")),
            )? {
                return Ok(());
            }
            fire(player_id);
        }
        _ => {}
    }
    Ok(())
}

/// The card decays out -- C# `if (Crystals <= 0) Unplace(discard)`.
///
/// TODO(规则书): `Mujica:#J11` has no rulebook entry at all, so this follows the
/// C# alone. The book needs a clause before this is more than a port of it.
///
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("j11_decayed")).player_id("who", player_id),
    );
    Ok(())
}
