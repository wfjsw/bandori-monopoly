//! Validate a directory of freshly built card modules, copy each to
//! `<out>/<sha256>.wasm`, and write `<out>/index.json`.
//!
//!   cargo run -p game-rules --example ruleset_index -- <in_dir> <out_dir>
//!
//! The index lets the server and browser fetch exactly the modules a match needs,
//! cached by content hash.

use std::path::Path;

use game_rules::{CardInfo, Ruleset};
use serde::Serialize;

#[derive(Serialize)]
struct ModuleEntry {
    file: String,
    sha256: String,
    bytes: usize,
    source: String,
    cards: Vec<CardInfo>,
}

#[derive(Serialize)]
struct Index {
    abi: i32,
    ruleset_sha256: String,
    modules: Vec<ModuleEntry>,
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
    // Fails on duplicate card ids across modules.
    let set = builder.build().unwrap_or_else(|e| panic!("{e}"));
    modules.sort_by(|a, b| a.cards[0].id.cmp(&b.cards[0].id));

    let index = Index {
        abi: game_rules::ABI_VERSION,
        ruleset_sha256: set.sha256().to_string(),
        modules,
    };
    // Atomic: readers load index.json to learn which modules exist, so it must
    // never appear half-written while a concurrent build publishes.
    let tmp = out.join("index.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&index).unwrap()).expect("write index tmp");
    std::fs::rename(&tmp, out.join("index.json")).expect("rename index");
    let total: usize = index.modules.iter().map(|m| m.bytes).sum();
    println!(
        "{}: {} modules, {} cards, {total} bytes, ruleset {}",
        out.display(),
        index.modules.len(),
        set.cards().len(),
        &set.sha256()[..16]
    );
}
