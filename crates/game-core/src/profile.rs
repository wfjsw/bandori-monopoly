//! Player profile and settings (`PlayerProfile.cs`, `ProfileService.cs`,
//! `SoundSettings.cs`).
//!
//! Only the pure logic lives here. The caller supplies anything that came from the
//! environment in C# -- today's date (`yyyy-MM-dd`), the timestamp
//! (`yyyy-MM-dd HH:mm`) and a random 9-digit player id -- and persists the result
//! (server: file/DB; browser: IndexedDB). That keeps `game-core` wasm-safe and the
//! logic deterministic under test.

use serde::{Deserialize, Serialize};

use crate::data::{CharacterData, GameData};
use crate::progression::{
    self, MatchReward, FIRE_DAILY_MAX, FIRE_MAX_PER_GAME, MAX_LEVEL, STARS_PER_GAME,
};
use crate::MatchMode;

pub const SAVE_VERSION: i32 = 4;
/// Match history entries kept on the profile.
pub const HISTORY_CAP: usize = 30;
/// Deck name cap in **characters** (not bytes), so CJK names count as one each.
pub const DECK_NAME_MAX: usize = 24;

/// Trim and cap a user-entered deck name. Empty means "auto" -- the UI shows
/// the localized 「卡组 n」/"Deck n" from the deck's id. Duplicates are allowed.
pub fn sanitize_deck_name(name: &str) -> String {
    let t = name.trim();
    if t.is_empty() {
        return String::new();
    }
    t.chars().take(DECK_NAME_MAX).collect()
}

/// `PlayerProfile.cs` -- `profile.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlayerProfile {
    pub save_version: i32,
    pub player_name: String,
    pub player_id: String,
    pub created_at: String,
    pub level: i32,
    pub exp: i32,
    pub total_exp: i64,
    pub fire: i32,
    pub fire_date: String,
    pub fire_per_game: i32,
    pub coins: i32,
    pub stars: i32,
    pub home_character: String,
    pub games: i32,
    pub solo_games: i32,
    pub casual_games: i32,
    pub ranked_games: i32,
    pub ranked_wins: i32,
    pub seen_characters: Vec<String>,
    pub seen_cards: Vec<String>,
    pub seen_rules_version: i32,
    pub history: Vec<MatchRecord>,
    pub unread_result: bool,
    pub character_stats: Vec<CharacterStat>,
    pub decks: Vec<SavedDeck>,
    pub deck_choices: Vec<DeckChoice>,
    /// `"<character name>=<live2d model id>"` for non-default Live2D variants.
    pub live2d_picks: Vec<String>,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            save_version: SAVE_VERSION,
            player_name: String::new(),
            player_id: String::new(),
            created_at: String::new(),
            level: 0,
            exp: 0,
            total_exp: 0,
            fire: 0,
            fire_date: String::new(),
            fire_per_game: 1,
            coins: 0,
            stars: 0,
            home_character: String::new(),
            games: 0,
            solo_games: 0,
            casual_games: 0,
            ranked_games: 0,
            ranked_wins: 0,
            seen_characters: vec![],
            seen_cards: vec![],
            seen_rules_version: 0,
            history: vec![],
            unread_result: false,
            character_stats: vec![],
            decks: vec![],
            deck_choices: vec![],
            live2d_picks: vec![],
        }
    }
}

/// `MatchRecord.cs` -- one line of match history. `mode` is an integer on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchRecord {
    pub time: String,
    pub mode: MatchMode,
    pub ranked: bool,
    pub rank: i32,
    pub players: i32,
    pub character: String,
    pub exp: i32,
    pub fire_used: i32,
    pub coins: i32,
    pub stars: i32,
    pub level_after: i32,
}

/// `CharacterStat.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterStat {
    pub name: String,
    pub uses: i32,
    pub firsts: i32,
}

/// `SavedDeck.cs` -- one named deck of one character.
///
/// `slot` is the stable deck id within the character (it used to be the fixed
/// 1..=3 slot index; profiles from that era keep their numbers as ids). It is
/// what [`DeckChoice`] remembers. Display order is the order of
/// [`PlayerProfile::decks`] itself, so reordering swaps entries without
/// renumbering anyone.
///
/// `name` is user-editable UI metadata: empty means "auto" and the client
/// renders the localized 「卡组 n」/"Deck n" from the id. Never enters match
/// state or records.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedDeck {
    pub character: String,
    pub slot: i32,
    pub name: String,
    pub cards: Vec<String>,
}

/// `DeckChoice.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckChoice {
    pub character: String,
    pub slot: i32,
}

/// "NEW" badge areas (`ProfileService.HasNew` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Gallery,
    Deck,
    Rules,
    History,
}

impl PlayerProfile {
    /// `ProfileService.Create` -- a fresh profile with everything marked seen.
    /// `player_id`: random 9-digit string; `now`: `yyyy-MM-dd HH:mm`; `today`: `yyyy-MM-dd`.
    pub fn create(data: &GameData, name: &str, player_id: &str, now: &str, today: &str) -> Self {
        let name = name.trim();
        let mut p = Self {
            player_name: name.into(),
            player_id: player_id.into(),
            created_at: now.into(),
            fire_date: today.into(),
            ..Self::default()
        };
        p.mark_all_seen(data);
        p.normalize(data);
        p
    }

    /// `ProfileService.Normalize` -- repair and migrate a loaded profile. A missing
    /// or unknown home character falls back to the data's default.
    pub fn normalize(&mut self, data: &GameData) {
        self.level = self.level.clamp(0, MAX_LEVEL);
        self.fire_per_game = self.fire_per_game.clamp(0, FIRE_MAX_PER_GAME);
        if data.character(&self.home_character).is_none() {
            self.home_character = data.match_rules.default_home_character.clone();
        }
        if self.save_version < 2 {
            // v1 only had the `ranked` flag.
            for r in &mut self.history {
                r.mode = if r.ranked {
                    MatchMode::Ranked
                } else {
                    MatchMode::Casual
                };
            }
            self.save_version = 2;
        }
        if self.save_version < 3 {
            self.save_version = 3;
        }
        if self.save_version < 4 {
            // v3 and earlier saved three fixed slots per character and no deck
            // names. The slot numbers become the stable deck ids; `name` stays
            // empty ("auto", displayed as 「卡组 n」/"Deck n" from the id), so
            // slot n reads exactly as it used to. Sort per character by slot so
            // the old 1,2,3 order is the display order.
            self.decks
                .sort_by(|a, b| (&a.character, a.slot).cmp(&(&b.character, b.slot)));
            self.save_version = 4;
        }
        self.repair_decks();
    }

    /// Unique positive deck ids per character and sanitized names, on every
    /// load (hand-edited JSON included).
    fn repair_decks(&mut self) {
        use std::collections::{BTreeSet, HashMap};
        let mut used: HashMap<String, BTreeSet<i32>> = HashMap::new();
        for d in &mut self.decks {
            d.name = sanitize_deck_name(&d.name);
            let set = used.entry(d.character.clone()).or_default();
            if d.slot > 0 && set.insert(d.slot) {
                continue;
            }
            let mut n = set.iter().next_back().copied().unwrap_or(0) + 1;
            while !set.insert(n) {
                n += 1;
            }
            d.slot = n;
        }
    }

    /// Parse `profile.json` and normalize it.
    pub fn from_json(data: &GameData, text: &str) -> Result<Self, serde_json::Error> {
        let mut p: Self = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
        p.normalize(data);
        Ok(p)
    }

    pub fn rename(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        self.player_name = name.into();
        true
    }

    /// Refill fire once per day. Returns whether it refilled.
    pub fn refresh_daily(&mut self, today: &str) -> bool {
        if self.fire_date == today {
            return false;
        }
        self.fire = FIRE_DAILY_MAX;
        self.fire_date = today.into();
        true
    }

    pub fn set_fire_per_game(&mut self, count: i32) {
        self.fire_per_game = count.clamp(0, FIRE_MAX_PER_GAME);
    }

    pub fn set_home_character(&mut self, name: &str) -> bool {
        if name.is_empty() || self.home_character == name {
            return false;
        }
        self.home_character = name.into();
        true
    }

    /// Progress toward the next level, 0..=1 (1 at the cap).
    pub fn level_progress(&self) -> f32 {
        let need = progression::exp_to_next(self.level);
        if need > 0 {
            (self.exp as f32 / need as f32).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    fn add_exp(&mut self, amount: i32) {
        self.total_exp += amount as i64;
        self.exp += amount;
        while self.level < MAX_LEVEL && self.exp >= progression::exp_to_next(self.level) {
            self.exp -= progression::exp_to_next(self.level);
            self.level += 1;
        }
        if self.level >= MAX_LEVEL {
            self.exp = 0;
        }
    }

    /// `ProfileService.ApplyMatch` -- spend fire, grant EXP/stars/coins, update
    /// counters and stats, and prepend to history (capped at 30).
    pub fn apply_match(
        &mut self,
        mode: MatchMode,
        rank: i32,
        players: i32,
        character: &str,
        now: &str,
    ) -> MatchReward {
        let ranked = mode == MatchMode::Ranked;
        let mut r = MatchReward {
            mode,
            rank,
            players,
            ..MatchReward::default()
        };
        r.level_before = self.level;
        r.progress_before = self.level_progress();
        r.base_exp = progression::base_exp(ranked, rank, players);
        r.fire_used = self.fire_per_game.min(self.fire);
        r.multiplier = progression::exp_multiplier(r.fire_used);
        r.exp = r.base_exp * r.multiplier;
        self.fire -= r.fire_used;
        self.add_exp(r.exp);
        r.level_after = self.level;
        r.progress_after = self.level_progress();
        r.stars = STARS_PER_GAME;
        self.stars += r.stars;
        if ranked {
            let before = self.coins;
            self.coins = (self.coins + progression::ranked_coins(rank, players)).max(0);
            r.coins = self.coins - before;
        }
        self.games += 1;
        match mode {
            MatchMode::Ranked => {
                self.ranked_games += 1;
                if rank == 1 {
                    self.ranked_wins += 1;
                }
            }
            MatchMode::Casual => self.casual_games += 1,
            MatchMode::Solo => self.solo_games += 1,
        }
        if !character.is_empty() {
            let i = match self
                .character_stats
                .iter()
                .position(|s| s.name == character)
            {
                Some(i) => i,
                None => {
                    self.character_stats.push(CharacterStat {
                        name: character.into(),
                        ..Default::default()
                    });
                    self.character_stats.len() - 1
                }
            };
            self.character_stats[i].uses += 1;
            if rank == 1 {
                self.character_stats[i].firsts += 1;
            }
        }
        self.history.insert(
            0,
            MatchRecord {
                time: now.into(),
                mode,
                ranked,
                rank,
                players,
                character: character.into(),
                exp: r.exp,
                fire_used: r.fire_used,
                coins: r.coins,
                stars: r.stars,
                level_after: self.level,
            },
        );
        self.history.truncate(HISTORY_CAP);
        self.unread_result = true;
        r
    }

    pub fn stat_of(&self, character: &str) -> Option<&CharacterStat> {
        self.character_stats.iter().find(|s| s.name == character)
    }

    pub fn has_new(&self, data: &GameData, what: Seen) -> bool {
        match what {
            Seen::Gallery => data
                .characters
                .iter()
                .any(|c| !self.seen_characters.contains(&c.name)),
            Seen::Deck => data.cards.iter().any(|c| !self.seen_cards.contains(&c.id)),
            Seen::Rules => data.rules_version > self.seen_rules_version,
            Seen::History => self.unread_result,
        }
    }

    pub fn has_any_new(&self, data: &GameData) -> bool {
        [Seen::Gallery, Seen::Deck, Seen::Rules, Seen::History]
            .into_iter()
            .any(|s| self.has_new(data, s))
    }

    pub fn is_new_character(&self, name: &str) -> bool {
        !self.seen_characters.iter().any(|n| n == name)
    }

    pub fn mark_character_seen(&mut self, name: &str) -> bool {
        if self.is_new_character(name) {
            self.seen_characters.push(name.into());
            true
        } else {
            false
        }
    }

    pub fn mark_seen(&mut self, data: &GameData, what: Seen) {
        match what {
            Seen::Gallery => {
                for c in &data.characters {
                    if !self.seen_characters.contains(&c.name) {
                        self.seen_characters.push(c.name.clone());
                    }
                }
            }
            Seen::Deck => {
                for c in &data.cards {
                    if !self.seen_cards.contains(&c.id) {
                        self.seen_cards.push(c.id.clone());
                    }
                }
            }
            Seen::Rules => self.seen_rules_version = data.rules_version,
            Seen::History => self.unread_result = false,
        }
    }

    fn mark_all_seen(&mut self, data: &GameData) {
        self.seen_characters.clear();
        self.seen_cards.clear();
        self.mark_seen(data, Seen::Gallery);
        self.mark_seen(data, Seen::Deck);
        self.seen_rules_version = data.rules_version;
    }

    /// `ProfileService.Live2DFor` -- chosen Live2D variant, or the character's own.
    /// `options`: ids valid for this character (`Live2DPortrait.OptionsFor(artId)`).
    pub fn live2d_for<'a>(&'a self, c: &'a CharacterData, options: &[&str]) -> &'a str {
        let prefix = format!("{}=", c.name);
        match self
            .live2d_picks
            .iter()
            .find_map(|p| p.strip_prefix(&prefix))
        {
            Some(id) if options.contains(&id) => id,
            _ => c.art_id(),
        }
    }

    /// `ProfileService.SetLive2D`. Returns whether the profile changed.
    pub fn set_live2d(&mut self, c: &CharacterData, id: &str, options: &[&str]) -> bool {
        if self.live2d_for(c, options) == id {
            return false;
        }
        let prefix = format!("{}=", c.name);
        self.live2d_picks.retain(|p| !p.starts_with(&prefix));
        if id != c.art_id() {
            self.live2d_picks.push(format!("{prefix}{id}"));
        }
        true
    }
}

/// `SoundSettings.cs` -- `settings.json`. Levels are 0-10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SoundSettings {
    pub bgm: i32,
    pub voice: i32,
    pub se: i32,
    pub skip_line: bool,
    pub greet: bool,
    pub idle_talk: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            bgm: 5,
            voice: 10,
            se: 7,
            skip_line: true,
            greet: true,
            idle_talk: true,
        }
    }
}

impl SoundSettings {
    pub fn bgm_volume(&self) -> f32 {
        self.bgm as f32 / 10.0
    }
    pub fn voice_volume(&self) -> f32 {
        self.voice as f32 / 10.0
    }
    pub fn se_volume(&self) -> f32 {
        self.se as f32 / 10.0
    }
}
