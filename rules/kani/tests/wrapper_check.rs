//! Guard: every `On::*` body ident must have a `NAME_kani` wrapper in the
//! same module. The body-dispatch generator (`build.rs` `parse_on_bodies`)
//! emits `{module}::{body}_kani(player)` for each entry; a missing wrapper
//! is a compile error under `cfg(kani)` that `cargo build` (no cfg) misses.
//!
//! This test walks the same source trees `build.rs` scans and shares the
//! Phase 3 DSL parser (`on_parse.rs`). It is intentionally independent of the
//! build script's codegen so a parser regression in either is caught.

#[path = "../on_parse.rs"]
mod on_parse;
use on_parse::*;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn kind_shapes() -> HashMap<String, BodyShape> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rules = manifest.parent().expect("rules/");
    let mut sources = Vec::new();
    for rel in ["card-sdk/src/kinds.rs", "bandori-std/src/kind.rs"] {
        let p = rules.join(rel);
        sources.push(fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}")));
    }
    kind_shapes_from(&sources)
}

/// Expected `NAME_kani` signature fragment for a shape (param list + return).
fn expected_sig(shape: BodyShape) -> &'static str {
    match shape {
        BodyShape::Action => "(player_id: i32) -> card_sdk::Asked",
        BodyShape::Value => "(player_id: i32, value: i32) -> i32",
        BodyShape::Flag => "(player_id: i32, value: i32) -> bool",
        BodyShape::Gate => "(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue",
        BodyShape::Plan => "(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue",
        BodyShape::Roll => "(player_id: i32, v: card_sdk::RollValue) -> card_sdk::RollValue",
        BodyShape::Deal => "(player_id: i32, v: card_sdk::DealValue) -> card_sdk::DealValue",
        BodyShape::PayAdd => "(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue",
        BodyShape::Redirect => {
            "(player_id: i32, v: card_sdk::RedirectValue) -> card_sdk::RedirectValue"
        }
    }
}

/// Every `On::*` body ident in `file` must have a `pub fn {body}_kani` in the
/// same file with the shape's signature. Returns the problems.
fn missing_wrappers(file: &Path, shapes: &HashMap<String, BodyShape>) -> Vec<String> {
    let src = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut missing = Vec::new();
    for on in parse_on_bodies(&src, shapes) {
        let wrapper = format!("{}_kani", on.body);
        let head = format!("pub fn {wrapper}");
        let Some(at) = src.find(&head) else {
            missing.push(format!("{wrapper} (missing)"));
            continue;
        };
        // Signature check: the next ~120 chars must carry the shape's param list.
        let window = &src[at..src.len().min(at + 160)];
        if !window.contains(expected_sig(on.shape)) {
            missing.push(format!(
                "{wrapper} (wrong shape {:?}: expected `{}`)",
                on.shape,
                expected_sig(on.shape)
            ));
        }
    }
    missing
}

/// Walk `rules/{cards,skills,events,tiles,markers,status}` and collect
/// every file with a missing wrapper.
fn scan_rules_tree(root: &Path, shapes: &HashMap<String, BodyShape>) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for dir in [
        "cards",
        "skills",
        "events",
        "tiles",
        "markers",
        "status",
    ] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        walk(&base, shapes, &mut out);
    }
    out
}

fn walk(dir: &Path, shapes: &HashMap<String, BodyShape>, out: &mut Vec<(String, Vec<String>)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, shapes, out);
        } else if path.extension().map_or(false, |e| e == "rs") {
            let missing = missing_wrappers(&path, shapes);
            if !missing.is_empty() {
                out.push((path.display().to_string(), missing));
            }
        }
    }
}

#[test]
fn every_on_body_has_a_kani_wrapper() {
    // `CARGO_MANIFEST_DIR` is `rules/kani`; the source trees are siblings.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let rules_root = Path::new(&manifest).parent().expect("rules/");
    let shapes = kind_shapes();
    let problems = scan_rules_tree(rules_root, &shapes);
    for (file, missing) in &problems {
        eprintln!("MISSING wrappers in {file}: {missing:?}");
    }
    assert!(
        problems.is_empty(),
        "{} file(s) have On::* bodies without NAME_kani wrappers (build.rs will emit them into body_dispatch_fn.rs and cfg(kani) compiles will fail)",
        problems.len()
    );
}