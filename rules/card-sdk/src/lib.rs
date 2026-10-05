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

/// One card behaviour. Mirrors the C# `Card` overrides the port needs first;
/// more hooks are added as cards require them (see `abi::ABI_VERSION`).
#[derive(Clone, Copy)]
pub struct CardDef {
    /// Exact id from `game-data/cards.json`, e.g. `"AG:Y.O.L.O"`.
    pub id: &'static str,
    /// `Card.Play(PlayCtx)` -- the effect when played from hand.
    pub play: Option<fn(seat: i32)>,
    /// `Card.CanReact(seat, Trigger)` -- may this card be played now as a reaction?
    pub can_react: Option<fn(seat: i32) -> bool>,
    /// `Card.React(PlayCtx)` -- the reaction effect.
    pub react: Option<fn(seat: i32)>,
    /// `Card.WhyNot(seat)` -- why it cannot be played right now (a pure query,
    /// no prompts: `None` = playable, `Some(reason)` = show the reason).
    pub why_not: Option<fn(seat: i32) -> Option<Msg>>,
}

impl CardDef {
    pub const fn new(id: &'static str) -> Self {
        Self { id, play: None, can_react: None, react: None, why_not: None }
    }
}
