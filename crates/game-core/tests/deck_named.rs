//! Named decks: arbitrary count per character, rename / duplicate / delete /
//! reorder, and lossless migration of the old fixed-slot profile JSON.

use game_core::data::GameData;
use game_core::deck;
use game_core::profile::{sanitize_deck_name, PlayerProfile, SavedDeck, DECK_NAME_MAX};

fn data() -> GameData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
        .expect("data/ should load -- run tools/asset-pipe/extract.py")
}

#[test]
fn create_duplicate_rename_delete_and_reorder() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let preset = deck::preset(&d, c);
    let mut p = PlayerProfile::default();

    // Any number of decks: five in a row, each with a distinct id.
    let mut ids = Vec::new();
    for i in 0..5 {
        let name = if i == 0 { "快攻" } else { "" };
        let id = deck::create(&d, &mut p, c, name, &preset).expect("under the cap");
        ids.push(id);
    }
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    let names: Vec<&str> = deck::list(&p, c).iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, vec!["快攻", "", "", "", ""]);

    // Duplicate lands right after the source and is auto-named.
    let copy = deck::duplicate(&d, &mut p, c, 1).unwrap();
    assert_eq!(copy, 6);
    let order: Vec<i32> = deck::list(&p, c).iter().map(|x| x.slot).collect();
    assert_eq!(order, vec![1, 6, 2, 3, 4, 5]);
    assert_eq!(deck::list(&p, c)[1].name, "", "the copy is auto-named");
    assert_eq!(deck::cards(&d, &p, c, copy), preset);

    // Rename: trimmed, capped, duplicates allowed, blank restores the auto name.
    assert!(deck::rename(&mut p, c, 2, "  要乐奈专用  "));
    assert_eq!(deck::list(&p, c)[2].name, "要乐奈专用");
    assert!(deck::rename(&mut p, c, 3, "要乐奈专用"), "duplicates allowed");
    assert!(!deck::rename(&mut p, c, 2, "要乐奈专用"), "no change");
    let long = "あ".repeat(DECK_NAME_MAX + 10);
    assert!(deck::rename(&mut p, c, 4, &long));
    assert_eq!(deck::list(&p, c)[4].name.chars().count(), DECK_NAME_MAX);
    assert!(deck::rename(&mut p, c, 2, "   "), "blank restores auto");
    assert_eq!(deck::list(&p, c)[2].name, "");

    // Reorder: ids and names travel with the deck.
    assert!(deck::move_by(&mut p, c, 6, -1), "move the copy up");
    let order: Vec<i32> = deck::list(&p, c).iter().map(|x| x.slot).collect();
    assert_eq!(order, vec![6, 1, 2, 3, 4, 5]);
    assert!(!deck::move_by(&mut p, c, 6, -1), "already on top");
    assert!(!deck::move_by(&mut p, c, 5, 1), "already at the bottom");
    assert!(!deck::move_by(&mut p, c, 5, 0), "zero delta");

    // Delete: the chosen default falls back to the preset; ids stay put.
    assert!(deck::choose(&mut p, c, 6));
    assert!(deck::delete(&mut p, c, 6));
    assert_eq!(deck::chosen_slot(&d, &p, c), 0);
    assert!(p.deck_choices.is_empty());
    let order: Vec<i32> = deck::list(&p, c).iter().map(|x| x.slot).collect();
    assert_eq!(order, vec![1, 2, 3, 4, 5]);

    // Cap at DECKS_MAX per character (other characters unaffected).
    while deck::list(&p, c).len() < deck::DECKS_MAX {
        assert!(deck::create(&d, &mut p, c, "", &Vec::<String>::new()).is_some());
    }
    assert!(
        deck::create(&d, &mut p, c, "", &Vec::<String>::new()).is_none(),
        "at the cap"
    );
    let other = d.character("花园多惠").unwrap();
    assert!(
        deck::create(&d, &mut p, other, "", &Vec::<String>::new()).is_some(),
        "the cap is per character"
    );
}

#[test]
fn old_slot_profile_migrates_to_named_decks() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let other = d.character("花园多惠").unwrap();
    let preset = deck::preset(&d, c);
    let full = serde_json::to_string(&preset).unwrap();
    let a = preset[0].clone();

    // A v3 profile as the old web client saved it: three fixed slots, no `name`
    // on SavedDeck, camelCase outer keys / snake_case deck keys.
    let old = format!(
        r#"{{
      "saveVersion": 3,
      "playerName": "灯",
      "playerId": "123456789",
      "homeCharacter": "户山香澄",
      "decks": [
        {{"character": "户山香澄", "slot": 2, "cards": {full}}},
        {{"character": "花园多惠", "slot": 1, "cards": []}},
        {{"character": "户山香澄", "slot": 1, "cards": []}},
        {{"character": "户山香澄", "slot": 3, "cards": ["{a}"]}}
      ],
      "deckChoices": [
        {{"character": "户山香澄", "slot": 2}},
        {{"character": "花园多惠", "slot": 1}}
      ]
    }}"#
    );
    let p = PlayerProfile::from_json(&d, &old).unwrap();
    assert_eq!(p.save_version, 4);
    assert_eq!(p.player_name, "灯", "everything else survives");

    let decks = deck::list(&p, c);
    // Slot order 1,2,3 is the display order; names are empty (auto) so the UI
    // shows exactly the old 「卡组 1」/「卡组 2」/「卡组 3」 from the ids.
    let got: Vec<(i32, &str)> = decks.iter().map(|x| (x.slot, x.name.as_str())).collect();
    assert_eq!(got, vec![(1, ""), (2, ""), (3, "")]);
    assert_eq!(
        deck::cards(&d, &p, c, 2),
        preset,
        "the card list is kept"
    );
    // The chosen slot still points at the same deck.
    assert_eq!(deck::chosen_slot(&d, &p, c), 2);
    // The empty old slot is kept as a deck (clearable/deletable, not dropped).
    assert!(deck::is_empty(&d, &p, c, 1));
    assert_eq!(deck::list(&p, c).len(), 3);
    assert_eq!(deck::list(&p, other).len(), 1, "other characters keep theirs");
    assert_eq!(
        deck::chosen_slot(&d, &p, other),
        0,
        "its empty choice falls back to the preset"
    );

    // Round-trip: what normalize() writes out loads back identically.
    let json = serde_json::to_string(&p).unwrap();
    let back = PlayerProfile::from_json(&d, &json).unwrap();
    assert_eq!(back, p);

    // A deck whose cards are all stale ids cleans to empty but keeps its slot;
    // the chosen deck is not offered until it is complete again.
    let stale = r#"{
      "saveVersion": 3,
      "decks": [{"character": "户山香澄", "slot": 1, "cards": ["nope:gone"]}],
      "deckChoices": [{"character": "户山香澄", "slot": 1}]
    }"#;
    let p2 = PlayerProfile::from_json(&d, stale).unwrap();
    assert_eq!(deck::list(&p2, c).len(), 1);
    assert_eq!(deck::list(&p2, c)[0].slot, 1);
    assert!(deck::cards(&d, &p2, c, 1).is_empty());
    assert_eq!(
        deck::chosen_slot(&d, &p2, c),
        0,
        "an incomplete deck is not offered"
    );
}

#[test]
fn profile_json_carries_deck_names_and_stays_compatible() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let mut p = PlayerProfile::default();
    let id = deck::create(&d, &mut p, c, "  双刀流  ", &deck::preset(&d, c)).unwrap();
    let json = serde_json::to_string(&p).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let saved = &v["decks"][0];
    assert_eq!(saved["name"], "双刀流");
    assert_eq!(saved["slot"], id);
    assert!(saved.get("cards").is_some());
    assert_eq!(v["saveVersion"], 4);
    // The new `name` field is the only addition to SavedDeck; old slot JSON
    // (no `name`) still parses with an auto name.
    let no_name: SavedDeck =
        serde_json::from_str(r#"{"character":"x","slot":1,"cards":["a"]}"#).unwrap();
    assert_eq!(no_name.name, "");
    assert_eq!(sanitize_deck_name("  a  "), "a");
    assert_eq!(sanitize_deck_name("   "), "");
}

#[test]
fn hand_edited_deck_ids_are_repaired_on_load() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let messy = r#"{
      "saveVersion": 4,
      "decks": [
        {"character": "户山香澄", "slot": 0, "cards": [], "name": "  "},
        {"character": "户山香澄", "slot": 2, "cards": [], "name": "A"},
        {"character": "户山香澄", "slot": 2, "cards": [], "name": "B"},
        {"character": "户山香澄", "slot": -3, "cards": [], "name": "C"}
      ]
    }"#;
    let p = PlayerProfile::from_json(&d, messy).unwrap();
    let ids: Vec<i32> = deck::list(&p, c).iter().map(|x| x.slot).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(ids.len(), 4);
    assert_eq!(sorted.len(), 4, "ids are unique and positive");
    assert!(ids.iter().all(|&i| i > 0));
    let names: Vec<&str> = deck::list(&p, c).iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, vec!["", "A", "B", "C"]);
}