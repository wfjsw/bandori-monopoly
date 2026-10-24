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
pub mod text;

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
/// Re-exported so a card can `use card_sdk::serde;` and derive
/// [`ActivationArg`](crate::ActivationArg)'s bounds on its own activation
/// struct without adding a direct `serde` dependency.
pub use serde;
pub use msg::Msg;
pub use text::{
    it, it_sub, items, para, sec, steps, CardText, TextBlock, TextItem, TextPiece, TextSection,
};

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

/// A typed activation argument. Blanket-impl'd for any postcard record.
///
/// `T` is the **card's own** struct -- a pick, a tile, a card, a seat, a count
/// -- whatever that activation decides. Nothing about it is shared with other
/// cards: the collector and the body live in one file and share the type, and
/// only `postcard` crosses (docs/ACTIVATION.md §2).
pub trait ActivationArg: serde::Serialize + serde::de::DeserializeOwned + Default {}
impl<T: serde::Serialize + serde::de::DeserializeOwned + Default> ActivationArg for T {}

/// What a `declare` collector returns.
///
/// A two-arm rather than `Option<T>` so "declined" is never confused with
/// "proceed": `Decline` is the entry not firing at all (docs/ACTIVATION.md §2).
/// `T = i32` is the default, so the common "which of N" case keeps its
/// zero-ceremony spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation<T: ActivationArg = i32> {
    /// The player declined. The entry does not fire at all -- no body, no
    /// header, no flash, no log. For a [反击] this is "do not declare"; for a
    /// play it is "the card never left the hand"; for a hook it is the
    /// 「可选择」 opt-out.
    Decline,
    /// Proceed; the body receives `T` as its second parameter.
    Arg(T),
}

/// Collect the activation argument **before** the activation commits.
///
/// `Err(Prompt)` pauses and re-runs, same multi-pass as [`Asked`]. Runs at the
/// offer layer, before the point of no return (docs/ACTIVATION.md §2).
pub type Declared<T: ActivationArg = i32> = Result<Activation<T>, Prompt>;

/// Monomorphized collector driver: casts the erased collector back to
/// `fn(i32) -> Declared<T>` and folds it to the wire
/// (`EXIT_DECLINED` / `EXIT_NEED_INPUT` / packed `(ptr<<32)|len` of postcard).
///
/// The cast lives only in [`drive_declare`] (the [`crate::payload`] rule: only
/// `drive` may cast).
pub type DeclareDrive = fn(i32, *const ()) -> i64;

/// Monomorphized body driver: casts `body` back to its fn type and runs it.
/// `(player_id, body) -> Asked`. A [`BodyShape::WithArg`] body pulls its `T`
/// with [`ctx::declared_arg`](crate::ctx::declared_arg) and passes it on, so
/// `T` is still a literal parameter of `fn(i32, T) -> Asked`.
///
/// The cast lives only in [`drive_body_plain`] / [`drive_body_arg`].
pub type BodyDrive = fn(i32, *const ()) -> Asked;

/// Which function type an entry's erased `body` pointer is (the
/// [`crate::kinds::FilterShape`] pattern, so the ~310 existing `fn(i32) ->
/// Asked` bodies compile unchanged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyShape {
    /// `fn(i32) -> Asked` -- no [`On::declare_arg`].
    Plain,
    /// `fn(i32, T) -> Asked` -- second parameter is the [`Activation::Arg`].
    WithArg,
}

/// Fold one `fn(i32) -> Declared<T>` collector to the wire. The returned `i64`
/// is `EXIT_DECLINED`, `EXIT_NEED_INPUT`, or packed `(ptr<<32)|len` of
/// `postcard(T)` -- a buffer address, so no sentinel collision is possible.
pub fn drive_declare<T: ActivationArg>(player_id: i32, decl: *const ()) -> i64 {
    // SAFETY: `decl` was packed from a `fn(i32) -> Declared<T>` and is never
    // reinterpreted as another `T`.
    let f: fn(i32) -> Declared<T> = unsafe { core::mem::transmute(decl) };
    match f(player_id) {
        Ok(Activation::Decline) => crate::abi::EXIT_DECLINED as i64,
        Ok(Activation::Arg(v)) => {
            #[cfg(any(target_arch = "wasm32", feature = "guest"))]
            {
                let bytes = postcard::to_allocvec(&v).unwrap_or_default();
                crate::rt::leak(bytes)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
            {
                let _ = v;
                0
            }
        }
        Err(Prompt) => crate::abi::EXIT_NEED_INPUT as i64,
    }
}

/// `fn(i32) -> Asked`.
pub fn drive_body_plain(player_id: i32, body: *const ()) -> Asked {
    // SAFETY: `body` was packed from a `fn(i32) -> Asked` and is never
    // reinterpreted as another shape.
    let body: fn(i32) -> Asked = unsafe { core::mem::transmute(body) };
    body(player_id)
}

/// `fn(i32, T) -> Asked` -- the second parameter is the [`Activation::Arg`],
/// pulled from the host's per-activation store.
pub fn drive_body_arg<T: ActivationArg>(player_id: i32, body: *const ()) -> Asked {
    // SAFETY: `body` was packed from a `fn(i32, T) -> Asked` and is never
    // reinterpreted as another shape.
    let body: fn(i32, T) -> Asked = unsafe { core::mem::transmute(body) };
    #[cfg(any(target_arch = "wasm32", feature = "guest"))]
    {
        let arg = crate::ctx::declared_arg::<T>().unwrap_or_default();
        body(player_id, arg)
    }
    #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
    {
        // Declaration-only build (manifest / pin tests): never called.
        body(player_id, T::default())
    }
}

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
///     On::Play("", None, play),
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
    /// flash policy. Both ride the entry itself ([`On::silent`] /
    /// [`On::quiet_when_empty`] / [`On::flash`]), so they cannot drift when
    /// entries are reordered. An unknown index is
    /// [`abi::Presentation::Visible`] / [`abi::Flash::Auto`]. The one lookup.
    pub const fn presentation(&self, index: i32) -> crate::abi::EntryPresentation {
        let mut header = crate::abi::Presentation::Visible;
        let mut flash = crate::abi::Flash::Auto;
        let mut i = 0;
        while i < self.on.len() {
            if i as i32 == index {
                header = self.on[i].header_of();
                flash = self.on[i].flash_of();
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
        while i < self.on.len() {
            if i as i32 == index {
                return match self.on[i].header_of() {
                    crate::abi::Presentation::Silent => true,
                    _ => false,
                };
            }
            i += 1;
        }
        false
    }
}

/// Play-from-hand entry ([`On::Play`]).
#[derive(Clone, Copy)]
pub struct PlayEntry {
    /// Stable **internal** identifier for cross-card applicability queries
    /// ([`On::id`], `ctx::query_guard`, docs/ACTIVATION.md §3). An ASCII slug
    /// (`[A-Za-z0-9_][A-Za-z0-9_.:-]*`) -- never display text; that is
    /// [`Self::label`]. `""` = not externally addressable (the common case,
    /// free). Unique per card, not globally.
    pub id: &'static str,
    /// Gate condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// The 「…时」 clause the counteraction prompt shows (locale key; `None` =
    /// no clause).
    pub counter_reason: Option<MsgKey>,
    /// The gate (the old `On::CantPlay`): a pure query with no prompts,
    /// `None` = playable, `Some(why)` = blocked and `why` is the reason to
    /// show. `None` for the gate itself means no gate.
    pub why_not: Option<fn(player_id: i32) -> Option<Msg>>,
    /// The effect, erased. See [`BodyShape`] / [`BodyDrive`].
    pub body: *const (),
    /// Monomorphized driver; casts `body` back to its fn type.
    pub drive: BodyDrive,
    /// Which fn type `body` is.
    pub shape: BodyShape,
    /// Activation-argument collector ([`On::declare`], docs/ACTIVATION.md §2).
    /// Runs before the point of no return; `Activation::Arg` becomes the
    /// body's second parameter. `None` = no declare (body is [`BodyShape::Plain`]).
    pub declare: Option<*const ()>,
    pub declare_drive: Option<DeclareDrive>,
    /// Opaque display text for the 「效果适用」 line (「（1）」 and kin). Moves
    /// with the entry; `""` = the line shows just the card name. **Not** a key
    /// into the card text -- nothing resolves this against [`crate::text::CardText`].
    /// If the line needs prose, put a copy of it here.
    pub label: &'static str,
    /// Per-entry flash policy ([`On::flash`]). Whether the activation's centre
    /// flash (and field-slot pulse) shows: [`abi::Flash::Auto`] (the default)
    /// lets the host classify by scope (owner-self-only ⇒ no flash);
    /// [`abi::Flash::Always`] / [`abi::Flash::Never`] are the rule's own
    /// opinion and win over that classification. Body code can still override
    /// at runtime via `ctx::flash` / `ctx::no_flash` under the precedence
    /// documented on [`abi::Flash`].
    pub flash: crate::abi::Flash,
    /// How this entry's 「效果适用」 header presents ([`On::silent`] /
    /// [`On::quiet_when_empty`]). [`abi::Presentation::Visible`] (the default)
    /// announces normally; [`abi::Presentation::Silent`] is bookkeeping that
    /// posts no header and no flash; [`abi::Presentation::QuietWhenEmpty`]
    /// retracts the header when the body produced nothing visible.
    pub header: crate::abi::Presentation,
}

/// [反击] entry ([`On::Counteract`]).
#[derive(Clone, Copy)]
pub struct CounteractEntry {
    /// See [`PlayEntry::id`].
    pub id: &'static str,
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
    /// The effect, erased. See [`PlayEntry::body`].
    pub body: *const (),
    /// See [`PlayEntry::drive`].
    pub drive: BodyDrive,
    /// See [`PlayEntry::shape`].
    pub shape: BodyShape,
    /// See [`PlayEntry::declare`]. Erased; see [`DeclareDrive`].
    pub declare: Option<*const ()>,
    /// See [`PlayEntry::declare_drive`].
    pub declare_drive: Option<DeclareDrive>,
    /// See [`PlayEntry::label`].
    pub label: &'static str,
    /// See [`PlayEntry::flash`].
    pub flash: crate::abi::Flash,
    /// See [`PlayEntry::header`].
    pub header: crate::abi::Presentation,
}

/// Shared by Settle / Mortgage / Redeem / Buy / Build -- the docs already say
/// "Same shape as On::Settle" for all of them.
#[derive(Clone, Copy)]
pub struct RuleEntry {
    /// See [`PlayEntry::id`].
    pub id: &'static str,
    /// Condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// Residual guard. `None` = G4-deleted (condition-only entry).
    pub guard: Option<fn(player_id: i32) -> bool>,
    /// The effect, erased. See [`PlayEntry::body`].
    pub body: *const (),
    /// See [`PlayEntry::drive`].
    pub drive: BodyDrive,
    /// See [`PlayEntry::shape`].
    pub shape: BodyShape,
    /// See [`PlayEntry::declare`]. Erased; see [`DeclareDrive`].
    pub declare: Option<*const ()>,
    /// See [`PlayEntry::declare_drive`].
    pub declare_drive: Option<DeclareDrive>,
    /// See [`PlayEntry::label`].
    pub label: &'static str,
    /// See [`PlayEntry::flash`].
    pub flash: crate::abi::Flash,
    /// See [`PlayEntry::header`].
    pub header: crate::abi::Presentation,
}

/// Cross-card message entry ([`On::Message`]).
#[derive(Clone, Copy)]
pub struct MessageEntry {
    /// See [`PlayEntry::id`].
    pub id: &'static str,
    /// Message names this card answers (`ctx::send`); empty = never dispatched.
    pub names: &'static [&'static str],
    /// Condition (docs/GUARDS.md §4.3). `""` = no condition.
    pub pre: &'static str,
    /// Residual guard. `None` = G4-deleted (condition-only entry).
    pub guard: Option<fn(player_id: i32) -> bool>,
    /// The effect, erased. See [`PlayEntry::body`].
    pub body: *const (),
    /// See [`PlayEntry::drive`].
    pub drive: BodyDrive,
    /// See [`PlayEntry::shape`].
    pub shape: BodyShape,
    /// See [`PlayEntry::declare`]. Erased; see [`DeclareDrive`].
    pub declare: Option<*const ()>,
    /// See [`PlayEntry::declare_drive`].
    pub declare_drive: Option<DeclareDrive>,
    /// See [`PlayEntry::label`].
    pub label: &'static str,
    /// See [`PlayEntry::flash`].
    pub flash: crate::abi::Flash,
    /// See [`PlayEntry::header`].
    pub header: crate::abi::Presentation,
}

// The `body` pointer is code, not data: immutable and never freed (the same
// contract [`crate::kinds::FilterEntry`] states). Safe to share across threads.
unsafe impl Send for PlayEntry {}
unsafe impl Sync for PlayEntry {}
unsafe impl Send for CounteractEntry {}
unsafe impl Sync for CounteractEntry {}
unsafe impl Send for RuleEntry {}
unsafe impl Sync for RuleEntry {}
unsafe impl Send for MessageEntry {}
unsafe impl Sync for MessageEntry {}

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
    /// `pre` is the gate's condition (see the type doc). `why_not` is the gate
    /// (the old `On::CantPlay`): a pure query with no prompts, `None` =
    /// playable, `Some(why)` = blocked and `why` is the reason to show. `None`
    /// for the gate itself means no gate. `body` is the effect. Attach the
    /// 「…时」 clause with [`Self::counter_reason`].
    #[allow(non_snake_case)]
    pub const fn Play(
        pre: &'static str,
        why_not: Option<fn(player_id: i32) -> Option<Msg>>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::PlayBody(PlayEntry {
            id: "",
            pre,
            counter_reason: None,
            why_not,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// A [反击] at these chain links: the guard decides whether the card is
    /// offered in the hand window, then the effect resolves. Attach the 「…时」
    /// clause with [`Self::counter_reason`].
    #[allow(non_snake_case)]
    pub const fn Counteract(
        links: &'static [abi::ChainKind],
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::CounteractBody(CounteractEntry {
            id: "",
            links,
            pre,
            counter_reason: None,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// The rule's **settle body** (`docs/TILES.md`).
    #[allow(non_snake_case)]
    pub const fn Settle(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::SettleBody(RuleEntry {
            id: "",
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// The rule's **mortgage body**.
    #[allow(non_snake_case)]
    pub const fn Mortgage(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::MortgageBody(RuleEntry {
            id: "",
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// The rule's **redeem body**.
    #[allow(non_snake_case)]
    pub const fn Redeem(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::RedeemBody(RuleEntry {
            id: "",
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// The rule's **buy body**.
    #[allow(non_snake_case)]
    pub const fn Buy(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::BuyBody(RuleEntry {
            id: "",
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
    }

    /// The rule's **build body**.
    #[allow(non_snake_case)]
    pub const fn Build(
        pre: &'static str,
        guard: Option<fn(player_id: i32) -> bool>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self {
        Self::BuildBody(RuleEntry {
            id: "",
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
        })
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
            id: "",
            names,
            pre,
            guard,
            body: body as *const (),
            drive: drive_body_plain,
            shape: BodyShape::Plain,
            declare: None,
            declare_drive: None,
            label: "",
            flash: crate::abi::Flash::Auto,
            header: crate::abi::Presentation::Visible,
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

    /// Attach an **activation-argument collector** (docs/ACTIVATION.md §2).
    /// Decline-only: the constructor's `fn(i32) -> Asked` body is unchanged and
    /// ignores the arg (it is always 0). Use [`Self::declare_arg`] when the
    /// body needs the pick.
    ///
    /// `pick` runs at the offer layer, **before** the point of no return
    /// (`hand.remove` for a play / a counter declaration; before announce for
    /// a hook). [`Activation::Decline`] means the entry does not fire at all --
    /// no body, no header, no flash, no log.
    ///
    /// ```ignore
    /// // 规则书[持续]（1）: 「可选择移除此卡的1个[奇迹水晶]」 -- the offer is
    /// // applicability: decline = the pay hook does not fire.
    /// On::Action(Action::PaySplitChooseBefore(pre, Some(key!("when")), None, pay))
    ///     .declare(pay_offer),
    /// ```
    ///
    /// **Not allowed on `On::Filter`** -- a filter body is a pure value fold
    /// (`docs/ACTIVATION.md` "On::Filter cannot declare"); pausing a
    /// `PaySplitMul` / `MoveGate` / `DiceRollPlan` mid-pipeline is the same
    /// mistake as letting a filter emit effects. This arm panics at compile
    /// time, and `build-ruleset` rejects it too.
    pub const fn declare<T: ActivationArg>(self, pick: fn(player_id: i32) -> Declared<T>) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::Action(e)
            }
            On::SettleBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.declare = Some(pick as *const ());
                e.declare_drive = Some(drive_declare::<T>);
                On::MessageBody(e)
            }
            // Fail closed at compile time: a filter is a pure value fold and
            // cannot pause for a player (docs/ACTIVATION.md §2).
            On::Filter(_) => {
                panic!("On::Filter cannot declare -- use an On::Action pre-stage instead")
            }
        }
    }

    /// Give this entry a stable **internal** identifier for cross-card
    /// applicability queries (`ctx::query_guard`, docs/ACTIVATION.md §3).
    /// `""` (the default) means not externally addressable. Ids are unique
    /// **per card**, not globally.
    ///
    /// **This is identity, not display text.** It is never shown to players.
    /// It must be an ASCII slug -- `[A-Za-z0-9_][A-Za-z0-9_.:-]*` -- which
    /// `build-ruleset` and `Ruleset::build` enforce. Rulebook wording belongs
    /// in [`Self::label`] ("（2）") or [`Self::counter_reason`]; renaming a
    /// clause must not silently break a query, and full-width vs half-width
    /// parens are different strings that look identical in a diff.
    ///
    /// ```ignore
    /// // the skill's press entry:
    /// On::Play("", Some(can_use), use_skill).id("press").label("（2）")
    /// // elsewhere -- the slug, never the label:
    /// ctx::query_guard(player_id, "skill:美竹兰:叛逆的红挑染", "press")
    /// ```
    pub const fn id(self, id: &'static str) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.id = id;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.id = id;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.id = id;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.id = id;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.id = id;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.id = id;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.id = id;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.id = id;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.id = id;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.id = id;
                On::MessageBody(e)
            }
        }
    }

    /// [`Self::declare`] plus an arg-taking body: `body` replaces the
    /// constructor's `fn(i32) -> Asked` and receives the [`Activation::Arg`]
    /// as its second parameter.
    ///
    /// ```ignore
    /// // detour: the four-way 「"afterglow"式的移动」 pick is part of declaring
    /// // the counter, so it runs here -- not in the body, after commit.
    /// On::Counteract(links, pre, None, Some(detour_eligible), unused)
    ///     .declare_arg(pick_afterglow_2, counteract),
    /// ```
    ///
    /// Same [`Self::declare`] rules: not allowed on `On::Filter`.
    pub const fn declare_arg<T: ActivationArg>(
        self,
        pick: fn(player_id: i32) -> Declared<T>,
        body: fn(player_id: i32, arg: T) -> Asked,
    ) -> Self {
        match self.declare(pick) {
            On::PlayBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::Action(e)
            }
            On::SettleBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.body = body as *const ();
                e.drive = drive_body_arg::<T>;
                e.shape = BodyShape::WithArg;
                On::MessageBody(e)
            }
            On::Filter(_) => {
                panic!("On::Filter cannot declare -- use an On::Action pre-stage instead")
            }
        }
    }

    /// Does this entry collect an activation argument before its body?
    /// `On::Filter` never does (and cannot -- see [`Self::declare`]).
    pub const fn has_declare(&self) -> bool {
        match self {
            On::PlayBody(e) => e.declare.is_some(),
            On::CounteractBody(e) => e.declare.is_some(),
            On::Action(e) => e.declare.is_some(),
            On::Filter(_) => false,
            On::SettleBody(e) => e.declare.is_some(),
            On::MortgageBody(e) => e.declare.is_some(),
            On::RedeemBody(e) => e.declare.is_some(),
            On::BuyBody(e) => e.declare.is_some(),
            On::BuildBody(e) => e.declare.is_some(),
            On::MessageBody(e) => e.declare.is_some(),
        }
    }

    /// Opaque display text for this entry's 「效果适用」 line (「（1）」 and kin).
    /// Attaches to the entry, so it moves with it; `""` (the default) shows
    /// just the card name.
    ///
    /// Not a key: nothing resolves this against [`crate::text::CardText`] or
    /// the rulebook text. If the line needs prose, **duplicate** it here.
    ///
    /// ```ignore
    /// On::Action(Action::Pass("", None, on_pass)).label("（1）")
    /// ```
    pub const fn label(self, label: &'static str) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.label = label;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.label = label;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.label = label;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.label = label;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.label = label;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.label = label;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.label = label;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.label = label;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.label = label;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.label = label;
                On::MessageBody(e)
            }
        }
    }

    /// Override this entry's **flash policy**. Attaches to the entry, so it
    /// rides along when entries are reordered -- no index to drift.
    ///
    /// [`abi::Flash::Auto`] (the default) lets the host classify by scope
    /// (owner-self-only ⇒ no flash). [`abi::Flash::Always`] /
    /// [`abi::Flash::Never`] are the rule's own opinion and win over that
    /// classification: `Always` flashes even an owner-self-only effect the
    /// player should notice; `Never` keeps the log entry but suppresses the
    /// flash / pulse even for a cross-player one. Body code can still override
    /// at runtime via `ctx::flash` / `ctx::no_flash` under the precedence
    /// documented on [`abi::Flash`].
    ///
    /// ```ignore
    /// pub const T: CardDef = CardDef::new("skill:…", &[
    ///     On::Play("", None, use_skill),                                    // Auto
    ///     On::Action(Action::TurnEnd("", None, tick)).flash(Flash::Always), // Always (self-only)
    /// ]);
    /// ```
    pub const fn flash(self, policy: crate::abi::Flash) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.flash = policy;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.flash = policy;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.flash = policy;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.flash = policy;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.flash = policy;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.flash = policy;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.flash = policy;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.flash = policy;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.flash = policy;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.flash = policy;
                On::MessageBody(e)
            }
        }
    }

    /// Mark this entry **silent** (bookkeeping): it runs its body but posts
    /// no activation flash and no 「效果适用」 header. Attaches to the entry,
    /// so it rides along when entries are reordered -- no index to drift.
    ///
    /// Use for declarations and resets the player never sees as a clause
    /// firing -- `declare_cap` / `set_bounds` / initial values / latch clears
    /// (docs/CARDS.md 「Silent bookkeeping entries」). An effect the rulebook
    /// describes as happening (gain, pay, place, move, draw, status, mark)
    /// must **not** be silent.
    ///
    /// Mutually exclusive with [`Self::quiet_when_empty`] -- last call wins.
    ///
    /// ```ignore
    /// pub const T: CardDef = CardDef::new("skill:…", &[
    ///     On::Play("", None, use_skill),                                    // flashes
    ///     On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
    ///     On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
    /// ]);
    /// ```
    pub const fn silent(self) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.header = crate::abi::Presentation::Silent;
                On::MessageBody(e)
            }
        }
    }

    /// Mark this entry **quiet when empty**: it runs normally but its
    /// 「效果适用」 header is retracted when the body produced nothing visible
    /// (no children, no logs). For clauses that apply and change nothing -- a
    /// gain already at its cap, 「已满」, a set that writes the same value. A
    /// body that logs / pays / moves still flashes. Do **not** use for an
    /// opt-in the player can decline (「放弃」 is information). Attaches to the
    /// entry, so it rides along when entries are reordered.
    ///
    /// Mutually exclusive with [`Self::silent`] -- last call wins.
    ///
    /// ```ignore
    /// On::Action(Action::PaySplitChooseBefore("", None, None, pay)).quiet_when_empty(),
    /// ```
    pub const fn quiet_when_empty(self) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::Action(e)
            }
            On::Filter(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::Filter(e)
            }
            On::SettleBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::SettleBody(e)
            }
            On::MortgageBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::MortgageBody(e)
            }
            On::RedeemBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::RedeemBody(e)
            }
            On::BuyBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::BuyBody(e)
            }
            On::BuildBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::BuildBody(e)
            }
            On::MessageBody(mut e) => {
                e.header = crate::abi::Presentation::QuietWhenEmpty;
                On::MessageBody(e)
            }
        }
    }

    /// This entry's 「效果适用」 display text (see [`Self::label`]); `""` = none.
    pub const fn label_of(&self) -> &'static str {
        match self {
            On::PlayBody(e) => e.label,
            On::CounteractBody(e) => e.label,
            On::Action(e) => e.label,
            On::Filter(e) => e.label,
            On::SettleBody(e) => e.label,
            On::MortgageBody(e) => e.label,
            On::RedeemBody(e) => e.label,
            On::BuyBody(e) => e.label,
            On::BuildBody(e) => e.label,
            On::MessageBody(e) => e.label,
        }
    }

    /// This entry's flash policy (see [`Self::flash`]); [`abi::Flash::Auto`] =
    /// host classifies by scope.
    pub const fn flash_of(&self) -> crate::abi::Flash {
        match self {
            On::PlayBody(e) => e.flash,
            On::CounteractBody(e) => e.flash,
            On::Action(e) => e.flash,
            On::Filter(e) => e.flash,
            On::SettleBody(e) => e.flash,
            On::MortgageBody(e) => e.flash,
            On::RedeemBody(e) => e.flash,
            On::BuyBody(e) => e.flash,
            On::BuildBody(e) => e.flash,
            On::MessageBody(e) => e.flash,
        }
    }

    /// This entry's 「效果适用」 header presentation (see [`Self::header`]);
    /// [`abi::Presentation::Visible`] announces normally.
    pub const fn header_of(&self) -> crate::abi::Presentation {
        match self {
            On::PlayBody(e) => e.header,
            On::CounteractBody(e) => e.header,
            On::Action(e) => e.header,
            On::Filter(e) => e.header,
            On::SettleBody(e) => e.header,
            On::MortgageBody(e) => e.header,
            On::RedeemBody(e) => e.header,
            On::BuyBody(e) => e.header,
            On::BuildBody(e) => e.header,
            On::MessageBody(e) => e.header,
        }
    }

    /// This entry's stable internal identifier (see [`Self::id`]); `""` = not
    /// externally addressable. Never display text -- that is [`Self::label_of`].
    pub const fn id_of(&self) -> &'static str {
        match self {
            On::PlayBody(e) => e.id,
            On::CounteractBody(e) => e.id,
            On::Action(e) => e.id,
            On::Filter(e) => e.id,
            On::SettleBody(e) => e.id,
            On::MortgageBody(e) => e.id,
            On::RedeemBody(e) => e.id,
            On::BuyBody(e) => e.id,
            On::BuildBody(e) => e.id,
            On::MessageBody(e) => e.id,
        }
    }

    /// Attach the 「…时」 clause the counteraction prompt shows (a locale key).
    /// Entries that carry a clause are [反击] windows ([`On::Play`] /
    /// [`On::Counteract`]) and field hooks ([`On::Action`]); on any other entry
    /// this is a no-op. `None` (the default) = no clause.
    ///
    /// ```ignore
    /// On::Counteract(&[ChainKind::DiceRollResult], "", Some(can_counteract), counteract)
    ///     .counter_reason(key!("yolo_pay")),
    /// ```
    pub const fn counter_reason(self, reason: MsgKey) -> Self {
        match self {
            On::PlayBody(mut e) => {
                e.counter_reason = Some(reason);
                On::PlayBody(e)
            }
            On::CounteractBody(mut e) => {
                e.counter_reason = Some(reason);
                On::CounteractBody(e)
            }
            On::Action(mut e) => {
                e.counter_reason = Some(reason);
                On::Action(e)
            }
            other => other,
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
    /// carry no clause. Named `_key` to leave [`Self::counter_reason`] free for
    /// the builder.
    pub const fn counter_reason_key(&self) -> Option<MsgKey> {
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
