//! Dump the roster metadata as JSON.
//!
//!     cargo run -p rules-native --example dump_roster
//!     cargo run -p rules-native --example dump_roster -- /tmp/roster
//!     cargo run -p rules-native --example dump_roster -- --fixture crates/game-core/tests/fixtures/roster.json
//!
//! * `--fixture <path>` writes the single-file test fixture
//!   (`{cards, personas, groups}`) that `Roster::from_fixture` loads. This is
//!   the checked-in gate artifact: `crates/rules-native/tests/roster_equiv.rs`
//!   asserts it is byte-equal to a fresh dump.
//! * A bare directory argument writes the three-file shape
//!   (`{cards,personas,groups}.json`) that `tools/roster-dump.mjs` caches
//!   under `target/roster-dump/` for `tools/roster.py`.
//! * No argument prints the three files' rows to stdout.
//!
//! Display-only columns that live outside the engine structs (`counterReason`,
//! card `rawName` / `cell`, persona `display` / `color` /
//! `variantTag` / `art` / `costume` / `cnId`, group `shortName` / `color` /
//! `logo`) stay in `display.json` and are **not**
//! printed here. Rule-body `text` IS printed: `card_text_mentions` searches it,
//! so it is part of `CardData` / `PersonaData` / `GroupData`.

use std::fmt::Write as _;
use std::path::PathBuf;

use game_core::data::{GroupData, CardData, PersonaData};

fn main() {
    let mut args = std::env::args().skip(1);
    let roster = rules_native::roster();
    match args.next().as_deref() {
        Some("--fixture") => {
            let path = args.next().expect("--fixture needs a path");
            let json = roster.to_fixture_json();
            if let Some(parent) = std::path::Path::new(&path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).unwrap();
                }
            }
            std::fs::write(&path, &json).unwrap();
            eprintln!(
                "wrote {path} ({} cards, {} personas, {} groups)",
                roster.cards.len(),
                roster.personas.len(),
                roster.groups.len()
            );
        }
        Some(dir) => {
            let dir = PathBuf::from(dir);
            let cards = cards_json(&roster.cards);
            let personas = personas_json(&roster.personas);
            let groups = groups_json(&roster.groups);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("cards.json"), &cards).unwrap();
            std::fs::write(dir.join("personas.json"), &personas).unwrap();
            std::fs::write(dir.join("groups.json"), &groups).unwrap();
            eprintln!(
                "wrote {}/{{cards,personas,groups}}.json ({} cards, {} personas, {} groups)",
                dir.display(),
                roster.cards.len(),
                roster.personas.len(),
                roster.groups.len()
            );
        }
        None => {
            print!("// ===== cards.json =====\n{}", cards_json(&roster.cards));
            print!("// ===== personas.json =====\n{}", personas_json(&roster.personas));
            print!("// ===== groups.json =====\n{}", groups_json(&roster.groups));
        }
    }
}

/// 4-space indent, like the hand-maintained `data/*.json` files.
fn indent(n: usize) -> String {
    " ".repeat(n * 4)
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn key_str(k: &str, v: &str, level: usize) -> String {
    format!("{}{}: {}", indent(level), esc(k), esc(v))
}

fn key_bool(k: &str, v: bool, level: usize) -> String {
    format!("{}{}: {}", indent(level), esc(k), v)
}

fn key_str_array(k: &str, vs: &[String], level: usize) -> String {
    if vs.is_empty() {
        return format!("{}{}: []", indent(level), esc(k));
    }
    let mut out = format!("{}{}: [\n", indent(level), esc(k));
    for (i, v) in vs.iter().enumerate() {
        out.push_str(&indent(level + 1));
        out.push_str(&esc(v));
        out.push_str(if i + 1 < vs.len() { ",\n" } else { "\n" });
    }
    out.push_str(&indent(level));
    out.push(']');
    out
}

/// `cards.json` row order: id, group, name, owner, derived, tags, text.
fn cards_json(cards: &[CardData]) -> String {
    let mut out = String::from("{\n    \"cards\": [\n");
    for (i, c) in cards.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let fields = [
            key_str("id", &c.id, 3),
            key_str("group", &c.group, 3),
            key_str("name", &c.name, 3),
            key_str("owner", &c.owner, 3),
            key_bool("derived", c.derived, 3),
            key_str_array("tags", &c.tags, 3),
            key_str("text", &c.text, 3),
        ];
        out.push_str(&fields.join(",\n"));
        out.push('\n');
        out.push_str(&indent(2));
        out.push('}');
        out.push_str(if i + 1 < cards.len() { ",\n" } else { "\n" });
    }
    out.push_str("    ]\n}\n");
    out
}

/// `personas.json` row order: name, [variantOf], group, skill, text,
/// exclusiveCards, [preset].
fn personas_json(chars: &[PersonaData]) -> String {
    let mut out = String::from("{\n    \"personas\": [\n");
    for (i, c) in chars.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let mut fields = vec![key_str("name", &c.name, 3)];
        if !c.variant_of.is_empty() {
            fields.push(key_str("variantOf", &c.variant_of, 3));
        }
        fields.push(key_str("group", &c.group, 3));
        fields.push(key_str("skill", &c.skill, 3));
        fields.push(key_str("text", &c.text, 3));
        fields.push(key_str_array("exclusiveCards", &c.exclusive_cards, 3));
        if !c.preset.is_empty() {
            fields.push(key_str_array("preset", &c.preset, 3));
        }
        out.push_str(&fields.join(",\n"));
        out.push('\n');
        out.push_str(&indent(2));
        out.push('}');
        out.push_str(if i + 1 < chars.len() { ",\n" } else { "\n" });
    }
    out.push_str("    ]\n}\n");
    out
}

/// `groups.json` row order: name, skill, text, [sharesExclusives].
fn groups_json(groups: &[GroupData]) -> String {
    let mut out = String::from("{\n    \"groups\": [\n");
    for (i, b) in groups.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let mut fields = vec![
            key_str("name", &b.name, 3),
            key_str("skill", &b.skill, 3),
            key_str("text", &b.text, 3),
        ];
        if b.shares_exclusives {
            fields.push(key_bool("sharesExclusives", true, 3));
        }
        out.push_str(&fields.join(",\n"));
        out.push('\n');
        out.push_str(&indent(2));
        out.push('}');
        out.push_str(if i + 1 < groups.len() { ",\n" } else { "\n" });
    }
    out.push_str("    ]\n}\n");
    out
}