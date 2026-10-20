//! No log message may render an internal key as verbatim text.
//!
//! `Arg::Text` is for player-entered names only (`msg.rs`). Counter names,
//! state keys, mark kinds and trigger-kind wire names must ride as a nested
//! `Msg` whose key has i18n display text. This test walks a seeded game's
//! event tail (including `results` and nested args) and fails on a raw
//! `cards:` / `mark:` key, a lower-camel state key, or a known counter name
//! appearing as `Text`.

mod common;

use common::*;
use game_core::msg::{Arg, Msg};
use game_core::state::MatchEvent;

/// Internal-key shapes that must never appear as `Arg::Text`.
fn is_internal_key(s: &str) -> bool {
    if s.starts_with("cards:") || s.starts_with("mark:") || s.starts_with("skill.") {
        return true;
    }
    // Known on-card counter / mark names (ruleset wire keys).
    if matches!(s, "bonus" | "points" | "pointsField" | "skillBlock") {
        return true;
    }
    // Lower-camel engine state / prop keys (handLimit, startHand, skillState, …).
    // Rulebook terms are Chinese or bracketed; card-declared counters are display names.
    if s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars().any(|c| c.is_ascii_uppercase())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return true;
    }
    false
}

fn walk_arg(arg: &Arg, where_: &str, bad: &mut Vec<String>) {
    match arg {
        Arg::Text(t) => {
            if is_internal_key(t) {
                bad.push(format!("{where_}: Text({t:?})"));
            }
        }
        Arg::Msg(m) => walk_msg(m, where_, bad),
        Arg::List(items) => {
            for (i, a) in items.iter().enumerate() {
                walk_arg(a, &format!("{where_}[{i}]"), bad);
            }
        }
        _ => {}
    }
}

fn walk_msg(m: &Msg, where_: &str, bad: &mut Vec<String>) {
    let w = format!("{where_} {}", m.key());
    for (name, arg) in m.a.iter() {
        walk_arg(arg, &format!("{w}.{name}"), bad);
    }
}

fn check_events(events: &[MatchEvent], label: &str) {
    let mut bad = Vec::new();
    for e in events {
        let w = format!("{label}#{} {}", e.id, e.r#type);
        walk_msg(&e.msg, &w, &mut bad);
        for (i, r) in e.results.iter().enumerate() {
            walk_msg(r, &format!("{w}.results[{i}]"), &mut bad);
        }
    }
    assert!(
        bad.is_empty(),
        "internal keys rendered as Text ({} lines):\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// A seeded game with skills and cards in play: every outcome / why line that
/// ran must name its subject via a typed arg, never a raw key.
#[test]
fn no_log_message_renders_an_internal_key_as_text() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    // Drive several shapes: a field hook, a money line, a roll, a turn.
    t.give_play(0, "TEST:tileHook").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.set_pos(0, 0);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.end(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.begin_turn(1);
    while t.prompt().is_some() {
        t.decline();
    }
    check_events(&t.events_since(mark), "fixture-game");
}

/// Shipped skill/card outcomes (counter writes, state writes, plan rewrites)
/// across a short real game: same contract.
#[test]
fn no_shipped_outcome_renders_an_internal_key_as_text() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.set_persona_raw(0, "濑田薰");
    t.set_persona_raw(1, "和奏瑞依");
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let mark = t.mark();
    // A few turns: hooks run, caps re-arm, opt-ins decline.
    for _ in 0..3 {
        t.set_pos(0, 0);
        t.dice(&[3]);
        t.roll(0).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.end(0).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.begin_turn(1);
        while t.prompt().is_some() {
            t.decline();
        }
        t.set_pos(1, 0);
        t.dice(&[3]);
        t.roll(1).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.end(1).unwrap();
        while t.prompt().is_some() {
            t.decline();
        }
        t.begin_turn(0);
        while t.prompt().is_some() {
            t.decline();
        }
    }
    check_events(&t.events_since(mark), "shipped-game");
}
/// Every trigger-kind wire name the chain start can show has an
/// `anim.chainKind.*` label, and every counter name a `log.counter.*` phrase,
/// in both locales -- otherwise the player reads the raw key (`moveBefore`).
#[test]
fn chain_kinds_and_counters_have_locale_text() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../webui/src/i18n/locales");
    let load = |loc: &str, file: &str| -> serde_json::Value {
        let p = format!("{dir}/{loc}/{file}.json");
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
    };
    let mut missing = Vec::new();
    for loc in ["zh-CN", "en"] {
        let (ui, game) = (load(loc, "ui"), load(loc, "game"));
        for id in 1..=400 {
            let k = game_rules::TriggerKind::engine(id).spelling();
            if k.is_empty() {
                continue;
            }
            let key = format!("anim.chainKind.{k}");
            if ui.get(&key).is_none() && ui.get(format!("anim.chainKind.{id}")).is_none() {
                missing.push(format!("{loc}: {key}"));
            }
        }
        // Kinds with no dedicated `log.why.*` phrase, named in the fallback.
        for k in [
            "diceRollPlan",
            "playGate",
            "markerSpent",
            "cardUsed",
            "houseAdded",
            "r:bandori:26",
            "r:bandori:78",
            "r:bandori:107",
        ] {
            if game.get(format!("log.kind.{k}")).is_none() {
                missing.push(format!("{loc}: log.kind.{k}"));
            }
        }
        for c in ["crystals", "cp", "points", "bonus"] {
            let key = format!("log.counter.{c}");
            if game.get(&key).is_none() {
                missing.push(format!("{loc}: {key}"));
            }
        }
    }
    assert!(missing.is_empty(), "missing locale text: {missing:?}");
}

/// `TileMark::kind` **is** the i18n key (`mark_kind_name` -> `Msg::new(kind)`),
/// so every kind a shipped rule places has to carry display text in both
/// locales. The keys are looked up at render time (nothing references them as
/// message keys), which is exactly how an unused-key sweep once deleted every
/// one of them and left the board printing raw kinds. Keep this list in step
/// with `tools/i18n/check.py`'s `RUNTIME_KEYS`.
#[test]
fn mark_kinds_have_locale_text() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../webui/src/i18n/locales");
    let kinds = [
        card_sdk::abi::mark::CP_KIND,
        card_sdk::abi::mark::DESIGNATE_KIND,
        card_sdk::abi::mark::NO_TARGET,
        // Ruleset kinds that are not ABI constants (events/skills/cards).
        "ember",
        "skillBlock",
        "skill.uikaIdol.pot",
    ];
    let mut missing = Vec::new();
    for loc in ["zh-CN", "en"] {
        let p = format!("{dir}/{loc}/game.json");
        let game: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        for kind in kinds {
            let text = game.get(kind).and_then(|v| v.as_str()).unwrap_or("");
            if text.is_empty() {
                missing.push(format!("{loc}: {kind}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "mark kinds without locale text (a mark would render as its raw key): {missing:?}"
    );
}

/// The name helpers must produce nested Msg, never a Text arg for a known key.
#[test]
fn name_helpers_never_text_a_known_key() {
    use game_rules::{counter_name, mark_kind_name, state_key_name};
    let mut bad = Vec::new();
    for m in [
        counter_name("bonus"),
        counter_name("points"),
        state_key_name("handLimit"),
        state_key_name("skillState"),
        mark_kind_name("mark:points"),
        mark_kind_name("skillBlock"),
    ] {
        walk_msg(&m, "helper", &mut bad);
    }
    assert!(bad.is_empty(), "helpers leaked internal keys as Text: {bad:?}");
    // Known keys resolve to their own i18n names (wire keys, kept verbatim).
    assert_eq!(counter_name("bonus").key(), "log.counter.bonus");
    assert_eq!(state_key_name("handLimit").key(), "log.state.hand_limit");
    // Generic fallback quotes the key as data (`log.state.other`).
    assert_eq!(state_key_name("someKey").key(), "log.state.other");
    assert_eq!(mark_kind_name("mark:points").key(), "mark:points");
}
