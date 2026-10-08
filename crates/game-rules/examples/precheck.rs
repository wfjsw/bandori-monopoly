//! G3 `precheck` lint (docs/GUARDS.md §5.2).
//!
//!   cargo run -p game-rules --example precheck -- [rules_src_dir] [cards_dir]
//!
//! Loads the ruleset and reports, per guarded entry, the condition / guard
//! split. Flags guards that still reject on the trigger **kind** -- the
//! category filter owns that (GUARDS.md "distinct layers" ruling). Also
//! reports parse errors / unknown vars / float literals (already fail-closed
//! at `RulesetBuilder::build`, so a bad `pre` is a hard error here too).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use card_sdk::abi::OnKind;
use game_rules::Ruleset;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("rules"));
    let cards_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("dist/cards"));

    // ---- 1. load the ruleset (build fails closed on a bad `pre`) ----
    let mut b = Ruleset::builder();
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(cards_dir.join("index.json"))
            .unwrap_or_else(|e| panic!("{}: {e}", cards_dir.join("index.json").display())),
    )
    .expect("index.json");
    for m in index["modules"].as_array().expect("modules") {
        let f = cards_dir.join(m["file"].as_str().expect("file"));
        let bytes = std::fs::read(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        b.add(&bytes).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
    }
    let rules = b.build().expect("ruleset build (a bad `pre` is an error)");
    let cards = rules.cards();

    // ---- 2. per-entry condition / guard split ----
    let mut with_pre = 0usize;
    let mut with_guard = 0usize;
    let mut deleted_guard = 0usize;
    let mut with_legacy = 0usize;
    let mut entries = 0usize;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for c in cards {
        for o in &c.on {
            let name = OnKind::from_i32(o.kind)
                .map(|k| format!("{k:?}"))
                .unwrap_or_else(|| format!("?{}", o.kind));
            // Only the guarded entries matter for the split.
            if !matches!(
                OnKind::from_i32(o.kind),
                Some(OnKind::Play) | Some(OnKind::Counteract) | Some(OnKind::Hook)
            ) {
                continue;
            }
            entries += 1;
            *kinds.entry(name.clone()).or_default() += 1;
            let pre = o.pre.as_deref().unwrap_or("");
            if !pre.is_empty() {
                with_pre += 1;
            }
            if o.has_guard {
                with_guard += 1;
            } else {
                deleted_guard += 1;
            }
            if o.has_legacy {
                with_legacy += 1;
            }
            if std::env::var("PRECHECK_VERBOSE").is_ok() {
                println!(
                    "{:40} {name:10} triggers={:?} pre={:?} guard={} legacy={}",
                    c.id, o.triggers, pre, o.has_guard, o.has_legacy
                );
            }
        }
    }
    println!("=== precheck: condition / guard split ===");
    println!("guarded entries:      {entries}");
    println!("  with condition:     {with_pre}");
    println!("  with residual guard:{with_guard}");
    println!("  guard deleted:      {deleted_guard}");
    println!("  legacy audit copy:  {with_legacy}");
    for (k, n) in &kinds {
        println!("  {k:12} {n}");
    }

    // ---- 3. flag guards that still reject on the kind ----
    println!("\n=== precheck: kind re-checks in guard fns (category owns the kind) ===");
    let mut flagged = 0usize;
    for p in walk(&src_dir) {
        if p.components().any(|c| c.as_os_str() == "card-sdk") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        // `legacy_*` copies keep the pre-migration kind checks on purpose
        // (GUARDS.md §5.1) -- they are the audit oracle, not live guards.
        let text = filter_legacy(&text);
        for (line, msg) in kind_rejects(&text) {
            println!("  {}:{line}: {msg}", p.display());
            flagged += 1;
        }
    }
    if flagged == 0 {
        println!("  (none)");
    } else {
        println!("  {flagged} kind re-check(s) -- move them into the category list or delete");
    }

    // ---- 4. summary ----
    println!("\n=== precheck: done ===");
    if flagged > 0 {
        std::process::exit(1);
    }
}

/// Blank out `fn legacy_*` bodies so the kind-recheck scan skips the G3 audit
/// copies (they intentionally keep the pre-migration checks).
fn filter_legacy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_legacy = false;
    let mut depth = 0i32;
    for line in text.lines() {
        let t = line.trim_start();
        if !in_legacy && (t.starts_with("fn legacy_") || t.starts_with("pub fn legacy_")) {
            in_legacy = true;
            depth = 0;
        }
        if in_legacy {
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            out.push('\n');
            if depth <= 0 && line.contains('}') {
                in_legacy = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Lines where a guard body rejects on the trigger kind: a `trigger::kind()`
/// / `TriggerKind::` compare that leads to `return false`. The category filter
/// (the `On::*` kind list) already owns this (GUARDS.md "distinct layers").
fn kind_rejects(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        let is_kind = t.contains("trigger::kind()")
            || t.contains("TriggerKind::")
            || t.contains("trigger.kind")
            || t.contains("t.kind ==");
        if !is_kind {
            continue;
        }
        // A kind compare that rejects: `==` with `return false`, or `!=` as a
        // whole-expression guard. `matches!` / `contains` over a kind list is
        // a multi-kind arm (authoring-legal) and is not flagged.
        let rejects = (t.contains("==") && (t.contains("return false") || t.ends_with("false")))
            || (t.contains("!=") && !t.contains("&&") && !t.contains("||"));
        let kind_filter = t.contains("matches!") || t.contains(".contains(");
        if rejects && !kind_filter {
            out.push((
                i + 1,
                format!("kind re-check (category owns this): {}", t.trim()),
            ));
        }
    }
    out
}