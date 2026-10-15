//! The CEL-only gates' window (`docs/STATUS-HOSTS.md` / `docs/GUARDS.md`):
//! the cheapest possible CEL window for the `ActGate` / `PayGate` / `MoveGate`
//! / `ExclusiveUse` conditions.
//!
//! Status-gate conditions read only per-seat status layers via
//! `state(p, 'key')` (`state(actor,'stay')`, `state(actor,'stun') +
//! state(actor,'stunStart')`, `state(actor,'exile')`,
//! `state(actor,'unstoppable')`), so that window is `actor` plus one
//! [`PlayerSnap`] per seat -- no tile tables. Built per read (the view, a
//! rollout's `can_pay`), so it must stay O(players).
//!
//! [`ExclusiveUse`] additionally carries the questioned exclusive: the card id
//! hash in `trigger_card`, the card's `owner` persona name in `name`
//! (`counter_is('名')`), `value = 1` when that owner is the actor's base
//! persona, and `step = 1` when it is the actor's own persona. Player
//! snaps also carry `persona` / `group` hashes so `group_is` / `persona_is`
//! can name the asked seat's identity (CRYCHIC's denial).

use rules_cond::{id_of, PlayerSnap, WindowCtx};

use game_core::data::GameData;
use game_core::engine::{ExclusiveAsk, Gate, World};
use game_core::state::key;

use crate::world::CardWorld;
use crate::TriggerKind;

/// The trigger kind that asks `gate`.
pub fn kind_of(gate: Gate) -> TriggerKind {
    match gate {
        Gate::Act => TriggerKind::ActGate,
        Gate::Pay => TriggerKind::PayGate,
        Gate::Move => TriggerKind::MoveGate,
        Gate::Exclusive => TriggerKind::ExclusiveUse,
    }
}

/// One seat's status layers, however the caller reads them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Layers {
    pub out: bool,
    pub stay: i32,
    pub stun: i32,
    pub stun_start: i32,
    pub exile: i32,
    pub unstoppable: i32,
}

fn window(actor: i32, seats: impl Iterator<Item = Layers>) -> WindowCtx {
    WindowCtx {
        kind: 0,
        actor: actor as i64,
        target: -1,
        by: -1,
        players: seats
            .map(|l| {
                let mut state = std::collections::BTreeMap::new();
                if l.stay != 0 {
                    state.insert(key::STAY.to_string(), l.stay as i64);
                }
                if l.stun != 0 {
                    state.insert(key::STUN.to_string(), l.stun as i64);
                }
                if l.stun_start != 0 {
                    state.insert(key::STUN_START.to_string(), l.stun_start as i64);
                }
                if l.exile != 0 {
                    state.insert(key::EXILE.to_string(), l.exile as i64);
                }
                if l.unstoppable != 0 {
                    state.insert(key::UNSTOPPABLE.to_string(), l.unstoppable as i64);
                }
                PlayerSnap {
                    out: l.out as i64,
                    stay: l.stay as i64,
                    stun: l.stun as i64,
                    exile: l.exile as i64,
                    state,
                    ..PlayerSnap::default()
                }
            })
            .collect(),
        ..WindowCtx::default()
    }
}

/// The window over the engine's own world.
pub fn window_of_world(w: &World, actor: usize) -> WindowCtx {
    window(
        actor as i32,
        w.st.players.iter().map(|p| Layers {
            out: p.out(),
            stay: p.state_get(key::STAY),
            stun: p.state_get(key::STUN),
            stun_start: p.state_get(key::STUN_START),
            exile: p.state_get(key::EXILE),
            unstoppable: p.state_get(key::UNSTOPPABLE),
        }),
    )
}

/// The window over a card run's world (the guest-import path).
pub fn window_of_card_world<W: CardWorld>(w: &W, actor: i32) -> WindowCtx {
    window(
        actor,
        (0..w.player_count()).map(|p| Layers {
            out: w.player_out(p) != 0,
            stay: w.state_get(p, key::STAY),
            stun: w.state_get(p, key::STUN),
            stun_start: w.state_get(p, key::STUN_START),
            exile: w.state_get(p, key::EXILE),
            unstoppable: w.state_get(p, key::UNSTOPPABLE),
        }),
    )
}

/// The [`Gate::Exclusive`] window: status layers **plus** the questioned
/// exclusive and each seat's `persona` / `group` identity hashes (so
/// `group_is(actor, 'CRYCHIC')` reads the persona row's group -- the stable
/// spelling, not the currently attached group-skill id).
pub fn window_of_exclusive(
    w: &World,
    data: &GameData,
    actor: usize,
    ask: &ExclusiveAsk<'_>,
) -> WindowCtx {
    let mut win = window_of_world(w, actor);
    win.kind = TriggerKind::ExclusiveUse as i64;
    win.trigger_card = id_of(ask.card);
    win.name = ask.owner.to_string();
    win.value = ask.owner_is_base as i64;
    win.step = ask.owner_is_self as i64;
    for (i, p) in win.players.iter_mut().enumerate() {
        let ch = w
            .st
            .players
            .get(i)
            .map(|s| s.persona.clone())
            .unwrap_or_default();
        p.persona = id_of(&ch);
        p.group = id_of(&data.persona(&ch).map(|c| c.group.clone()).unwrap_or_default());
    }
    win
}
