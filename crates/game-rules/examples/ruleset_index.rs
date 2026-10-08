//! Validate a directory of freshly built card modules, copy each to
//! `<out>/<sha256>.wasm`, and write `<out>/index.json`.
//!
//!   cargo run -p game-rules --example ruleset_index -- <in_dir> <out_dir>
//!
//! The index lets the server and browser fetch exactly the modules a match needs,
//! cached by content hash.
//!
//! Guard **conditions** (`pre`, docs/GUARDS.md §8.2) are compiled here on the
//! host and shipped as one lean postcard blob, `conds-<sha256>.bin`, named in
//! the index. The browser glue is `runtime-only` (no CEL parser) and loads that
//! blob via `RulesetBuilder::precompiled`; a compile error fails this build
//! (fail-closed, same as `RulesetBuilder::build`).

use std::path::Path;

use game_rules::{CardInfo, PrecompiledConds, Ruleset};
use serde::Serialize;

#[derive(Serialize)]
struct ModuleEntry {
    file: String,
    sha256: String,
    bytes: usize,
    source: String,
    cards: Vec<CardInfo>,
}

/// The precompiled-condition blob, when the set declares any `pre`
/// (`conds-<sha256>.bin` beside this index). Absent means no entry has a
/// condition and every older loader keeps working unchanged.
#[derive(Serialize)]
struct CondsEntry {
    file: String,
    sha256: String,
    bytes: usize,
    entries: usize,
}

#[derive(Serialize)]
struct Index {
    abi: i32,
    ruleset_sha256: String,
    modules: Vec<ModuleEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conds: Option<CondsEntry>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, out) = match args.as_slice() {
        [_, i, o] => (Path::new(i), Path::new(o)),
        _ => {
            eprintln!("usage: ruleset_index <in_dir> <out_dir>");
            std::process::exit(2);
        }
    };
    std::fs::create_dir_all(out).expect("create out dir");

    let mut files: Vec<_> = std::fs::read_dir(input)
        .expect("read in dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wasm"))
        .collect();
    files.sort();

    let mut builder = Ruleset::builder();
    let mut modules = vec![];
    for path in &files {
        let bytes = std::fs::read(path).expect("read module");
        let one = Ruleset::load(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let sha = builder
            .add(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let file = format!("{sha}.wasm");
        std::fs::write(out.join(&file), &bytes).expect("write module");
        modules.push(ModuleEntry {
            file,
            sha256: sha,
            bytes: bytes.len(),
            source: path.file_stem().unwrap().to_string_lossy().into_owned(),
            cards: one.cards().to_vec(),
        });
    }
    // Fails on duplicate card ids across modules, and on any `pre` that does
    // not compile (docs/GUARDS.md §5.3 -- fail-closed).
    let set = builder.build().unwrap_or_else(|e| panic!("{e}"));
    modules.sort_by(|a, b| a.cards[0].id.cmp(&b.cards[0].id));

    // The lean compiled form of every guard condition, content-addressed. The
    // browser cannot compile these (no parser), so this file is what it loads.
    let conds: PrecompiledConds = set.precompiled_conds();
    let conds = if conds.entries.is_empty() {
        None
    } else {
        let bytes = conds.to_bytes();
        let sha = conds.sha256();
        let file = format!("conds-{sha}.bin");
        std::fs::write(out.join(&file), &bytes).expect("write conds blob");
        Some(CondsEntry {
            file,
            sha256: sha,
            bytes: bytes.len(),
            entries: conds.entries.len(),
        })
    };

    let index = Index {
        abi: game_rules::ABI_VERSION,
        ruleset_sha256: set.sha256().to_string(),
        modules,
        conds,
    };
    // Atomic: readers load index.json to learn which modules exist, so it must
    // never appear half-written while a concurrent build publishes.
    let tmp = out.join("index.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&index).unwrap()).expect("write index tmp");
    std::fs::rename(&tmp, out.join("index.json")).expect("rename index");
    let total: usize = index.modules.iter().map(|m| m.bytes).sum();
    let conds_note = match &index.conds {
        Some(c) => format!(", {} conds ({} B)", c.entries, c.bytes),
        None => String::new(),
    };
    println!(
        "{}: {} modules, {} cards, {total} bytes{conds_note}, ruleset {}",
        out.display(),
        index.modules.len(),
        set.cards().len(),
        &set.sha256()[..16]
    );
}