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
//! * guards (`can_counteract`) run against a throwaway copy of the world, so they are
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
///     On::Play("", None, play),
///     On::Hook(&[HookKind::TurnEnd], pre::MINE, guard, decay),
///     On::Gate(&[GateKind::ImmuneAll], pre::MINE, None, immune),
/// ]);
/// ```
///
/// A card also declares its **static properties** here -- named `key -> i32`
/// facts about the card rule, not effects that run at a trigger -- so the
/// engine never has to read them back out of the rulebook prose (which would
/// break the moment a card is reworded). The keys are named constants in
/// [`abi::prop`]; they ride the manifest to the host and the engine queries
/// them by key (`CardRules::card_prop`), default `0`.
///
/// ```ignore
/// use card_sdk::abi::prop;
/// // 规则书[持续]（1）: 「手卡上限数量减1」 -- C# `Card.HandLimitDelta`
/// pub const CUT: CardDef = CardDef::new("PP:不要背负期待", &[...])
///     .props(&[(prop::HAND_LIMIT_DELTA, -1)]);
/// // 规则书: 「（此卡可在眩晕时打出）」 -- C# `Card.PlayableStunned`
/// pub const H: CardDef = CardDef::new("MyGO:壱雫空", &[...])
///     .props(&[(prop::PLAYABLE_STUNNED, 1)]);
/// ```
#[derive(Clone, Copy)]
pub struct CardDef {
    /// Exact id from `game-data/cards.json`, e.g. `"AG:Y.O.L.O"`.
    pub id: &'static str,
    pub on: &'static [On],
    /// The card's declared static properties, `(key, value)` in declaration
    /// order (the manifest sorts them by key for deterministic wire bytes).
    /// Keys are [`abi::prop`] constants; a key not declared reads as its
    /// default (`0`).
    pub props: &'static [(&'static str, i32)],
    /// G3 migration audit (docs/GUARDS.md §5.1): the card's pre-migration
    /// guard(s), as `(entry index, legacy fn)`. Kept beside the migrated entry
    /// while the `guard-audit` host feature is in play; deleted once the card
    /// is clean. Empty for a card that is not under audit.
    pub legacy: &'static [(i32, fn(i32) -> bool)],
}

impl CardDef {
    pub const fn new(id: &'static str, on: &'static [On]) -> Self {
        Self {
            id,
            on,
            props: &[],
            legacy: &[],
        }
    }

    /// Declare this card's static properties (see [`abi::prop`] for the keys
    /// the engine reads). Replaces any earlier `props` call.
    pub const fn props(self, props: &'static [(&'static str, i32)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props,
            legacy: self.legacy,
        }
    }

    /// Keep the pre-migration guard of entry `entry` as `legacy` for the G3
    /// equivalence audit (docs/GUARDS.md §5.1). The `guard-audit` host calls it
    /// through `export::OP_LEGACY_GUARD` and panics on any mismatch with
    /// `pre ∧ guard`. Delete the copy once the card's audit is clean.
    pub const fn legacy(self, legacy: &'static [(i32, fn(i32) -> bool)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy,
        }
    }
}

/// One entry point of a card (a C# `Card` override). The variant says what the
/// entry is for, and its kind list is typed to match -- a counteraction can only be
/// declared at a [`abi::ChainKind`], a field hook at a [`abi::HookKind`], a gate
/// at a [`abi::GateKind`]. An empty list is never dispatched.
///
/// Guarded entries (`Play` gate / `Counteract` / `Hook` / `Gate` / `AtEnd` /
/// `RollPlan` / `Settle`) carry a **condition** string (`pre`) immediately
/// before the residual guard -- category → condition → guard → body
/// (docs/GUARDS.md §4.3, the three-layer model). `""` = no condition. Sugar:
/// [`pre::MINE`] (`actor == owner`); or attach one with [`On::pre`]. The
/// condition is compiled once at ruleset build (host) and evaluated natively
/// before the wasm guard is instantiated.
#[derive(Clone, Copy)]
pub enum On {
    /// `Card.Play` -- play this card from hand. The first field is the gate's
    /// condition (see the type doc). The second is the gate
    /// (`Card.WhyNot`, the old `On::CantPlay`): a pure query with no prompts,
    /// `None` = playable, `Some(why)` = blocked and `why` is the reason to
    /// show. `None` for the gate itself means no gate. The third field is the
    /// effect.
    Play(
        &'static str,
        Option<fn(player_id: i32) -> Option<Msg>>,
        fn(player_id: i32) -> Asked,
    ),
    /// A [反击] at these chain links: the guard decides whether the card is
    /// offered in the hand window, then the effect resolves. Second field: the
    /// guard's condition (see the type doc); third, the residual guard -- a
    /// pure query, `Option`: `None` means G4 deleted it because the condition
    /// alone decides (docs/GUARDS.md §2/§4.3). Fourth: the effect.
    Counteract(
        &'static [abi::ChainKind],
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
    /// A field-card (`Fx`) hook at these settlement points: runs automatically
    /// while the card is in play. Same shape as [`On::Counteract`].
    Hook(
        &'static [abi::HookKind],
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
    /// A question posed to this placed card at declaration or at resolution.
    /// Same shape as [`On::Hook`]: the condition / residual guard decide
    /// whether the entry answers at all; a rejecting entry runs no body (and so
    /// never flashes).
    Gate(
        &'static [abi::GateKind],
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
    /// `Card.RollPlan` -- this card has a movement routine. Same shape as
    /// [`On::Hook`]; the condition sees the move being planned (docs/GUARDS.md
    /// §4.2c).
    RollPlan(
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
    /// What `ctx::at_turn_end` schedules: run once at that turn end. Same shape
    /// as [`On::Hook`]; the condition sees the owner and the turn fields.
    AtEnd(
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
    /// The rule's **settle body** (`docs/TILES.md`) -- what runs when the tile
    /// this rule instance governs is [结算]d. Tile rules (`tile:*`) are one
    /// `CardDef` per tile kind and this is their body; the engine keeps
    /// rent / buy / build / draw-event as `ctx` primitives so it stays thin.
    /// Runs inside the settle chain: a counteraction to the settle link, or a
    /// field hook that replaces the body, shapes whether and how it runs. Same
    /// shape as [`On::Hook`]; the condition sees the settling tile / actor.
    Settle(
        &'static str,
        Option<fn(player_id: i32) -> bool>,
        fn(player_id: i32) -> Asked,
    ),
}

/// Condition-authoring sugar (docs/GUARDS.md §4.3). The strings are CEL; the
/// schema is the §4.2 window/candidate vocabulary, compiled at ruleset build.
pub mod pre {
    /// `actor == owner` -- the 118 `mine` guards. Turns a same-seat-only
    /// guard into a condition plus (at most) a residual guard.
    pub const MINE: &str = "actor == owner";
}

impl On {
    /// Replace the entry's condition string (docs/GUARDS.md §4.3). A const
    /// builder so a declaration can stay a one-liner:
    ///
    /// ```ignore
    /// On::Counteract(&[ChainKind::MoveRoll], "", Some(can_counteract), counteract).pre(pre::MINE)
    /// ```
    pub const fn pre(self, pre: &'static str) -> Self {
        match self {
            On::Play(_, g, r) => On::Play(pre, g, r),
            On::Counteract(k, _, g, r) => On::Counteract(k, pre, g, r),
            On::Hook(k, _, g, r) => On::Hook(k, pre, g, r),
            On::Gate(k, _, g, r) => On::Gate(k, pre, g, r),
            On::RollPlan(_, g, r) => On::RollPlan(pre, g, r),
            On::AtEnd(_, g, r) => On::AtEnd(pre, g, r),
            On::Settle(_, g, r) => On::Settle(pre, g, r),
        }
    }

    /// Delete the residual guard (G4): the condition alone decides. The host
    /// then skips the `OP_GUARD` instantiation entirely (`admits_pre`).
    pub const fn no_guard(self) -> Self {
        match self {
            On::Counteract(k, pre, _, r) => On::Counteract(k, pre, None, r),
            On::Hook(k, pre, _, r) => On::Hook(k, pre, None, r),
            On::Gate(k, pre, _, r) => On::Gate(k, pre, None, r),
            On::RollPlan(pre, _, r) => On::RollPlan(pre, None, r),
            On::AtEnd(pre, _, r) => On::AtEnd(pre, None, r),
            On::Settle(pre, _, r) => On::Settle(pre, None, r),
            other => other,
        }
    }

    /// The residual guard, if any. `None` = G4-deleted (condition-only entry).
    /// For `On::Play` this is the gate (`WhyNot`), which has a different
    /// signature -- use [`Self::has_guard`] for the host's skip-the-wasm test.
    pub const fn guard(&self) -> Option<fn(i32) -> bool> {
        match self {
            On::Counteract(_, _, g, _)
            | On::Hook(_, _, g, _)
            | On::Gate(_, _, g, _)
            | On::RollPlan(_, g, _)
            | On::AtEnd(_, g, _)
            | On::Settle(_, g, _) => *g,
            _ => None,
        }
    }

    /// Does this entry have a residual wasm guard / gate the host must call?
    /// `false` = G4-deleted: the condition alone decides (`admits_pre`).
    pub const fn has_guard(&self) -> bool {
        match self {
            On::Play(_, g, _) => g.is_some(),
            On::Counteract(_, _, g, _)
            | On::Hook(_, _, g, _)
            | On::Gate(_, _, g, _)
            | On::RollPlan(_, g, _)
            | On::AtEnd(_, g, _)
            | On::Settle(_, g, _) => g.is_some(),
        }
    }

    /// The entry's condition source (`""` = none). Compiled by the host at
    /// ruleset build (`docs/GUARDS.md` §4.3).
    pub const fn condition(&self) -> &'static str {
        match self {
            On::Play(pre, _, _)
            | On::Counteract(_, pre, _, _)
            | On::Hook(_, pre, _, _)
            | On::Gate(_, pre, _, _)
            | On::RollPlan(pre, _, _)
            | On::AtEnd(pre, _, _)
            | On::Settle(pre, _, _) => *pre,
        }
    }

    pub const fn kind(&self) -> abi::OnKind {
        match self {
            On::Play(..) => abi::OnKind::Play,
            On::Counteract(..) => abi::OnKind::Counteract,
            On::Hook(..) => abi::OnKind::Hook,
            On::Gate(..) => abi::OnKind::Gate,
            On::AtEnd(..) => abi::OnKind::AtEnd,
            On::RollPlan(..) => abi::OnKind::RollPlan,
            On::Settle(..) => abi::OnKind::Settle,
        }
    }

    /// The wire values this entry answers (empty for non-trigger entries).
    /// The three kind enums share [`abi::TriggerKind`]'s numbering, so this is
    /// what the manifest carries.
    pub fn triggers(&self) -> Vec<i32> {
        match self {
            On::Counteract(k, ..) => k.iter().map(|x| *x as i32).collect(),
            On::Hook(k, ..) => k.iter().map(|x| *x as i32).collect(),
            On::Gate(k, ..) => k.iter().map(|x| *x as i32).collect(),
            _ => Vec::new(),
        }
    }
}
