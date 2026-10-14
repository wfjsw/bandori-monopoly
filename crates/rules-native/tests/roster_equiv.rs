//! Roster equivalence gate (ABI v63, `docs/CARDS.md` 「Card metadata」).
//!
//! Builds the roster from the linked ruleset (`bandori_metadata` / native
//! `roster_metadata`) and asserts deep equality -- **order included** -- with
//! the JSON-loaded `CardData` / `CharacterData` / `BandData` for every entry
//! that has Rust metadata.
//!
//! Coverage is an explicit constant. The JSON-lift phase has raised it to the
//! full `184 / 54 / 12`; this test is the gate for deleting the JSON and fails
//! if a lifted row drifts from its `data/*.json` source.

use game_core::data::{BandData, CardData, CharacterData, GameData, Roster};

/// Cards the Rust side currently defines metadata for (full `cards.json` lift).
const EXPECTED_CARD_COVERAGE: usize = 184;
/// Characters with a `CharacterDef` (full `characters.json` lift).
const EXPECTED_CHARACTER_COVERAGE: usize = 54;
/// Bands with a `BandDef` (full `bands.json` lift).
const EXPECTED_BAND_COVERAGE: usize = 12;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn json_roster() -> Roster {
    let dir = data_dir();
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string());
    let d = GameData::load(read).expect("data/*.json");
    Roster {
        cards: d.cards,
        characters: d.characters,
        bands: d.bands,
    }
}

fn rust_roster() -> Roster {
    rules_native::roster()
}

/// Pair json rows with rust rows by primary key, in **json order**.
fn keyed<'a, T, F>(json: &'a [T], rust: &'a [T], key: F) -> Vec<(&'a T, &'a T)>
where
    F: Fn(&T) -> String,
{
    let mut pairs = Vec::new();
    for item in json {
        let k = key(item);
        if let Some(hit) = rust.iter().find(|x| key(x) == k) {
            pairs.push((item, hit));
        }
    }
    pairs
}

fn assert_cards(json: &Roster, rust: &Roster) -> usize {
    // Every rust card must be present in json (no orphans).
    for c in &rust.cards {
        assert!(
            json.cards.iter().any(|x| x.id == c.id),
            "rust card {} is not in cards.json",
            c.id
        );
    }
    let pairs = keyed(&json.cards, &rust.cards, |c: &CardData| c.id.clone());
    assert_eq!(
        pairs.len(),
        rust.cards.len(),
        "card key sets disagree (json has {} matching, rust has {} rows)",
        pairs.len(),
        rust.cards.len()
    );
    for (a, b) in &pairs {
        assert_eq!(*a, *b, "card {} metadata differs from cards.json", a.id);
    }
    // Order: rust.cards must appear in cards.json order.
    let rust_ids: Vec<String> = rust.cards.iter().map(|c| c.id.clone()).collect();
    let json_ids: Vec<String> = json
        .cards
        .iter()
        .map(|c| c.id.clone())
        .filter(|id| rust_ids.contains(id))
        .collect();
    assert_eq!(rust_ids, json_ids, "card order differs from cards.json");
    rust.cards.len()
}

fn assert_characters(json: &Roster, rust: &Roster) -> usize {
    for c in &rust.characters {
        assert!(
            json.characters.iter().any(|x| x.name == c.name),
            "rust character {} is not in characters.json",
            c.name
        );
    }
    let pairs = keyed(&json.characters, &rust.characters, |c: &CharacterData| {
        c.name.clone()
    });
    assert_eq!(pairs.len(), rust.characters.len(), "character key sets disagree");
    for (a, b) in &pairs {
        assert_eq!(*a, *b, "character {} metadata differs from characters.json", a.name);
    }
    let rust_names: Vec<String> = rust.characters.iter().map(|c| c.name.clone()).collect();
    let json_names: Vec<String> = json
        .characters
        .iter()
        .map(|c| c.name.clone())
        .filter(|n| rust_names.contains(n))
        .collect();
    assert_eq!(rust_names, json_names, "character order differs from characters.json");
    rust.characters.len()
}

fn assert_bands(json: &Roster, rust: &Roster) -> usize {
    for b in &rust.bands {
        assert!(
            json.bands.iter().any(|x| x.name == b.name),
            "rust band {} is not in bands.json",
            b.name
        );
    }
    let pairs = keyed(&json.bands, &rust.bands, |b: &BandData| b.name.clone());
    assert_eq!(pairs.len(), rust.bands.len(), "band key sets disagree");
    for (a, b) in &pairs {
        assert_eq!(*a, *b, "band {} metadata differs from bands.json", a.name);
    }
    let rust_names: Vec<String> = rust.bands.iter().map(|b| b.name.clone()).collect();
    let json_names: Vec<String> = json
        .bands
        .iter()
        .map(|b| b.name.clone())
        .filter(|n| rust_names.contains(n))
        .collect();
    assert_eq!(rust_names, json_names, "band order differs from bands.json");
    rust.bands.len()
}

#[test]
fn roster_metadata_matches_json() {
    let json = json_roster();
    let rust = rust_roster();

    let cards = assert_cards(&json, &rust);
    let characters = assert_characters(&json, &rust);
    let bands = assert_bands(&json, &rust);

    // Coverage is the gate: the next phase raises these to 184 / 54 / 12.
    assert_eq!(
        cards,
        EXPECTED_CARD_COVERAGE,
        "card coverage drifted -- update EXPECTED_CARD_COVERAGE when lifting more bands"
    );
    assert_eq!(
        characters,
        EXPECTED_CHARACTER_COVERAGE,
        "character coverage drifted -- update EXPECTED_CHARACTER_COVERAGE when lifting more bands"
    );
    assert_eq!(
        bands,
        EXPECTED_BAND_COVERAGE,
        "band coverage drifted -- update EXPECTED_BAND_COVERAGE when lifting more bands"
    );

    // Report what the slice covers (visible with --nocapture).
    eprintln!(
        "roster coverage: {cards} of {} cards, {characters} of {} characters, {bands} of {} bands",
        json.cards.len(),
        json.characters.len(),
        json.bands.len()
    );
}

/// `load_with_roster` must accept the converted roster and produce the same
/// lookups `load` does for the lifted slice.
#[test]
fn load_with_roster_sees_lifted_rows() {
    let dir = data_dir();
    let mut read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string());
    let json = GameData::load(&mut read).unwrap();
    let rust = rust_roster();
    let d = GameData::load_with_roster(&mut read, rust).unwrap();
    // The lifted Sumimi card is visible by id and carries its band.
    let card = d.card("Sumimi:Sumimi不会解散哦").expect("lifted card");
    assert_eq!(card.band, "Sumimi");
    // The lifted character is visible by name.
    let c = d.character("纯田真奈").expect("lifted character");
    assert_eq!(c.band, "Sumimi");
    // The lifted band is visible by name.
    let b = d.band("Sumimi").expect("lifted band");
    assert!(b.shares_exclusives);
    // Non-roster files still come from JSON (tiles exist either way).
    assert_eq!(d.tiles.len(), json.tiles.len());
}