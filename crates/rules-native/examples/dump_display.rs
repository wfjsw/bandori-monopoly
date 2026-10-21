//! Dump the display-only roster columns as `display.json`.
//!
//!     cargo run -q -p rules-native --example dump_display
//!     cargo run -q -p rules-native --example dump_display -- /tmp/display.json
//!     cargo run -q -p rules-native --example dump_display -- --out /tmp/display.json
//!
//! Printed text / display strings are authored in the rule code beside the
//! rule bodies (`CardMeta` / `PersonaDef` / `GroupDef`) but no longer ride
//! `ruleset_metadata`. This example reads those defs directly and serializes
//! them into the shape the webui bundles (`docs/DISPLAY.md`):
//!
//! ```json
//! {
//!   "cards":    { "<cardId>":      { "text": "...", "rawName": "...",
//!                                     "counterReason": "..." } },
//!   "personas": { "<personaName>": { "display": "...", "text": "...", "color": "...",
//!                                     "variantTag": "...", "art": "...", "costume": "...",
//!                                     "cnId": "..." } },
//!   "groups":   { "<groupName>":   { "text": "...", "shortName": "...", "color": "...",
//!                                     "logo": "..." } },
//!   "events":   { "<eventId>":     { "name": "...", "text": "..." } }
//! }
//! ```
//!
//! * camelCase keys; empty / absent fields are omitted (so only [反击] cards
//!   carry `counterReason`).
//! * Event keys are the **bare** event id (`对邦`), not the `event:` rule-id
//!   prefix. `derived` is engine data and is not exported.
//! * `counterReason` is read from the card's [`On::counter_reason`] -- the
//!   「…时」 clause the counteraction prompt shows, as a **locale key** the
//!   client resolves (`cards:<crate>.…` / `counter.*`). Several entries join
//!   their keys with `\n`.
//! * Deterministic: every map is a `BTreeMap` (sorted keys), pretty JSON,
//!   one trailing newline -- byte-stable across runs.
//! * No argument prints the document to stdout. A path argument (or
//!   `--out <path>`) writes it there, creating parent directories.
//!
//! `tools/export-display.mjs` wraps this for the webui build.

use std::path::PathBuf;

use serde_json::{Map, Value};

type JsonMap = Map<String, Value>;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => {
                let p = args.next().expect("--out needs a path");
                out = Some(PathBuf::from(p));
            }
            "--help" | "-h" => {
                eprintln!(
                    "usage: dump_display [--out <path>]\n\
                     \n\
                     Prints display.json (rule-authored display columns) to stdout,\n\
                     or writes it to <path>. See docs/DISPLAY.md."
                );
                return;
            }
            other if !other.starts_with('-') => {
                out = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(2);
            }
        }
    }

    let json = display_json();
    match out {
        Some(path) => {
            // Skip the write when the bytes are unchanged: the export runs on
            // every `predev` / `prebuild`, and an unconditional write bumps
            // mtime and invalidates the bundler cache for the importing
            // module even when no rule text moved.
            if std::fs::read(&path).ok().as_deref() == Some(json.as_bytes()) {
                eprintln!("unchanged {}", path.display());
                return;
            }
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).unwrap();
                }
            }
            std::fs::write(&path, &json).unwrap();
            eprintln!("wrote {}", path.display());
        }
        None => print!("{json}"),
    }
}

/// `{"cards":…,"personas":…,"groups":…,"events":…}` -- pretty, sorted,
/// trailing newline.
fn display_json() -> String {
    let mut root = JsonMap::new();
    root.insert("cards".into(), Value::Object(cards_map()));
    root.insert("personas".into(), Value::Object(personas_map()));
    root.insert("groups".into(), Value::Object(groups_map()));
    root.insert("events".into(), Value::Object(events_map()));
    let mut s = serde_json::to_string_pretty(&Value::Object(root)).unwrap();
    s.push('\n');
    s
}

/// Every `CardDef` that declares a [`card_sdk::meta::CardMeta`] and is **not**
/// an event (those go in [`events_map`]), keyed by card id. The same set
/// `ruleset_metadata` emits as `kind = "card"` (cards without meta are
/// rule-only pseudo cards: tiles / marks / status).
fn cards_map() -> JsonMap {
    let mut map = JsonMap::new();
    for module in rules_native::RULESET {
        for card in module.iter() {
            if card.id.starts_with("event:") {
                continue;
            }
            let Some(meta) = card.meta else { continue };
            let mut row = JsonMap::new();
            insert_str(&mut row, "text", meta.text);
            insert_str(&mut row, "rawName", meta.raw_name);
            insert_str(&mut row, "counterReason", &counter_reason(card));
            map.insert(card.id.to_string(), Value::Object(row));
        }
    }
    map
}

/// The [反击] trigger's `counterReason`: every [`card_sdk::On::counter_reason`]
/// key on this card, joined with `\n`. The manifest carries the key; the UI
/// resolves it at render.
fn counter_reason(card: &card_sdk::CardDef) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for entry in card.on {
        if let Some(reason) = entry.counter_reason() {
            if !reason.is_empty() {
                parts.push(reason);
            }
        }
    }
    parts.join("\n")
}

/// Every event card, keyed by the **bare** event id (`对邦`, matching
/// `data`'s `EventData::id` / `display.json`). `derived` is engine data and
/// remains out.
fn events_map() -> JsonMap {
    let mut map = JsonMap::new();
    for card in rules_events::CARDS {
        let Some(meta) = card.meta else { continue };
        let bare = card.id.strip_prefix("event:").unwrap_or(card.id);
        let mut row = JsonMap::new();
        insert_str(&mut row, "name", meta.name);
        insert_str(&mut row, "text", meta.text);
        map.insert(bare.to_string(), Value::Object(row));
    }
    map
}

/// Every persona row, keyed by persona name (`personas::roster_table::PERSONAS`).
fn personas_map() -> JsonMap {
    let mut map = JsonMap::new();
    for p in personas::roster_table::PERSONAS {
        let mut row = JsonMap::new();
        insert_str(&mut row, "display", p.display);
        insert_str(&mut row, "text", p.text);
        insert_str(&mut row, "color", p.color);
        insert_str(&mut row, "variantTag", p.variant_tag);
        insert_str(&mut row, "art", p.art);
        insert_str(&mut row, "costume", p.costume);
        insert_str(&mut row, "cnId", p.cn_id);
        map.insert(p.name.to_string(), Value::Object(row));
    }
    map
}

/// Every band row, keyed by group name (`skill_groups::roster_table::GROUPS`).
fn groups_map() -> JsonMap {
    let mut map = JsonMap::new();
    for g in skill_groups::roster_table::GROUPS {
        let mut row = JsonMap::new();
        insert_str(&mut row, "text", g.text);
        insert_str(&mut row, "shortName", g.short_name);
        insert_str(&mut row, "color", g.color);
        insert_str(&mut row, "logo", g.logo);
        map.insert(g.name.to_string(), Value::Object(row));
    }
    map
}

/// Omit empty / absent fields.
fn insert_str(row: &mut JsonMap, key: &str, value: &str) {
    if !value.is_empty() {
        row.insert(key.to_string(), Value::String(value.to_string()));
    }
}