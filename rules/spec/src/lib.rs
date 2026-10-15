//! # rules-spec -- game-specific spec harness
//!
//! Wires the generic [`spec_kit`] kit to this game's engine, data, and
//! rulebook vocabulary. Units 1-21 add clauses under [`global`] and card
//! contracts under `tests/`; this crate owns the harness they stand on.
//!
//! * [`table`] -- the `Table` arrange/drive harness, adapted from
//!   `crates/game-rules/tests/common` (which stays untouched). Every player
//!   input goes through [`spec_kit::observe::observe`].
//! * [`fuzz`] -- arrange + drive adapted from `crates/game-rules/tests/fuzz`,
//!   with every act observed, so a soak checks clauses after every input.
//! * [`global`] -- the eight rulebook areas, one `CLAUSES` slice each.
//!
//! Data and the ruleset load from the repo's `data/` and `dist/` (relative to
//! `CARGO_MANIFEST_DIR` = `rules/spec`, so `../../`). Build the ruleset first:
//! `node tools/build-ruleset.mjs` (without `CARGO_TARGET_DIR` set).

pub mod fuzz;
pub mod global;
pub mod table;

use std::path::Path;
use std::sync::{Arc, OnceLock};

use game_core::data::{GameData, Roster};
use game_rules::{Ruleset, WasmRules};

// Re-export the generic kit so a test can `use rules_spec::...` only.
pub use spec_kit::{
    check, event_key, forall, int_arg, player_arg, snapshot, diff, frame, observe, Clause, Delta,
    Observed, Rng, Snap, Step, StepAct, Violation,
};
pub use table::Table;

/// The shared `GameData` (`data/*.json`), loaded once.
pub fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let roster = Roster::from_fixture(&std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/game-core/tests/fixtures/roster.json"),
        )
        .expect("roster fixture"))
        .expect("roster fixture parses");
        Arc::new(
            GameData::load(
                |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
                roster,
            )
            .expect("load data/"),
        )
    })
    .clone()
}

/// The shipped cards and skills plus the `TEST:*` fixtures (`dist/`), loaded
/// once. Run `node tools/build-ruleset.mjs` first.
pub fn rules() -> Arc<WasmRules> {
    static RULES: OnceLock<Arc<WasmRules>> = OnceLock::new();
    RULES
        .get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
            let mut b = Ruleset::builder();
            for sub in ["cards", "fixtures"] {
                let dir = root.join(sub);
                let index: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(dir.join("index.json"))
                        .expect("run node tools/build-ruleset.mjs"),
                )
                .unwrap();
                for m in index["modules"].as_array().unwrap() {
                    b.add(&std::fs::read(dir.join(m["file"].as_str().unwrap())).unwrap())
                        .unwrap();
                }
            }
            Arc::new(WasmRules::new(b.build().unwrap(), data()))
        })
        .clone()
}

/// Board index of the tile with this exact name (`data/board.json`). The
/// rulebook's 「#N格」 is index `N - 1`.
pub fn tile(name: &str) -> usize {
    data()
        .tiles
        .iter()
        .position(|t| t.name == name)
        .unwrap_or_else(|| panic!("no tile named {name:?}"))
}

/// Every character name in the roster, in roster order.
pub fn characters() -> Vec<String> {
    data().personas.iter().map(|c| c.name.clone()).collect()
}