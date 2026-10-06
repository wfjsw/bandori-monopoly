#![cfg_attr(target_arch = "wasm32", no_std)]

//! Test-only cards for the host's cross-module tests.

use card_sdk::abi::{ChainKind, GateKind, HookKind};
use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::{key, CardDef, Msg, On};

/// Plays another card (in another module) in the middle of its own effect.
const RELAY: CardDef = CardDef::new("TEST:relay", &[On::Play(None, relay)]);
/// Plays itself forever; the host must stop it at the depth limit.
const RECURSE: CardDef = CardDef::new("TEST:recurse", &[On::Play(None, recurse)]);

/// Logs from both `play` and `react`, with no kind guard: the host must run its
/// `react` exactly once per play (at its own `card` trigger), not again at
/// `cardAfter` / `cardPlayed`.
const ECHO: CardDef = CardDef::new(
    "TEST:echo",
    &[
        On::Play(None, echo_play),
        On::CounterAct(&[ChainKind::Card], never, echo_react),
    ],
);
/// Lists its player's hand through `cards_in` (the host->guest list) and logs
/// the count next to `hand_size`, so a test can check the two agree.
const LISTER: CardDef = CardDef::new("TEST:lister", &[On::Play(None, lister)]);
/// Stuns the first other player through the [abnormal] gate, then logs how many
/// abnormal effects reached that player this turn.
const STUNNER: CardDef = CardDef::new("TEST:stunner", &[On::Play(None, stunner)]);
/// Placed on the first other player's field; guards that player against every
/// abnormal effect (C# `IAbnormalGuard`).
const GUARD: CardDef = CardDef::new(
    "TEST:guard",
    &[
        On::Play(None, guard_play),
        On::Gate(&[GateKind::AbnormalGuard], guard),
    ],
);

/// Targets the first other player (C# `H.Target`) and logs what it got and that
/// player's `_targeted` counter.
const AIMER: CardDef = CardDef::new("TEST:aimer", &[On::Play(None, aimer)]);
/// Placed on the first other player's field; makes that player immune to other
/// players' effects (C# `ImmuneAll`). A **resolution** gate: the effect names
/// the player and the chain forms, and only what lands is voided.
const SHIELD: CardDef = CardDef::new(
    "TEST:shield",
    &[
        On::Play(None, shield_play),
        On::Gate(&[GateKind::ImmuneAll], shield),
    ],
);

/// A [反击] that answers an **effect declaration** and negates its activation --
/// the link never happened, so nothing settles. This is the Yu-Gi-Oh "negate
/// the activation" as against "negate the effect".
const COUNTER: CardDef = CardDef::new(
    "TEST:counter",
    &[On::CounterAct(&[ChainKind::Effect], counter_yes, counter)],
);

fn aimer(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    let got = ctx::target(target).unwrap_or(-1);
    ctx::log(
        player_id,
        &Msg::new(key!("aimer_done"))
            .i("got", got as i64)
            .i("count", ctx::targeted_count(target) as i64),
    );
    Ok(())
}

fn shield_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:shield", &Msg::new(key!("shield_note")));
    Ok(())
}

fn shield(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("shield_held")));
    }
    Ok(())
}

/// Shapes a 3-step walk via the plan ops and runs it immediately with
/// `card_move` (C# `H.CardMove`): the run pauses, the engine walks, the effect
/// resumes. Logs where the player ended up.
const MOVER: CardDef = CardDef::new("TEST:mover", &[On::Play(None, mover)]);

/// Places itself with one [奇迹水晶] and spends it inside the same effect -- the
/// AG:绯红之魂 (3) shape: 「不再拥有[奇迹水晶]时」 is a `crystalsChanged` handler,
/// so the write that empties the card is what leaves the field. The spend site
/// does not re-check the count.
const CRYSTAL: CardDef = CardDef::new(
    "TEST:crystal",
    &[
        On::Play(None, crystal_play),
        On::Hook(
            &[HookKind::CrystalsChanged],
            crystal_changed_guard,
            crystal_changed,
        ),
    ],
);

fn mover(player_id: i32) -> card_sdk::Asked {
    ctx::plan::set_steps(3);
    ctx::log(player_id, &Msg::new(key!("mover_planned")));
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

fn crystal_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:crystal", &Msg::new(key!("crystal_note")));
    ctx::set_crystals(1);
    // The decay tick: a write that takes the count to zero. `decay` no longer
    // discards at 0 -- only the handler below is the 「no crystals -> discard」
    // check.
    ctx::decay();
    Ok(())
}

fn crystal_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is("TEST:crystal")
        && ctx::crystals() == 0
        // Only the write that did not raise the count speaks for the empty
        // state -- an earlier write in the same run must not speak for the
        // count the run ended on.
        && trigger::value() <= 0
}

fn crystal_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    // Which write emptied it: a removal, or a write that landed on none.
    let why = if trigger::value() < 0 {
        key!("crystal_empty")
    } else {
        key!("crystal_none")
    };
    ctx::log(player_id, &Msg::new(why));
    Ok(())
}

/// Places itself and leaves **immediately** -- `send_to_dest` rather than
/// `set_dest` -- so the rest of the effect runs against a card that is already
/// gone. Logs `is_placed` right after, which is the whole distinction.
const DEST_NOW: CardDef = CardDef::new("TEST:dest_now", &[On::Play(None, dest_now)]);

fn dest_now(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:dest_now", &Msg::new(key!("dest_note")));
    ctx::send_to_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("dest_now_left")).i("placed", ctx::is_placed() as i64),
    );
    Ok(())
}

/// Places itself and names **another** player's discard -- 「将此卡放入[使用者]
/// 弃卡区」 when [使用者] is not the one holding it.
const DEST_TO: CardDef = CardDef::new("TEST:dest_to", &[On::Play(None, dest_to)]);

fn dest_to(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:dest_to", &Msg::new(key!("dest_note")));
    if let Some(&user) = ctx::others(player_id).first() {
        ctx::set_transfer_to_dest(user, ctx::Dest::Graveyard);
    }
    Ok(())
}

fn never(_player: i32) -> bool {
    false
}

fn echo_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("echo_play")));
    Ok(())
}

fn echo_react(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("echo_react")));
    Ok(())
}

fn lister(player_id: i32) -> card_sdk::Asked {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    ctx::log(
        player_id,
        &Msg::new(key!("lister_count"))
            .i("n", hand.len() as i64)
            .i("size", ctx::hand_size(player_id) as i64),
    );
    // Round-trip a real id: take the first card out of the hand and put it on
    // the discard pile, then confirm the discard listing shows it.
    if let Some(first) = hand.first() {
        if ctx::take_card(player_id, CardPile::Hand, first) {
            ctx::to_discard(player_id, first);
            let found = ctx::cards_in(player_id, CardPile::Discard)
                .iter()
                .any(|c| c == first);
            ctx::log(
                player_id,
                &Msg::new(key!("lister_moved")).i("found", found as i64),
            );
        }
    }
    Ok(())
}

fn stunner(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::give_stun(target, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("stunner_done")).i("count", ctx::abnormal_count(target) as i64),
    );
    Ok(())
}

fn guard_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:guard", &Msg::new(key!("guard_note")));
    Ok(())
}

fn guard(player_id: i32) -> card_sdk::Asked {
    if trigger::target() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("guard_blocked")));
    }
    Ok(())
}

fn relay(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("relay_start")));
    ctx::play_card("HHW:（育美）", player_id)?;
    ctx::log(player_id, &Msg::new(key!("relay_end")));
    Ok(())
}

fn recurse(player_id: i32) -> card_sdk::Asked {
    ctx::play_card("TEST:recurse", player_id)?;
    Ok(())
}

fn counter_yes(player_id: i32) -> bool {
    // Whole-list view: any effect another player's card declared at me.
    trigger::by_card().is_some_and(|by| by != player_id) && ctx::effect::hits(player_id)
}

fn counter(player_id: i32) -> card_sdk::Asked {
    // Negate the **activation**: the declaration never happened, so the
    // targeting never lands. (`negate_effect` would let it be seen as an effect
    // and settle to nothing -- the two are deliberately distinct.)
    trigger::set_cancelled();
    ctx::log(player_id, &Msg::new(key!("counter_fired")));
    Ok(())
}

card_sdk::bandori_ruleset!(&[
    RELAY, RECURSE, ECHO, LISTER, STUNNER, GUARD, AIMER, SHIELD, MOVER, COUNTER, CRYSTAL, DEST_NOW,
    DEST_TO
]);
