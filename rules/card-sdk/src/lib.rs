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
//! * no coroutines, no async, no callbacks -- write it top to bottom;
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
pub mod meta;
pub mod msg;

/// Generic interned-name table (`u16` ids). Under `cfg(kani)` the ctx boundary
/// interns every `&str` so the symbolic host never runs `memcmp`. Game
/// vocabulary lives in `rules-kani`, not here.
pub mod intern;

/// Host-import dispatch used by the Kani verifier crate (see module docs).
/// Compiled always so `rules-kani` typechecks outside `cargo kani`; production
/// builds never call it (`sys` still uses the `extern` block).
pub mod kani_host;

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
///     On::Hook(&[HookKind::TurnEnd], pre::MINE, guard, tick),
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
/// // 规则书[持续]（1）: 「手卡上限数量减1」
/// pub const CUT: CardDef = CardDef::new("PP:不要背负期待", &[...])
///     .props(&[(prop::HAND_LIMIT_DELTA, -1)]);
/// // 规则书: 「（此卡可在眩晕时打出）」
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
    /// Per-entry labels -- the rulebook's numbered clauses, 「（1）」 / 「（2）」
    /// and kin -- as `(entry index, label)`. Rides the manifest so the
    /// 「效果适用」 line can name which clause of a multi-entry card ran.
    /// An entry with no label shows just the card name.
    pub labels: &'static [(i32, &'static str)],
    /// Entry indices that are **bookkeeping**: they run their body but post no
    /// activation flash and no 「效果适用」 header. For declarations and resets
    /// the player never sees as a clause firing (`declare_cap` / `set_bounds` /
    /// initial values / latch clears). Fills [`abi::Presentation::Silent`].
    pub silent: &'static [i32],
    /// Entry indices that flash only when their body produced something
    /// visible (`abi::Presentation::QuietWhenEmpty`). For clauses that apply
    /// and change nothing (capped gain, 「已满」) -- the player does not need
    /// a flash for a no-op. A body that logs / pays / moves still flashes.
    pub quiet_when_empty: &'static [i32],
    /// Per-entry flash policy overrides -- `(entry index, Flash)`. An entry
    /// with no override is [`abi::Flash::Auto`] (host classifies by scope).
    /// [`abi::Flash::Always`] / [`abi::Flash::Never`] are the rule's own
    /// opinion and win over the Auto classification. `CardDef::flash`.
    pub flash: &'static [(i32, crate::abi::Flash)],
    /// Per-entry **tile text** -- the deed-popup description this entry speaks
    /// for, as `(entry index, slot, tile, key)`.
    /// - `slot`: [`abi::text_slot::SETTLE`] or [`abi::text_slot::PASS`].
    /// - `tile`: `""` = the instance's own tile (a `tile:*` rule bound to one
    ///   tile); otherwise a board tile *name* (`"CiRCLE"`, `"CiRCLE 咖啡厅"`) for
    ///   a tile-bound text on a tile-less instance (`event:*`).
    /// - `key`: i18n key in the `ui` namespace (`deed.note.*`); `""` means this
    ///   entry **suppresses** the slot (the effective text is nothing).
    ///
    /// `CardDef::texts`. The view composes per-tile speakers in
    /// body-precedence order (docs/TILES.md 「Tile text」).
    pub texts: &'static [(i32, &'static str, &'static str, &'static str)],
    /// Deck/gallery metadata for this card (`CardDef::meta`, ABI v63). Lives
    /// beside the rule so the roster is authored with the rule; rides the
    /// separate `bandori_metadata` export, **not** [`abi::ManifestEntry`].
    /// `None` = the card has no Rust metadata yet (partial lift).
    pub meta: Option<&'static crate::meta::CardMeta>,
}

impl CardDef {
    pub const fn new(id: &'static str, on: &'static [On]) -> Self {
        Self {
            id,
            on,
            props: &[],
            legacy: &[],
            labels: &[],
            silent: &[],
            quiet_when_empty: &[],
            flash: &[],
            texts: &[],
            meta: None,
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
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// Name this card's entries after their rulebook clauses -- 「（1）」,
    /// 「（2）」, … -- as `(entry index, label)` pairs. The 「效果适用」 line
    /// shows the label of the entry that ran; unlabelled entries stay bare.
    pub const fn labels(self, labels: &'static [(i32, &'static str)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// Mark these entry indices **silent** (bookkeeping): they run their body
    /// but post no activation flash and no 「效果适用」 header. Use for
    /// declarations and resets the player never sees as a clause firing --
    /// `declare_cap` / `set_bounds` / initial values / latch clears. An effect
    /// the rulebook describes as happening (gain, pay, place, move, draw,
    /// status, mark) must **not** be listed here. Replaces any earlier
    /// `silent` call.
    ///
    /// ```ignore
    /// pub const T: CardDef = CardDef::new("skill:…", &[
    ///     On::Play("", None, use_skill),                                     // 0: flashes
    ///     On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
    ///              "", None, declare_cap),                                   // 1: bookkeeping
    /// ]).silent(&[1]);
    /// ```
    pub const fn silent(self, entries: &'static [i32]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels: self.labels,
            silent: entries,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// Mark these entry indices **quiet when empty**: the entry runs normally
    /// but its 「效果适用」 header is retracted when the body produced nothing
    /// visible (no children, no logs). Use for clauses that apply and change
    /// nothing -- a gain already at its cap, 「已满」, a set that writes the
    /// same value. A body that logs / pays / moves still flashes. Do **not**
    /// use for an opt-in the player can decline (「放弃」 is information).
    pub const fn quiet_when_empty(self, entries: &'static [i32]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: entries,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// Override the flash policy of these entries -- `(entry index, Flash)`.
    /// [`abi::Flash::Always`] flashes even an owner-self-only effect the
    /// player should notice; [`abi::Flash::Never`] keeps the log entry but
    /// suppresses the flash / pulse even for a cross-player one. Entries with
    /// no override stay [`abi::Flash::Auto`] (host classifies by scope).
    /// Replaces any earlier `flash` call.
    ///
    /// ```ignore
    /// pub const T: CardDef = CardDef::new("skill:…", &[
    ///     On::Play("", None, use_skill),                 // 0: Auto
    ///     On::Hook(&[HookKind::TurnEnd], "", None, tick), // 1: Always (self-only)
    /// ]).flash(&[(1, Flash::Always)]);
    /// ```
    pub const fn flash(self, entries: &'static [(i32, crate::abi::Flash)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: entries,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// How entry `index` presents itself in the log: the header mode plus the
    /// flash policy, folded from the `silent` / `quiet_when_empty` / `flash`
    /// side tables. The one lookup.
    pub const fn presentation(&self, index: i32) -> crate::abi::EntryPresentation {
        let mut header = crate::abi::Presentation::Visible;
        let mut i = 0;
        while i < self.silent.len() {
            if self.silent[i] == index {
                header = crate::abi::Presentation::Silent;
            }
            i += 1;
        }
        let mut i = 0;
        while i < self.quiet_when_empty.len() {
            if self.quiet_when_empty[i] == index {
                header = crate::abi::Presentation::QuietWhenEmpty;
            }
            i += 1;
        }
        let mut flash = crate::abi::Flash::Auto;
        let mut i = 0;
        while i < self.flash.len() {
            if self.flash[i].0 == index {
                flash = self.flash[i].1;
            }
            i += 1;
        }
        crate::abi::EntryPresentation { header, flash }
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
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
        }
    }

    /// Declare the deed-popup **tile text** these entries speak for --
    /// `(entry index, slot, tile, key)`. See [`Self::texts`] for the shape.
    /// Replaces any earlier `texts` call.
    ///
    /// ```ignore
    /// use card_sdk::abi::text_slot;
    /// pub const CIRCLE: CardDef = CardDef::new("tile:circle", &[
    ///     On::Settle("", None, settle),                                       // 0
    ///     On::Hook(&[HookKind::PassTile], "", Some(pass_pays), on_pass),      // 1
    /// ]).texts(&[
    ///     (0, text_slot::SETTLE, "", "deed.note.circleSettle"),
    ///     (1, text_slot::PASS, "", "deed.note.circlePass"),
    /// ]);
    /// ```
    pub const fn texts(
        self,
        texts: &'static [(i32, &'static str, &'static str, &'static str)],
    ) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts,
            meta: self.meta,
        }
    }

    /// Deck/gallery metadata for this card (`docs/CARDS.md` 「Card metadata」).
    /// Lives beside the rule; exported through `bandori_metadata`, never
    /// through [`abi::ManifestEntry`]. Replaces any earlier `meta` call.
    ///
    /// ```ignore
    /// use card_sdk::meta::CardMeta;
    /// pub const CARD: CardDef = CardDef::new("Sumimi:Sumimi不会解散哦", &[...])
    ///     .meta(&CardMeta::new()
    ///         .name("Sumimi不会解散哦")
    ///         .raw_name("Sumimi不会解散哦")
    ///         .text("...")
    ///         .tags(&["手"])
    ///         .cell("B2")
    ///         .fields(&[("band", "Sumimi"), ("owner", "")]));
    /// ```
    pub const fn meta(self, meta: &'static crate::meta::CardMeta) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            legacy: self.legacy,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: Some(meta),
        }
    }

    /// Is entry `index` marked silent? (See [`Self::presentation`] for the
    /// full three-way presentation.)
    pub const fn is_silent(&self, index: i32) -> bool {
        let mut i = 0;
        while i < self.silent.len() {
            if self.silent[i] == index {
                return true;
            }
            i += 1;
        }
        false
    }
}

/// One entry point of a card. The variant says what the
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
    /// Play this card from hand. The first field is the gate's
    /// condition (see the type doc). The second is the gate
    /// (the old `On::CantPlay`): a pure query with no prompts,
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
    /// This card has a movement routine ([移动] shaping). Same shape as
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
    /// A **cross-card message** this card answers (`ctx::send`). First field
    /// is the list of message names it answers (empty is never dispatched).
    /// Same condition / residual guard / body shape as [`On::Hook`]. The
    /// handler reads the sender and payload from [`ctx::message`] and returns
    /// a reply value via `ctx::message::reply`. Handling a message does not
    /// flash the card (internal plumbing).
    Message(
        &'static [&'static str],
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
            On::Message(n, _, g, r) => On::Message(n, pre, g, r),
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
            On::Message(n, pre, _, r) => On::Message(n, pre, None, r),
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
            | On::Settle(_, g, _)
            | On::Message(_, _, g, _) => *g,
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
            | On::Settle(_, g, _)
            | On::Message(_, _, g, _) => g.is_some(),
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
            | On::Settle(pre, _, _)
            | On::Message(_, pre, _, _) => *pre,
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
            On::Message(..) => abi::OnKind::Message,
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
            // Message names travel as the manifest's `triggers` string list
            // packed as hashes? No -- `triggers` is `Vec<i32>` of kind codes.
            // The names ride a separate manifest field; see `rt.rs`.
            On::Message(..) => Vec::new(),
            _ => Vec::new(),
        }
    }

    /// The message names this [`On::Message`] entry answers (empty otherwise).
    pub fn message_names(&self) -> Vec<&'static str> {
        match self {
            On::Message(names, ..) => names.to_vec(),
            _ => Vec::new(),
        }
    }
}
