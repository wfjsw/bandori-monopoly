//! The status gates' window (`docs/STATUS-HOSTS.md`): the cheapest possible
//! CEL window for the CEL-only `ActGate` / `PayGate` / `MoveGate` conditions.
//!
//! Those conditions read only per-seat status layers (`stay(actor)`,
//! `stunned(actor)`, `exile(actor)`, `unstoppable(actor)`), so the window is
//! `actor` plus one [`PlayerSnap`] per seat -- no tile tables. Built per read
//! (the view, a rollout's `can_pay`), so it must stay O(players).

use rules_cond::{PlayerSnap, WindowCtx};

use game_core::engine::{Gate, World};
use game_core::state::key;

use crate::world::CardWorld;
use crate::TriggerKind;

/// The trigger kind that asks `gate`.
pub fn kind_of(gate: Gate) -> TriggerKind {
    match gate {
        Gate::Act => TriggerKind::ActGate,
        Gate::Pay => TriggerKind::PayGate,
        Gate::Move => TriggerKind::MoveGate,
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
            .map(|l| PlayerSnap {
                out: l.out as i64,
                stay: l.stay as i64,
                stun: l.stun as i64,
                stun_start: l.stun_start as i64,
                exile: l.exile as i64,
                unstoppable: l.unstoppable as i64,
                ..PlayerSnap::default()
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
