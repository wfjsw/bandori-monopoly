//! The CEL-only gates' window (`docs/STATUS-HOSTS.md` / `docs/GUARDS.md`):
//! the cheapest possible CEL window for the `ActGate` / `PayGate` / `MoveGate`
//! / `OnBoardGate` / `TargetableGate` / `ExclusiveUse` / `PlayGate` conditions.
//!
//! Gate conditions read per-seat keyed state via `state(p, 'key')`, so that
//! window is `actor` plus one [`PlayerSnap`] per seat -- no tile tables. Built
//! per read (the view, a rollout's `can_pay`), so it must remain O(players).
//! The whole state map is copied in; the CEL names live in `rules/cond`.
//!
//! [`ExclusiveUse`] additionally carries the questioned exclusive: the card id
//! hash in `trigger_card`, the card's owner name in `name`
//! (`counter_is('名')`), `value = 1` when that owner is the actor's base
//! identity, and `step = 1` when it is the actor's own identity. Player
//! snaps also carry the identity-hash `state` rows so sugar guards can name
//! the asked seat's identity.

use rules_cond::{id_of, PlayerSnap, WindowCtx};

use game_core::data::GameData;
use game_core::engine::{ExclusiveAsk, Gate, World};

use crate::world::CardWorld;
use crate::TriggerKind;

/// The trigger kind that asks `gate`.
pub fn kind_of(gate: Gate) -> TriggerKind {
    match gate {
        Gate::Act => TriggerKind::ActGate,
        Gate::Pay => TriggerKind::PayGate,
        Gate::Move => TriggerKind::MoveGate,
        Gate::OnBoard => TriggerKind::OnBoardGate,
        Gate::Targetable => TriggerKind::TargetableGate,
        Gate::Exclusive => TriggerKind::ExclusiveUse,
        Gate::Play => TriggerKind::PlayGate,
    }
}

/// One seat's snapshot for a gate window.
#[derive(Clone, Debug, Default)]
pub struct Layers {
    pub out: bool,
    /// Whole keyed state map (opaque names).
    pub state: std::collections::BTreeMap<String, i64>,
    /// Hand size (for a standing play lock like CRYCHIC's 「大于等于6」).
    pub hand: i64,
}

fn window(actor: i32, seats: impl Iterator<Item = Layers>) -> WindowCtx {
    WindowCtx {
        kind: String::new(),
        actor: actor as i64,
        target: -1,
        by: -1,
        players: seats
            .map(|l| {
                let mut p = PlayerSnap {
                    out: l.out as i64,
                    hand: l.hand,
                    state: l.state,
                    ..PlayerSnap::default()
                };
                rules_cond::fill_named_layers(&mut p);
                p
            })
            .collect(),
        ..WindowCtx::default()
    }
}

/// The window over the engine's own world.
pub fn window_of_world(w: &World, actor: usize) -> WindowCtx {
    window(
        actor as i32,
        w.st.players.iter().enumerate().map(|(i, p)| Layers {
            out: p.out(),
            state: p
                .state
                .iter()
                .map(|(k, v)| (k.clone(), i64::from(v.value)))
                .collect(),
            // Real hand pile, not the (often stale) `MatchPlayer::hand` size.
            hand: w.hidden.get(i).map(|h| h.hand.len() as i64).unwrap_or(0),
        }),
    )
}

/// The window over a card run's world (the guest-import path).
pub fn window_of_card_world<W: CardWorld>(w: &W, actor: i32) -> WindowCtx {
    window(
        actor,
        (0..w.player_count()).map(|p| {
            let mut state = std::collections::BTreeMap::new();
            // Copy every key the guest exposes; missing keys answer 0 later.
            for (k, v) in w.state_pairs(p) {
                state.insert(k, i64::from(v));
            }
            Layers {
                out: w.player_out(p) != 0,
                state,
                hand: w.hand_size(p) as i64,
            }
        }),
    )
}

/// The [`Gate::Exclusive`] window: keyed state **plus** the questioned
/// exclusive and each seat's identity hashes (so a `state(p,'group')` sugar
/// guard reads the persona row's group -- the stable spelling, not the
/// currently attached group-skill id).
pub fn window_of_exclusive(
    w: &World,
    data: &GameData,
    actor: usize,
    ask: &ExclusiveAsk<'_>,
) -> WindowCtx {
    let mut win = window_of_world(w, actor);
    win.kind = TriggerKind::ExclusiveUse.spelling().into_owned();
    win.trigger_card = id_of(ask.card);
    win.name = ask.owner.to_string();
    win.value = ask.owner_is_base as i64;
    win.step = ask.owner_is_self as i64;
    for (i, p) in win.players.iter_mut().enumerate() {
        let ch =
            w.st.players
                .get(i)
                .map(|s| s.persona.clone())
                .unwrap_or_default();
        p.state.insert(
            bandori_std::cel::KEY_PERSONA.to_string(),
            id_of(&ch),
        );
        p.state.insert(
            bandori_std::cel::KEY_GROUP.to_string(),
            id_of(
                &data
                    .persona(&ch)
                    .map(|c| c.group.clone())
                    .unwrap_or_default(),
            ),
        );
    }
    win
}
