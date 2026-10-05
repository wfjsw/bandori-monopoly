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
    card_by_id: HashMap<String, usize>,
}

/// Data files, as named in `web/data/`.
pub const DATA_FILES: [&str; 11] = [
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
            card_by_id: HashMap::new(),
        };
        // First card wins on duplicate ids, like the C# GroupBy(...).First().
        for (i, c) in d.cards.iter().enumerate() {
            if !c.id.is_empty() {
                d.card_by_id.entry(c.id.clone()).or_insert(i);
            }
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
