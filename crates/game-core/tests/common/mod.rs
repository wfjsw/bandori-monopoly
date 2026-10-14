//! Shared test harness: [`GameData`] from `data/` with a roster parsed from
//! the three legacy JSON files.
//!
//! `game-core` must not depend on the card crates, so tests cannot ask a
//! ruleset for the roster. Until `data/{cards,characters,bands}.json` retire
//! (tools still read them), [`Roster::from_json_dir`] is the least invasive
//! bridge; afterwards swap this helper for a `dump_roster` fixture.
#![allow(dead_code)] // each test binary uses a subset of the helpers

use std::path::Path;
use std::sync::Arc;

use game_core::data::{GameData, Roster};

pub fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// `GameData` from `data/`, roster included (legacy JSON side-load).
pub fn data() -> GameData {
    let dir = data_dir();
    let roster = Roster::from_json_dir(&dir).expect("data/{cards,characters,bands}.json");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), roster)
        .expect("data/ should load -- run tools/asset-pipe/extract.py")
}

/// [`data`], wrapped for the match engine.
pub fn data_arc() -> Arc<GameData> {
    Arc::new(data())
}