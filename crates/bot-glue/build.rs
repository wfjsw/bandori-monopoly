//! Bake the sandbox ruleset identity into the monolith.
//!
//! The card rules are linked as rlibs, so this bundle cannot recompute the
//! sandbox's `Ruleset::sha256()` (order-independent hash of the built module
//! file hashes -- `game-rules` `host.rs`, written into `index.json` as
//! `ruleset_sha256` by `tools/build-ruleset.mjs`). We therefore bake the same
//! value at build time: `tools/build-bot-glue.mjs` passes it, or we read it
//! from the just-built `index.json`. The worker compares it to the page's
//! `index.json` and refuses to answer on mismatch.

use std::fs;
use std::path::{Path, PathBuf};

fn read_index_sha(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    // Tiny scrape: `"ruleset_sha256": "<hex>"`. No serde in build-deps.
    let key = "\"ruleset_sha256\"";
    let i = text.find(key)?;
    let rest = &text[i + key.len()..];
    let rest = rest.trim_start().strip_prefix(':')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let sha = &rest[..end];
    if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(sha.to_ascii_lowercase())
    } else {
        None
    }
}

fn baked_sha() -> String {
    if let Ok(sha) = std::env::var("BOT_GLUE_RULESET_SHA256") {
        let sha = sha.trim();
        if sha.len() == 64 {
            return sha.to_ascii_lowercase();
        }
    }
    // CARGO_MANIFEST_DIR is crates/bot-glue.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for rel in ["dist/cards/index.json", "webui/public/assets/rules/index.json"] {
        if let Some(sha) = read_index_sha(&root.join(rel)) {
            return sha;
        }
    }
    String::new()
}

fn main() {
    println!("cargo:rerun-if-env-changed=BOT_GLUE_RULESET_SHA256");
    println!("cargo:rerun-if-changed=../../dist/cards/index.json");
    println!("cargo:rerun-if-changed=../../webui/public/assets/rules/index.json");
    let sha = baked_sha();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ruleset_identity.rs");
    fs::write(
        &out,
        format!(
            "/// Sandbox `Ruleset::sha256()` of the cards this monolith was built\n\
             /// against (`index.json`'s `ruleset_sha256`). Empty when the build ran\n\
             /// without a built ruleset -- the worker then refuses to answer.\n\
             pub const RULESET_SHA256: &str = \"{sha}\";\n"
        ),
    )
    .unwrap();
}