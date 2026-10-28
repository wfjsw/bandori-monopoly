//! Card-face markers: this ruleset's marker vocabulary and its display rows.
//!
//! A marker is a named counter, or the display-only `turnsLeft` prop, on a
//! field card / event instance. Everything that *names* a marker lives here
//! rather than in `card-sdk`: the counter wire names, the countdown prop and
//! its helpers, and one [`MarkerDef`] row per marker kind.
//!
//! The rows are the client's rendering table: print order is the array order,
//! and a row carries the chip's style tone, its glyph, and the sentence the
//! note / marker list prints. `crates/rules-native/examples/dump_display.rs`
//! exports [`MARKERS`] into `display.json`, so adding a marker is one row here
//! -- no `webui` edit (`docs/DISPLAY.md`).
//!
//! A named counter without a row renders through the [`fallback`] row with
//! its `log.counter.*` phrase; a counter with neither a row nor a phrase is
//! dropped from the face.

/// Named-counter wire names this ruleset's cards use. The strings are ABI
/// codes the host matches on; the values are game vocabulary (the engine
/// stores them as plain names in `FieldCard::counters` / `FieldCard::cp` /
/// `FieldCard::crystals`).
pub mod counter {
    /// On-card [CP点] (「自己[场上]N个[CP点]」). Storage `FieldCard::cp`.
    pub const CP: &str = "cp";
    /// 奇迹水晶. Storage `FieldCard::crystals`.
    pub const CRYSTALS: &str = "crystals";
}

/// Display-only **remaining-turn countdown** prop (「…回合后」 / 「…结束时」
/// removal). Set by a rule at placement and decremented at the card's own turn
/// boundary; the card still owns the expiry (this prop never drives it). The
/// client renders it as the countdown row of [`MARKERS`] -- a field card's
/// prop, or `ActiveEvent.turnsLeft` for an event instance. `0` / absent = no
/// marker.
pub const TURNS_LEFT: &str = "turnsLeft";

/// Remaining turns before a rule-scheduled expiry, as the client's countdown
/// marker. Display-only; the rule body still owns the actual expiry. A fixed
/// initial count is better declared as a static `CardDef::props` entry
/// (stamped by `place_card` / `bind_event`); use this setter for a chosen or
/// re-armed value. Returns the stored value.
#[cfg_attr(kani, inline(always))]
pub fn set_turns_left(n: i32) -> i32 {
    crate::ctx::set_prop(TURNS_LEFT, n)
}

/// The running instance's remaining-turn marker ([`TURNS_LEFT`]); `0` = no
/// marker.
#[cfg_attr(kani, inline(always))]
pub fn turns_left() -> i32 {
    crate::ctx::prop(TURNS_LEFT)
}

/// One turn less on the marker ([`TURNS_LEFT`]), for the body's own
/// turn-boundary tick; returns the new value (never below `0`).
#[cfg_attr(kani, inline(always))]
pub fn dec_turns_left() -> i32 {
    set_turns_left((turns_left() - 1).max(0))
}

/// Where a marker's count is read from on the instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A static / dynamic prop (`props.turnsLeft`).
    Prop,
    /// A named counter (`counters.<key>`).
    Counter,
}

/// One text slot the client prints: a locale key it resolves, or a literal
/// template it fills.
///
/// A literal may use `{n}` (the count) and `{name}` (a named counter's
/// phrase); anything else is printed as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSlot {
    /// A `tr` key, resolved at render.
    Key(&'static str),
    /// A literal template.
    Lit(&'static str),
}

impl TextSlot {
    /// A locale key slot.
    pub const fn key(key: &'static str) -> Self {
        Self::Key(key)
    }

    /// A literal template slot.
    pub const fn lit(text: &'static str) -> Self {
        Self::Lit(text)
    }
}

/// One marker kind: where its count lives and how the face writes it.
///
/// `tone` is the chip's style class -- `turns` / `count` / `crystal` / `cp`
/// (`webui`'s `FieldSheet.module.css`); a new tone needs a CSS class there,
/// every other part of a new row does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkerDef {
    /// The counter / prop name this row describes. Unused by a fallback row.
    pub key: &'static str,
    /// Which instance field carries the count.
    pub source: Source,
    /// Style tone (see above).
    pub tone: &'static str,
    /// Glyph on the face: `{n}` = the count, `{name}` = a named counter.
    pub glyph: &'static str,
    /// Sentence for the chip's tooltip and the live note.
    pub note: TextSlot,
    /// Label the player marker list pairs with the card name.
    pub label: TextSlot,
    /// This row is the countdown: the face drops it when another marker on
    /// the same face already carries the same number. Display-only -- the
    /// value stays in the data.
    pub countdown: bool,
    /// This row takes every counter no other row covers (the named counters),
    /// with `key` resolved to the counter's phrase.
    pub fallback: bool,
}

impl MarkerDef {
    /// A row, in the `MARKERS` order it is listed at.
    pub const fn new(
        source: Source,
        key: &'static str,
        tone: &'static str,
        glyph: &'static str,
        note: TextSlot,
        label: TextSlot,
    ) -> Self {
        Self {
            key,
            source,
            tone,
            glyph,
            note,
            label,
            countdown: false,
            fallback: false,
        }
    }

    /// Mark this row as the countdown.
    pub const fn countdown(mut self) -> Self {
        self.countdown = true;
        self
    }

    /// Make this row the named-counter fallback.
    pub const fn fallback(mut self) -> Self {
        self.fallback = true;
        self
    }
}

/// The marker rows, in face print order: the countdown, the unnamed pools
/// (`counter` / `counter2` are the event phase counters -- a field card's
/// counters map may carry them too, harmlessly), the crystal and CP counts,
/// then every remaining named counter.
///
/// `crates/rules-native/examples/dump_display.rs` exports this table;
/// `docs/DISPLAY.md` describes the JSON.
pub const MARKERS: &[MarkerDef] = &[
    MarkerDef::new(
        Source::Prop,
        TURNS_LEFT,
        "turns",
        "⌛{n}",
        TextSlot::key("board.turnsLeft"),
        TextSlot::key("board.turnsLabel"),
    )
    .countdown(),
    MarkerDef::new(
        Source::Counter,
        "counter",
        "count",
        "×{n}",
        TextSlot::lit("×{n}"),
        TextSlot::lit("×{n}"),
    ),
    MarkerDef::new(
        Source::Counter,
        "counter2",
        "count",
        "×{n}",
        TextSlot::lit("×{n}"),
        TextSlot::lit("×{n}"),
    ),
    MarkerDef::new(
        Source::Counter,
        counter::CRYSTALS,
        "crystal",
        "◆{n}",
        TextSlot::key("board.crystals"),
        TextSlot::key("board.crystalsLabel"),
    ),
    MarkerDef::new(
        Source::Counter,
        counter::CP,
        "cp",
        "CP{n}",
        TextSlot::key("board.cp"),
        TextSlot::lit("CP"),
    ),
    MarkerDef::new(
        Source::Counter,
        "",
        "count",
        "{name}×{n}",
        TextSlot::lit("{name} ×{n}"),
        TextSlot::lit("{name}"),
    )
    .fallback(),
];
