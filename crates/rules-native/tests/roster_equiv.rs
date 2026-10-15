//! Roster fixture gate (ABI v63, `docs/CARDS.md` 「Card metadata」).
//!
//! `crates/game-core/tests/fixtures/roster.json` is a **tiny synthetic**
//! fixture (neutral persona/group names) so engine tests stay game-agnostic.
//! The live ruleset roster is `rules-native::roster()` and belongs with the
//! shipped cards; this gate only pins that the synthetic fixture stays
//! parseable and loadable.
//!
//! The historical byte-equal dump check against the live roster lived here
//! when the fixture was a generated dump of the shipped ruleset. Regenerating
//! that dump is `cargo run -p rules-native --example dump_roster -- --fixture`
//! if a full dump is ever needed again (keep it out of crates/).

use game_core::data::{GameData, Roster};

/// Path to the checked-in fixture (from this crate's manifest dir).
fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../game-core/tests/fixtures/roster.json")
}

fn checked_in_fixture() -> String {
    std::fs::read_to_string(fixture_path()).unwrap_or_else(|e| {
        panic!(
            "roster fixture missing ({}): {e}",
            fixture_path().display()
        )
    })
}

/// The synthetic fixture parses and is internally consistent.
#[test]
fn fixture_parses_as_a_tiny_synthetic_roster() {
    let fixture = Roster::from_fixture(&checked_in_fixture()).expect("fixture parses");
    assert!(!fixture.cards.is_empty(), "fixture has cards");
    assert!(!fixture.personas.is_empty(), "fixture has personas");
    assert!(!fixture.groups.is_empty(), "fixture has groups");
    // Tiny on purpose: engine tests must not need the shipped roster.
    assert!(
        fixture.personas.len() <= 16,
        "synthetic roster stays tiny ({} personas)",
        fixture.personas.len()
    );
}

/// `GameData::load` accepts the fixture roster and the lookups land.
#[test]
fn load_sees_fixture_rows() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string());
    let fixture = Roster::from_fixture(&checked_in_fixture()).unwrap();
    let persona0 = fixture.personas[0].name.clone();
    let group0 = fixture.personas[0].group.clone();
    let d = GameData::load(&mut read, fixture).unwrap();
    let c = d.persona(&persona0).expect("fixture persona");
    assert_eq!(c.group, group0);
    assert!(d.group(&group0).is_some(), "fixture group");
    // Non-roster files still come from the data dir (tiles exist either way).
    assert!(!d.tiles.is_empty());
}