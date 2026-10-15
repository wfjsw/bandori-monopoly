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
//! **Roster (ABI v63).** `cards` / `characters` / `groups` no longer come from
//! `data/{cards,characters,groups}.json`. They are authored beside the rule code
//! and handed over as [`Roster`] (ruleset `ruleset_metadata` ->
//! [`crate::data::Roster`]). [`DATA_FILES`] covers only the remaining tables
//! and the data hash (`EngineStamp::data_sha256`) hashes exactly those; the
//! roster is covered by the ruleset hash (`EngineStamp::ruleset_sha256`),
//! which already names the modules the metadata lives in.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::deck_book::{DeckBook, DECK_BOOK_FILE};
use crate::strategy::{StrategyBook, STRATEGY_BOOK_FILE};

/// What sort of tile this is: an **opaque string id** from the board data.
///
/// The JSON data files carry short strings (`"circle"`, `"property"`, ...);
/// the engine never interprets them. Meaning (start square, deed, which rule
/// settles it) comes from the ruleset via [`crate::engine::rules::CardRules`]
/// and from tile props stamped at bind time. Serde round-trips the raw
/// spelling so the data format is unchanged.
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

/// One tile of the board, as the data file names it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TileData {
    pub index: i32,
    pub name: String,
    pub short_name: String,
    pub kind: TileKind,
    pub group: i32,
    pub color: String,
    pub tier: i32,
    pub area: String,
    pub price: i32,
    pub house: i32,
    pub rent: Vec<i32>,
    pub note: String,
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CardData {
    pub id: String,
    pub group: String,
    pub name: String,
    pub raw_name: String,
    pub owner: String,
    pub derived: bool,
    pub tags: Vec<String>,
    pub text: String,
    pub cell: String,
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

    /// Usable by every band (the data's general band, [`GENERAL_GROUP`]).
    pub fn is_universal(&self) -> bool {
        self.group == GENERAL_GROUP
    }

    /// Bound to one persona (`owner` set).
    pub fn exclusive(&self) -> bool {
        !self.owner.is_empty()
    }
}

/// One playable persona of the roster (was `characters.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PersonaData {
    pub name: String,
    pub display: String,
    pub group: String,
    pub color: String,
    pub skill: String,
    pub text: String,
    pub cn_id: String,
    pub art: String,
    pub costume: String,
    pub exclusive_cards: Vec<String>,
    pub preset: Vec<String>,
    /// The persona this one is an alternate form of (`高松灯（CRYCHIC）` ->
    /// `高松灯`); empty when it is a base persona.
    pub variant_of: String,
    /// Short label for compact displays of a variant (`c`).
    pub variant_tag: String,
}

impl PersonaData {
    /// Key for art and Live2D: `art` if set, else `cn_id`.
    pub fn art_id(&self) -> &str {
        if self.art.is_empty() {
            &self.cn_id
        } else {
            &self.art
        }
    }
}

/// One band of the roster (was `groups.json`; skill text + branding).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupData {
    pub name: String,
    pub short_name: String,
    pub color: String,
    pub skill: String,
    pub text: String,
    pub logo: String,
    /// Band-skill trait: members may carry each other's exclusive cards in a
    /// starting deck. Deck-legal only -- the in-match play gate is the
    /// band-skill card's `ExclusiveUse` entry (or the generic own/base rule).
    pub shares_exclusives: bool,
    /// Boot-screen art id for a home persona of this band; empty = default.
    pub boot_art: String,
}

/// The roster half of [`GameData`] -- cards / characters / groups -- handed over
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
    /// Parse the generated test fixture (`crates/game-core/tests/fixtures/
    /// roster.json`, written by `cargo run -p rules-native --example
    /// dump_roster -- --fixture`). One JSON object `{cards, characters, groups}`
    /// in the same camelCase row shapes the roster metadata carries. Tests of
    /// crates that have no ruleset (game-core / game-rules / bot-core / server)
    /// load this instead of depending on the card crates.
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
    /// the byte-stable form `cargo run -p rules-native --example dump_roster --
    /// --fixture` writes. The rules-native gate compares this against the
    /// checked-in fixture so the two can never drift.
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

/// One event card from `events.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventData {
    pub id: String,
    pub name: String,
    pub derived: bool,
    pub text: String,
    pub cell: String,
}

/// `band` value of the cards every persona may take. A roster band key (the
/// data set is Chinese), not display text.
pub const GENERAL_GROUP: &str = "通用";

/// Character → school tile-name map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchoolsData {
    /// Tile name to use when a persona's school is not in the data.
    pub fallback: String,
    pub schools: Vec<SchoolEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchoolEntry {
    pub persona: String,
    pub school: String,
}

/// Home-screen line tags per persona.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeLinesData {
    pub personas: Vec<PersonaLines>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersonaLines {
    pub name: String,
    pub tag: String,
}

/// Voice / motion lines per persona.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceLinesData {
    pub personas: Vec<PersonaVoiceLines>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersonaVoiceLines {
    pub id: String,
    pub name: String,
    pub lines: Vec<VoiceLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceLine {
    pub text: String,
    pub voice: String,
    pub motion: String,
    pub from: String,
}

/// Tunable match numbers from `match_rules.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchRulesData {
    pub note: String,
    pub money: f32,
    pub property: f32,
    pub houses: f32,
    /// Generic named rule parameters (the ruleset reads them by name).
    /// The shipped ruleset uses `rentUnitMult` as the dice-rent unit multiplier.
    pub rule_params: std::collections::BTreeMap<String, i32>,
    /// Names given to bot players, in order.
    pub bot_names: Vec<String>,
    /// Home persona of a new profile (a persona `name`).
    pub default_home_persona: String,
}

impl Default for MatchRulesData {
    fn default() -> Self {
        let mut rule_params = std::collections::BTreeMap::new();
        rule_params.insert("rentUnitMult".to_string(), 10);
        Self {
            note: String::new(),
            money: 1.0,
            property: 1.0,
            houses: 1.0,
            rule_params,
            bot_names: vec![],
            default_home_persona: String::new(),
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

/// Bump when the rules book changes; drives the "NEW" badge (`rules_version`).
pub const RULES_VERSION: i32 = 1;

/// The rule id of a persona or band skill (the crates under `rules/skills`).
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
/// The id names the event (`data/events.json`), so drawing 「对邦」 resolves the
/// rule `event:对邦`. The `event:` prefix keeps it from ever colliding with a
/// card id or a `tile:*` / `skill:*` rule id, the same way [`skill_id`] and
/// [`tile_rule_id`] do. Gaps in the text are the rule body's `TODO(规则书)`;
/// an event with no rule in the ruleset falls back to the engine's
/// `StubRules::event` (log 「还没有移植」 and file it away).
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
    pub schools: SchoolsData,
    pub home_lines: HomeLinesData,
    pub voice_lines: VoiceLinesData,
    pub match_rules: MatchRulesData,
    pub rules_text: String,
    pub rules_version: i32,
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

/// Data files, as named in `web/data/`. The roster (`cards` / `characters` /
/// `groups`) is **not** here: it comes from the ruleset (ABI v63
/// `ruleset_metadata`) or, in tests, from the generated fixture
/// (`Roster::from_fixture`), and is covered by the ruleset hash, not this one.
/// `skill_simple.json` and the card display overlay stay because they are
/// display-only and the client merges them.
pub const DATA_FILES: [&str; 8] = [
    "board.json",
    "events.json",
    "schools.json",
    "home_lines.json",
    "voice_lines.json",
    "match_rules.json",
    "rules.txt",
    // Hand-written simplified skill bodies (and the card `counterReason`
    // overlay), for the client's skill-text setting and the [反击] prompt
    // reason. Display-only: the engine reads the full skill text from the
    // roster.
    "skill_simple.json",
];

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{file}: {e}"))
}

impl GameData {
    /// The rule ids of `persona`'s own skill and of its band's skill, in that
    /// order. This is the binding: see [`skill_id`]. Empty entries (a persona
    /// or band with no skill) are skipped, so the result is 0..=2 ids.
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

    /// Is `id` a **band** skill rule (`skill:<band>:<skill>` for some band in
    /// roster groups), as against a persona skill (`skill:<persona>:<skill>`)?
    /// This is what stamps `FieldCard::group_skill` at placement, and through it
    /// what 「乐队卡 / 团卡」 crystal reads and writes land on.
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
            schools: parse("schools.json", &get("schools.json")?)?,
            home_lines: parse("home_lines.json", &get("home_lines.json")?)?,
            voice_lines: parse("voice_lines.json", &get("voice_lines.json")?)?,
            match_rules: parse("match_rules.json", &get("match_rules.json")?)?,
            rules_text: get("rules.txt")?.trim_start_matches('\u{feff}').to_string(),
            rules_version: RULES_VERSION,
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

    /// Character by `name`, first match.
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

    /// Whether `c`'s band lets its members carry each other's exclusive cards
    /// in a starting deck (deck-building only; see [`crate::deck`]).
    pub fn group_shares_exclusives(&self, c: &PersonaData) -> bool {
        self.group(&c.group).is_some_and(|b| b.shares_exclusives)
    }

    /// Band by `name`, first match.
    pub fn group(&self, name: &str) -> Option<&GroupData> {
        self.groups.iter().find(|b| b.name == name)
    }

    /// Event card by id.
    pub fn event(&self, id: &str) -> Option<&EventData> {
        self.events.iter().find(|e| e.id == id)
    }

    /// School tile name for `persona`, or [`SchoolsData::fallback`].
    pub fn school_of(&self, persona: &str) -> &str {
        self.schools
            .schools
            .iter()
            .find(|s| s.persona == persona)
            .map(|s| s.school.as_str())
            .unwrap_or(&self.schools.fallback)
    }

    /// Short name on the home screen.
    pub fn home_tag<'a>(&'a self, c: &'a PersonaData) -> &'a str {
        self.home_lines
            .personas
            .iter()
            .find(|x| x.name == c.name && !x.tag.is_empty())
            .map(|x| x.tag.as_str())
            .unwrap_or(&c.display)
    }

    /// Lines with both text and audio.
    pub fn voice_lines_for(&self, c: &PersonaData) -> Vec<&VoiceLine> {
        if c.cn_id.is_empty() {
            return vec![];
        }
        self.voice_lines
            .personas
            .iter()
            .find(|x| x.id == c.cn_id)
            .map(|x| {
                x.lines
                    .iter()
                    .filter(|l| !l.text.is_empty() && !l.voice.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}
