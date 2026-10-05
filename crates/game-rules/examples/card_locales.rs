//! Bundle the card crates' locale files for the client.
//!
//!   cargo run -p game-rules --example card_locales -- <crates dir> <out dir>
//!
//! Every `<crates dir>/*/Cargo.toml` with a `locales/<lang>.json` contributes its
//! strings under its package name; the result is `<out dir>/locales/<lang>.json`:
//! `{"card-ag-yolo": {"boost": "..."}, ...}`, loaded by the web client as the
//! `cards` namespace (card keys are `cards:<package>.<key>`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package {
            if let Some(v) = line.strip_prefix("name").map(str::trim).and_then(|r| r.strip_prefix('=')) {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [crates, out] = args.as_slice() else { return Err("usage: card_locales <crates dir> <out dir>".into()) };
    let mut bundles: BTreeMap<String, BTreeMap<String, serde_json::Value>> = BTreeMap::new();
    let mut dirs: Vec<_> = fs::read_dir(crates)?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join("Cargo.toml").is_file()).collect();
    dirs.sort();
    for dir in dirs {
        let manifest = fs::read_to_string(dir.join("Cargo.toml"))?;
        let name = package_name(&manifest).ok_or_else(|| format!("{}: no package name", dir.display()))?;
        let Ok(files) = fs::read_dir(dir.join("locales")) else { continue };
        for f in files.filter_map(Result::ok).map(|e| e.path()) {
            if f.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let lang = f.file_stem().and_then(|s| s.to_str()).ok_or("bad locale file name")?.to_string();
            let strings: serde_json::Value = serde_json::from_str(&fs::read_to_string(&f)?).map_err(|e| format!("{}: {e}", f.display()))?;
            if !strings.is_object() {
                return Err(format!("{}: expected a JSON object", f.display()).into());
            }
            bundles.entry(lang).or_default().insert(name.clone(), strings);
        }
    }
    let out = Path::new(out).join("locales");
    fs::create_dir_all(&out)?;
    for (lang, bundle) in &bundles {
        fs::write(out.join(format!("{lang}.json")), serde_json::to_string_pretty(bundle)? + "\n")?;
    }
    println!("{}: {} locales ({})", out.display(), bundles.len(), bundles.keys().cloned().collect::<Vec<_>>().join(", "));
    Ok(())
}
