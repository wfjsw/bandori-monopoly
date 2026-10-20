//! Guard: every `On::*` body ident must have a `NAME_kani` wrapper in the
//! same module. The body-dispatch generator (`build.rs` `parse_on_bodies`)
//! emits `{module}::{body}_kani(player)` for each entry; a missing wrapper
//! is a compile error under `cfg(kani)` that `cargo build` (no cfg) misses.
//!
//! This test walks the same source trees `build.rs` scans and mirrors its
//! trailing-comma-tolerant parser. It is intentionally independent of the
//! build script so a parser regression in either is caught.

use std::fs;
use std::path::Path;

/// Extract the body ident from one `On::*` constructor (same algorithm as
/// `build.rs::parse_on_bodies`, including the trailing-comma fix).
/// Strip `//` line comments and `/* */` block comments (so `On::` in doc
/// comments is not mistaken for a constructor).
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn bodies_in(src: &str) -> Vec<String> {
    let src = strip_comments(src);
    let src = src.as_str();
    let mut bodies = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = src[search_from..].find("On::") {
        let start = search_from + rel;
        let after = start + 4;
        let mut name_end = after;
        while name_end < src.len()
            && (src.as_bytes()[name_end].is_ascii_alphanumeric() || src.as_bytes()[name_end] == b'_')
        {
            name_end += 1;
        }
        let _variant = &src[after..name_end];
        // Skip whitespace to '('
        let mut p = name_end;
        while p < src.len() && src.as_bytes()[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= src.len() || src.as_bytes()[p] != b'(' {
            search_from = name_end;
            continue;
        }
        // Balanced parens
        let mut depth = 0usize;
        let mut j = p;
        while j < src.len() {
            let c = src.as_bytes()[j];
            if c == b'(' {
                depth += 1;
            } else if c == b')' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        let inner = &src[p + 1..j];
        // Split top-level commas; last non-empty part is the body ident.
        let mut parts: Vec<&str> = Vec::new();
        let mut depth2 = 0i32;
        let mut cur = 0usize;
        for (k, c) in inner.char_indices() {
            match c {
                '(' | '[' | '{' => depth2 += 1,
                ')' | ']' | '}' => depth2 -= 1,
                ',' if depth2 == 0 => {
                    parts.push(inner[cur..k].trim());
                    cur = k + 1;
                }
                _ => {}
            }
        }
        parts.push(inner[cur..].trim());
        if let Some(last) = parts.iter().rev().find(|p| !p.trim().is_empty()) {
            if let Some(body) = body_ident(last.trim()) {
                bodies.push(body);
            }
        }
        search_from = j + 1;
    }
    bodies
}

/// Last-arg body: a bare ident, or `FilterBody::Arm(ident)` (path prefix ok).
fn body_ident(last: &str) -> Option<String> {
    let is_ident = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_') == Some(true)
    };
    if is_ident(last) {
        return Some(last.to_string());
    }
    if last.ends_with(')') {
        if let Some(open) = last.rfind('(') {
            let inner = last[open + 1..last.len() - 1].trim();
            if is_ident(inner) {
                return Some(inner.to_string());
            }
        }
    }
    None
}

/// Every `On::*` body ident in `file` must have a `pub fn {body}_kani(` in
/// the same file. Returns the missing wrapper names.
fn missing_wrappers(file: &Path) -> Vec<String> {
    let src = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut missing = Vec::new();
    for body in bodies_in(&src) {
        let wrapper = format!("{body}_kani");
        if !src.contains(&format!("pub fn {wrapper}(")) {
            missing.push(wrapper);
        }
    }
    missing
}

/// Walk `rules/{cards,skills,events,tiles,markers,status}` and collect
/// every file with a missing wrapper.
fn scan_rules_tree(root: &Path) -> Vec<(String, Vec<String>)> {
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
        walk(&base, &mut out);
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<(String, Vec<String>)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().map_or(false, |e| e == "rs") {
            let missing = missing_wrappers(&path);
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
    let problems = scan_rules_tree(rules_root);
    for (file, missing) in &problems {
        eprintln!("MISSING wrappers in {file}: {missing:?}");
    }
    assert!(
        problems.is_empty(),
        "{} file(s) have On::* bodies without NAME_kani wrappers (build.rs will emit them into body_dispatch_fn.rs and cfg(kani) compiles will fail)",
        problems.len()
    );
}
