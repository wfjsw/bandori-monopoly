//! Shared test harness: [`GameData`] from `data/` with a roster parsed from
//! the generated fixture.
//!
//! `game-core` must not depend on the card crates, so tests cannot ask a
//! ruleset for the roster. The checked-in fixture
//! (`crates/game-core/tests/fixtures/roster.json`, written by
//! `cargo run -p rules-native --example dump_roster -- --fixture`) is the
//! bridge: [`Roster::from_fixture`] loads it without `game-core` touching the
//! card crates. Regenerate with the command in `roster_equiv.rs`.
#![allow(dead_code)] // each test binary uses a subset of the helpers

use std::path::Path;
use std::sync::Arc;

use game_core::data::{GameData, Roster};

pub fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// `GameData` from `data/`, roster from the generated fixture.
pub fn data() -> GameData {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(root.join("tests/fixtures/roster.json")).expect("roster fixture"),
    )
    .expect("roster fixture parses");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), roster)
        .expect("data/ should load -- run tools/asset-pipe/extract.py")
}

/// [`data`], wrapped for the match engine.
pub fn data_arc() -> Arc<GameData> {
    Arc::new(data())
}