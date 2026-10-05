//! Card authoring SDK and the host/guest ABI.
//!
//! Guest builds are `no_std` (plus `alloc`): the only thing a card module needs
//! from the standard library is the allocator, and dropping std takes its panic
//! runtime and init glue out of every module. Native builds (host tests) keep
//! std.
//!
//! # Execution model: deterministic replay
//!
//! A card effect is an ordinary straight-line Rust function. When it needs a player
//! decision it calls an `ask_*` helper. If the host already has the answer (in this
//! effect's answer log) the call simply returns it. If not, the host **aborts** the
//! run, throws away every side effect the run made, and publishes the prompt. When
//! the answer arrives the host re-runs the effect from the same snapshot (including
//! the RNG), so dice re-roll identically and the effect reaches the prompt again,
//! this time with an answer.
//!
//! Consequences for card authors:
//! * no coroutines, no async, no callbacks -- write it top to bottom like the C#;
//! * **all** randomness and world access must go through `ctx` (never keep your
//!   own state in `static mut`; the module is re-instantiated for every run);
//! * guards (`can_react`) run against a throwaway copy of the world, so they are
//!   pure by construction even if they call a mutating function by mistake.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
extern crate alloc;

pub mod abi;
pub mod msg;

pub use msg::Msg;

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
pub mod ctx;

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
#[doc(hidden)]
pub mod rt;

/// One card: its id and its entry points, declared as a table keyed by trigger
/// kind -- the same shape the engine's triggers have. The host reads the table
/// from the manifest and calls only the entries that match what is happening,
/// through one export (`bandori_on`).
///
/// ```ignore
/// pub const J11: CardDef = CardDef::new("Mujica:#J11", &[
///     On::Play(play),
///     On::Hook(&[TriggerKind::TurnEnd], decay),
///     On::Hook(&[TriggerKind::PayChoose], pay_choose),
/// ]);
/// ```
#[derive(Clone, Copy)]
pub struct CardDef {
    /// Exact id from `game-data/cards.json`, e.g. `"AG:Y.O.L.O"`.
    pub id: &'static str,
    pub on: &'static [On],
}

impl CardDef {
    pub const fn new(id: &'static str, on: &'static [On]) -> Self {
        Self { id, on }
    }
}

/// One entry point of a card (a C# `Card` override). Trigger-keyed entries list
/// the kinds they answer; an empty list is never dispatched.
#[derive(Clone, Copy)]
pub enum On {
    /// `Card.Play` -- the effect when played from hand.
    Play(fn(player_id: i32)),
    /// `Card.WhyNot` -- why it cannot be played right now: a pure query with no
    /// prompts (`None` = playable).
    CantPlay(fn(player_id: i32) -> Option<Msg>),
    /// A [反击] at these trigger kinds: the guard (`Card.CanReact`) decides
    /// whether the card is offered in the hand window, then the effect
    /// (`Card.React`) resolves. The guard is a pure query.
    React(&'static [abi::TriggerKind], fn(player_id: i32) -> bool, fn(player_id: i32)),
    /// A field-card (`Fx`) hook at these trigger kinds: runs automatically, with
    /// no declaration, while the card is in play (`Drawn`: the card just drawn,
    /// still in hand). `player_id` is where the card is placed.
    Hook(&'static [abi::TriggerKind], fn(player_id: i32)),
    /// `Card.RollPlan` -- this card has a movement routine. Called when the
    /// walk is being planned (C# `RollPlan(MoveCtx)`); the routine shapes the
    /// walk through `ctx` (steps, dice, teleport, stop-at, ...).
    RollPlan(fn(player_id: i32)),
    /// What `ctx::at_turn_end` / `at_next_turn_end` schedule: run once at that
    /// turn end, for the player it was scheduled for. The card need not be in play.
    AtEnd(fn(player_id: i32)),
}

impl On {
    pub const fn kind(&self) -> abi::OnKind {
        match self {
            On::Play(_) => abi::OnKind::Play,
            On::CantPlay(_) => abi::OnKind::CantPlay,
            On::React(..) => abi::OnKind::React,
            On::Hook(..) => abi::OnKind::Hook,
            On::AtEnd(_) => abi::OnKind::AtEnd,
            On::RollPlan(_) => abi::OnKind::RollPlan,
        }
    }

    pub const fn triggers(&self) -> &'static [abi::TriggerKind] {
        match self {
            On::React(k, ..) | On::Hook(k, _) => k,
            _ => &[],
        }
    }
}
