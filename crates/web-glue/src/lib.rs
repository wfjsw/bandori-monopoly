//! Browser bridge over `game-core`.
//!
//! Everything crosses the boundary as JSON strings in the same shapes the server
//! uses, so the web client has one set of types for online and solo play.
//!
//! * `load_data` -- once, with the contents of `/data/*.json`
//! * deck rules and saved deck slots (`deck_*`)
//! * profile and progression (`profile_*`), persisted by the client
//! * [`SoloMatch`] -- the full match engine running locally

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use game_core::data::{CharacterData, GameData};
use game_core::deck;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::profile::{PlayerProfile, Seen};
use game_core::progression;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use game_rules::WasmRules;
use serde::Serialize;
use wasm_bindgen::prelude::*;

thread_local! {
    static DATA: RefCell<Option<Arc<GameData>>> = const { RefCell::new(None) };
}

fn data() -> Result<Arc<GameData>, JsError> {
    DATA.with(|d| d.borrow().clone())
        .ok_or_else(|| JsError::new("game data not loaded (call load_data first)"))
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn parse<T: serde::de::DeserializeOwned>(what: &str, s: &str) -> Result<T, JsError> {
    serde_json::from_str(s).map_err(|e| JsError::new(&format!("{what}: {e}")))
}

fn character<'a>(d: &'a GameData, name: &str) -> Result<&'a CharacterData, JsError> {
    d.character(name)
        .ok_or_else(|| JsError::new(&format!("unknown character {name}")))
}

/// `files_json`: `{"board.json": "<contents>", ...}` for every file in `DATA_FILES`.
#[wasm_bindgen]
pub fn load_data(files_json: &str) -> Result<(), JsError> {
    let files: HashMap<String, String> = parse("files", files_json)?;
    let d = GameData::load(|f| files.get(f).cloned().ok_or_else(|| "missing".to_string()))
        .map_err(|e| JsError::new(&e))?;
    DATA.with(|slot| *slot.borrow_mut() = Some(Arc::new(d)));
    Ok(())
}

/// The file names `load_data` needs.
#[wasm_bindgen]
pub fn data_files() -> String {
    json(&game_core::data::DATA_FILES)
}

// ------------------------------------------------------------------ decks

/// Card ids `character` may put in a deck, in display order.
#[wasm_bindgen]
pub fn deck_pool(character_name: &str) -> Result<String, JsError> {
    let d = data()?;
    let c = character(&d, character_name)?;
    Ok(json(
        &deck::pool(&d, c).iter().map(|k| &k.id).collect::<Vec<_>>(),
    ))
}

#[wasm_bindgen]
pub fn deck_preset(character_name: &str) -> Result<String, JsError> {
    let d = data()?;
    Ok(json(&deck::preset(&d, character(&d, character_name)?)))
}

/// Usable ids from `ids_json`, at most 10, in pool order.
#[wasm_bindgen]
pub fn deck_clean(character_name: &str, ids_json: &str) -> Result<String, JsError> {
    let d = data()?;
    let ids: Vec<String> = parse("ids", ids_json)?;
    Ok(json(&deck::clean(&d, character(&d, character_name)?, &ids)))
}

/// Why `character` may not use `card` (a `Msg` as JSON), or `undefined`.
#[wasm_bindgen]
pub fn deck_why_not(character_name: &str, card: &str) -> Result<Option<String>, JsError> {
    let d = data()?;
    let c = character(&d, character_name)?;
    let k = d.card(card).ok_or_else(|| JsError::new("unknown card"))?;
    Ok(deck::cant_play(&d, c, k).map(|m| json(&m)))
}

#[wasm_bindgen]
pub fn deck_slot_cards(
    profile_json: &str,
    character_name: &str,
    slot: i32,
) -> Result<String, JsError> {
    let d = data()?;
    let p: PlayerProfile = parse("profile", profile_json)?;
    Ok(json(&deck::cards(
        &d,
        &p,
        character(&d, character_name)?,
        slot,
    )))
}

/// Save a slot (an empty list deletes it). Returns the updated profile.
#[wasm_bindgen]
pub fn deck_save(
    profile_json: &str,
    character_name: &str,
    slot: i32,
    ids_json: &str,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    let ids: Vec<String> = parse("ids", ids_json)?;
    deck::save(&d, &mut p, character(&d, character_name)?, slot, &ids);
    Ok(json(&p))
}

#[wasm_bindgen]
pub fn deck_choose(profile_json: &str, character_name: &str, slot: i32) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::choose(&mut p, character(&d, character_name)?, slot);
    Ok(json(&p))
}

/// Last chosen slot if it still holds a complete deck, else 0 (preset).
#[wasm_bindgen]
pub fn deck_chosen_slot(profile_json: &str, character_name: &str) -> Result<i32, JsError> {
    let d = data()?;
    let p: PlayerProfile = parse("profile", profile_json)?;
    Ok(deck::chosen_slot(&d, &p, character(&d, character_name)?))
}

// ------------------------------------------------------------------ profile

/// `player_id`: random 9-digit string; `now`: `yyyy-MM-dd HH:mm`; `today`: `yyyy-MM-dd`.
#[wasm_bindgen]
pub fn profile_create(
    name: &str,
    player_id: &str,
    now: &str,
    today: &str,
) -> Result<String, JsError> {
    let d = data()?;
    Ok(json(&PlayerProfile::create(
        &d, name, player_id, now, today,
    )))
}

/// Parse and repair/migrate a saved profile.
#[wasm_bindgen]
pub fn profile_load(profile_json: &str) -> Result<String, JsError> {
    let d = data()?;
    let p = PlayerProfile::from_json(&d, profile_json).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(json(&p))
}

#[wasm_bindgen]
pub fn profile_refresh_daily(profile_json: &str, today: &str) -> Result<String, JsError> {
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    p.refresh_daily(today);
    Ok(json(&p))
}

#[wasm_bindgen]
pub fn profile_set_fire_per_game(profile_json: &str, count: i32) -> Result<String, JsError> {
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    p.set_fire_per_game(count);
    Ok(json(&p))
}

#[derive(Serialize)]
struct Applied {
    profile: PlayerProfile,
    reward: progression::MatchReward,
}

/// Record a finished match: `{profile, reward}`.
#[wasm_bindgen]
pub fn profile_apply_match(
    profile_json: &str,
    mode: i32,
    rank: i32,
    players: i32,
    character_name: &str,
    now: &str,
) -> Result<String, JsError> {
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    let mode = MatchMode::from_i32(mode).unwrap_or_default();
    let reward = p.apply_match(mode, rank, players, character_name, now);
    Ok(json(&Applied { profile: p, reward }))
}

fn seen(what: &str) -> Result<Seen, JsError> {
    Ok(match what {
        "gallery" => Seen::Gallery,
        "deck" => Seen::Deck,
        "rules" => Seen::Rules,
        "history" => Seen::History,
        _ => return Err(JsError::new("what: gallery | deck | rules | history")),
    })
}

#[wasm_bindgen]
pub fn profile_has_new(profile_json: &str, what: &str) -> Result<bool, JsError> {
    let p: PlayerProfile = parse("profile", profile_json)?;
    let d = data()?;
    if what == "any" {
        return Ok(p.has_any_new(&d));
    }
    Ok(p.has_new(&d, seen(what)?))
}

#[wasm_bindgen]
pub fn profile_mark_seen(profile_json: &str, what: &str) -> Result<String, JsError> {
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    let d = data()?;
    p.mark_seen(&d, seen(what)?);
    Ok(json(&p))
}

#[wasm_bindgen]
pub fn profile_level_progress(profile_json: &str) -> Result<f32, JsError> {
    let p: PlayerProfile = parse("profile", profile_json)?;
    Ok(p.level_progress())
}

#[wasm_bindgen]
pub fn exp_to_next(level: i32) -> i32 {
    progression::exp_to_next(level)
}

/// The next free bot name (from `match_rules.json`) given the names in `taken_json`.
#[wasm_bindgen]
pub fn bot_name(taken_json: &str) -> Result<String, JsError> {
    let taken: Vec<String> = parse("names", taken_json)?;
    Ok(game_core::net::bot_name(
        &data()?.match_rules.bot_names,
        taken.iter().map(String::as_str),
    ))
}

// ------------------------------------------------------------------ card modules

thread_local! {
    /// Modules the page fetched (`/assets/rules/*.wasm`), then built once.
    static RULES: std::cell::RefCell<Option<Arc<dyn CardRules>>> = const { std::cell::RefCell::new(None) };
    static PENDING: std::cell::RefCell<Option<game_rules::RulesetBuilder>> = const { std::cell::RefCell::new(None) };
}

/// Feed one card module (the bytes of `<sha>.wasm`); call `ruleset_build` after.
#[wasm_bindgen]
pub fn ruleset_add(bytes: &[u8]) -> Result<(), JsError> {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let b = p.get_or_insert_with(game_rules::Ruleset::builder);
        b.add(bytes).map_err(|e| JsError::new(&format!("{e:?}")))?;
        Ok(())
    })
}

/// Build the ruleset from the modules added so far.
#[wasm_bindgen]
pub fn ruleset_build() -> Result<usize, JsError> {
    PENDING.with(|p| {
        let built = p
            .borrow_mut()
            .take()
            .ok_or_else(|| JsError::new("ruleset_add was never called"))?;
        let set = built.build().map_err(|e| JsError::new(&format!("{e:?}")))?;
        let n = set.module_count();
        let rules = WasmRules::new(set, data()?);
        RULES.with(|r| *r.borrow_mut() = Some(Arc::new(rules)));
        Ok(n)
    })
}

fn rules() -> Result<Arc<dyn CardRules>, JsError> {
    Ok(RULES
        .with(|r| r.borrow().clone())
        .unwrap_or_else(|| Arc::new(StubRules) as Arc<dyn CardRules>))
}

// ------------------------------------------------------------------ solo match

/// A match running entirely in the browser (solo mode vs bots).
#[wasm_bindgen]
pub struct SoloMatch {
    m: Match,
}

#[wasm_bindgen]
impl SoloMatch {
    /// `members_json`: `RoomMember[]`; `mode`: 0 solo, 1 casual, 2 ranked.
    #[wasm_bindgen(constructor)]
    pub fn new(
        members_json: &str,
        seed: u32,
        mode: i32,
        weights_json: &str,
    ) -> Result<SoloMatch, JsError> {
        let members: Vec<RoomMember> = parse("members", members_json)?;
        let weights: ScoreWeights = if weights_json.is_empty() {
            ScoreWeights::default()
        } else {
            parse("weights", weights_json)?
        };
        let mode = MatchMode::from_i32(mode).unwrap_or_default();
        Ok(SoloMatch {
            m: Match::new(data()?, rules()?, &members, seed as u64, mode, weights),
        })
    }

    /// Rebuild a match from [`SoloMatch::save`] (page refresh).
    pub fn restore(json: &str) -> Result<SoloMatch, JsError> {
        Ok(SoloMatch {
            m: Match::restore(data()?, rules()?, json).map_err(|e| JsError::new(&e.to_string()))?,
        })
    }

    /// The whole match as JSON, for `restore`.
    pub fn save(&self) -> String {
        self.m.save()
    }

    pub fn tick(&mut self, dt: f32) {
        self.m.tick(dt);
    }

    /// Skip ban/pick/deck with random characters and preset decks.
    pub fn quick_start(&mut self) {
        self.m.quick_start();
    }

    /// A command (`NetMessage` JSON). Returns "" on success, else the error as
    /// `Msg` JSON (the client renders it).
    pub fn act(&mut self, member: i32, msg_json: &str) -> String {
        match serde_json::from_str::<NetMessage>(msg_json) {
            Ok(msg) => self
                .m
                .act(member, &msg)
                .err()
                .map(|e| json(&e))
                .unwrap_or_default(),
            Err(e) => json(&Msg::new("err.bad_command").text("detail", e.to_string())),
        }
    }

    /// `{state, hand, handNotes, draw, you, player_id}` -- the same shape as the
    /// server's `match` frame.
    pub fn view(&self, member: i32) -> String {
        let state = self.m.state();
        let player_id = state.player_of(member);
        json(&serde_json::json!({
            "state": state,
            "hand": self.m.hand_of(member),
            "handNotes": self.m.hand_notes_of(member),
            "draw": self.m.draw_of(member),
            "you": member,
            "playerId": player_id,
        }))
    }

    pub fn events_since(&self, last_id: i32) -> String {
        json(&self.m.events_since(last_id))
    }

    pub fn take_changed(&mut self) -> bool {
        self.m.take_changed()
    }

    pub fn ended(&self) -> bool {
        self.m.ended()
    }

    pub fn finish(&mut self) {
        self.m.finish();
    }
}
