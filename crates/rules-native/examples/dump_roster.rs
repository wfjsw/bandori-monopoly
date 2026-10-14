//! Dump the roster metadata as `data/{cards,characters,bands}.json`-shaped JSON.
//!
//!     cargo run -p rules-native --example dump_roster
//!     cargo run -p rules-native --example dump_roster -- /tmp/roster
//!
//! Prints (or writes into a directory) the roster the ruleset defines in Rust
//! (ABI v63 `bandori_metadata`), in the exact JSON shape / key order /
//! formatting of today's `data/*.json` so tools (asset-pipe check, rulebook
//! cite/check, Kani generator) can read it later.
//!
//! Display-only columns that live outside the engine structs (`simple`,
//! `counterReason`) stay in `data/*.json` and are **not** printed here -- they
//! are not part of `CardData` / `CharacterData` / `BandData`.

use std::fmt::Write as _;
use std::path::PathBuf;

use game_core::data::{BandData, CardData, CharacterData};

fn main() {
    let out_dir = std::env::args().nth(1).map(PathBuf::from);
    let roster = rules_native::roster();
    let cards = cards_json(&roster.cards);
    let characters = characters_json(&roster.characters);
    let bands = bands_json(&roster.bands);
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cards.json"), &cards).unwrap();
        std::fs::write(dir.join("characters.json"), &characters).unwrap();
        std::fs::write(dir.join("bands.json"), &bands).unwrap();
        eprintln!(
            "wrote {}/{{cards,characters,bands}}.json ({} cards, {} characters, {} bands)",
            dir.display(),
            roster.cards.len(),
            roster.characters.len(),
            roster.bands.len()
        );
    } else {
        print!("// ===== cards.json =====\n{cards}");
        print!("// ===== characters.json =====\n{characters}");
        print!("// ===== bands.json =====\n{bands}");
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

/// `cards.json` row order: id, band, rawName, name, owner, derived, tags, text, cell.
fn cards_json(cards: &[CardData]) -> String {
    let mut out = String::from("{\n    \"cards\": [\n");
    for (i, c) in cards.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let fields = [
            key_str("id", &c.id, 3),
            key_str("band", &c.band, 3),
            key_str("rawName", &c.raw_name, 3),
            key_str("name", &c.name, 3),
            key_str("owner", &c.owner, 3),
            key_bool("derived", c.derived, 3),
            key_str_array("tags", &c.tags, 3),
            key_str("text", &c.text, 3),
            key_str("cell", &c.cell, 3),
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

/// `characters.json` row order: name, display, [variantOf], [variantTag], band,
/// color, skill, text, cnId, [art], [costume], exclusiveCards, [preset].
fn characters_json(chars: &[CharacterData]) -> String {
    let mut out = String::from("{\n    \"characters\": [\n");
    for (i, c) in chars.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let mut fields = vec![
            key_str("name", &c.name, 3),
            key_str("display", &c.display, 3),
        ];
        if !c.variant_of.is_empty() {
            fields.push(key_str("variantOf", &c.variant_of, 3));
        }
        if !c.variant_tag.is_empty() {
            fields.push(key_str("variantTag", &c.variant_tag, 3));
        }
        fields.push(key_str("band", &c.band, 3));
        fields.push(key_str("color", &c.color, 3));
        fields.push(key_str("skill", &c.skill, 3));
        fields.push(key_str("text", &c.text, 3));
        fields.push(key_str("cnId", &c.cn_id, 3));
        if !c.art.is_empty() {
            fields.push(key_str("art", &c.art, 3));
        }
        if !c.costume.is_empty() {
            fields.push(key_str("costume", &c.costume, 3));
        }
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

/// `bands.json` row order: name, shortName, [sharesExclusives], [bootArt],
/// color, skill, text, logo.
fn bands_json(bands: &[BandData]) -> String {
    let mut out = String::from("{\n    \"bands\": [\n");
    for (i, b) in bands.iter().enumerate() {
        out.push_str(&indent(2));
        out.push_str("{\n");
        let mut fields = vec![
            key_str("name", &b.name, 3),
            key_str("shortName", &b.short_name, 3),
        ];
        if b.shares_exclusives {
            fields.push(key_bool("sharesExclusives", true, 3));
        }
        if !b.boot_art.is_empty() {
            fields.push(key_str("bootArt", &b.boot_art, 3));
        }
        fields.push(key_str("color", &b.color, 3));
        fields.push(key_str("skill", &b.skill, 3));
        fields.push(key_str("text", &b.text, 3));
        fields.push(key_str("logo", &b.logo, 3));
        out.push_str(&fields.join(",\n"));
        out.push('\n');
        out.push_str(&indent(2));
        out.push('}');
        out.push_str(if i + 1 < bands.len() { ",\n" } else { "\n" });
    }
    out.push_str("    ]\n}\n");
    out
}