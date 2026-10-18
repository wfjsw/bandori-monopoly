//! P2 tests against the data dir (board / events) plus the synthetic roster fixture.

use game_core::net::{clean_name, NetMessage};
use game_core::state::{MatchEvent, MatchPlayer, MatchState};
use game_core::deck;
mod common;
use common::data;

#[test]
fn group_traits_and_variants_come_from_data() {
    let d = data();
    assert_eq!(d.base_persona("Gamma"), "Alpha");
    assert_eq!(d.base_persona("Epsilon"), "Delta");
    assert_eq!(d.base_persona("Alpha"), "Alpha");
    let sharing = d.persona("Gamma").unwrap();
    let exclusive = d.persona("Epsilon").unwrap();
    assert!(
        d.group_shares_exclusives(sharing) && !d.group_shares_exclusives(exclusive),
        "GroupA shares exclusives; GroupB does not"
    );
    assert!(d.groups.iter().all(|g| !g.boot_art.is_empty() || g.name == "通用"));
}

#[test]
fn game_data_counts_match_the_board_and_fixture() {
    let d = data();
    // Board / events come from `data/`; roster comes from the synthetic fixture.
    assert_eq!(d.tiles.len(), 60);
    assert_eq!(d.cards.len(), 21);
    assert_eq!(d.personas.len(), 5);
    assert_eq!(d.groups.len(), 3);
    assert_eq!(d.events.len(), 28);
    assert_eq!(d.match_rules.rule_params.get("rentUnitMult"), Some(&10));
    assert!(!d.rules_text.is_empty());

    assert_eq!(d.tiles.iter().filter(|t| t.is_buyable()).count(), 45);
    assert_eq!(
        d.tiles
            .iter()
            .filter(|t| t.is_buyable() && t.rent.len() < 2)
            .count(),
        4,
        "four non-upgradable deeds"
    );
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
    assert_eq!(d.card("B:01").unwrap().group, "GroupB");
    assert!(d.card("nope").is_none());
    assert_eq!(d.cards.iter().filter(|c| c.is_universal()).count(), 13);

    let alpha = d.persona("Alpha").unwrap();
    assert_eq!(alpha.art_id(), "syn-001");
    assert!(d.voice_lines_for(alpha).is_empty(), "fixture personas ship no voice lines");
    // Personas added later carry an explicit `art` id instead of cnId.
    assert!(d
        .personas
        .iter()
        .any(|c| !c.art.is_empty() && c.art_id() == c.art));

    // Fixture personas are not in `schools.json`; both fall back.
    let fallback = d.school_of("nobody");
    assert_eq!(d.school_of("Alpha"), fallback);
    assert_eq!(d.group("GroupA").unwrap().short_name, "GA");
    assert!(d.group("nope").is_none());
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
    assert_eq!((player_id.hand_limit(), player_id.exile_to()), (Some(5), -1));
    assert!(player_id.out());

    let e: MatchEvent = serde_json::from_str(r#"{"id":7,"type":"dice"}"#).unwrap();
    assert_eq!(
        (e.id, e.r#type.as_str(), e.player_id, e.other),
        (7, "dice", -1, -1)
    );

    let m: NetMessage = serde_json::from_str(r#"{"t":"act","act":"diceRollBefore"}"#).unwrap();
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
fn every_persona_has_a_complete_legal_preset() {
    let d = data();
    for c in &d.personas {
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
    let alpha = d.persona("Alpha").unwrap();
    let derived = d.cards.iter().find(|c| c.derived).unwrap();
    assert_eq!(
        deck::cant_play(&d, alpha, derived).unwrap().key(),
        "err.deck_derived"
    );

    let other_group = d
        .cards
        .iter()
        .find(|c| !c.is_universal() && !c.exclusive() && c.group != alpha.group && !c.derived)
        .unwrap();
    assert_eq!(
        deck::cant_play(&d, alpha, other_group).unwrap().key(),
        "err.deck_group"
    );

    let someone_elses = d
        .cards
        .iter()
        .find(|c| {
            c.exclusive()
                && c.owner != alpha.name
                && !c.derived
                && !deck::shares_exclusives(&d, alpha, c)
        })
        .unwrap();
    assert_eq!(
        deck::cant_play(&d, alpha, someone_elses).unwrap().key(),
        "err.deck_exclusive"
    );

    // GroupA members share exclusives (the fixture has three members).
    let sharing: Vec<_> = d.personas.iter().filter(|c| c.group == "GroupA").collect();
    assert_eq!(sharing.len(), 3);
    let (a, b) = (sharing[0], sharing[1]);
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
