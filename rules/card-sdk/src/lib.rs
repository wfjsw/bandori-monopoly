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

// `ctx` is compiled whenever `guest` is on, not just on wasm32 -- and it uses
// `alloc::` throughout -- so `alloc` has to come along on native too. (A native
// `cargo check` of the card crates is how this shows up.)
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
extern crate alloc;

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::vec::Vec;

pub mod abi;
pub mod msg;

/// Native-only: how guest buffers cross the ABI on a 64-bit build (wasm32 uses
/// real 32-bit pointers; see the module docs).
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

pub use msg::Msg;

/// The run stopped at a prompt: the host has published the question and will
/// re-run this effect from the top with the answer plugged in (see the replay
/// note in `game-rules`). Propagate it with `?`.
///
/// `#[must_use]` because swallowing it is a real bug: the effect would carry on
/// with a made-up answer instead of stopping for the player. The trap this
/// replaces could not be ignored; the type now says so in the docs and the
/// lints. Pair it with a CI check over `rules/cards` for
/// `ask_.*\.(unwrap_or|ok\(\)|let _)` -- `Result` can be ignored where the
/// trap could not, and that is the one regression this migration introduces.
#[must_use = "a prompt must be propagated with `?` -- swallowing it runs the effect on a made-up answer"]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt;

/// What every effect entry returns: done, or "asked, re-run me".
pub type Asked = Result<(), Prompt>;

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
///     On::Hook(&[HookKind::TurnEnd], decay),
///     On::Gate(&[GateKind::ImmuneAll], immune),
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

/// One entry point of a card (a C# `Card` override). The variant says what the
/// entry is for, and its kind list is typed to match -- a reaction can only be
/// declared at a [`abi::ChainKind`], a field hook at a [`abi::HookKind`], a gate
/// at a [`abi::GateKind`]. An empty list is never dispatched.
#[derive(Clone, Copy)]
pub enum On {
    /// `Card.Play` -- play this card from hand. The first field is the gate
    /// (`Card.WhyNot`, the old `On::CantPlay`): a pure query with no prompts,
    /// `None` = playable, `Some(why)` = blocked and `why` is the reason to
    /// show. `None` for the gate itself means no gate. The second field is the
    /// effect.
    Play(
        Option<fn(player_id: i32) -> Option<Msg>>,
        fn(player_id: i32) -> Asked,
    ),
    /// A [反击] at these chain links: the guard decides whether the card is
    /// offered in the hand window, then the effect resolves. The guard is a
    /// pure query.
    CounterAct(
        &'static [abi::ChainKind],
        fn(player_id: i32) -> bool,
        fn(player_id: i32) -> Asked,
    ),
    /// A field-card (`Fx`) hook at these settlement points: runs automatically
    /// while the card is in play. Same shape as [`On::CounterAct`].
    Hook(
        &'static [abi::HookKind],
        fn(player_id: i32) -> bool,
        fn(player_id: i32) -> Asked,
    ),
    /// A question posed to this placed card at declaration or at resolution.
    Gate(&'static [abi::GateKind], fn(player_id: i32) -> Asked),
    /// `Card.RollPlan` -- this card has a movement routine.
    RollPlan(fn(player_id: i32) -> Asked),
    /// What `ctx::at_turn_end` schedules: run once at that turn end.
    AtEnd(fn(player_id: i32) -> Asked),
}

impl On {
    pub const fn kind(&self) -> abi::OnKind {
        match self {
            On::Play(..) => abi::OnKind::Play,
            On::CounterAct(..) => abi::OnKind::CounterAct,
            On::Hook(..) => abi::OnKind::Hook,
            On::Gate(..) => abi::OnKind::Gate,
            On::AtEnd(..) => abi::OnKind::AtEnd,
            On::RollPlan(..) => abi::OnKind::RollPlan,
        }
    }

    /// The wire values this entry answers (empty for non-trigger entries).
    /// The three kind enums share [`abi::TriggerKind`]'s numbering, so this is
    /// what the manifest carries.
    pub fn triggers(&self) -> Vec<i32> {
        match self {
            On::CounterAct(k, ..) => k.iter().map(|x| *x as i32).collect(),
            On::Hook(k, ..) => k.iter().map(|x| *x as i32).collect(),
            On::Gate(k, _) => k.iter().map(|x| *x as i32).collect(),
            _ => Vec::new(),
        }
    }
}
