//! `Mujica:#J11` -- C# `CardJ11` (MatchHost.cs:5876-5928): a crystal-decay card
//! you may discard to stun yourself and your 1-tile neighbours.
//!
//! Not in `docs/rulebook/cards.json`; translated from the C# class.
//! C# `CardJ11 : DecayCard` (MatchHost.cs:5876-5928): placed with 2 miracle
//! crystals; on pay-choose / targeted, may discard to 1 [stun] self and the
//! players within 1 tile (and cancel the payment when stunned).
//!

use card_sdk::abi::{HookKind, TriggerKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mujica:#J11";

pub const J11: CardDef = CardDef::new(
    "Mujica:#J11",
    &[
        On::Play("", None, play),
        // `DecayCard.TurnEnd` (the crystal tick) + `CardJ11.Targeted` -- event
        // reactions, not [反击]s. `CardJ11.PayChoose` is a value modifier and
        // rides its own FilterFlag entry (mixed kinds split, ABI v73).
        //
        // PayChoose split (ABI v74, Pattern E): the C# `p.cancel = true when
        // Me.Stunned` cancel is a non-interactive FilterFlag; the `ask_yes` +
        // discard + stun offer cannot prompt from a Filter, so it moves to
        // `HookKind::Effect` (the payment declaration -- `t.value` is applied
        // there, unlike PayAfter which runs after the money has moved). Shared
        // body `counteract` matches on `trigger::kind()`; the Effect arm is the
        // old PayChoose arm (`ask_yes` → `charge` → `set_pay_amount(0)`).
        On::Action(
            &[HookKind::TurnEnd, HookKind::Targeted, HookKind::Effect],
            "card.placed",
            None,
            counteract,
        ),
        On::FilterFlag(&[HookKind::PayChoose], "card.placed", None, pay_choose_cancel),
        On::Action(&[HookKind::CounterChanged], "", Some(crystals_changed_guard), on_crystals_changed),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name("")
        .tags(&[])
        .text(r#"将此卡置于自身场上，为其添加2个[奇迹水晶]，每回合结束时移除1个，为0时置入弃牌堆。在此期间任意需要失去资金或被其他卡指定的时刻，可将此卡置入弃牌堆并使自身及前后1格内的其他玩家获得1层眩晕。"#)
        .cell(r#"J11"#)
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

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
fn charge(player_id: i32) {
    ctx::set_dest(ctx::Dest::Graveyard);
    rules_status::stun::add(player_id, 1);
    let here = ctx::player_pos(player_id);
    for p in ctx::others(player_id) {
        // C# `H.Within(Seat, 1)` (includeSame: true): `H.Dist(seat.pos, p.pos) <= 1`.
        if ctx::dist(here, ctx::player_pos(p)) <= 1 {
            rules_status::stun::add(p, 1);
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
fn counteract(player_id: i32) -> card_sdk::Asked {
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
            ctx::decay()?;
        }
        // C# `CardJ11.PayChoose` (MatchHost.cs:5895-5901): about to pay money
        // -> may discard; the stun then cancels the payment (C# `p.cancel =
        // true` when `Me.Stunned`). Moved from PayChoose to Effect (ABI v74
        // Pattern E): the FilterFlag half cannot prompt, and Effect is the
        // last non-filter stage whose `t.value` the engine folds.
        TriggerKind::Effect => {
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
            charge(player_id);
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
                &Msg::new(key!("j11_ask_targeted")).card("card", ID),
            )? {
                return Ok(());
            }
            charge(player_id);
        }
        _ => {}
    }
    Ok(())
}

/// PayChoose cancel half (ABI v74 FilterFlag). C# `p.cancel = true when
/// Me.Stunned` -- an already-stunned owner's payment is cancelled outright.
/// Pure: no prompt (the interactive offer lives on the Effect arm of
/// [`counteract`]).
///
/// Note: the engine's `payChoose` stage currently folds only `t.value` /
/// `t.target` and ignores `set_cancelled()` (unlike the `effect` and `pay`
/// windows). The live cancel for the accept path is still the Effect arm's
/// `set_pay_amount(0)`; this flag is the non-interactive stun condition and
/// becomes live as soon as the stage honours negation. Pre-existing stun is
/// already refused earlier by `can_pay`.
fn pay_choose_cancel(player_id: i32, value: i32) -> bool {
    if trigger::player_id() != player_id || value <= 0 {
        return true;
    }
    rules_status::stun::get(player_id) <= 0
}

/// The card decays out -- C# `if (Crystals <= 0) Unplace(discard)`.
///
/// TODO(规则书): `Mujica:#J11` has no rulebook entry at all, so this follows the
/// C# alone. The book needs a clause before this is more than a port of it.
///
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all. Name-filtered to `counter::CRYSTALS`.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == card_sdk::abi::counter::CRYSTALS
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

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
