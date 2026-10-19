//! Roster metadata that lives beside the rule code (ABI v63).
//!
//! Cards, characters and bands used to be described by flat JSON that the host
//! loaded separately from the ruleset. The metadata is now authored in the same
//! Rust that defines the rule (`CardDef::meta`, the skill crates' `GroupDef` /
//! `PersonaDef`) and exported on request through `ruleset_metadata` -- never
//! through [`crate::abi::ManifestEntry`], so the match path remains lean.
//!
//! This module is the **generic** half: an authoring shape for one card row
//! ([`CardMeta`]), a wire row ([`MetaRow`]) / whole-roster blob ([`MetaDoc`]),
//! and the ordinal lookup that restores the canonical data-file order. Game
//! vocabulary (`band`, `owner`, `PersonaDef`, `GroupDef`) lives in the game
//! crates; this game's card-row keys are the `fields` map entries named in
//! `bandori_std::meta_key`.

// String/Vec come from `alloc` on guest builds and from std on the host.
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::string::{String, ToString};
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::vec::Vec;

/// Authoring: one card's data-set metadata -- the row the deck builder, gallery
/// and home screen read. Generic shape: the fields every roster row shares,
/// plus a key/value map for game-specific slots (this game: `band`, `owner`).
///
/// Multi-line CJK text is a plain (preferably raw) `&'static str` -- byte-exact
/// newlines, full-width punctuation and trailing spaces round-trip untouched.
///
/// ```ignore
/// pub const CARD: CardDef = CardDef::new("Sumimi:Sumimi不会解散哦", &[...])
///     .meta(&CardMeta::new()
///         .name("Sumimi不会解散哦")
///         .raw_name("Sumimi不会解散哦")
///         .text("当你本回合未进行过抵押/赎回操作且…")
///         .tags(&["手"])
///         .cell("B2")
///         .fields(&[("group", "Sumimi"), ("owner", "")]));
/// ```
#[derive(Clone, Copy)]
pub struct CardMeta {
    /// Display name (the `name` column). Empty = unnamed (the title falls back
    /// to the id). Rides `MetaRow` into `ruleset_metadata`.
    pub name: &'static str,
    /// Untrimmed name as printed on the sheet (`rawName`). May carry the
    /// character prefix the display name drops. Display-only: exported to
    /// `display.json`, not emitted on [`MetaRow`].
    pub raw_name: &'static str,
    /// Rule text (the `text` column). Display-only: exported to
    /// `display.json`, not emitted on [`MetaRow`].
    pub text: &'static str,
    /// Deck-building tags (「手」 / 「衍生」 / ...).
    pub tags: &'static [&'static str],
    /// Derived / token card (not in the physical deck pool).
    pub derived: bool,
    /// Sheet cell this row was lifted from (`cell`). Display-only: exported to
    /// `display.json`, not emitted on [`MetaRow`].
    pub cell: &'static str,
    /// Game-specific string slots, `(key, value)`. This game's card-row keys
    /// are `band` and `owner` (`bandori_std::meta_key`). Missing key = empty.
    pub fields: &'static [(&'static str, &'static str)],
}

impl CardMeta {
    pub const fn new() -> Self {
        Self {
            name: "",
            raw_name: "",
            text: "",
            tags: &[],
            derived: false,
            cell: "",
            fields: &[],
        }
    }

    pub const fn name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    pub const fn raw_name(mut self, raw_name: &'static str) -> Self {
        self.raw_name = raw_name;
        self
    }

    pub const fn text(mut self, text: &'static str) -> Self {
        self.text = text;
        self
    }

    pub const fn tags(mut self, tags: &'static [&'static str]) -> Self {
        self.tags = tags;
        self
    }

    pub const fn derived(mut self, derived: bool) -> Self {
        self.derived = derived;
        self
    }

    pub const fn cell(mut self, cell: &'static str) -> Self {
        self.cell = cell;
        self
    }

    pub const fn fields(mut self, fields: &'static [(&'static str, &'static str)]) -> Self {
        self.fields = fields;
        self
    }

    /// The value of a `fields` key (`""` when absent).
    pub fn field(&self, key: &str) -> &'static str {
        self.fields
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or("")
    }

    /// Wire form, tagged `kind = "card"`. `text` is emitted: the ruleset
    /// queries it at runtime (`card_text_mentions`, 「效果包含…」), so it is a
    /// rule input as well as display. `raw_name` / `cell` are display-only and
    /// remain on the def for `display.json` (`docs/DISPLAY.md`).
    pub fn to_row(&self, id: &str, ordinal: i32) -> MetaRow {
        MetaRow {
            kind: "card".to_string(),
            ordinal,
            id: id.to_string(),
            name: self.name.to_string(),
            raw_name: String::new(),
            text: self.text.to_string(),
            tags: self.tags.iter().map(|s| (*s).to_string()).collect(),
            derived: self.derived,
            cell: String::new(),
            fields: self
                .fields
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            lists: Vec::new(),
            flags: Vec::new(),
        }
    }
}

impl Default for CardMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// Wire: one metadata row. Cards and every other roster entry (whatever the
/// game calls them) share this shape -- the `kind` tag and the `fields` /
/// `lists` / `flags` maps carry the game-specific slots.
///
/// Postcard is not self-describing: field order is the wire contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetaRow {
    /// Caller-defined kind tag. Cards use `"card"`; the game's other roster
    /// lists supply their own (`"character"`, `"band"`, ...).
    pub kind: String,
    /// Canonical data-file order (0-based). Export sorts by this.
    pub ordinal: i32,
    /// Primary key: card id, or the row's `name` for name-keyed lists.
    pub id: String,
    pub name: String,
    pub raw_name: String,
    pub text: String,
    pub tags: Vec<String>,
    pub derived: bool,
    pub cell: String,
    /// Game-specific string slots `(key, value)`.
    pub fields: Vec<(String, String)>,
    /// Game-specific string lists `(key, values)`.
    pub lists: Vec<(String, Vec<String>)>,
    /// Game-specific boolean flags `(key, value)`.
    pub flags: Vec<(String, bool)>,
}

impl MetaRow {
    /// The value of a `fields` key (`""` when absent).
    pub fn field(&self, key: &str) -> &str {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    /// The value of a `lists` key (empty when absent).
    pub fn list(&self, key: &str) -> &[String] {
        self.lists
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// The value of a `flags` key (`false` when absent).
    pub fn flag(&self, key: &str) -> bool {
        self.flags
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .unwrap_or(false)
    }
}

/// Wire: the whole roster metadata blob (`ruleset_metadata`'s postcard payload).
/// Sorted by `(kind, ordinal)` so the bytes are deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetaDoc {
    pub rows: Vec<MetaRow>,
}

impl MetaDoc {
    /// Rows of one kind, in ordinal order.
    pub fn of_kind(&self, kind: &str) -> Vec<&MetaRow> {
        self.rows.iter().filter(|r| r.kind == kind).collect()
    }
}

/// Convert a typed game def (`PersonaDef`, `GroupDef`, ...) into a wire row.
/// Implemented by the game's skill crates; `card-sdk` remains vocabulary-free.
pub trait ToMetaRow {
    fn to_meta_row(&self) -> MetaRow;
}

/// `(key, ordinal)` pairs restoring one roster list's data-file order.
pub type OrdinalTable = &'static [(&'static str, i32)];

/// One non-card roster list, ready for [`crate::rt::metadata`].
pub struct MetaListRef<'a> {
    /// Kind tag written onto every row (`"character"`, `"band"`, ...).
    pub kind: &'a str,
    pub rows: &'a [MetaRow],
    /// Canonical order for this list's keys (the `id` / `name` column).
    pub ordinals: OrdinalTable,
}

/// Convert a slice of typed defs into wire rows.
pub fn rows_of<T: ToMetaRow>(defs: &[T]) -> Vec<MetaRow> {
    defs.iter().map(|d| d.to_meta_row()).collect()
}

/// Ordinal of `key` in `table`, or `i32::MAX` when the lift script has not
/// listed it (the equivalence test fails such a row's position loudly).
pub fn ordinal_of(table: OrdinalTable, key: &str) -> i32 {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, o)| *o)
        .unwrap_or(i32::MAX)
}

/// One non-card roster list: its kind tag, its ordinal table, and the fn that
/// materialises its wire rows. A `fn() -> Vec<MetaRow>` (rather than the rows
/// themselves) so the source can be a `'static` without leaking allocations.
pub struct RosterList {
    /// Kind tag written onto every row (`"character"`, `"band"`, ...).
    pub kind: &'static str,
    /// Canonical data-file order for this list's keys.
    pub ordinals: OrdinalTable,
    /// Builds this list's rows (`rows_of` over the skill crate's table).
    pub rows: fn() -> Vec<MetaRow>,
}

/// What `ruleset_metadata` needs beyond `BAND_TABLE`: the card ordinal table
/// and the non-card roster lists. Defined at the ruleset assembly site
/// (`card-all`), which is the one place that sees every skill table.
pub struct RosterSources {
    /// Canonical roster order: card id -> index.
    pub card_ordinals: OrdinalTable,
    pub lists: &'static [RosterList],
}

impl RosterSources {
    /// No non-card rows and no ordinals (fixtures / partial lifts).
    pub const fn empty() -> Self {
        Self {
            card_ordinals: &[],
            lists: &[],
        }
    }
}

/// `'static` empty sources for `ruleset!` without a `; roster` clause.
pub static EMPTY_ROSTER: RosterSources = RosterSources::empty();