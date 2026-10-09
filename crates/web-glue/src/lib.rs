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

use game_core::data::{CharacterData, GameData, DATA_FILES};
use game_core::deck;
use game_core::engine::{CardRules, Match, StubRules, SAVE_VERSION};
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::profile::{PlayerProfile, Seen};
use game_core::progression;
use game_core::record::{
    decode_record, encode_record_zst, parse_header, parse_header_bytes, parse_record, EngineStamp,
    MatchSetup, RecordedMatch, Replayer, RECORD_VERSION,
};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use game_rules::WasmRules;
use serde::Serialize;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// The browser engine is long-lived and allocates on every tick, so the
/// allocator is a real cost. wasm32 defaults to dlmalloc; talc's dynamic wasm
/// heap is smaller and faster for this shape of load. (talc 5.x named the old
/// `TalckWasm::new_global` API away -- `new_wasm_dynamic_allocator` is its
/// replacement, and the crate is pinned in Cargo.toml.)
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

thread_local! {
    static DATA: RefCell<Option<Arc<GameData>>> = const { RefCell::new(None) };
    /// Sha256 over the `DATA_FILES` contents in order, captured by [`load_data`].
    static DATA_SHA: RefCell<String> = const { RefCell::new(String::new()) };
    /// `Ruleset::sha256()` of the last [`ruleset_build`]; `"stub"` when the
    /// page runs without card modules.
    static RULESET_SHA: RefCell<String> = const { RefCell::new(String::new()) };
    /// sha256 of this build's `glue.js` + `glue_bg.wasm` (hex), pushed by
    /// [`set_glue_sha`] from `webui/src/wasm/engine_id.json`. The last input
    /// of the engine-bundle id (`game_core::record::bundle_id`); empty means
    /// "no bundle identity" and the stamp's `bundle` stays empty.
    static GLUE_SHA: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The stable replay interface every archived engine bundle exposes
/// (`docs/REPLAY.md` §9). Bump only when the frozen surface below changes in
/// a way an older driver would notice; additive methods do not bump it.
pub const REPLAY_API_VERSION: u32 = 1;

/// The frozen replay surface (v1): `ReplayMatch::{from_record,
/// from_record_bytes, header, compat, step, next_input, index, seek, turns,
/// total_ticks, view, events_since, take_changed, ended, status}` plus
/// `record_header` / `record_header_bytes`. The current UI drives any
/// archived bundle through exactly this set.
#[wasm_bindgen]
pub fn replay_api_version() -> u32 {
    REPLAY_API_VERSION
}

/// Whether this engine was built with `cfg(debug_assertions)` and therefore
/// carries the console cheats (`game-core`'s `engine/debug.rs`). The web
/// console hides its cheat commands when this is false (`docs/SERVER.md`);
/// `tools/build-glue.mjs` picks the cargo profile from `NODE_ENV`, so a
/// production build answers `false` and a dev build `true`.
#[wasm_bindgen]
pub fn cheats_enabled() -> bool {
    cfg!(debug_assertions)
}

/// Record the glue identity for [`engine_stamp`] / record export: the hex
/// sha256 of this build's `glue.js` bytes followed by its `glue_bg.wasm`
/// bytes. The webui reads it from `webui/src/wasm/engine_id.json` (written by
/// `tools/build-glue.mjs`) once, before any match is recorded.
#[wasm_bindgen]
pub fn set_glue_sha(sha: &str) {
    GLUE_SHA.with(|s| *s.borrow_mut() = sha.to_string());
}

fn glue_sha() -> String {
    GLUE_SHA.with(|s| s.borrow().clone())
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

/// `files_json`: `{"board.json": "<contents>", ...}` for every file in
/// `DATA_FILES`, plus the optional `deck_book.json` (bot deck book,
/// `docs/BOT.md` §3.7) when the client has one.
#[wasm_bindgen]
pub fn load_data(files_json: &str) -> Result<(), JsError> {
    let files: HashMap<String, String> = parse("files", files_json)?;
    let d = GameData::load(|f| files.get(f).cloned().ok_or_else(|| "missing".to_string()))
        .map_err(|e| JsError::new(&e))?;
    // Stamp input: sha256 over the contents in `DATA_FILES` order, so a record
    // can tell whether the board / card data it was written against is here.
    // A leading UTF-8 BOM is not content: the browser's decoder strips it and
    // `read_to_string` keeps it, so hash without it to get one recipe.
    let mut hasher = Sha256::new();
    for name in DATA_FILES {
        if let Some(contents) = files.get(name) {
            hasher.update(contents.strip_prefix('\u{feff}').unwrap_or(contents).as_bytes());
        }
    }
    let sha = hex(&hasher.finalize());
    DATA_SHA.with(|s| *s.borrow_mut() = sha);
    DATA.with(|slot| *slot.borrow_mut() = Some(Arc::new(d)));
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn data_sha() -> String {
    DATA_SHA.with(|s| s.borrow().clone())
}

fn ruleset_sha() -> String {
    RULESET_SHA.with(|s| {
        let s = s.borrow().clone();
        if s.is_empty() {
            "stub".to_string()
        } else {
            s
        }
    })
}

/// The [`EngineStamp`] of the engine that is running right now: format
/// versions from game-core, the ABI from `card-sdk` via game-rules, and the
/// data / ruleset hashes captured by [`load_data`] and [`ruleset_build`].
/// `bundle` is the engine-bundle id (`game_core::record::bundle_id`) once
/// [`set_glue_sha`] has run -- what a record needs in order to name the
/// archived bundle that can replay it (`docs/REPLAY.md` §9).
#[wasm_bindgen]
pub fn engine_stamp() -> String {
    let stamp = EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: game_rules::ABI_VERSION as u32,
        ruleset_sha256: ruleset_sha(),
        data_sha256: data_sha(),
        engine: "game-core".into(),
        build: env!("CARGO_PKG_VERSION").into(),
        bundle: String::new(),
    };
    let bundle = game_core::record::bundle_id(&glue_sha(), &stamp);
    json(&EngineStamp { bundle, ..stamp })
}

fn stamp() -> EngineStamp {
    parse::<EngineStamp>("stamp", &engine_stamp()).unwrap_or_default()
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

/// The deck 托管 / a standard bot should submit at this table: the deck book's
/// entry for the public key when it has one, else the character's preset
/// (`docs/BOT.md` §3.7). Pure -- no RNG.
///
/// * `seat` -- own seat (turn order index).
/// * `opponents_json` -- the other seats' characters in seat order
///   (`["...", ...]`). Never an opponent's deck.
///
/// The book must match the running ruleset's hash (the one `ruleset_build`
/// produced, `"stub"` without card modules) and the `standard` policy;
/// otherwise the whole book is ignored and this is just [`deck_preset`].
#[wasm_bindgen]
pub fn deck_suggest(
    character_name: &str,
    seat: i32,
    opponents_json: &str,
) -> Result<String, JsError> {
    let d = data()?;
    let c = character(&d, character_name)?;
    let opponents: Vec<String> = parse("opponents", opponents_json)?;
    Ok(json(&game_core::deck_book::suggest(
        &d,
        c,
        seat.max(0) as usize,
        &opponents,
        &ruleset_sha(),
    )))
}

/// Usable ids from `ids_json`, at most 10, in pool order.
#[wasm_bindgen]
pub fn deck_clean(character_name: &str, ids_json: &str) -> Result<String, JsError> {
    let d = data()?;
    let ids: Vec<String> = parse("ids", ids_json)?;
    Ok(json(&deck::clean(&d, character(&d, character_name)?, &ids)))
}

/// The standard seat's resolved **strategy parameters** for this public table
/// (`docs/BOT.md` §3.8): the strategy book's entry when it has one, else
/// [`game_core::strategy::StrategyParams::default`] (today's constants). One
/// source of truth for the browser 托管 (`autopilot.ts`), exactly like
/// [`deck_suggest`]. Pure -- no RNG.
///
/// * `seat` -- own seat (turn order index). The strategy key is public and
///   seat-less (`(character, opponents' bands)` → `(character)` → `(band)`);
///   the argument mirrors [`deck_suggest`]'s shape.
/// * `opponents_json` -- the other seats' characters in seat order. Only
///   their bands enter the key.
///
/// The book must match the running ruleset's hash, the `standard` policy and
/// this build's `params_version`; otherwise the whole book is ignored and
/// this is just the defaults.
#[wasm_bindgen]
pub fn strategy_for(
    character_name: &str,
    seat: i32,
    opponents_json: &str,
) -> Result<String, JsError> {
    let d = data()?;
    let c = character(&d, character_name)?;
    let opponents: Vec<String> = parse("opponents", opponents_json)?;
    let _ = seat;
    Ok(json(&d.strategy_book.resolve(
        &d,
        c,
        &opponents,
        None,
        Some(&ruleset_sha()),
    )))
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

/// Save cards into an existing deck (an empty list clears it; `deck_delete`
/// removes it). Returns the updated profile.
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

/// Saved custom decks of `character`, in display order:
/// `[{"id": <int>, "name": <str>, "cards": [id, ...]}, ...]`. `name` is the
/// user-editable label ("" = auto, shown as 「卡组 n」/"Deck n"); `cards` are
/// cleaned to what is usable today. Id 0 (the preset) is not included.
#[wasm_bindgen]
pub fn deck_list(profile_json: &str, character_name: &str) -> Result<String, JsError> {
    let d = data()?;
    let p: PlayerProfile = parse("profile", profile_json)?;
    let c = character(&d, character_name)?;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Entry {
        id: i32,
        name: String,
        cards: Vec<String>,
    }
    let list: Vec<Entry> = deck::list(&p, c)
        .into_iter()
        .map(|sd| Entry {
            id: sd.slot,
            name: sd.name.clone(),
            cards: deck::cards(&d, &p, c, sd.slot),
        })
        .collect();
    Ok(json(&list))
}

/// Create a deck from `ids_json` (may be empty) named `name` ("" = auto).
/// Returns the updated profile (no-op past `deck::DECKS_MAX`).
#[wasm_bindgen]
pub fn deck_create(
    profile_json: &str,
    character_name: &str,
    name: &str,
    ids_json: &str,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    let ids: Vec<String> = parse("ids", ids_json)?;
    deck::create(&d, &mut p, character(&d, character_name)?, name, &ids);
    Ok(json(&p))
}

/// Copy deck `slot` to a new deck right after it (auto-named). Returns the
/// updated profile.
#[wasm_bindgen]
pub fn deck_duplicate(
    profile_json: &str,
    character_name: &str,
    slot: i32,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::duplicate(&d, &mut p, character(&d, character_name)?, slot);
    Ok(json(&p))
}

/// Rename deck `slot` ("" restores the auto name; duplicates allowed).
/// Returns the updated profile.
#[wasm_bindgen]
pub fn deck_rename(
    profile_json: &str,
    character_name: &str,
    slot: i32,
    name: &str,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::rename(&mut p, character(&d, character_name)?, slot, name);
    Ok(json(&p))
}

/// Delete deck `slot` (a chosen default pointing at it falls back to the
/// preset). Returns the updated profile.
#[wasm_bindgen]
pub fn deck_delete(
    profile_json: &str,
    character_name: &str,
    slot: i32,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::delete(&mut p, character(&d, character_name)?, slot);
    Ok(json(&p))
}

/// Move deck `slot` by `delta` places in display order (-1 up, +1 down).
/// Returns the updated profile.
#[wasm_bindgen]
pub fn deck_move(
    profile_json: &str,
    character_name: &str,
    slot: i32,
    delta: i32,
) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::move_by(&mut p, character(&d, character_name)?, slot, delta);
    Ok(json(&p))
}

#[wasm_bindgen]
pub fn deck_choose(profile_json: &str, character_name: &str, slot: i32) -> Result<String, JsError> {
    let d = data()?;
    let mut p: PlayerProfile = parse("profile", profile_json)?;
    deck::choose(&mut p, character(&d, character_name)?, slot);
    Ok(json(&p))
}

/// Last chosen deck id if it still holds a complete deck, else 0 (preset).
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
    static RULES: std::cell::RefCell<Option<Arc<game_rules::WasmRules>>> = const { std::cell::RefCell::new(None) };
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

/// Feed the precompiled-condition blob (`conds-<sha>.bin`, postcard) that
/// `tools/build-ruleset.mjs` ships beside `index.json` (`docs/GUARDS.md` §8.2).
/// Required whenever any loaded card declares a `pre`: this build links
/// `rules-cond` with `runtime-only` (no CEL parser, §8.3), so a source-only
/// condition is a loud `ruleset_build` error and never "treat as true".
/// Call after `ruleset_add`, before `ruleset_build`.
#[wasm_bindgen]
pub fn ruleset_precompiled(bytes: &[u8]) -> Result<(), JsError> {
    let conds = game_rules::PrecompiledConds::from_bytes(bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let b = p.get_or_insert_with(game_rules::Ruleset::builder);
        for e in conds.entries {
            b.precompiled(&e.card, e.entry, e.blob);
        }
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
        let sha = set.sha256().to_string();
        // `data()` first: if it fails, `RULES` stays empty (StubRules) and the
        // stamp must say `"stub"` too. Writing the hash before this point would
        // make `engine_stamp` claim a ruleset the engine is not actually running,
        // so a record sealed here would pass `compat` against a later session
        // that really does run those rules -- and then diverge at the first
        // checkpoint with no stamp warning. See `docs/REPLAY.md`.
        let rules = WasmRules::new(set, data()?);
        RULESET_SHA.with(|s| *s.borrow_mut() = sha);
        RULES.with(|r| *r.borrow_mut() = Some(Arc::new(rules)));
        Ok(n)
    })
}

/// Evaluate the compiled guard condition of `card`'s guarded `entry` against
/// JSON `WindowCtx` / `CandidateCtx` snapshots (missing fields default).
///
/// A test seam for `webui/src/game/ruleset.test.ts`: it proves the shipped
/// precompiled blob does not merely *load* in this runtime-only build but
/// actually evaluates -- the thing that broke when cards first gained `pre`s
/// and nothing shipped the compiled form. Throws when the entry declares no
/// condition, so a card with a `pre` can be asserted to be present and live.
#[wasm_bindgen]
pub fn ruleset_pre_eval(
    card: &str,
    entry: i32,
    window_json: &str,
    candidate_json: &str,
) -> Result<bool, JsError> {
    use game_rules::cond_pre::{CandidateCtx, WindowCtx};
    let win: WindowCtx = serde_json::from_str(window_json)
        .map_err(|e| JsError::new(&format!("window: {e}")))?;
    let cand: CandidateCtx = serde_json::from_str(candidate_json)
        .map_err(|e| JsError::new(&format!("candidate: {e}")))?;
    let set = RULES
        .with(|r| r.borrow().clone())
        .ok_or_else(|| JsError::new("ruleset_build was never called"))?;
    let handle = set
        .ruleset()
        .card(card)
        .ok_or_else(|| JsError::new(&format!("no card {card:?}")))?;
    let pre = set
        .ruleset()
        .pre_at(handle, entry)
        .ok_or_else(|| JsError::new(&format!("card {card:?} entry {entry} declares no condition")))?;
    Ok(pre.eval(&win, &cand))
}

fn rules() -> Result<Arc<dyn CardRules>, JsError> {
    Ok(match RULES.with(|r| r.borrow().clone()) {
        Some(w) => w as Arc<dyn CardRules>,
        None => Arc::new(StubRules) as Arc<dyn CardRules>,
    })
}

// ------------------------------------------------------------------ solo match

/// A match running entirely in the browser (solo mode vs bots). Wraps a
/// [`RecordedMatch`], so the whole game can be exported as a `.bdrec`.
#[wasm_bindgen]
pub struct SoloMatch {
    m: RecordedMatch,
}

#[wasm_bindgen]
impl SoloMatch {
    /// `members_json`: `RoomMember[]` (each carries its `mentality`);
    /// `seed`: the match seed; `mode`: 0 solo, 1 casual, 2 ranked;
    /// `weights_json`: `ScoreWeights` (empty = default).
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
        let setup = MatchSetup {
            members,
            seed: seed as u64,
            weights,
        };
        Ok(SoloMatch {
            m: RecordedMatch::new(data()?, rules()?, setup, mode),
        })
    }

    /// Rebuild a match from [`SoloMatch::save`] (page refresh). The record
    /// restarts at the snapshot and is marked `partial`; use
    /// [`SoloMatch::restore_with_record`] to keep the log.
    pub fn restore(json: &str) -> Result<SoloMatch, JsError> {
        let m = Match::restore(data()?, rules()?, json)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(SoloMatch {
            m: RecordedMatch::from_snapshot(m),
        })
    }

    /// [`SoloMatch::restore`], but with the recorder JSON from
    /// [`SoloMatch::record_state`] beside it: the log continues where it left
    /// off, so the exported record covers the whole match.
    pub fn restore_with_record(&self, save: &str, rec: &str) -> Result<SoloMatch, JsError> {
        let m = RecordedMatch::restore(data()?, rules()?, save, rec)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(SoloMatch { m })
    }

    /// The whole match as JSON, for `restore`.
    pub fn save(&self) -> String {
        self.m.inner().save()
    }

    /// The recorder beside [`SoloMatch::save`] (persist the two together).
    pub fn record_state(&self) -> String {
        self.m.recorder_json()
    }

    /// One recorded tick quantum: `dt = k * 0.05`, k in 1..=10. This is what
    /// the solo driver should call; [`SoloMatch::tick`] is kept for the
    /// unrecorded path and writes nothing to the log.
    pub fn tick_steps(&mut self, k: u8) {
        self.m.tick_steps(k);
    }

    /// Unrecorded tick. Prefer [`SoloMatch::tick_steps`].
    pub fn tick(&mut self, dt: f32) {
        self.m.inner_mut().tick(dt);
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

    /// `{state, hand, handNotes, draw, you, playerId, aiAnswer, playable}` -- the
    /// same shape as the server's `match` frame. `aiAnswer` / `playable` are the
    /// per-viewer extras from [`Match::view_extra`] (the 托管 autopilot's inputs).
    pub fn view(&self, member: i32) -> String {
        let m = self.m.inner();
        let state = m.state();
        let player_id = state.player_of(member);
        let extra = m.view_extra(member);
        json(&serde_json::json!({
            "state": state,
            "hand": m.hand_of(member),
            "handNotes": m.hand_notes_of(member),
            "draw": m.draw_of(member),
            "you": member,
            "playerId": player_id,
            "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(serde_json::Value::Null),
            "playable": extra.get("playable").cloned().unwrap_or(serde_json::Value::Null),
            "estCost": extra.get("estCost").cloned().unwrap_or(serde_json::Value::Null),
        }))
    }

    pub fn events_since(&self, last_id: i32) -> String {
        json(&self.m.inner().events_since(last_id))
    }

    pub fn take_changed(&mut self) -> bool {
        self.m.inner_mut().take_changed()
    }

    pub fn ended(&self) -> bool {
        self.m.inner().ended()
    }

    pub fn finish(&mut self) {
        self.m.finish();
    }

    /// Seal the record as a `.bdrec` JSON string (`created` is a display
    /// timestamp, `yyyy-MM-dd HH:mm`).
    pub fn record(&self, created: &str) -> Result<String, JsError> {
        Ok(json(&self.m.export(stamp(), created)))
    }

    /// [`SoloMatch::record`], with the public event log bundled into the body
    /// (re-simulated at export). Bigger, but the log-only view works even if
    /// the engine's rules drift later.
    pub fn record_with_events(&self, created: &str) -> Result<String, JsError> {
        let file = self
            .m
            .export_with_events(data()?, rules()?, stamp(), created)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(json(&file))
    }

    /// [`SoloMatch::record`], zstd-framed: what goes to IndexedDB and what a
    /// download writes. The compression runs here (the browser has no reliable
    /// zstd), so the bytes are a standard zstd frame of the record JSON.
    pub fn record_zst(&self, created: &str) -> Result<Vec<u8>, JsError> {
        Ok(encode_record_zst(&self.m.export(stamp(), created)))
    }

    /// [`SoloMatch::record_zst`], with the public event log bundled.
    pub fn record_with_events_zst(&self, created: &str) -> Result<Vec<u8>, JsError> {
        let file = self
            .m
            .export_with_events(data()?, rules()?, stamp(), created)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(encode_record_zst(&file))
    }
}

// ------------------------------------------------------------------ replay

/// Plays a `.bdrec` back through the same engine the solo match runs.
#[wasm_bindgen]
pub struct ReplayMatch {
    rp: Replayer,
}

#[wasm_bindgen]
impl ReplayMatch {
    /// `json` is a `.bdrec` (or its plain-JSON form). `force` plays through a
    /// stamp mismatch; without it any difference refuses the replay and the
    /// error carries the [`game_core::record::Mismatch`] list.
    #[wasm_bindgen(constructor)]
    pub fn from_record(json: &str, force: bool) -> Result<ReplayMatch, JsError> {
        let rec = parse_record(json).map_err(|e| JsError::new(&e.to_string()))?;
        let rp = Replayer::new_with_stamp(data()?, rules()?, &rec, &stamp(), force)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(ReplayMatch { rp })
    }

    /// [`ReplayMatch::from_record`] over raw `.bdrec` bytes: zstd (the current
    /// form), gzip (older downloads and IndexedDB rows) or plain JSON. The
    /// sniffing and decompression happen in `game_core::record::decode_record`,
    /// so every framing plays in the browser.
    pub fn from_record_bytes(bytes: &[u8], force: bool) -> Result<ReplayMatch, JsError> {
        let rec = decode_record(bytes).map_err(|e| JsError::new(&e.to_string()))?;
        let rp = Replayer::new_with_stamp(data()?, rules()?, &rec, &stamp(), force)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(ReplayMatch { rp })
    }

    /// The `RecordHeader` JSON (seats, stamp, ticks, ...).
    pub fn header(&self) -> String {
        json(self.rp.header())
    }

    /// What would block this replay right now: the [`Mismatch`] list as JSON
    /// (empty array = play). `want` is the record's stamp, `got` is this
    /// engine's.
    pub fn compat(&self) -> String {
        json(&game_core::record::compat(
            &self.rp.header().engine,
            &stamp(),
        ))
    }

    /// Advance at most `n` tick quanta. Returns the `Status` JSON
    /// (`{tick, ended, diverged}`).
    pub fn step(&mut self, n: u32) -> String {
        json(&self.rp.step_ticks(n))
    }

    /// Apply exactly one log entry (a whole tick run, or one call).
    pub fn next_input(&mut self) -> String {
        json(&self.rp.next_input())
    }

    /// Advance the background keyframe pass by at most `budget` inputs.
    /// Returns the `IndexStatus` JSON.
    pub fn index(&mut self, budget: u32) -> String {
        json(&self.rp.index(budget as usize))
    }

    /// Jump to `tick` (clamped to [`ReplayMatch::total_ticks`]).
    pub fn seek(&mut self, tick: u32) -> Result<String, JsError> {
        let st = self
            .rp
            .seek(tick as u64)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(json(&st))
    }

    /// Turn marks for the scrub bar: `[{round, turn, tick}, ...]`.
    pub fn turns(&self) -> String {
        json(&self.rp.turns())
    }

    /// Ticks in the whole record (the scrub bar's right edge).
    pub fn total_ticks(&self) -> u32 {
        self.rp.total_ticks().min(u32::MAX as u64) as u32
    }

    /// The same frame shape as [`SoloMatch::view`]. `member` 0 is the
    /// spectator: no hand, no draw, no per-viewer extras.
    pub fn view(&self, member: i32) -> String {
        let m = self.rp.match_ref();
        let state = m.state();
        if member == 0 {
            return json(&serde_json::json!({
                "state": state,
                "hand": [],
                "handNotes": [],
                "draw": [],
                "you": 0,
                "playerId": -1,
                "aiAnswer": serde_json::Value::Null,
                "playable": serde_json::Value::Null,
                "estCost": serde_json::Value::Null,
            }));
        }
        let player_id = state.player_of(member);
        let extra = m.view_extra(member);
        json(&serde_json::json!({
            "state": state,
            "hand": m.hand_of(member),
            "handNotes": m.hand_notes_of(member),
            "draw": m.draw_of(member),
            "you": member,
            "playerId": player_id,
            "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(serde_json::Value::Null),
            "playable": extra.get("playable").cloned().unwrap_or(serde_json::Value::Null),
            "estCost": extra.get("estCost").cloned().unwrap_or(serde_json::Value::Null),
        }))
    }

    /// Events the replayer has produced so far (the full stream, not the
    /// engine's 400-tail).
    pub fn events_since(&self, last_id: i32) -> String {
        json(&self.rp.events_since(last_id))
    }

    pub fn take_changed(&mut self) -> bool {
        self.rp.match_mut().take_changed()
    }

    /// Nothing left to play (log exhausted, or the match ended early).
    pub fn ended(&self) -> bool {
        self.rp.ended()
    }

    /// The `Status` JSON without advancing.
    pub fn status(&self) -> String {
        json(&self.rp.status())
    }
}

/// Cheap header-only parse of a `.bdrec` for the replay list (no body check).
#[wasm_bindgen]
pub fn record_header(json_str: &str) -> Result<String, JsError> {
    let h = parse_header(json_str).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(json(&h))
}

/// [`record_header`] over raw `.bdrec` bytes -- zstd, gzip or plain JSON.
///
/// Portable files (`docs/REPLAY.md` §10) work unchanged: the codec stops at
/// the end of the record frame and ignores the appended engine section, so a
/// portable file lists exactly like the plain record it wraps.
#[wasm_bindgen]
pub fn record_header_bytes(bytes: &[u8]) -> Result<String, JsError> {
    let h = parse_header_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(json(&h))
}

// ---------------------------------------------------------------- portable codec
//
// The portable record extension (`docs/REPLAY.md` §10) embeds an engine bundle
// in a zstd frame. Packing and unpacking happen on the page (and in
// `tools/bdrec-portable.mjs`), and both need the same codec the record framing
// uses -- the browser has no other zstd. These are **not** part of the frozen
// replay API v1: only the page's own engine is asked for them, never an
// archived bundle.

/// zstd-compress `bytes` into one standard frame (portable-record packing).
#[wasm_bindgen]
pub fn zst_compress(bytes: &[u8]) -> Vec<u8> {
    game_core::record::zst_encode(bytes)
}

/// zstd-decompress one standard frame, refusing to grow past `max_out` bytes
/// (portable-record unpacking; the cap is the decompression-bomb limit).
#[wasm_bindgen]
pub fn zst_decompress(bytes: &[u8], max_out: u32) -> Result<Vec<u8>, JsError> {
    game_core::record::zst_decode(bytes, max_out as usize).map_err(|e| JsError::new(&e.to_string()))
}
