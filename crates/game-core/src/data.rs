//! Static game data (`data/*.json`, minus the roster) and the lookups
//! [`GameData`] provides.
//!
//! Field names and defaults mirror the JSON the data files already carry:
//! missing fields take `#[serde(default)]`, and unknown keys (e.g. the
//! `_说明` notes) are ignored. The on-disk / on-wire spelling is the contract
//! -- where the Rust side is typed ([`TileKind`]), serde round-trips the same
//! strings.
//!
//! `game-core` never touches the filesystem: [`GameData::load`] takes a reader so the
//! server (fs) and the browser (fetch) can both supply the files.
//!
//! **Roster (ABI v63).** `cards` / `personas` / `groups` no longer come from
//! `data/{cards,personas,groups}.json`. They are authored beside the rule code
//! and handed over as [`Roster`] (ruleset `ruleset_metadata` ->
//! [`crate::data::Roster`]). [`DATA_FILES`] covers only the remaining tables
//! and the data hash (`EngineStamp::data_sha256`) hashes exactly those; the
//! roster is covered by the ruleset hash (`EngineStamp::ruleset_sha256`),
//! which already names the modules the metadata lives in.
//!
//! **Display text is not engine data.** Card / persona / group / event copy,
//! tile chrome (short names, colors, notes), home lines, voice lines, schools
//! and the rules book live in the webui (`display.json` and the `data/*.json`
//! files the client already parses). The structs here carry only what the
//! engine resolves and the match rules read -- so editing display text no
//! longer moves the replay bundle id.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::deck_book::{DeckBook, DECK_BOOK_FILE};
use crate::strategy::{StrategyBook, STRATEGY_BOOK_FILE};

/// What sort of tile this is: an **opaque string id** from the board data.
///
/// The JSON data files carry short strings; the engine never interprets
/// them. Meaning (start square, deed, which rule settles it) comes from the
/// ruleset via [`crate::engine::rules::CardRules`] and from tile props
/// stamped at bind time. Serde round-trips the raw spelling so the data
/// format is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileKind(pub String);

impl TileKind {
    /// The data-file / wire spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode the data-file spelling.
    pub fn parse(s: &str) -> Self {
        TileKind(s.to_string())
    }
}

impl std::fmt::Display for TileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for TileKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TileKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(TileKind(raw))
    }
}

/// One tile of the board, as the data file names it. `name` **stays**: the
/// engine resolves tiles by name (`ctx::tile_named`, `engine/ops.rs`).
/// Display chrome (`shortName`, `color`, `note`) is client-side.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TileData {
    pub index: i32,
    pub name: String,
    pub kind: TileKind,
    pub group: i32,
    pub tier: i32,
    pub area: String,
    pub price: i32,
    pub house: i32,
    pub rent: Vec<i32>,
}

impl TileData {
    /// Generic deed shape: a priced tile (the land price). A deed may have an
    /// empty rent table (no upgrade ladder). The ruleset may override via
    /// [`crate::state::prop::BUYABLE`] on the tile's rule instance; this is
    /// the data-only heuristic for paths with no ruleset.
    pub fn is_buyable(&self) -> bool {
        self.price > 0
    }
}

/// Rule-id supply moved to [`crate::engine::rules::CardRules`]: the ruleset
/// declares which rule settles each opaque tile kind (`tile_rule_id`) and
/// which standing board rules exist (`mark_rule_ids` / `status_rule_ids`).
/// The engine keeps no kind→rule table.

/// One card row of the roster (was `cards.json`; now ruleset metadata).
/// Display-only copy (`rawName`, `cell`) lives in `display.json`. `text` is
/// dual-role and stays here: the ruleset queries it at runtime
/// (`card_text_mentions`, 「效果包含…」), so it is a rule input as well as the
/// printed body the client shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CardData {
    pub id: String,
    pub group: String,
    pub name: String,
    pub owner: String,
    pub derived: bool,
    pub tags: Vec<String>,
    /// Printed rule body. Searched by `card_text_mentions`.
    pub text: String,
}

impl CardData {
    /// The name, or the id for unnamed cards (the client shows its own
    /// "unnamed" label).
    pub fn title(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }

    /// Usable by every group (the data's general group, [`GENERAL_GROUP`]).
    pub fn is_universal(&self) -> bool {
        self.group == GENERAL_GROUP
    }

    /// Bound to one persona (`owner` set).
    pub fn exclusive(&self) -> bool {
        !self.owner.is_empty()
    }
}

/// One playable persona of the roster (was `personas.json`). Display-only copy
/// (`display`, `color`, `variantTag`, `art`, `costume`, `cnId`) lives in
/// `display.json`. `text` is dual-role and stays here: `card_text_mentions`
/// searches it over `skill:<persona>:<skill>` (「技能中含有…」).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PersonaData {
    pub name: String,
    pub group: String,
    pub skill: String,
    pub exclusive_cards: Vec<String>,
    pub preset: Vec<String>,
    /// The persona this one is an alternate form of (`高松灯（CRYCHIC）` ->
    /// `高松灯`); empty when it is a base persona.
    pub variant_of: String,
    /// Printed skill body. Searched by `card_text_mentions`.
    pub text: String,
}

/// One group of the roster (was `groups.json`). Display-only copy (`shortName`,
/// `color`, `logo`) lives in `display.json`. `text` is dual-role and stays
/// here: `card_text_mentions` searches it over `skill:<group>:<skill>`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupData {
    pub name: String,
    pub skill: String,
    /// Group-skill trait: members may carry each other's exclusive cards in a
    /// starting deck. Deck-legal only -- the in-match play gate is the
    /// group-skill card's `ExclusiveUse` entry (or the generic own/base rule).
    pub shares_exclusives: bool,
    /// Printed skill body. Searched by `card_text_mentions`.
    pub text: String,
}

/// The roster half of [`GameData`] -- cards / personas / groups -- handed over
/// by the ruleset host (ABI v63 `ruleset_metadata`, converted in `game-rules`).
/// `game-core` stays free of the card crates; this is the plain struct of
/// vectors it already iterates in roster order (the Rust ordinal table).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Roster {
    pub cards: Vec<CardData>,
    pub personas: Vec<PersonaData>,
    pub groups: Vec<GroupData>,
}

impl Roster {
    /// Parse the synthetic test fixture (`crates/game-core/tests/fixtures/
    /// roster.json`). One JSON object `{cards, personas, groups}` in the same
    /// camelCase row shapes the roster metadata carries. Tests of crates that
    /// have no ruleset (game-core / game-rules / bot-core / server) load this
    /// instead of depending on the card crates.
    pub fn from_fixture(text: &str) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Fixture {
            #[serde(default)]
            cards: Vec<CardData>,
            #[serde(default)]
            personas: Vec<PersonaData>,
            #[serde(default)]
            groups: Vec<GroupData>,
        }
        let f: Fixture = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("roster fixture: {e}"))?;
        Ok(Roster {
            cards: f.cards,
            personas: f.personas,
            groups: f.groups,
        })
    }

    /// Serialize back to the fixture document (`{cards, personas, groups}`),
    /// the byte-stable pretty JSON form [`Self::from_fixture`] reads. External
    /// gates may compare this against a checked-in fixture so the two cannot
    /// drift.
    pub fn to_fixture_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct Fixture<'a> {
            cards: &'a [CardData],
            personas: &'a [PersonaData],
            groups: &'a [GroupData],
        }
        let mut s = serde_json::to_string_pretty(&Fixture {
            cards: &self.cards,
            personas: &self.personas,
            groups: &self.groups,
        })
        .expect("roster fixture serializes");
        s.push('\n');
        s
    }
}

/// One event card from `events.json`. `name` and `text` are game content (the
/// rulebook body and what the card is called) and stay on the engine's view of
/// the table -- the spec harness classifies 「将此卡…」 discard overrides by
/// reading `text`. `cell` (the sheet column the row was lifted from) is
/// authoring chrome and stays in `events.json` for the client only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventData {
    pub id: String,
    pub name: String,
    pub derived: bool,
    /// Rulebook body. Game content, like `CardData::text`.
    pub text: String,
}

/// `group` value of the cards every persona may take. A roster group key (the
/// data set is Chinese), not display text.
pub const GENERAL_GROUP: &str = "通用";

/// Tunable match numbers from `match_rules.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchRulesData {
    pub money: f32,
    pub property: f32,
    pub houses: f32,
    /// Generic named rule parameters (the ruleset reads them by name).
    /// The shipped ruleset uses `rentUnitMult` as the dice-rent unit multiplier.
    pub rule_params: std::collections::BTreeMap<String, i32>,
    /// Names given to bot players, in order. Match naming policy, not chrome.
    pub bot_names: Vec<String>,
}

impl Default for MatchRulesData {
    fn default() -> Self {
        let mut rule_params = std::collections::BTreeMap::new();
        rule_params.insert("rentUnitMult".to_string(), 10);
        Self {
            money: 1.0,
            property: 1.0,
            houses: 1.0,
            rule_params,
            bot_names: vec![],
        }
    }
}

#[derive(Deserialize)]
struct BoardFile {
    #[serde(default)]
    tiles: Vec<TileData>,
}

#[derive(Deserialize)]
struct EventFile {
    #[serde(default)]
    events: Vec<EventData>,
}

/// The rule id of a persona or group skill (the crates under `rules/skills`).
///
/// The id **names its owner**, which is the binding: a player's two skills
/// follow mechanically from the persona they picked, and a skill can be
/// resolved the same way a card is -- by id. The `skill:` prefix keeps it from
/// ever colliding with a card id (`AG:即使夕阳落山`).
pub fn skill_id(owner: &str, skill: &str) -> String {
    format!("skill:{owner}:{skill}")
}

/// The rule id of an **event card**'s effect (the crate under `rules/events`).
///
/// The id names the event (`data/events.json`), so drawing a ruleset event id
/// resolves the rule `event:<id>`. The `event:` prefix keeps it from ever
/// colliding with a card id or a `tile:*` / `skill:*` rule id, the same way
/// [`skill_id`] and [`tile_rule_id`] do. Gaps in the text are the rule body's
/// `TODO(规则书)`; an event with no rule in the ruleset falls back to the
/// engine's `StubRules::event` (log 「还没有移植」 and file it away).
pub fn event_rule_id(id: &str) -> String {
    format!("event:{id}")
}

/// All static game data plus the [`GameData`] lookups.
#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub tiles: Vec<TileData>,
    pub cards: Vec<CardData>,
    pub personas: Vec<PersonaData>,
    pub groups: Vec<GroupData>,
    pub events: Vec<EventData>,
    pub match_rules: MatchRulesData,
    /// Bot deck book (`docs/BOT.md` §3.7) from the optional
    /// `data/deck_book.json`. Empty when the file is absent or unparsable --
    /// bots then keep using [`crate::deck::preset`]. Not part of
    /// [`DATA_FILES`] / the data hash: it tunes bots, not the match.
    pub deck_book: DeckBook,
    /// Bot strategy book (`docs/BOT.md` §3.8) from the optional
    /// `data/strategy_book.json`. Empty when the file is absent or unparsable
    /// -- bots then keep using the default [`crate::strategy::StrategyParams`]
    /// (today's constants). Not part of [`DATA_FILES`] / the data hash, like
    /// [`Self::deck_book`].
    pub strategy_book: StrategyBook,
    card_by_id: HashMap<String, usize>,
}

/// Data files, as named in `web/data/`, that the **engine** loads and hashes.
/// Three tables only: the board, the event roster and the match numbers.
///
/// The hash covers each file's bytes as a whole, so *every* column in these
/// three moves `data_sha256` -- including the game prose (`EventData::name` /
/// `text`, the rulebook body the spec harness reads) and the chrome that
/// happens to sit beside it in the same file (tile `shortName` / `color`).
/// That is deliberate for the prose: it is game content, not presentation.
///
/// What left the hash is the display-only files the client parses on its own
/// (`rules.txt`, `home_lines.json`, `voice_lines.json`, `skill_simple.json`,
/// `schools.json`) and the roster's display columns (`display.json`). The
/// roster (`cards` / `personas` / `groups`) is **not** here either: it comes
/// from the ruleset (ABI v63 `ruleset_metadata`) or, in tests, from the
/// synthetic fixture (`Roster::from_fixture`), and is covered by the ruleset
/// hash, not this one.
pub const DATA_FILES: [&str; 3] = ["board.json", "events.json", "match_rules.json"];

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{file}: {e}"))
}

impl GameData {
    /// The rule ids of `persona`'s own skill and of its group's skill, in that
    /// order. This is the binding: see [`skill_id`]. Empty entries (a persona
    /// or group with no skill) are skipped, so the result is 0..=2 ids.
    pub fn skill_rules_of(&self, persona: &str) -> Vec<String> {
        let Some(c) = self.personas.iter().find(|c| c.name == persona) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if !c.skill.is_empty() {
            out.push(skill_id(&c.name, &c.skill));
        }
        if let Some(b) = self.groups.iter().find(|b| b.name == c.group) {
            if !b.skill.is_empty() {
                out.push(skill_id(&b.name, &b.skill));
            }
        }
        out
    }

    /// Is `id` a **group** skill rule (`skill:<group>:<skill>` for some group in
    /// roster groups), as against a persona skill (`skill:<persona>:<skill>`)?
    /// This is what stamps `FieldCard::group_skill` at placement, and through it
    /// what the group-skill named counter reads and writes land on.
    pub fn is_group_skill(&self, id: &str) -> bool {
        self.groups
            .iter()
            .any(|b| !b.skill.is_empty() && skill_id(&b.name, &b.skill) == id)
    }

    /// Load every file in [`DATA_FILES`] through `read(file_name) -> contents`,
    /// plus the [`Roster`] the ruleset host supplies (ABI v63 `ruleset_metadata`
    /// -> `game_rules::roster::roster_from_meta`, or `rules_native::roster()`),
    /// or, in tests, [`Roster::from_fixture`]. There is no JSON-roster path
    /// left in the runtime.
    ///
    /// The deck book / strategy book are loaded here so every entry point
    /// shares that path.
    pub fn load(
        mut read: impl FnMut(&str) -> Result<String, String>,
        roster: Roster,
    ) -> Result<Self, String> {
        let mut get = |f: &str| read(f).map_err(|e| format!("{f}: {e}"));
        let mut d = GameData {
            tiles: parse::<BoardFile>("board.json", &get("board.json")?)?.tiles,
            cards: roster.cards,
            personas: roster.personas,
            groups: roster.groups,
            events: parse::<EventFile>("events.json", &get("events.json")?)?.events,
            match_rules: parse("match_rules.json", &get("match_rules.json")?)?,
            deck_book: DeckBook::default(),
            strategy_book: StrategyBook::default(),
            card_by_id: HashMap::new(),
        };
        // First card wins on duplicate ids.
        for (i, c) in d.cards.iter().enumerate() {
            if !c.id.is_empty() {
                d.card_by_id.entry(c.id.clone()).or_insert(i);
            }
        }
        // Optional, outside `DATA_FILES` (so the data hash and records do not
        // move with it): a missing file is an empty book and bots stay on the
        // preset decks.
        if let Ok(text) = read(DECK_BOOK_FILE) {
            d.deck_book = DeckBook::parse_or_empty(&text);
        }
        // Same for the strategy book (`docs/BOT.md` §3.8): absent / unparsable
        // = default parameters = today's heuristics.
        if let Ok(text) = read(STRATEGY_BOOK_FILE) {
            d.strategy_book = StrategyBook::parse_or_empty(&text);
        }
        Ok(d)
    }

    /// Card by id, first match.
    pub fn card(&self, id: &str) -> Option<&CardData> {
        self.card_by_id.get(id).map(|&i| &self.cards[i])
    }

    /// Persona by `name`, first match.
    pub fn persona(&self, name: &str) -> Option<&PersonaData> {
        self.personas.iter().find(|c| c.name == name)
    }

    /// The base persona of a variant (`variant_of`). Itself when it is not
    /// a variant.
    pub fn base_persona<'a>(&'a self, name: &'a str) -> &'a str {
        self.persona(name)
            .map(|c| c.variant_of.as_str())
            .filter(|b| !b.is_empty())
            .unwrap_or(name)
    }

    /// Whether `c`'s group lets its members carry each other's exclusive cards
    /// in a starting deck (deck-building only; see [`crate::deck`]).
    pub fn group_shares_exclusives(&self, c: &PersonaData) -> bool {
        self.group(&c.group).is_some_and(|b| b.shares_exclusives)
    }

    /// Group by `name`, first match.
    pub fn group(&self, name: &str) -> Option<&GroupData> {
        self.groups.iter().find(|b| b.name == name)
    }

    /// Event card by id.
    pub fn event(&self, id: &str) -> Option<&EventData> {
        self.events.iter().find(|e| e.id == id)
    }
}