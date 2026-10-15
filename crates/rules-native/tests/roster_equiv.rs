//! Roster fixture gate (ABI v63, `docs/CARDS.md` 「Card metadata」).
//!
//! The Rust rule code is the single source of card / character / band metadata.
//! The checked-in test fixture
//! (`crates/game-core/tests/fixtures/roster.json`) is a **generated dump** of
//! that roster for crates that have no ruleset (game-core / game-rules /
//! bot-core / server). This test keeps the two byte-identical: if the fixture
//! drifts from a fresh dump, the test fails and names the regenerate command.
//!
//! Equivalence to the retired `data/{cards,characters,bands}.json` baseline was
//! verified at the cutover (commit `7c3d115`, the last tree with those files as
//! the baseline). The old JSON-vs-Rust comparison is gone; what remains here is
//! ordering / field fidelity of the fixture against the live ruleset roster.

use game_core::data::{GameData, Roster};

/// Path to the checked-in fixture (from this crate's manifest dir).
fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../game-core/tests/fixtures/roster.json")
}

const REGENERATE: &str =
    "cargo run -p rules-native --example dump_roster -- --fixture \
     crates/game-core/tests/fixtures/roster.json";

fn checked_in_fixture() -> String {
    std::fs::read_to_string(fixture_path()).unwrap_or_else(|e| {
        panic!(
            "roster fixture missing ({}): {e}\n  {REGENERATE}",
            fixture_path().display()
        )
    })
}

#[test]
fn fixture_is_byte_equal_to_fresh_dump() {
    let checked_in = checked_in_fixture();
    let fresh = rules_native::roster().to_fixture_json();
    assert_eq!(
        checked_in, fresh,
        "roster fixture drifted from the Rust metadata.\n  Regenerate with:\n    {REGENERATE}"
    );
}

/// Ordering and field fidelity: the fixture must carry the same rows, in the
/// same order, as the live ruleset roster (which sorts by the shared Rust
/// ordinal table).
#[test]
fn fixture_preserves_order_and_fields() {
    let fixture = Roster::from_fixture(&checked_in_fixture()).expect("fixture parses");
    let rust = rules_native::roster();
    assert_eq!(
        fixture, rust,
        "fixture roster differs from the live ruleset roster"
    );
    assert_eq!(fixture.cards.len(), 184, "full card coverage");
    assert_eq!(fixture.personas.len(), 54, "full character coverage");
    assert_eq!(fixture.groups.len(), 12, "full band coverage");
}

/// `GameData::load` accepts the fixture roster and the lookups land.
#[test]
fn load_sees_fixture_rows() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string());
    let roster = Roster::from_fixture(&checked_in_fixture()).unwrap();
    let d = GameData::load(&mut read, roster).unwrap();
    // A lifted Sumimi card is visible by id and carries its band.
    let card = d.card("Sumimi:Sumimi不会解散哦").expect("fixture card");
    assert_eq!(card.group, "Sumimi");
    // The lifted character is visible by name.
    let c = d.persona("纯田真奈").expect("fixture character");
    assert_eq!(c.group, "Sumimi");
    // The lifted band is visible by name.
    let b = d.group("Sumimi").expect("fixture band");
    assert!(b.shares_exclusives);
    // Non-roster files still come from the data dir (tiles exist either way).
    assert!(!d.tiles.is_empty());
}