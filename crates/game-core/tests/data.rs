//! P2 tests against the real game data in `web/data/` (copied by the asset pipeline).

use game_core::data::GameData;
use game_core::net::{clean_name, NetMessage};
use game_core::profile::{PlayerProfile, Seen};
use game_core::state::{MatchEvent, MatchPlayer, MatchState};
use game_core::{deck, MatchMode};

fn data() -> GameData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
        .expect("web/data should load -- run tools/asset-pipe/extract.py")
}

#[test]
fn game_data_counts_match_the_original() {
    let d = data();
    assert_eq!(d.tiles.len(), 60);
    assert_eq!(d.cards.len(), 184);
    assert_eq!(d.characters.len(), 54);
    assert_eq!(d.bands.len(), 12);
    assert_eq!(d.events.len(), 28);
    assert_eq!(d.match_rules.ring_multiplier, 10);
    assert!(!d.rules_text.is_empty());

    let kind = |k: &str| d.tiles.iter().filter(|t| t.kind == k).count();
    assert_eq!((kind("property"), kind("agent"), kind("ring")), (41, 11, 4));
    assert_eq!(d.tiles.iter().filter(|t| t.is_corner()).count(), 4);
    assert_eq!(d.tiles.iter().filter(|t| t.is_buyable()).count(), 45);
    assert!(
        d.tiles
            .iter()
            .enumerate()
            .all(|(i, t)| t.index == i as i32 + 1),
        "tiles are 1-indexed in order"
    );
}

#[test]
fn lookups() {
    let d = data();
    assert_eq!(d.card("AG:Y.O.L.O").unwrap().band, "Afterglow");
    assert!(d.card("nope").is_none());
    assert_eq!(d.cards.iter().filter(|c| c.general()).count(), 14);

    let kasumi = d.character("户山香澄").unwrap();
    assert_eq!(kasumi.art_id(), "001");
    assert!(!d.voice_lines_for(kasumi).is_empty());
    // Characters added later carry an explicit `art` id instead of cnId.
    assert!(d
        .characters
        .iter()
        .any(|c| !c.art.is_empty() && c.art_id() == c.art));

    assert_eq!(d.school_of("户山香澄"), "花咲川女子学院");
    assert_eq!(d.school_of("nobody"), "周边学区");
    assert!(d.cards.iter().any(|c| d.is_song_card(c)));
    assert_eq!(d.band("Poppin' Party").unwrap().short_name, "PPP");
    assert!(d.band("nope").is_none());
}

#[test]
fn json_defaults_match_csharp_initializers() {
    let s: MatchState = serde_json::from_str("{}").unwrap();
    assert_eq!((s.turn, s.roller, s.landed, s.winner), (-1, -1, -1, -1));
    assert_eq!(
        (s.score_money, s.score_property, s.score_houses),
        (1.0, 1.0, 1.0)
    );
    assert_eq!((s.prompt.tile, s.prompt.bidder, s.vote.by), (-1, -1, -1));
    assert!(!s.active());

    let player_id: MatchPlayer = serde_json::from_str(r#"{"bankrupt":true}"#).unwrap();
    assert_eq!((player_id.hand_limit(), player_id.exile_to()), (5, -1));
    assert!(player_id.out());

    let e: MatchEvent = serde_json::from_str(r#"{"id":7,"type":"dice"}"#).unwrap();
    assert_eq!(
        (e.id, e.r#type.as_str(), e.player_id, e.other),
        (7, "dice", -1, -1)
    );

    let m: NetMessage = serde_json::from_str(r#"{"t":"act","act":"roll"}"#).unwrap();
    assert_eq!(m.target, -1);
    let json = serde_json::to_value(&m).unwrap();
    assert!(
        json.get("match").is_none() && json.get("room").is_none(),
        "absent nested objects stay absent"
    );
}

#[test]
fn match_state_round_trips_with_camel_case_names() {
    let mut s = MatchState {
        phase: "play".into(),
        turn: 2,
        ..Default::default()
    };
    s.players.push(MatchPlayer {
        member: 3,
        money: 15000,
        ..Default::default()
    });
    let v = serde_json::to_value(&s).unwrap();
    for key in [
        "matchId",
        "skipMove",
        "eventActive",
        "endReason",
        "scoreHouses",
    ] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
    // Keyed state is one map of {value, min, max, expires} items.
    assert_eq!(v["players"][0]["state"]["handLimit"]["value"], 5);
    assert_eq!(v["players"][0]["state"]["exileTo"]["value"], -1);
    let back: MatchState = serde_json::from_value(v).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.player_of(3), 0);
    assert_eq!(back.current(), None, "turn 2 with one player");
}

#[test]
fn every_character_has_a_complete_legal_preset() {
    let d = data();
    for c in &d.characters {
        let p = deck::preset(&d, c);
        assert_eq!(p.len(), deck::SIZE, "{} preset", c.name);
        for id in &p {
            let card = d.card(id).unwrap();
            assert!(deck::can_use(&d, c, card), "{} cannot use {id}", c.name);
            assert!(!card.derived);
        }
        // clean() is order-insensitive: it returns pool order.
        let rev: Vec<String> = p.iter().rev().cloned().collect();
        assert_eq!(deck::clean(&d, c, &rev), p);
        assert!(deck::pool(&d, c).iter().all(|k| !k.derived));
    }
}

#[test]
fn deck_rules_reject_with_csharp_reasons() {
    let d = data();
    let kasumi = d.character("户山香澄").unwrap();
    let derived = d.cards.iter().find(|c| c.derived).unwrap();
    assert_eq!(
        deck::cant_play(&d, kasumi, derived).unwrap().key(),
        "err.deck_derived"
    );

    let other_band = d
        .cards
        .iter()
        .find(|c| !c.general() && !c.exclusive() && c.band != kasumi.band && !c.derived)
        .unwrap();
    assert_eq!(
        deck::cant_play(&d, kasumi, other_band).unwrap().key(),
        "err.deck_band"
    );

    let someone_elses = d
        .cards
        .iter()
        .find(|c| c.exclusive() && c.owner != kasumi.name && !c.derived)
        .unwrap();
    assert_eq!(
        deck::cant_play(&d, kasumi, someone_elses).unwrap().key(),
        "err.deck_exclusive"
    );

    // Sumimi members share exclusives (the original has exactly two members).
    let sumimi: Vec<_> = d.characters.iter().filter(|c| c.band == "Sumimi").collect();
    assert_eq!(sumimi.len(), 2);
    let (a, b) = (sumimi[0], sumimi[1]);
    let shared = d
        .cards
        .iter()
        .filter(|k| k.exclusive() && k.owner == b.name && !k.derived)
        .count();
    let usable = d
        .cards
        .iter()
        .filter(|k| k.exclusive() && k.owner == b.name && !k.derived && deck::can_use(&d, a, k))
        .count();
    assert!(shared > 0, "{} should own exclusive cards", b.name);
    assert_eq!(
        usable, shared,
        "{} should be able to use all of {}'s exclusives",
        a.name, b.name
    );
}

#[test]
fn deck_save_choose_and_clear() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let mut p = PlayerProfile::default();
    let preset = deck::preset(&d, c);
    let id = deck::create(&d, &mut p, c, "", &preset).unwrap();
    assert_eq!(id, 1);
    assert_eq!(deck::cards(&d, &p, c, id), preset);
    assert_eq!(deck::list(&p, c).len(), 1);
    assert!(!deck::save(&d, &mut p, c, 9, &preset), "unknown deck");

    assert!(deck::choose(&mut p, c, id));
    assert!(!deck::choose(&mut p, c, id), "no change");
    assert_eq!(deck::chosen_slot(&d, &p, c), id);

    let half: Vec<&String> = preset.iter().take(5).collect();
    assert!(deck::save(&d, &mut p, c, id, &half));
    assert_eq!(
        deck::chosen_slot(&d, &p, c),
        0,
        "incomplete deck falls back to preset"
    );

    assert!(
        deck::save(&d, &mut p, c, id, &Vec::<String>::new()),
        "empty clears"
    );
    assert_eq!(deck::list(&p, c).len(), 1, "the deck stays after a clear");
    assert!(deck::cards(&d, &p, c, id).is_empty());

    assert!(deck::delete(&mut p, c, id));
    assert!(p.decks.is_empty());
    assert!(!deck::delete(&mut p, c, id), "already gone");
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
    );
    assert_eq!(p.player_name, "Tomori");
    assert!(!p.has_any_new(&d), "a new profile has seen everything");
    assert!(p.refresh_daily("2026-10-05"));
    assert!(!p.refresh_daily("2026-10-05"));
    assert_eq!(p.fire, 5);

    p.set_fire_per_game(3);
    let r = p.apply_match(MatchMode::Ranked, 1, 6, "户山香澄", "2026-10-05 20:00");
    assert_eq!(
        (r.base_exp, r.fire_used, r.multiplier, r.exp),
        (200, 3, 4, 800)
    );
    assert_eq!(p.fire, 2);
    // 800 EXP from level 0: 100 + 115 + 130 + 145 + 160 = 650 -> level 5, 150 left.
    assert_eq!((p.level, p.exp, p.total_exp), (5, 150, 800));
    assert_eq!((r.coins, p.coins, p.ranked_wins, p.stars), (500, 500, 1, 1));
    assert_eq!(
        p.stat_of("户山香澄").map(|s| (s.uses, s.firsts)),
        Some((1, 1))
    );
    assert!(p.has_new(&d, Seen::History));

    // Coins never go below zero, and the reward reports the actual change.
    let mut poor = PlayerProfile::default();
    let r = poor.apply_match(MatchMode::Ranked, 6, 6, "", "t");
    assert_eq!((r.coins, poor.coins), (0, 0));

    for _ in 0..40 {
        poor.apply_match(MatchMode::Casual, 2, 4, "", "t");
    }
    assert_eq!(poor.history.len(), 30);
    assert_eq!(poor.casual_games, 40);
}

#[test]
fn profile_normalize_migrates_v1_saves() {
    let v1 = r#"{"saveVersion":1,"level":900,"firePerGame":9,"homeCharacter":"",
                 "history":[{"ranked":true,"rank":1},{"ranked":false,"rank":2}]}"#;
    let d = data();
    let p = PlayerProfile::from_json(&d, v1).unwrap();
    assert_eq!(p.save_version, 4);
    assert_eq!((p.level, p.fire_per_game), (500, 3));
    assert_eq!(
        p.home_character, d.match_rules.default_home_character,
        "empty home -> data default"
    );
    assert!(d.character(&p.home_character).is_some());
    assert_eq!(p.history[0].mode, MatchMode::Ranked);
    assert_eq!(p.history[1].mode, MatchMode::Casual);
    // MatchMode is an integer on disk, like JsonUtility writes it.
    assert_eq!(serde_json::to_value(&p.history[0]).unwrap()["mode"], 2);
}

#[test]
fn live2d_picks() {
    let d = data();
    let c = d.character("户山香澄").unwrap();
    let mut p = PlayerProfile::default();
    let opts = ["001", "001b"];
    assert_eq!(p.live2d_for(c, &opts), "001");
    assert!(p.set_live2d(c, "001b", &opts));
    assert_eq!(p.live2d_for(c, &opts), "001b");
    assert_eq!(
        p.live2d_for(c, &["001"]),
        "001",
        "pick no longer offered falls back"
    );
    assert!(p.set_live2d(c, "001", &opts));
    assert!(p.live2d_picks.is_empty(), "default is not stored");
}

#[test]
fn clean_name_counts_utf16_units_like_csharp() {
    assert_eq!(clean_name("   "), "");
    assert_eq!(
        clean_name("一二三四五六七八九十一二三四五六七"),
        "一二三四五六七八九十一二三四五六"
    );
    // 15 units + a 2-unit emoji would be 17: the emoji is dropped, not split.
    assert_eq!(clean_name("aaaaaaaaaaaaaaa🎸"), "aaaaaaaaaaaaaaa");
}

#[test]
fn bot_names_follow_the_list_then_suffix() {
    use game_core::net::bot_name;
    let names = &data().match_rules.bot_names;
    assert!(names.len() >= 3, "bot names come from match_rules.json");
    assert_eq!(bot_name(names, [].iter().copied()), names[0]);
    assert_eq!(
        bot_name(names, [names[0].as_str(), "someone"].iter().copied()),
        names[1]
    );
    let all: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(
        bot_name(names, all.iter().copied()),
        format!("{}2", names[0])
    );
}
