//! Deck book format + back-off lookup (`docs/BOT.md` §3.7 D1).

use std::sync::Arc;

use game_core::data::{CharacterData, GameData};
use game_core::deck;
use game_core::deck_book::{self, DeckBook, DeckBookEntry, DECK_BOOK_VERSION, POLICY_STANDARD};
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

fn data() -> GameData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
        .expect("web/data should load -- run tools/asset-pipe/extract.py")
}

/// A distinct complete deck for `c`: `SIZE` pool cards starting at `skip`.
/// `skip = 0` is what `deck::preset` lands on (the pool is filled front to back).
fn deck_slice(d: &GameData, c: &CharacterData, skip: usize) -> Vec<String> {
    let ids: Vec<String> = deck::pool(d, c).into_iter().map(|k| k.id.clone()).collect();
    assert!(
        ids.len() >= skip + deck::SIZE,
        "{}'s pool is too small for a test deck",
        c.name
    );
    ids[skip..skip + deck::SIZE].to_vec()
}

fn entry(me: &str, seat: Option<u32>, opponents: &[&str], bands: &[&str], cards: Vec<String>) -> DeckBookEntry {
    DeckBookEntry {
        me: me.into(),
        seat,
        opponents: opponents.iter().map(|s| s.to_string()).collect(),
        opponent_bands: bands.iter().map(|s| s.to_string()).collect(),
        cards,
    }
}

fn book(entries: Vec<DeckBookEntry>) -> DeckBook {
    let mut b = DeckBook::default();
    b.version = DECK_BOOK_VERSION;
    b.ruleset_sha256 = "stub".into();
    b.policy = POLICY_STANDARD.into();
    b.generated_at = "test".into();
    // Drop entries into the level their key shape says.
    for e in entries {
        match (e.seat.is_some(), !e.opponents.is_empty(), !e.opponent_bands.is_empty()) {
            (true, true, _) => b.exact.push(e),
            (true, false, _) => b.seat_bands.push(e),
            (false, false, true) => b.bands.push(e),
            (false, false, false) => b.me.push(e),
            (false, true, _) => panic!("opponents without a seat belong to the exact level"),
        }
    }
    b
}

/// 戸山香澄 (Poppin' Party) at a 3-seat table.
fn me<'a>(d: &'a GameData) -> &'a CharacterData {
    d.character("户山香澄").unwrap()
}

// ---------------------------------------------------------------- lookup

#[test]
fn lookup_falls_back_in_order() {
    let d = data();
    let c = me(&d);
    let exact = deck_slice(&d, c, 1);
    let seat_bands = deck_slice(&d, c, 2);
    let bands = deck_slice(&d, c, 3);
    let me_only = deck_slice(&d, c, 4);
    let b = book(vec![
        entry("户山香澄", Some(1), &["花园多惠", "牛込里美"], &[], exact.clone()),
        entry("户山香澄", Some(1), &[], &["Poppin' Party", "Poppin' Party"], seat_bands.clone()),
        entry("户山香澄", None, &[], &["Poppin' Party", "Poppin' Party"], bands.clone()),
        entry("户山香澄", None, &[], &[], me_only.clone()),
    ]);
    let lookup = |seat: usize, opponents: &[&str]| {
        let opp: Vec<String> = opponents.iter().map(|s| s.to_string()).collect();
        b.lookup(&d, c, seat, &opp, "stub")
    };

    // 1. exact: (me, seat, opponents in seat order)
    assert_eq!(lookup(1, &["花园多惠", "牛込里美"]), Some(exact));
    // 2. same seat and band multiset, opponents swapped out of seat order.
    assert_eq!(lookup(1, &["牛込里美", "花园多惠"]), Some(seat_bands));
    // 3. same band multiset, other seat.
    assert_eq!(lookup(0, &["牛込里美", "花园多惠"]), Some(bands));
    // 4. other bands entirely.
    assert_eq!(lookup(1, &["美竹兰", "青叶摩卡"]), Some(me_only.clone()));
    // Unknown table shape still stops at level 4 before the preset.
    assert_eq!(lookup(3, &["美竹兰", "青叶摩卡"]), Some(me_only));
}

#[test]
fn first_hit_within_a_level_wins() {
    let d = data();
    let c = me(&d);
    let first = deck_slice(&d, c, 1);
    let second = deck_slice(&d, c, 2);
    let b = book(vec![
        entry("户山香澄", Some(0), &[], &["Afterglow"], first.clone()),
        entry("户山香澄", Some(0), &[], &["Afterglow"], second.clone()),
    ]);
    let opp = vec!["美竹兰".to_string()];
    assert_eq!(b.lookup(&d, c, 0, &opp, "stub"), Some(first));
}

#[test]
fn stale_hash_and_policy_mismatch_ignore_the_book() {
    let mut d = data();
    let cards = {
        let c = d.character("户山香澄").unwrap();
        deck_slice(&d, c, 1)
    };
    let opp: Vec<String> = vec!["美竹兰".to_string()];

    // Stale ruleset hash: the whole book is ignored, down to the preset.
    let mut b = book(vec![entry("户山香澄", None, &[], &[], cards.clone())]);
    b.ruleset_sha256 = "some-other-ruleset".into();
    d.deck_book = b;
    {
        let c = d.character("户山香澄").unwrap();
        assert_eq!(d.deck_book.lookup(&d, c, 0, &opp, "stub"), None);
        assert_eq!(
            deck_book::suggest(&d, c, 0, &opp, "stub"),
            deck::preset(&d, c)
        );
    }

    // Policy mismatch: same story.
    let mut b = book(vec![entry("户山香澄", None, &[], &[], cards)]);
    b.policy = "chaos".into();
    d.deck_book = b;
    {
        let c = d.character("户山香澄").unwrap();
        assert_eq!(d.deck_book.lookup(&d, c, 0, &opp, "stub"), None);
        assert_eq!(
            deck_book::suggest(&d, c, 0, &opp, "stub"),
            deck::preset(&d, c)
        );
    }
}

#[test]
fn invalid_entries_fall_through_to_the_next_level() {
    let mut d = data();
    let (good, mut short) = {
        let c = d.character("户山香澄").unwrap();
        (deck_slice(&d, c, 2), deck_slice(&d, c, 1))
    };
    short.pop(); // 9 cards: `clean` cannot make a complete deck of it
    let opp = vec!["花园多惠".to_string()];
    d.deck_book = book(vec![
        entry("户山香澄", Some(1), &["花园多惠"], &[], short),
        entry("户山香澄", Some(1), &[], &["Poppin' Party"], good.clone()),
    ]);
    {
        let c = d.character("户山香澄").unwrap();
        assert_eq!(d.deck_book.lookup(&d, c, 1, &opp, "stub"), Some(good));
    }

    // Every level invalid -> no hit; `suggest` still returns the preset.
    d.deck_book = book(vec![entry(
        "户山香澄",
        None,
        &[],
        &[],
        vec!["no-such-card".into(); 10],
    )]);
    {
        let c = d.character("户山香澄").unwrap();
        assert_eq!(d.deck_book.lookup(&d, c, 1, &opp, "stub"), None);
        assert_eq!(
            deck_book::suggest(&d, c, 1, &opp, "stub"),
            deck::preset(&d, c)
        );
    }
}

#[test]
fn lookup_is_pure_and_cleans_the_deck() {
    let d = data();
    let c = me(&d);
    let cards = deck_slice(&d, c, 1);
    let shuffled: Vec<String> = cards.iter().rev().cloned().collect();
    let b = book(vec![entry("户山香澄", None, &[], &[], shuffled)]);
    let opp: Vec<String> = vec!["美竹兰".to_string()];
    let once = b.lookup(&d, c, 0, &opp, "stub").unwrap();
    let twice = b.lookup(&d, c, 0, &opp, "stub").unwrap();
    assert_eq!(once, twice);
    // `clean` hands them back in pool order, the same list the engine plays.
    assert_eq!(once, deck::clean(&d, c, &cards));
}

// ---------------------------------------------------------------- format

#[test]
fn parses_the_documented_shape() {
    let body = r#"{
      "version": 1,
      "ruleset_sha256": "stub",
      "policy": "standard",
      "generated_at": "2026-10-07",
      "exact": [
        { "me": "户山香澄", "seat": 0, "opponents": ["美竹兰"], "cards": ["a", "b"] }
      ],
      "seat_bands": [
        { "me": "户山香澄", "seat": 1, "opponent_bands": ["Afterglow"], "cards": ["c"] }
      ],
      "bands": [
        { "me": "户山香澄", "opponent_bands": ["Afterglow"], "cards": ["d"] }
      ],
      "me": [
        { "me": "户山香澄", "cards": ["e"] }
      ]
    }"#;
    let b = DeckBook::parse(body).unwrap();
    assert_eq!(b.version, DECK_BOOK_VERSION);
    assert_eq!(b.ruleset_sha256, "stub");
    assert_eq!(b.policy, POLICY_STANDARD);
    assert_eq!(b.exact.len(), 1);
    assert_eq!(b.exact[0].seat, Some(0));
    assert_eq!(b.exact[0].opponents, vec!["美竹兰".to_string()]);
    assert_eq!(b.seat_bands[0].seat, Some(1));
    assert_eq!(b.bands[0].cards, vec!["d".to_string()]);
    assert_eq!(b.me[0].cards, vec!["e".to_string()]);
    assert!(b.usable("stub"));
    assert!(!b.usable("other"));
    assert!(DeckBook::parse("{").is_err());
}

#[test]
fn the_shipped_book_is_empty_and_inert() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let text = std::fs::read_to_string(dir.join("deck_book.json")).expect("shipped placeholder");
    let b = DeckBook::parse(&text).unwrap();
    assert!(b.is_empty(), "D2 has not derived entries yet");
    // An empty book is inert whatever it claims -- behaviour unchanged.
    let d = data();
    assert_eq!(DeckBook::default().lookup(&d, me(&d), 0, &[], "stub"), None);
}

// ---------------------------------------------------------------- engine

/// Solo with preset characters: skips ban / pick and opens the deck phase.
/// Seat 1 is human (so the deck phase holds); the rest are standard bots.
fn solo_bots(d: Arc<GameData>, seed: u64) -> Match {
    let names = ["户山香澄", "花园多惠", "美竹兰"];
    let members: Vec<RoomMember> = names
        .iter()
        .enumerate()
        .map(|(i, c)| RoomMember {
            id: i as i32 + 1,
            player: format!("P{}", i + 1),
            bot: i != 0,
            character: (*c).to_string(),
            ..Default::default()
        })
        .collect();
    Match::new(d, Arc::new(StubRules), &members, seed, MatchMode::Solo, ScoreWeights::default())
}

/// The bot seats of `solo_bots`, `(member, character)`.
fn bot_seats(m: &Match) -> Vec<(i32, String)> {
    m.state()
        .players
        .iter()
        .filter(|p| p.bot)
        .map(|p| (p.member, p.character.clone()))
        .collect()
}

#[test]
fn standard_bots_submit_the_book_entry_when_present() {
    let mut d = data();
    let mut per_character = Vec::new();
    for name in ["户山香澄", "花园多惠", "美竹兰"] {
        let c = d.character(name).unwrap();
        let cards = deck_slice(&d, c, 1);
        assert_ne!(cards, deck::preset(&d, c), "the book deck must beat the preset");
        per_character.push((name.to_string(), cards.clone()));
        // Level 4 keeps the test independent of the seed's seating.
        d.deck_book.me.push(entry(name, None, &[], &[], cards));
    }
    d.deck_book.version = DECK_BOOK_VERSION;
    d.deck_book.ruleset_sha256 = "stub".into(); // StubRules stamps "stub"
    d.deck_book.policy = POLICY_STANDARD.into();

    let m = solo_bots(Arc::new(d.clone()), 42);
    assert_eq!(m.state().phase, "deck", "the human holds the deck phase open");
    for (member, name) in bot_seats(&m) {
        let (_, cards) = per_character.iter().find(|(n, _)| *n == name).unwrap();
        let mut want = cards.clone();
        want.sort();
        assert_eq!(m.draw_of(member), want, "{name} runs the book deck");
    }
}

#[test]
fn standard_bots_keep_the_preset_without_a_book() {
    let d = data();
    assert!(d.deck_book.is_empty());
    let m = solo_bots(Arc::new(d.clone()), 42);
    assert_eq!(m.state().phase, "deck");
    for (member, name) in bot_seats(&m) {
        let c = d.character(&name).unwrap();
        let mut want = deck::preset(&d, c);
        want.sort();
        assert_eq!(m.draw_of(member), want, "{name}");
    }
}

#[test]
fn standard_bots_keep_the_preset_on_a_stale_book() {
    let mut d = data();
    for name in ["户山香澄", "花园多惠", "美竹兰"] {
        let c = d.character(name).unwrap();
        let cards = deck_slice(&d, c, 1);
        d.deck_book.me.push(entry(name, None, &[], &[], cards));
    }
    d.deck_book.ruleset_sha256 = "some-other-ruleset".into();
    d.deck_book.policy = POLICY_STANDARD.into();
    let m = solo_bots(Arc::new(d.clone()), 42);
    assert_eq!(m.state().phase, "deck");
    for (member, name) in bot_seats(&m) {
        let c = d.character(&name).unwrap();
        let mut want = deck::preset(&d, c);
        want.sort();
        assert_eq!(m.draw_of(member), want, "{name}");
    }
}