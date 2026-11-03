//! G06 §9.3 static sweep.
//!
//! The contracts a `Counteract` entry declares (`declares_negation` /
//! `arms_extreme`) and the join redirect a joiner's body must consult cannot be
//! checked by the compiler: the body is erased wasm and the manifest flag is a
//! separate literal. This test reads the card sources as text and pins the
//! three pairings the design requires:
//!
//! * a body that negates (`trigger::set_cancelled` / `negate_effect` / `spare`)
//!   declares [`AnswerEffect::NEGATES`](card_sdk::abi::AnswerEffect::NEGATES)
//!   (`.declares_negation()`), so `pending_negation` can see it;
//! * a body that arms an extreme (`ctx::set_extreme`) declares `ARMS_EXTREME`
//!   (`.arms_extreme()`), so a second copy does not re-pick;
//! * a body that targets its own seat (`ctx::target(player_id)` / a
//!   designation cancel) consults `join::opener()`, so a join redirects the
//!   hit to the opener (「改为你」).
//!
//! A card id is not needed: the walk is over the source files.

use std::path::{Path, PathBuf};

fn card_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cards");
    let mut out = Vec::new();
    for band in std::fs::read_dir(&root).expect("rules/cards") {
        let band = band.expect("band entry").path().join("src");
        let Ok(entries) = std::fs::read_dir(&band) else {
            continue;
        };
        for e in entries {
            let p = e.expect("source entry").path();
            if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The text of a top-level `fn name(` in `src`, through its closing `\n}`.
fn body_of(src: &str, name: &str) -> Option<String> {
    let head = format!("\nfn {name}(");
    let start = src.find(&head)? + 1;
    let rest = &src[start..];
    let end = rest.find("\n}\n").map(|i| i + 3).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// The last bare identifier of a call's argument list (the body name), when
/// it is a plain `fn` name preceded by a comma.
fn last_arg(call: &str) -> Option<String> {
    let close = call.rfind(')')?;
    let bytes = call.as_bytes();
    let mut end = close;
    // A multiline argument list often ends with a trailing comma and a newline
    // before the closing paren; both sit after the body name.
    for _ in 0..2 {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        if end > 0 && bytes[end - 1] == b',' {
            end -= 1;
        }
    }
    let mut start = end;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    if start == end || !call[..start].trim_end().ends_with(',') {
        return None;
    }
    Some(call[start..end].to_string())
}

/// Every `On::Counteract` entry in `src`: its argument-list text and body name.
fn entries(src: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while let Some(rel) = src[at..].find("On::Counteract(") {
        let start = at + rel;
        let rest = &src[start..];
        // Cut the call at its own matching closing paren so a trailing method
        // chain (`.counter_reason(..)`) does not lend its `)` to the last
        // argument.
        let mut depth = 0i32;
        let mut end = rest.len();
        for (i, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = last_arg(&rest[..end]);
        // The entry's text includes the method chain after the call
        // (`.declares_negation()`, `.arms_extreme()`), which ends at the next
        // entry or the enclosing array's close.
        let tail = rest[end..]
            .find("\n        On::")
            .or_else(|| rest[end..].find("\n    ],"))
            .map(|i| end + i)
            .unwrap_or(rest.len());
        out.push((rest[..tail].to_string(), body));
        at = start + "On::Counteract(".len();
    }
    out
}

#[test]
fn negation_bodies_declare_the_bit() {
    let mut bad = Vec::new();
    let mut seen = 0usize;
    for path in card_sources() {
        let src = std::fs::read_to_string(&path).expect("read card source");
        for (call, body) in entries(&src) {
            let Some(body) = body else { continue };
            let Some(text) = body_of(&src, &body) else {
                continue;
            };
            let negates = text.contains("set_cancelled(")
                || text.contains("negate_effect(")
                || text.contains("spare(");
            if negates {
                seen += 1;
            }
            if negates && !call.contains(".declares_negation()") {
                bad.push(format!("{}: {body}", path.display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "negating bodies without .declares_negation():\n{}",
        bad.join("\n")
    );
    assert!(seen >= 5, "the sweep found too few negating bodies: {seen}");
}

#[test]
fn extreme_bodies_declare_the_bit() {
    let mut bad = Vec::new();
    let mut seen = 0usize;
    for path in card_sources() {
        let src = std::fs::read_to_string(&path).expect("read card source");
        for (call, body) in entries(&src) {
            let Some(body) = body else { continue };
            let Some(text) = body_of(&src, &body) else {
                continue;
            };
            if text.contains("set_extreme(") {
                seen += 1;
            }
            if text.contains("set_extreme(") && !call.contains(".arms_extreme()") {
                bad.push(format!("{}: {body}", path.display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "extreme bodies without .arms_extreme():\n{}",
        bad.join("\n")
    );
    assert!(seen > 0, "the sweep found no extreme body");
}

#[test]
fn self_targeting_bodies_consult_the_join_opener() {
    let mut bad = Vec::new();
    let mut seen = 0usize;
    for path in card_sources() {
        let src = std::fs::read_to_string(&path).expect("read card source");
        for (_, body) in entries(&src) {
            let Some(body) = body else { continue };
            let Some(text) = body_of(&src, &body) else {
                continue;
            };
            let targets_self =
                text.contains("cancel_designation(") || text.contains("ctx::target(player_id)");
            if targets_self {
                seen += 1;
            }
            if targets_self && !text.contains("join::opener()") {
                bad.push(format!("{}: {body}", path.display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "self-targeting bodies without a join::opener() consult:\n{}",
        bad.join("\n")
    );
    assert!(seen > 0, "the sweep found no self-targeting body");
}
