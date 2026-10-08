//! Static game data (`web/data/*.json`) and the lookups `BandoriDatabase` provides.
//!
//! Field names and defaults mirror the C# `[Serializable]` classes exactly; Unity's
//! `JsonUtility` fills missing fields from the C# initializers, so `#[serde(default)]`
//! does the same here. Unknown keys (e.g. the `_说明` notes) are ignored.
//!
//! `game-core` never touches the filesystem: [`GameData::load`] takes a reader so the
//! server (fs) and the browser (fetch) can both supply the files.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::deck_book::{DeckBook, DECK_BOOK_FILE};
use crate::strategy::{StrategyBook, STRATEGY_BOOK_FILE};

/// `TileData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TileData {
    pub index: i32,
    pub name: String,
    pub short_name: String,
    pub kind: String,
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
    /// The four corner tiles (start, café, music store, Ryuseido).
    pub fn is_corner(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "circle" | "cafe" | "edogawa" | "ryuseido"
        )
    }

    pub fn is_buyable(&self) -> bool {
        matches!(self.kind.as_str(), "property" | "ring")
    }

    pub fn is_agent(&self) -> bool {
        self.kind == "agent"
    }
}

/// The rule id that settles a tile of this `kind` (`docs/TILES.md`). Empty when
/// the kind has no rule (unknown kinds fall through to the engine's built-in
/// settlement). `cafe` and `ryuseido` share `tile:event` -- the rulebook gives
/// them one passage (「CiRCLE咖啡厅和流星堂的的[结算]是…」).
pub fn tile_rule_id(kind: &str) -> &'static str {
    match kind {
        "property" => "tile:property",
        "ring" => "tile:ring",
        "agent" => "tile:agent",
        "circle" => "tile:circle",
        "edogawa" => "tile:edogawa",
        "cafe" | "ryuseido" => "tile:event",
        _ => "",
    }
}

/// Board-wide **mark-owner** rules (`mark:*`) -- one instance each on the
/// neutral board owner, governing no single tile (unlike [`tile_rule_id`]'s
/// `tile:*`, which binds one per board tile of the kind). `mark:cp` is the
/// [CP点] tile-mark owner (`rules/tiles/src/cp.rs`, `docs/TILES.md`).
pub fn mark_rule_ids() -> &'static [&'static str] {
    &["mark:cp"]
}

/// `CardData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CardData {
    pub id: String,
    pub band: String,
    pub name: String,
    pub raw_name: String,
    pub owner: String,
    pub derived: bool,
    pub tags: Vec<String>,
    pub text: String,
    pub cell: String,
}

impl CardData {
    /// `CardData.Title` -- the name, or the id for unnamed cards (the client shows
    /// its own "unnamed" label).
    pub fn title(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }

    /// Usable by every band (the data's general band, [`GENERAL_BAND`]).
    pub fn general(&self) -> bool {
        self.band == GENERAL_BAND
    }

    /// Bound to one character (`owner` set).
    pub fn exclusive(&self) -> bool {
        !self.owner.is_empty()
    }
}

/// `CharacterData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CharacterData {
    pub name: String,
    pub display: String,
    pub band: String,
    pub color: String,
    pub skill: String,
    pub text: String,
    pub cn_id: String,
    pub art: String,
    pub costume: String,
    pub exclusive_cards: Vec<String>,
    pub preset: Vec<String>,
}

impl CharacterData {
    /// `CharacterData.ArtId` -- key for art and Live2D: `art` if set, else `cnId`.
    pub fn art_id(&self) -> &str {
        if self.art.is_empty() {
            &self.cn_id
        } else {
            &self.art
        }
    }
}

/// `BandData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BandData {
    pub name: String,
    pub short_name: String,
    pub color: String,
    pub skill: String,
    pub text: String,
    pub logo: String,
}

/// `EventData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventData {
    pub id: String,
    pub name: String,
    pub derived: bool,
    pub text: String,
    pub cell: String,
}

/// `band` value of the cards every character may take. A data key from
/// `cards.json` (the data set is Chinese), not display text.
pub const GENERAL_BAND: &str = "通用";

/// `SchoolsData.cs` / `SchoolEntry.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchoolsData {
    /// Tile name to use when a character's school is not in the data.
    pub fallback: String,
    pub schools: Vec<SchoolEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchoolEntry {
    pub character: String,
    pub school: String,
}

/// `SongCardsData.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SongCardsData {
    pub cards: Vec<String>,
}

/// `HomeLinesData.cs` / `CharacterLines.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeLinesData {
    pub characters: Vec<CharacterLines>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterLines {
    pub name: String,
    pub tag: String,
}

/// `VoiceLinesData.cs` / `CharacterVoiceLines.cs` / `VoiceLine.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceLinesData {
    pub characters: Vec<CharacterVoiceLines>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterVoiceLines {
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

/// `MatchRulesData.cs`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchRulesData {
    pub note: String,
    pub money: f32,
    pub property: f32,
    pub houses: f32,
    pub ring_multiplier: i32,
    /// Names given to bot players, in order (`SoloMatch.BotNames`).
    pub bot_names: Vec<String>,
    /// Home character of a new profile (a character `name`).
    pub default_home_character: String,
}

impl Default for MatchRulesData {
    fn default() -> Self {
        Self {
            note: String::new(),
            money: 1.0,
            property: 1.0,
            houses: 1.0,
            ring_multiplier: 10,
            bot_names: vec![],
            default_home_character: String::new(),
        }
    }
}

#[derive(Deserialize)]
struct BoardFile {
    #[serde(default)]
    tiles: Vec<TileData>,
}

#[derive(Deserialize)]
struct CardFile {
    #[serde(default)]
    cards: Vec<CardData>,
}

#[derive(Deserialize)]
struct CharacterFile {
    #[serde(default)]
    characters: Vec<CharacterData>,
}

#[derive(Deserialize)]
struct BandFile {
    #[serde(default)]
    bands: Vec<BandData>,
}

#[derive(Deserialize)]
struct EventFile {
    #[serde(default)]
    events: Vec<EventData>,
}

/// Bump when the rules book changes; drives the "NEW" badge (`BandoriDatabase.rulesVersion`).
pub const RULES_VERSION: i32 = 1;

/// The rule id of a character or band skill (the crates under `rules/skills`).
///
/// The id **names its owner**, which is the binding: a player's two skills
/// follow mechanically from the character they picked, and a skill can be
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

/// All static game data plus the `BandoriDatabase` lookups.
#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub tiles: Vec<TileData>,
    pub cards: Vec<CardData>,
    pub characters: Vec<CharacterData>,
    pub bands: Vec<BandData>,
    pub events: Vec<EventData>,
    pub schools: SchoolsData,
    pub song_cards: SongCardsData,
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

/// Data files, as named in `web/data/`.
pub const DATA_FILES: [&str; 12] = [
    "board.json",
    "cards.json",
    "characters.json",
    "bands.json",
    "events.json",
    "schools.json",
    "song_cards.json",
    "home_lines.json",
    "voice_lines.json",
    "match_rules.json",
    "rules.txt",
    // Hand-written simplified skill bodies, for the client's skill-text
    // setting. Display-only: the engine reads the originals in `characters.json`
    // / `bands.json`.
    "skill_simple.json",
];

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{file}: {e}"))
}

impl GameData {
    /// The rule ids of `character`'s own skill and of its band's skill, in that
    /// order. This is the binding: see [`skill_id`]. Empty entries (a character
    /// or band with no skill) are skipped, so the result is 0..=2 ids.
    pub fn skill_rules_of(&self, character: &str) -> Vec<String> {
        let Some(c) = self.characters.iter().find(|c| c.name == character) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if !c.skill.is_empty() {
            out.push(skill_id(&c.name, &c.skill));
        }
        if let Some(b) = self.bands.iter().find(|b| b.name == c.band) {
            if !b.skill.is_empty() {
                out.push(skill_id(&b.name, &b.skill));
            }
        }
        out
    }

    /// Is `id` a **band** skill rule (`skill:<band>:<skill>` for some band in
    /// `bands.json`), as against a character skill (`skill:<character>:<skill>`)?
    /// This is what stamps `FieldCard::band_skill` at placement, and through it
    /// what 「乐队卡 / 团卡」 crystal reads and writes land on.
    pub fn is_band_skill(&self, id: &str) -> bool {
        self.bands
            .iter()
            .any(|b| !b.skill.is_empty() && skill_id(&b.name, &b.skill) == id)
    }

    /// Load every file in [`DATA_FILES`] through `read(file_name) -> contents`.
    pub fn load(mut read: impl FnMut(&str) -> Result<String, String>) -> Result<Self, String> {
        let mut get = |f: &str| read(f).map_err(|e| format!("{f}: {e}"));
        let mut d = GameData {
            tiles: parse::<BoardFile>("board.json", &get("board.json")?)?.tiles,
            cards: parse::<CardFile>("cards.json", &get("cards.json")?)?.cards,
            characters: parse::<CharacterFile>("characters.json", &get("characters.json")?)?
                .characters,
            bands: parse::<BandFile>("bands.json", &get("bands.json")?)?.bands,
            events: parse::<EventFile>("events.json", &get("events.json")?)?.events,
            schools: parse("schools.json", &get("schools.json")?)?,
            song_cards: parse("song_cards.json", &get("song_cards.json")?)?,
            home_lines: parse("home_lines.json", &get("home_lines.json")?)?,
            voice_lines: parse("voice_lines.json", &get("voice_lines.json")?)?,
            match_rules: parse("match_rules.json", &get("match_rules.json")?)?,
            rules_text: get("rules.txt")?.trim_start_matches('\u{feff}').to_string(),
            rules_version: RULES_VERSION,
            deck_book: DeckBook::default(),
            strategy_book: StrategyBook::default(),
            card_by_id: HashMap::new(),
        };
        // First card wins on duplicate ids, like the C# GroupBy(...).First().
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

    /// `BandoriDatabase.Card(id)`
    pub fn card(&self, id: &str) -> Option<&CardData> {
        self.card_by_id.get(id).map(|&i| &self.cards[i])
    }

    /// `BandoriDatabase.Character(name)` -- by `name`, first match.
    pub fn character(&self, name: &str) -> Option<&CharacterData> {
        self.characters.iter().find(|c| c.name == name)
    }

    /// The base character of a variant (`X（CRYCHIC）` -> `X`): the longest other
    /// character name this one starts with. Itself when it is not a variant.
    pub fn base_character<'a>(&'a self, name: &'a str) -> &'a str {
        self.characters
            .iter()
            .filter(|c| c.name != name && name.starts_with(c.name.as_str()))
            .max_by_key(|c| c.name.len())
            .map_or(name, |c| c.name.as_str())
    }

    /// `BandoriDatabase.Band(name)`
    pub fn band(&self, name: &str) -> Option<&BandData> {
        self.bands.iter().find(|b| b.name == name)
    }

    /// `BandoriDatabase.Event(id)`
    pub fn event(&self, id: &str) -> Option<&EventData> {
        self.events.iter().find(|e| e.id == id)
    }

    /// `BandoriDatabase.SchoolOf(character)`
    pub fn school_of(&self, character: &str) -> &str {
        self.schools
            .schools
            .iter()
            .find(|s| s.character == character)
            .map(|s| s.school.as_str())
            .unwrap_or(&self.schools.fallback)
    }

    /// `BandoriDatabase.IsSongCard(card)` -- matched by title.
    pub fn is_song_card(&self, card: &CardData) -> bool {
        self.song_cards.cards.iter().any(|t| t == card.title())
    }

    /// `BandoriDatabase.HomeTag(c)` -- short name on the home screen.
    pub fn home_tag<'a>(&'a self, c: &'a CharacterData) -> &'a str {
        self.home_lines
            .characters
            .iter()
            .find(|x| x.name == c.name && !x.tag.is_empty())
            .map(|x| x.tag.as_str())
            .unwrap_or(&c.display)
    }

    /// `BandoriDatabase.VoiceLinesFor(c)` -- lines with both text and audio.
    pub fn voice_lines_for(&self, c: &CharacterData) -> Vec<&VoiceLine> {
        if c.cn_id.is_empty() {
            return vec![];
        }
        self.voice_lines
            .characters
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
