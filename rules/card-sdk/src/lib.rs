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

// Guest wasm modules (one card crate per `.wasm`) are `no_std`. The bot
// monolith (`bot-glue` + `native-rules`) is a **std** cdylib that links the
// card crates as rlibs -- it keeps std (and the host's allocator / panic
// machinery), so the guest panic handler and bump `#[global_alloc]` must not
// be compiled in. See `rt.rs` and the `native-rules` feature below.
#![cfg_attr(all(target_arch = "wasm32", not(feature = "native-rules")), no_std)]

// `ctx` is compiled whenever `guest` is on, not just on wasm32 -- and it uses
// `alloc::` throughout -- so `alloc` has to come along on native too. (A native
// `cargo check` of the card crates is how this shows up.)
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
extern crate alloc;

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::string::String;
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::vec::Vec;

pub mod abi;
pub mod meta;
pub mod msg;
pub mod payload;

/// Generic interned-name table (`u16` ids). Under `cfg(kani)` the ctx boundary
/// interns every `&str` so the symbolic host never runs `memcmp`. Game
/// vocabulary lives in `rules-kani`, not here.
pub mod intern;

/// Host-import dispatch used by the Kani verifier crate (see module docs).
/// Compiled always so `rules-kani` typechecks outside `cargo kani`; production
/// builds never call it (`sys` still uses the `extern` block).
pub mod kani_host;

/// `cfg(kani)` static bump allocator (TUNING §11). Kills the system
/// allocator's OOM / `RawVec::reserve` CBMC model on every body `String`/`Vec`
/// (including `alloc::format!`).
///
/// Always on under `cfg(kani)` (class-E deep dive): validated on
/// `skills_mygo::band_04_ineffective_overcap_iff` (SUCCESSFUL, 3/3 covers)
/// and `sumimi::twoinone_01b_*` before the global flip. The earlier
/// `from_raw_parts` + `__rust_dealloc` vacuity (TRIAGE-LOG strvec v3) does not
/// apply — this arena returns real pointers and `dealloc` is a no-op.
/// Feature `kani-bump` is kept as a no-op alias so existing `--features`
/// lists keep compiling. Not compiled outside `cargo kani`.
#[cfg(kani)]
mod kani_alloc;

/// Native / wasm32-linked ABI support: how guest buffers cross the boundary as
/// 32-bit arena handles. Real wasm32 guest modules use real 32-bit pointers;
/// native 64-bit builds **and** the wasm32-linked monolith (whose host is
/// `rules-native`'s `NativeHost`, which always reads through this arena) use
/// the frame stack. See the module docs.
#[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
pub mod native;

pub mod kinds;

pub use kinds::{Action, ActionEntry, Filter, FilterEntry, FilterShape, KindInfo};
pub use msg::Msg;

/// A locale key the client resolves (`key!` / `err.*` / `log.*` / `counter.*`).
pub use abi::MsgKey;
pub use payload::FilterPayload;
pub use payload::{DealValue, GateValue, PayAddValue, PlanValue, RedirectValue, RollValue};

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
pub mod pure;

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
#[doc(hidden)]
pub mod rt;

/// One card: its id and its entry points, declared as a table keyed by trigger
/// kind -- the same shape the engine's triggers have. The host reads the table
/// from the manifest and calls only the entries that match what is happening,
/// through one export (`ruleset_on`).
///
/// ```ignore
/// pub const J11: CardDef = CardDef::new("Mujica:#J11", &[
///     On::Play("", None, None, play),
///     On::Action(Action::TurnEnd(pre::MINE, None, guard, tick)),
///     On::Filter(Filter::ImmuneAll(pre::MINE, None, immune)),
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
///     .props(&[("handPlayExempt", 1)]); // a ruleset prop key, as data
/// ```
#[derive(Clone, Copy)]
pub struct CardDef {
    /// Exact id from `game-the roster`, e.g. `"AG:Y.O.L.O"`.
    pub id: &'static str,
    pub on: &'static [On],
    /// The card's declared static properties, `(key, value)` in declaration
    /// order (the manifest sorts them by key for deterministic wire bytes).
    /// Keys are [`abi::prop`] constants; a key not declared reads as its
    /// default (`0`).
    pub props: &'static [(&'static str, i32)],
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
    /// separate `ruleset_metadata` export, **not** [`abi::ManifestEntry`].
    /// `None` = the card has no Rust metadata yet (partial lift).
    pub meta: Option<&'static crate::meta::CardMeta>,
    /// Board role of a standing rule (`CardDef::board_role`, ABI v66):
    /// `"tile"` (one instance per governed board-tile kind), `"mark"`
    /// (board-wide mark owner), `"status"` (board-wide status host), or `""`
    /// for an ordinary card. The engine never names the roles' contents.
    pub board_role: &'static str,
    /// Tile kinds this rule governs (`CardDef::governs`, ABI v66) when
    /// [`Self::board_role`] is `"tile"`. Empty = derive the single kind from
    /// the id's `tile:` suffix. Multi-kind rules list every kind.
    pub governs: &'static [&'static str],
    /// Refusal message keys for the capability gates this rule closes, used
    /// when a **play** is refused (`cannot_play`): `(gate kind i32, i18n key)`.
    /// Gate kind ids are `TriggerKind::{ActGate=95, PayGate=96, MoveGate=97,
    /// PlayGate=101}` plus `OnBoardGate` / `TargetableGate`. Default `&[]`.
    ///
    /// The same gate can speak a different line when it **skips a turn**
    /// (the skip log keys vs the play-refusal keys); that
    /// spelling rides [`Self::gate_skip_msgs`] so both frozen keys remain on the
    /// wire. Host `gate_refusal_msg` reads `gate_msgs` (play) and should read
    /// `gate_skip_msgs` for the turn-skip log.
    pub gate_msgs: &'static [(i32, &'static str)],
    /// Turn-skip message keys for the capability gates this rule closes
    /// (ActGate → its skip key, MoveGate → its skip key):
    /// `(gate kind i32, i18n key)`. Default `&[]` (fall back to
    /// [`Self::gate_msgs`]). Trailing field -- postcard is not self-describing.
    pub gate_skip_msgs: &'static [(i32, &'static str)],
    /// Card prop key that exempts a play from this rule's act gate (empty =
    /// none). The name is ruleset vocabulary; the engine reads it as data.
    pub exemption_prop: &'static str,
}

impl CardDef {
    pub const fn new(id: &'static str, on: &'static [On]) -> Self {
        Self {
            id,
            on,
            props: &[],
            labels: &[],
            silent: &[],
            quiet_when_empty: &[],
            flash: &[],
            texts: &[],
            meta: None,
            board_role: "",
            governs: &[],
            gate_msgs: &[],
            gate_skip_msgs: &[],
            exemption_prop: "",
        }
    }

    /// Declare this card's static properties (see [`abi::prop`] for the keys
    /// the engine reads). Replaces any earlier `props` call.
    pub const fn props(self, props: &'static [(&'static str, i32)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Name this card's entries after their rulebook clauses -- 「（1）」,
    /// 「（2）」, … -- as `(entry index, label)` pairs. The 「效果适用」 line
    /// shows the label of the entry that ran; unlabelled entries remain bare.
    pub const fn labels(self, labels: &'static [(i32, &'static str)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
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
    ///     On::Play("", None, None, use_skill),                                     // 0: flashes
    ///     On::Action(Action::TurnStartBefore("", None, None, declare_cap)),         // 1: bookkeeping
    ///     On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),         // 2: bookkeeping
    /// ]).silent(&[1, 2]);
    /// ```
    pub const fn silent(self, entries: &'static [i32]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: entries,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
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
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: entries,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Override the flash policy of these entries -- `(entry index, Flash)`.
    /// [`abi::Flash::Always`] flashes even an owner-self-only effect the
    /// player should notice; [`abi::Flash::Never`] keeps the log entry but
    /// suppresses the flash / pulse even for a cross-player one. Entries with
    /// no override remain [`abi::Flash::Auto`] (host classifies by scope).
    /// Replaces any earlier `flash` call.
    ///
    /// ```ignore
    /// pub const T: CardDef = CardDef::new("skill:…", &[
    ///     On::Play("", None, None, use_skill),                 // 0: Auto
    ///     On::Action(Action::TurnEnd("", None, None, tick)), // 1: Always (self-only)
    /// ]).flash(&[(1, Flash::Always)]);
    /// ```
    pub const fn flash(self, entries: &'static [(i32, crate::abi::Flash)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: entries,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
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

    /// Declare the deed-popup **tile text** these entries speak for --
    /// `(entry index, slot, tile, key)`. See [`Self::texts`] for the shape.
    /// Replaces any earlier `texts` call.
    ///
    /// ```ignore
    /// use card_sdk::abi::text_slot;
    /// pub const CIRCLE: CardDef = CardDef::new("tile:circle", &[
    ///     On::Settle("", None, settle),                                       // 0
    ///     On::Action(Action::PassTile("", None, Some(pass_pays), on_pass)),      // 1
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
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Deck/gallery metadata for this card (`docs/CARDS.md` 「Card metadata」).
    /// Lives beside the rule; exported through `ruleset_metadata`, never
    /// through [`abi::ManifestEntry`]. Replaces any earlier `meta` call.
    ///
    /// ```ignore
    /// use card_sdk::meta::CardMeta;
    /// pub const CARD: CardDef = CardDef::new("Sumimi:Sumimi不会解散哦", &[...])
    ///     .meta(&CardMeta::new()
    ///         .name("Sumimi不会解散哦") // sheet cell B2
    ///         .raw_name("Sumimi不会解散哦")
    ///         .text("...")
    ///         .tags(&["手"])
    ///         .fields(&[("group", "Sumimi"), ("owner", "")]));
    /// ```
    pub const fn meta(self, meta: &'static crate::meta::CardMeta) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: Some(meta),
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Board role of a standing rule (ABI v66): `"tile"` / `"mark"` /
    /// `"status"`, or `""` (the default) for an ordinary card. Replaces any
    /// earlier `board_role` call.
    ///
    /// ```ignore
    /// pub const MARK: CardDef = CardDef::new("mark:cp", &[...]).board_role("mark");
    /// ```
    pub const fn board_role(self, role: &'static str) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Tile kinds this rule governs (ABI v66) when [`Self::board_role`] is
    /// `"tile"`. Empty derives a single kind from the id's `tile:` suffix.
    /// Replaces any earlier `governs` call.
    ///
    /// ```ignore
    /// pub const EVENT: CardDef = CardDef::new("tile:event", &[...])
    ///     .board_role("tile")
    ///     .governs(&["cafe", "ryuseido"]);
    /// ```
    pub const fn governs(self, kinds: &'static [&'static str]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: kinds,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Refusal message keys for the capability gates this rule closes (play
    /// path): `(gate kind i32, i18n key)`. Replaces any earlier `gate_msgs`
    /// call. Turn-skip lines ride [`Self::gate_skip_msgs`].
    pub const fn gate_msgs(self, msgs: &'static [(i32, &'static str)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Turn-skip message keys for the capability gates this rule closes:
    /// `(gate kind i32, i18n key)`. Replaces any earlier `gate_skip_msgs`
    /// call. Play refusals ride [`Self::gate_msgs`].
    pub const fn gate_skip_msgs(self, msgs: &'static [(i32, &'static str)]) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: msgs,
            exemption_prop: self.exemption_prop,
        }
    }

    /// Card prop key that exempts a play from this rule's act gate (empty =
    /// none). Replaces any earlier `exemption_prop` call.
    pub const fn exemption_prop(self, key: &'static str) -> Self {
        Self {
            id: self.id,
            on: self.on,
            props: self.props,
            labels: self.labels,
            silent: self.silent,
            quiet_when_empty: self.quiet_when_empty,
            flash: self.flash,
            texts: self.texts,
            meta: self.meta,
            board_role: self.board_role,
            governs: self.governs,
            gate_msgs: self.gate_msgs,
            gate_skip_msgs: self.gate_skip_msgs,
            exemption_prop: key,
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

/// Play-from-hand entry ([`On::Play`]).
#[derive(Clone, Copy)]
pub struct PlayEntry {
    /// Gate condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// The 「…时」 clause the counteraction prompt shows (locale key; `None` =
    /// no clause).
    pub counter_reason: Option<MsgKey>,
    /// The gate (the old `On::CantPlay`): a pure query with no prompts,
    /// `None` = playable, `Some(why)` = blocked and `why` is the reason to
    /// show. `None` for the gate itself means no gate.
    pub why_not: Option<fn(player_id: i32) -> Option<Msg>>,
    /// The effect.
    pub body: fn(player_id: i32) -> Asked,
}

/// [反击] entry ([`On::Counteract`]).
#[derive(Clone, Copy)]
pub struct CounteractEntry {
    /// The chain links this [反击] answers.
    pub links: &'static [abi::ChainKind],
    /// The guard's condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// The 「…时」 clause the counteraction prompt shows (locale key; `None` =
    /// no clause).
    pub counter_reason: Option<MsgKey>,
    /// Residual guard (docs/GUARDS.md §2/§4.3). `None` means G4 deleted it
    /// because the condition alone decides.
    pub guard: Option<fn(player_id: i32) -> bool>,
    /// The effect.
    pub body: fn(player_id: i32) -> Asked,
}

/// Shared by Settle / Mortgage / Redeem / Buy / Build -- the docs already say
/// "Same shape as On::Settle" for all of them.
#[derive(Clone, Copy)]
pub struct RuleEntry {
    /// Condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// Residual guard. `None` = G4-deleted (condition-only entry).
    pub guard: Option<fn(player_id: i32) -> bool>,
    /// The effect.
    pub body: fn(player_id: i32) -> Asked,
}

/// Cross-card message entry ([`On::Message`]).
#[derive(Clone, Copy)]
pub struct MessageEntry {
    /// Message names this card answers (`ctx::send`); empty = never dispatched.
    pub names: &'static [&'static str],
    /// Condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// Residual guard. `None` = G4-deleted (condition-only entry).
    pub guard: Option<fn(player_id: i32) -> bool>,
    /// The effect.
    pub body: fn(player_id: i32) -> Asked,
}

/// One entry point of a card. The variant says what the
/// entry is for, and its kind list is typed to match -- a counteraction can only be
/// declared at a [`abi::ChainKind`], an event reaction at a [`abi::ActionKind`],
/// a modifier/gate at a [`abi::FilterKind`]. An empty list is never dispatched.
///
/// Construction goes through the [`On::Play`] / [`On::Counteract`] / … `const fn`s
/// (same spelling as the old tuple variants). Those cannot share a name with an
/// enum variant, so the variants are the `*Body` arms holding a named-field
/// entry ([`PlayEntry`] / [`CounteractEntry`] / [`RuleEntry`] /
/// [`MessageEntry`] / [`ActionEntry`] / [`FilterEntry`]). Match on the
/// `*Body` names and read named fields; declare with the const fns.
///
/// WordPress `do_action` / `apply_filters` split (ABI v73):
/// [`On::Action`] is an **event reaction** -- it may emit effects (`ctx::gain`,
/// log, place, …) and does not modify the triggering value. [`On::Filter`] is
/// a **value modifier** at a modifier hook kind ([`abi::TriggerKind::is_filter`]:
/// `PaySplitAdd` / `PaySplitMul` / `BuySet` / `TurnBaseTime` / …). A kind list may not
/// mix the two; a mixed declaration is split into one Action and one Filter
/// entry (kind/variant agreement is enforced at ruleset build).
///
/// Guarded entries (`Play` gate / `Counteract` / `Action` / `Filter*` / `Gate` /
/// `Settle`) carry a **condition** string (`pre`)
/// immediately before the residual guard -- category → condition → guard → body
/// (docs/GUARDS.md §4.3, the three-layer model). `""` = no condition. Sugar:
/// [`pre::MINE`] (`actor == owner`); or attach one with [`On::pre`]. The
/// condition is compiled once at ruleset build (host) and evaluated natively
/// before the wasm guard is instantiated.
#[derive(Clone, Copy)]
pub enum On {
    /// Play this card from hand. Built by [`On::Play`].
    PlayBody(PlayEntry),
    /// A [反击] at a set of chain links. Built by [`On::Counteract`].
    CounteractBody(CounteractEntry),
    /// An **event reaction** at one field-hook kind: runs automatically
    /// while the card is in play. Same body shape as [`On::Counteract`]. May
    /// emit effects; does not modify the triggering value. One kind per entry
    /// -- a card that listens at two raises declares two entries. Built from
    /// the `action_kinds!` constructors (`Action::TurnStart(pre, reason, guard, body)`).
    Action(ActionEntry),
    /// **Pure value / decision / gate / payload rewrite** at one filter kind.
    /// Built from the `filter_kinds!` constructors
    /// (`Filter::DiceRollPlan(pre, guard, body)`). The entry's [`FilterShape`]
    /// names the body type:
    /// - [`FilterShape::Value`] -- `fn(i32, i32) -> i32` scalar fold
    /// - [`FilterShape::Flag`] -- `fn(i32, i32) -> bool` decision
    /// - [`FilterShape::Gate`] -- `fn(i32, GateValue) -> GateValue`
    /// - [`FilterShape::Payload`] -- `fn(i32, P) -> P` typed rewrite
    ///
    /// Pure: copy, tweak, return. No prompts, no `ctx::gain`.
    Filter(FilterEntry),
    /// The rule's **settle body** (`docs/TILES.md`) -- what runs when the tile
    /// this rule instance governs is [结算]d. Tile rules (`tile:*`) are one
    /// `CardDef` per tile kind and this is their body; the engine keeps
    /// rent / buy / build / draw-event as `ctx` primitives so it remains thin.
    /// Runs inside the settle chain: a counteraction to the settle link, or a
    /// field hook that replaces the body, shapes whether and how it runs. Same
    /// shape as [`On::Action`]; the condition sees the settling tile / actor.
    /// Built by [`On::Settle`].
    SettleBody(RuleEntry),
    /// The rule's **mortgage body** -- what runs when the deed this rule
    /// instance governs is [抵押]d (net `mortgage`, pay-pipeline raise-funds /
    /// optional funding, or a card-driven mortgage). Same shape as
    /// [`On::Settle`]. The body owns eligibility (规则书 基础规则 5: 「运营阶段
    /// 及需[支付]或[消耗]资金且资金不足时可以选择抵押拥有的地契」) and the
    /// payout amount; the engine keeps only the flag write + direct money move
    /// via [`crate::ctx::apply_mortgage`]. Built by [`On::Mortgage`].
    MortgageBody(RuleEntry),
    /// The rule's **redeem body** -- what runs when the deed is [赎回]d
    /// (规则书 基础规则 6: 「运营阶段可赎回拥有的地契」). Same shape as
    /// [`On::Mortgage`]; the engine keeps only the flag clear + direct money
    /// move via [`crate::ctx::apply_redeem`]. Built by [`On::Redeem`].
    RedeemBody(RuleEntry),
    /// The rule's **buy body** -- what runs when the deed this rule instance
    /// governs is bought (settle offer, card-driven `ctx::buy`, agent branch,
    /// acquire / auction / card kinds). Same shape as [`On::Mortgage`]. The
    /// body owns eligibility, the quote, and the order of steps (hooks, pay,
    /// deed write, log); the engine keeps the money pipeline and deed write as
    /// `ctx` primitives (`ctx::pay_buy`, `ctx::assign_deed`). Force-buy money
    /// remains a direct transfer outside effects (规则书 其他规则 6).
    /// Built by [`On::Buy`].
    BuyBody(RuleEntry),
    /// The rule's **build body** -- what runs when a house is built on the deed
    /// this rule instance governs (规则书 「可选择[消耗]…进行升级建造」). Same
    /// shape as [`On::Buy`]; the engine keeps the money pipeline and house
    /// write as `ctx` primitives (`ctx::pay_build`, `ctx::add_house`).
    /// Built by [`On::Build`].
    BuildBody(RuleEntry),
    /// A **cross-card message** this card answers (`ctx::send`). The names list
    /// is the message names it answers (empty is never dispatched).
    /// Same condition / residual guard / body shape as [`On::Action`]. The
    /// handler reads the sender and payload from [`ctx::message`] and returns
    /// a reply value via `ctx::message::reply`. Handling a message does not
    /// flash the card (internal plumbing). Built by [`On::Message`].
    MessageBody(MessageEntry),
}

/// Condition-authoring sugar (docs/GUARDS.md §4.3). The strings are CEL; the
/// schema is the §4.2 window/candidate vocabulary, compiled at ruleset build.
pub mod pre {
    /// `actor == owner` -- the 118 `mine` guards. Turns a same-seat-only
    /// guard into a condition plus (at most) a residual guard.
    pub const MINE: &str = "actor == owner";
}

impl On {
    /// Play this card from hand.
    ///
    /// `pre` is the gate's condition (see the type doc). `counter_reason` is
    /// the 「…时」 clause the counteraction prompt shows (locale key; `None` =
    /// no clause). `why_not` is the gate (the old `On::CantPlay`): a pure query
    /// with no prompts, `None` = playable, `Some(why)` = blocked and `why` is
    /// the reason to show. `None` for the gate itself means no gate. `body` is
    /// the effect.
    #[allow(non_snake_case)]
    pub const fn Play(
        pre: &'static str,
        counter_reason: Option<MsgKey>,
        why_not: Option<fn(player_id: i32) -> Option<Msg>>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::PlayBody(PlayEntry {
            pre,
            counter_reason,
            why_not,
            body,
        })
    }

    /// A [反击] at these chain links: the guard decides whether the card is
    /// offered in the hand window, then the effect resolves.
    #[allow(non_snake_case)]
    pub const fn Counteract(
        links: &'static [abi::ChainKind],
        pre: &'static str,
        counter_reason: Option<MsgKey>,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::CounteractBody(CounteractEntry {
            links,
            pre,
            counter_reason,
            guard,
            body,
        })
    }

    /// The rule's **settle body** (`docs/TILES.md`).
    #[allow(non_snake_case)]
    pub const fn Settle(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::SettleBody(RuleEntry { pre, guard, body })
    }

    /// The rule's **mortgage body**.
    #[allow(non_snake_case)]
    pub const fn Mortgage(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::MortgageBody(RuleEntry { pre, guard, body })
    }

    /// The rule's **redeem body**.
    #[allow(non_snake_case)]
    pub const fn Redeem(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::RedeemBody(RuleEntry { pre, guard, body })
    }

    /// The rule's **buy body**.
    #[allow(non_snake_case)]
    pub const fn Buy(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::BuyBody(RuleEntry { pre, guard, body })
    }

    /// The rule's **build body**.
    #[allow(non_snake_case)]
    pub const fn Build(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::BuildBody(RuleEntry { pre, guard, body })
    }

    /// A **cross-card message** this card answers (`ctx::send`).
    #[allow(non_snake_case)]
    pub const fn Message(
        names: &'static [&'static str],
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::MessageBody(MessageEntry {
            names,
            pre,
            guard,
            body,
        })
    }

    /// Replace the entry's condition string (docs/GUARDS.md §4.3). A const
    /// builder so a declaration can remain a one-liner:
    ///
    /// ```ignore
    /// On::Counteract(&[ChainKind::DiceRollResult], "", Some(key!("yolo_pay")), Some(can_counteract), counteract).pre(pre::MINE)
    /// ```
    pub const fn pre(self, pre: &'static str) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.pre = pre;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.pre = pre;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.pre = pre;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.pre = pre;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.pre = pre;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.pre = pre;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.pre = pre;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.pre = pre;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.pre = pre;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.pre = pre;
                On::MessageBody(e)
            }
        }
    }

    /// Delete the residual guard (G4): the condition alone decides. The host
    /// then skips the `OP_GUARD` instantiation entirely (`admits_pre`).
    pub const fn no_guard(self) -> Self {
        match self {
            On::CounteractBody(mut e) => {
                e.guard = None;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.guard = None;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.guard = None;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.guard = None;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.guard = None;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.guard = None;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.guard = None;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.guard = None;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.guard = None;
                On::MessageBody(e)
            }
            other => other,
        }
    }

    /// The residual guard, if any. `None` = G4-deleted (condition-only entry).
    /// For `On::Play` this is the gate (`WhyNot`), which has a different
    /// signature -- use [`Self::has_guard`] for the host's skip-the-wasm test.
    pub const fn guard(&self) -> Option<fn(i32) -> bool> {
        match self {
            On::CounteractBody(e) => e.guard,
            On::SettleBody(e) => e.guard,
            On::MortgageBody(e) => e.guard,
            On::RedeemBody(e) => e.guard,
            On::BuyBody(e) => e.guard,
            On::BuildBody(e) => e.guard,
            On::MessageBody(e) => e.guard,
            On::Action(e) => e.guard,
            On::Filter(e) => e.guard,
            _ => None,
        }
    }

    /// Does this entry have a residual wasm guard / gate the host must call?
    /// `false` = G4-deleted: the condition alone decides (`admits_pre`).
    pub const fn has_guard(&self) -> bool {
        match self {
            On::PlayBody(e) => e.why_not.is_some(),
            On::CounteractBody(e) => e.guard.is_some(),
            On::SettleBody(e) => e.guard.is_some(),
            On::MortgageBody(e) => e.guard.is_some(),
            On::RedeemBody(e) => e.guard.is_some(),
            On::BuyBody(e) => e.guard.is_some(),
            On::BuildBody(e) => e.guard.is_some(),
            On::MessageBody(e) => e.guard.is_some(),
            On::Action(e) => e.guard.is_some(),
            On::Filter(e) => e.guard.is_some(),
        }
    }

    /// The entry's condition source (`""` = none). Compiled by the host at
    /// ruleset build (`docs/GUARDS.md` §4.3).
    pub const fn condition(&self) -> &'static str {
        match self {
            On::PlayBody(e) => e.pre,
            On::CounteractBody(e) => e.pre,
            On::SettleBody(e) => e.pre,
            On::MortgageBody(e) => e.pre,
            On::RedeemBody(e) => e.pre,
            On::BuyBody(e) => e.pre,
            On::BuildBody(e) => e.pre,
            On::MessageBody(e) => e.pre,
            On::Action(e) => e.pre,
            On::Filter(e) => e.pre,
        }
    }

    /// The trigger's `counterReason` -- the 「…时」 clause the counteraction
    /// prompt shows. A locale key the client resolves; `None` for entries that
    /// carry no clause.
    pub const fn counter_reason(&self) -> Option<MsgKey> {
        match self {
            On::PlayBody(e) => e.counter_reason,
            On::CounteractBody(e) => e.counter_reason,
            On::Action(e) => e.counter_reason,
            _ => None,
        }
    }

    pub const fn kind(&self) -> abi::OnKind {
        match self {
            On::PlayBody(..) => abi::OnKind::Play,
            On::CounteractBody(..) => abi::OnKind::Counteract,
            On::Action(..) => abi::OnKind::Action,
            On::Filter(..) => abi::OnKind::Filter,
            On::SettleBody(..) => abi::OnKind::Settle,
            On::MortgageBody(..) => abi::OnKind::Mortgage,
            On::RedeemBody(..) => abi::OnKind::Redeem,
            On::BuyBody(..) => abi::OnKind::Buy,
            On::BuildBody(..) => abi::OnKind::Build,
            On::MessageBody(..) => abi::OnKind::Message,
        }
    }

    /// The wire **spellings** this entry answers (empty for non-trigger
    /// entries): `TriggerKind::spelling` -- engine camelCase or
    /// `"r:{key}:{id}"`. `On::Action` / `On::Filter` are one kind each.
    pub fn triggers(&self) -> Vec<String> {
        match self {
            On::CounteractBody(e) => e.links.iter().map(|x| x.spelling().into_owned()).collect(),
            On::Action(e) => {
                let mut v = Vec::new();
                v.push(e.kind.spelling().into_owned());
                v
            }
            On::Filter(e) => {
                let mut v = Vec::new();
                v.push(e.kind.spelling().into_owned());
                v
            }
            // Message names ride a separate manifest field; see `rt.rs`.
            On::MessageBody(..) => Vec::new(),
            _ => Vec::new(),
        }
    }

    /// The message names this [`On::Message`] entry answers (empty otherwise).
    pub fn message_names(&self) -> Vec<&'static str> {
        match self {
            On::MessageBody(e) => e.names.to_vec(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::TriggerKind;

    /// Docs list every filter-pipeline kind as `Filter::KINDS` and every
    /// event / hook kind as `Action::KINDS`. Walk the registries the
    /// `filter_kinds!` / `action_kinds!` macros emit -- the old hardcoded
    /// const lists drifted every time a kind was added.
    #[test]
    fn is_filter_set_matches_docs() {
        for k in Filter::KINDS {
            assert!(k.is_filter, "Filter::{} KindInfo.is_filter", k.name);
            assert!(k.kind.is_filter(), "Filter::{} must be is_filter", k.name);
        }
        for k in Action::KINDS {
            assert!(!k.is_filter, "Action::{} KindInfo.is_filter", k.name);
            assert!(!k.kind.is_filter(), "Action::{} must be !is_filter", k.name);
        }
        // Count pin: the docs name 27 filter kinds (the 10 gates + the
        // modifier stages). Adding one means the docs table moves too.
        assert_eq!(Filter::KINDS.len(), 27, "Filter::KINDS row count");
    }

    /// Pin the typed sides against `TriggerKind::is_filter()` (the single
    /// declaration source of truth for **engine** kinds) and against the
    /// registry's own `KindInfo::is_filter`. Walk `Filter::KINDS` /
    /// `Action::KINDS` -- the macro table is the declaration, and every row
    /// already references its `FilterKind::$name` / `ActionKind::$name` const,
    /// so a wrong-side const fails here and at the constructor signature.
    ///
    /// Ruleset kinds (`bandori_std::kind::{Actions, Filters}`) are pinned in
    /// `crates/game-rules/tests/kind_table_pin.rs` (this crate cannot see
    /// them). `TriggerKind::is_filter()` answers `false` for every ruleset id
    /// -- the registration's `KindInfo` / constructor type is authoritative
    /// there.
    #[test]
    fn typed_kind_const_sides_match_is_filter() {
        for k in Filter::KINDS {
            assert!(k.is_filter, "Filter::{} KindInfo.is_filter", k.name);
            assert!(k.kind.is_filter(), "Filter::{} must be is_filter()", k.name);
            assert!(
                !k.spelling().is_empty(),
                "Filter::{} has empty wire spelling",
                k.name
            );
            assert_eq!(
                k.wire,
                game_kinds::wire(k.kind.id),
                "Filter::{} wire row",
                k.name
            );
        }
        for k in Action::KINDS {
            assert!(!k.is_filter, "Action::{} KindInfo.is_filter", k.name);
            assert!(
                !k.kind.is_filter(),
                "Action::{} must be !is_filter()",
                k.name
            );
            // `None`'s wire name is "" (unset); every other const has one.
            if k.name != "None" {
                assert!(
                    !k.spelling().is_empty(),
                    "Action::{} has empty wire spelling",
                    k.name
                );
            }
            if k.name != "None" {
                assert_eq!(
                    k.wire,
                    game_kinds::wire(k.kind.id),
                    "Action::{} wire row",
                    k.name
                );
            }
        }
    }

    /// The constructor type pins the side: `Filter::KINDS` is all `is_filter`
    /// and `Action::KINDS` is all `!is_filter`. (The old `check_kind_variant`
    /// runtime assert -- and its raw-id "smuggle" list-form setup, which built
    /// `On::Filter(&[FilterKind(TriggerKind::engine(45))], …)` to prove the
    /// runtime check fired -- are **deleted**. A wrong-side *const* is now a
    /// rustc error at the constructor signature; a raw `FilterKind(t)` can
    /// still be built, but nothing in the `On` DSL takes a bare list anymore,
    /// so there is no call site to smuggle through.)
    #[test]
    fn kind_variant_agreement() {
        for k in Filter::KINDS {
            assert!(k.kind.is_filter(), "Filter::{} must be is_filter", k.name);
        }
        for k in Action::KINDS {
            assert!(!k.kind.is_filter(), "Action::{} must be !is_filter", k.name);
        }
    }

    #[test]
    fn on_kind_ids() {
        assert_eq!(abi::OnKind::Action as i32, 3);
        assert_eq!(abi::OnKind::Filter as i32, 11);
        assert_eq!(abi::OnKind::Mortgage as i32, 9);
        assert_eq!(abi::OnKind::Redeem as i32, 10);
        assert_eq!(abi::OnKind::Buy as i32, 12);
        assert_eq!(abi::OnKind::Build as i32, 13);
        assert_eq!(abi::OnKind::from_i32(3), Some(abi::OnKind::Action));
        assert_eq!(abi::OnKind::from_i32(9), Some(abi::OnKind::Mortgage));
        assert_eq!(abi::OnKind::from_i32(10), Some(abi::OnKind::Redeem));
        assert_eq!(abi::OnKind::from_i32(11), Some(abi::OnKind::Filter));
        assert_eq!(abi::OnKind::from_i32(12), Some(abi::OnKind::Buy));
        assert_eq!(abi::OnKind::from_i32(13), Some(abi::OnKind::Build));
        assert_eq!(abi::OnKind::from_i32(1), None);
        // Retired AtEnd = 4 / DiceRollPlan = 5 / Gate = 6 -- never reuse.
        assert_eq!(abi::OnKind::from_i32(4), None);
        assert_eq!(abi::OnKind::from_i32(5), None);
        assert_eq!(abi::OnKind::from_i32(6), None);
    }

    #[test]
    fn trigger_kind_ids() {
        // TriggerKind ids are opaque; pin the retired wire names and holes.
        // FlipEffect = 94 / FlipAfter = 104 retired in v83 -- never reuse.
        assert_eq!(TriggerKind::engine(94).spelling(), "");
        assert_eq!(TriggerKind::engine(104).spelling(), "");
        assert_eq!(TriggerKind::from_str("flipEffect"), TriggerKind::None);
        assert_eq!(TriggerKind::from_str("flipAfter"), TriggerKind::None);
        // Ids 16 / 17 / 19 / 20 / 22 (retired abnormal / force kinds) retired in
        // v84 -- never reuse.
        assert_eq!(TriggerKind::engine(16).spelling(), "");
        assert_eq!(TriggerKind::engine(17).spelling(), "");
        assert_eq!(TriggerKind::engine(19).spelling(), "");
        assert_eq!(TriggerKind::engine(20).spelling(), "");
        assert_eq!(TriggerKind::engine(22).spelling(), "");
        assert_eq!(TriggerKind::from_str("stop"), TriggerKind::None);
        assert_eq!(TriggerKind::from_str("teleport"), TriggerKind::None);
        assert_eq!(TriggerKind::from_str("retiredAb19"), TriggerKind::None);
        assert_eq!(TriggerKind::from_str("retiredAb20"), TriggerKind::None);
        assert_eq!(TriggerKind::from_str("forced"), TriggerKind::None);
        // Live neighbours still round-trip.
        assert_eq!(TriggerKind::from_str("drewBefore").spelling(), "drewBefore");
        assert_eq!(TriggerKind::from_str("atEnd").spelling(), "atEnd");
        assert_eq!(
            TriggerKind::from_str("paySplitChooseBefore").spelling(),
            "paySplitChooseBefore"
        );
        assert_eq!(
            TriggerKind::from_str("paySplitMulBefore").spelling(),
            "paySplitMulBefore"
        );
        assert_eq!(TriggerKind::PaySplitMulBefore.id, 110);
    }
}
