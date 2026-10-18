//! Profile / progression tests: create + apply_match, v1 migration, Live2D
//! picks, and the save/choose/clear half of named decks.

use game_core::deck;
use game_core::MatchMode;
use player_meta::decks;
use player_meta::profile::{PlayerProfile, Seen};
mod common;
use common::data;

#[test]
fn deck_save_choose_and_clear() {
    let d = data();
    let c = d.persona("Alpha").unwrap();
    let mut p = PlayerProfile::default();
    let preset = deck::preset(&d, c);
    let id = decks::create(&d, &mut p, c, "", &preset).unwrap();
    assert_eq!(id, 1);
    assert_eq!(decks::cards(&d, &p, c, id), preset);
    assert_eq!(decks::list(&p, c).len(), 1);
    assert!(!decks::save(&d, &mut p, c, 9, &preset), "unknown deck");

    assert!(decks::choose(&mut p, c, id));
    assert!(!decks::choose(&mut p, c, id), "no change");
    assert_eq!(decks::chosen_slot(&d, &p, c), id);

    let half: Vec<&String> = preset.iter().take(5).collect();
    assert!(decks::save(&d, &mut p, c, id, &half));
    assert_eq!(
        decks::chosen_slot(&d, &p, c),
        0,
        "incomplete deck falls back to preset"
    );

    assert!(
        decks::save(&d, &mut p, c, id, &Vec::<String>::new()),
        "empty clears"
    );
    assert_eq!(decks::list(&p, c).len(), 1, "the deck stays after a clear");
    assert!(decks::cards(&d, &p, c, id).is_empty());

    assert!(decks::delete(&mut p, c, id));
    assert!(p.decks.is_empty());
    assert!(!decks::delete(&mut p, c, id), "already gone");
}

#[test]
fn profile_create_and_apply_match() {
    let d = data();
    let mut p = PlayerProfile::create(
        &d,
        "  Tomori  ",
        "123456789",
        "2026-10-04 12:00",
        "2026-10-04",
        "Alpha",
    );
    assert_eq!(p.player_name, "Tomori");
    assert!(!p.has_any_new(&d), "a new profile has seen everything");
    assert!(p.refresh_daily("2026-10-05"));
    assert!(!p.refresh_daily("2026-10-05"));
    assert_eq!(p.charge, 5);

    p.set_charge_per_game(3);
    let r = p.apply_match(MatchMode::Ranked, 1, 6, "Alpha", "2026-10-05 20:00", "r1a2b3");
    assert_eq!(
        (r.base_exp, r.charge_used, r.multiplier, r.exp),
        (200, 3, 4, 800)
    );
    assert_eq!(p.charge, 2);
    // 800 EXP from level 0: 100 + 115 + 130 + 145 + 160 = 650 -> level 5, 150 left.
    assert_eq!((p.level, p.exp, p.total_exp), (5, 150, 800));
    assert_eq!((r.coins, p.coins, p.ranked_wins, p.stars), (500, 500, 1, 1));
    assert_eq!(
        p.stat_of("Alpha").map(|s| (s.uses, s.firsts)),
        Some((1, 1))
    );
    assert!(p.has_new(&d, Seen::History));
    // The history row carries the local replay-store id it was written with.
    assert_eq!(p.history[0].replay_id, "r1a2b3");

    // Coins never go below zero, and the reward reports the actual change.
    let mut poor = PlayerProfile::default();
    let r = poor.apply_match(MatchMode::Ranked, 6, 6, "", "t", "");
    assert_eq!((r.coins, poor.coins), (0, 0));

    for _ in 0..40 {
        poor.apply_match(MatchMode::Casual, 2, 4, "", "t", "");
    }
    assert_eq!(poor.history.len(), 30);
    assert_eq!(poor.casual_games, 40);
}

#[test]
fn profile_normalize_migrates_v1_saves() {
    let v1 = r#"{"saveVersion":1,"level":900,"chargePerGame":9,"homePersona":"",
                 "history":[{"ranked":true,"rank":1},{"ranked":false,"rank":2}]}"#;
    let d = data();
    let p = PlayerProfile::from_json(&d, v1, "Alpha").unwrap();
    assert_eq!(p.save_version, 4);
    assert_eq!((p.level, p.charge_per_game), (500, 3));
    assert_eq!(
        p.home_persona, "Alpha",
        "empty home -> the caller-supplied default"
    );
    assert_eq!(p.history[0].mode, MatchMode::Ranked);
    assert_eq!(p.history[1].mode, MatchMode::Casual);
    // MatchMode is an integer on disk (`0`/`1`/`2`).
    assert_eq!(serde_json::to_value(&p.history[0]).unwrap()["mode"], 2);
    // Rows written before replay ids existed load with an empty one.
    assert_eq!(p.history[0].replay_id, "");
    assert!(serde_json::to_string(&p).unwrap().contains("replayId"));
}

#[test]
fn live2d_picks() {
    let d = data();
    let c = d.persona("Alpha").unwrap();
    let mut p = PlayerProfile::default();
    let opts = ["syn-001", "syn-001b"];
    assert_eq!(p.live2d_for(c, &opts), "syn-001", "options[0] is the default");
    assert!(p.set_live2d(c, "syn-001b", &opts));
    assert_eq!(p.live2d_for(c, &opts), "syn-001b");
    assert_eq!(
        p.live2d_for(c, &["syn-001"]),
        "syn-001",
        "pick no longer offered falls back"
    );
    assert!(p.set_live2d(c, "syn-001", &opts));
    assert!(p.live2d_picks.is_empty(), "default is not stored");
}