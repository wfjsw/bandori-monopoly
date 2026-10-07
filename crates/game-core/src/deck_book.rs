//! The bot **deck book** (`docs/BOT.md` §3.7): a small, versioned dictionary
//! from a public table key to a deck, derived offline and looked up here.
//!
//! Bots and the browser 托管 otherwise submit `deck::preset` (standard) or
//! `deck::random` (chaos), blind to the table. The book answers "what does a
//! standard seat with *this* character at *this* table want to run?", falling
//! back in levels when the exact table was never tuned.
//!
//! * **Key = public information only.** At the deck phase every pick and the
//!   seat order are public; opponents' decks are not. The key is
//!   `(my character, my seat, opponents' characters in seat order)`.
//! * **Back-off**, first hit wins ([`DeckBook::lookup`]): exact
//!   `(me, seat, opponents in order)` → `(me, seat, multiset of opponents'
//!   bands)` → `(me, multiset of opponents' bands)` → `(me)` → `deck::preset`.
//! * **Validity.** The book records the ruleset hash (the same string
//!   [`crate::record::EngineStamp::ruleset_sha256`] carries --
//!   `Ruleset::sha256()`, or `"stub"` without card modules) and the policy it
//!   was tuned for. A mismatch at lookup ignores the whole book (once, on
//!   stderr) and falls back to the preset: a stale book would otherwise
//!   silently mis-tune decks against changed cards / rules.
//! * **Every hit is still legal.** A looked-up deck passes `deck::clean` /
//!   `deck::is_complete`; an invalid entry falls through to the next level.
//! * **Pure.** Lookup uses no RNG, so a standard bot's deck is a function of
//!   the public table alone.
//!
//! File: `data/deck_book.json` (optional -- a missing or unparsable file is an
//! empty book, behaviour unchanged). Card ids are strings for now; index
//! compression is a later file-format bump.
//!
//! Field names are snake_case, like [`crate::record::EngineStamp`] and the
//! ruleset `index.json`, not the camelCase of the C#-mirrored `data/*.json`:
//! this file is engine-side data, not a Unity `JsonUtility` table.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::data::{CharacterData, GameData};
use crate::deck;

/// File format version (`DeckBook::version`). Bump on any schema change; a
/// book with a different version is ignored entirely.
pub const DECK_BOOK_VERSION: u32 = 1;

/// The policy a book may be tuned for -- the standard bot / 托管 deck pick,
/// `BotMentality::Standard.as_str()`. Chaos stays `deck::random` and never
/// reads the book.
pub const POLICY_STANDARD: &str = "standard";

/// The file name `GameData::load` looks the book up under, relative to the
/// other `data/*.json` tables. Absent = empty book.
pub const DECK_BOOK_FILE: &str = "deck_book.json";

/// One book row. Which fields matter depends on the level array it sits in
/// (`docs/BOT.md` §3.7):
///
/// | level | key fields |
/// |---|---|
/// | `exact` | `me`, `seat`, `opponents` (characters in seat order) |
/// | `seat_bands` | `me`, `seat`, `opponent_bands` (sorted multiset) |
/// | `bands` | `me`, `opponent_bands` (sorted multiset) |
/// | `me` | `me` |
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckBookEntry {
    /// Own character (`CharacterData::name`).
    pub me: String,
    /// Own seat (turn order index). Used by the two seat levels only.
    pub seat: Option<u32>,
    /// Opponents' characters in seat order (exact level).
    pub opponents: Vec<String>,
    /// Sorted multiset of the opponents' band names (band levels).
    pub opponent_bands: Vec<String>,
    /// Card ids; validated with `deck::clean` / `deck::is_complete` at lookup.
    pub cards: Vec<String>,
}

/// `data/deck_book.json` -- header plus one entry list per back-off level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckBook {
    pub version: u32,
    /// `Ruleset::sha256()` of the ruleset the entries were tuned against, or
    /// `"stub"`. Same string as [`crate::record::EngineStamp::ruleset_sha256`].
    pub ruleset_sha256: String,
    /// [`POLICY_STANDARD`]; any other policy makes the book inert.
    pub policy: String,
    /// Free-form generation stamp (date, tool version); display only.
    pub generated_at: String,
    /// 1. `(me, seat, opponents' characters in seat order)`.
    pub exact: Vec<DeckBookEntry>,
    /// 2. `(me, seat, sorted multiset of opponents' bands)`.
    pub seat_bands: Vec<DeckBookEntry>,
    /// 3. `(me, sorted multiset of opponents' bands)`.
    pub bands: Vec<DeckBookEntry>,
    /// 4. `(me)`.
    pub me: Vec<DeckBookEntry>,
    /// Warned once about being stale / mismatched (never serialized).
    #[serde(skip)]
    warned: Arc<AtomicBool>,
}

impl PartialEq for DeckBook {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.ruleset_sha256 == other.ruleset_sha256
            && self.policy == other.policy
            && self.generated_at == other.generated_at
            && self.exact == other.exact
            && self.seat_bands == other.seat_bands
            && self.bands == other.bands
            && self.me == other.me
    }
}

impl DeckBook {
    /// Parse the file body. Header mismatches are **not** checked here -- see
    /// [`Self::usable`], which lookup applies against the running ruleset.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
    }

    /// Parse, or fall back to an empty book with the reason on stderr. The
    /// optional-file loader ([`GameData::load`]) uses this so a corrupt book
    /// never breaks the game.
    pub fn parse_or_empty(text: &str) -> Self {
        Self::parse(text).unwrap_or_else(|e| {
            eprintln!("{DECK_BOOK_FILE}: {e} -- bots fall back to the preset decks");
            Self::default()
        })
    }

    /// Does the book hold any entries at all? An empty book (the shipped
    /// placeholder, or a missing file) is the pre-derivation state and never
    /// warns about being stale -- there is nothing to go stale.
    pub fn is_empty(&self) -> bool {
        self.exact.is_empty()
            && self.seat_bands.is_empty()
            && self.bands.is_empty()
            && self.me.is_empty()
    }

    /// Is the book tuned for this ruleset and policy? A stale hash or a policy
    /// mismatch means every entry is silently mis-tuned, so the whole book is
    /// ignored (logged once) and the caller falls back to `deck::preset`.
    pub fn usable(&self, ruleset_sha256: &str) -> bool {
        if self.version != DECK_BOOK_VERSION
            || self.policy != POLICY_STANDARD
            || self.ruleset_sha256 != ruleset_sha256
        {
            if !self.is_empty()
                && self
                    .warned
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                eprintln!(
                    "{DECK_BOOK_FILE}: version {} / policy {} / ruleset {} does not match this \
                     build (want version {DECK_BOOK_VERSION} / {POLICY_STANDARD} / {ruleset_sha256}) \
                     -- bots fall back to the preset decks",
                    self.version, self.policy, self.ruleset_sha256,
                );
            }
            return false;
        }
        true
    }

    /// Back-off lookup over the public table key. Pure: no RNG, first hit per
    /// level wins, an entry whose cards do not clean down to a complete deck
    /// falls through to the next level.
    ///
    /// * `me` -- own character.
    /// * `seat` -- own seat (turn order index).
    /// * `opponents` -- the other seats' characters in seat order.
    /// * `ruleset_sha256` -- the running ruleset's hash
    ///   ([`crate::engine::CardRules::ruleset_sha256`], `"stub"` when unknown).
    pub fn lookup(
        &self,
        data: &GameData,
        me: &CharacterData,
        seat: usize,
        opponents: &[String],
        ruleset_sha256: &str,
    ) -> Option<Vec<String>> {
        if !self.usable(ruleset_sha256) {
            return None;
        }
        let bands = opponent_bands(data, opponents);
        // 1. exact: (me, seat, opponents' characters in seat order)
        if let Some(d) = first_valid(
            data,
            me,
            self.exact.iter().filter(|e| {
                e.me == me.name && e.seat == Some(seat as u32) && e.opponents == opponents
            }),
        ) {
            return Some(d);
        }
        // 2. (me, seat, sorted multiset of opponents' bands)
        if let Some(d) = first_valid(
            data,
            me,
            self.seat_bands.iter().filter(|e| {
                e.me == me.name && e.seat == Some(seat as u32) && e.opponent_bands == bands
            }),
        ) {
            return Some(d);
        }
        // 3. (me, sorted multiset of opponents' bands)
        if let Some(d) = first_valid(
            data,
            me,
            self.bands
                .iter()
                .filter(|e| e.me == me.name && e.opponent_bands == bands),
        ) {
            return Some(d);
        }
        // 4. (me)
        first_valid(
            data,
            me,
            self.me.iter().filter(|e| e.me == me.name),
        )
    }
}

/// Sorted multiset of the opponents' band names (`CharacterData::band`). An
/// unknown character contributes `""` -- deterministic, and it only ever makes
/// a band-level key miss.
fn opponent_bands(data: &GameData, opponents: &[String]) -> Vec<String> {
    let mut bands: Vec<String> = opponents
        .iter()
        .map(|c| data.character(c).map(|c| c.band.clone()).unwrap_or_default())
        .collect();
    bands.sort();
    bands
}

/// The first entry whose cards clean down to a complete deck, cleaned.
fn first_valid<'e>(
    data: &GameData,
    me: &CharacterData,
    entries: impl Iterator<Item = &'e DeckBookEntry>,
) -> Option<Vec<String>> {
    for e in entries {
        let list = deck::clean(data, me, &e.cards);
        if list.len() == deck::SIZE {
            return Some(list);
        }
    }
    None
}

/// The deck a standard bot (or the browser 托管) submits for this public table:
/// [`DeckBook::lookup`], else the designer's [`deck::preset`]. Pure -- no RNG.
/// Chaos never calls this; it stays `deck::random`.
pub fn suggest(
    data: &GameData,
    me: &CharacterData,
    seat: usize,
    opponents: &[String],
    ruleset_sha256: &str,
) -> Vec<String> {
    data.deck_book
        .lookup(data, me, seat, opponents, ruleset_sha256)
        .unwrap_or_else(|| deck::preset(data, me))
}