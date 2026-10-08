//! Guard: every `card-sdk` `mod sys` import must carry a native `link_name`.
//!
//! On wasm32 the externs live in the `bandori` import module and the sandbox
//! linker registers them by their bare names. On a native build there is no
//! import module, so the bare names would collide with libc (`log`, `draw`,
//! `target`, ...) -- each one is renamed to `bandori_<name>` via
//! `#[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_<name>")]`,
//! and `game-rules/src/native_shims.rs` defines that symbol (the crate every
//! `card-sdk/guest` consumer links, so a unified `cargo test` resolves them).
//!
//! A new import added to `mod sys` without the attribute links as the bare
//! name and fails the native build with an opaque `LNK2019: unresolved
//! external symbol <name>` -- or, worse, silently binds to libc. This test
//! turns that into a clear failure at `cargo test -p rules-native` time.
//!
//! See docs/BOT.md §3.1 (B1) and the coordination note in that section.

use std::path::Path;

#[test]
fn every_sys_import_has_a_native_link_name() {
    let ctx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/card-sdk/src/ctx.rs");
    let src = std::fs::read_to_string(&ctx).unwrap_or_else(|e| panic!("read {}: {e}", ctx.display()));

    let start = src
        .find("mod sys {")
        .unwrap_or_else(|| panic!("no `mod sys` block in {}", ctx.display()));
    let mut depth = 0usize;
    let mut end = start;
    for (k, ch) in src[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + k;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &src[start..end];

    let mut missing: Vec<String> = vec![];
    let mut prev_has_attr = false;
    for line in block.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg_attr") && trimmed.contains("link_name") {
            prev_has_attr = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !prev_has_attr {
                missing.push(name);
            }
            prev_has_attr = false;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            // keep the flag across blank/comment lines between attr and fn
            continue;
        }
        // any other line resets the flag
        if !trimmed.starts_with("#[") {
            prev_has_attr = false;
        }
    }

    assert!(
        missing.is_empty(),
        "rules/card-sdk/src/ctx.rs `mod sys` imports missing \
         `#[cfg_attr(not(target_arch = \"wasm32\"), link_name = \"bandori_<name>\")]`: \
         {missing:?}.\n\
         Without it the native build (rules-native, docs/BOT.md B1) links the bare \
         name, which collides with libc or fails as an unresolved external. \
         Add the attribute above each `pub fn` in the extern block; `bandori_<name>` \
         must also exist in crates/game-rules/src/native_shims.rs."
    );
}

#[test]
fn every_sys_import_has_a_native_symbol() {
    let ctx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/card-sdk/src/ctx.rs");
    let src = std::fs::read_to_string(&ctx).unwrap();
    let syms = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/game-rules/src/native_shims.rs"),
    )
    .unwrap();

    let start = src.find("mod sys {").unwrap();
    let mut depth = 0usize;
    let mut end = start;
    for (k, ch) in src[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + k;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &src[start..end];

    let mut missing: Vec<String> = vec![];
    for line in block.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let needle = format!("fn bandori_{name}(");
            if !syms.contains(&needle) {
                missing.push(name);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "crates/game-rules/src/native_shims.rs has no `bandori_<name>` shim for \
         these `mod sys` imports: {missing:?}.\n\
         Add a `#[no_mangle] pub unsafe extern \"C-unwind\" fn bandori_<name>(...)` that \
         forwards to `game_rules::hostfns::<name>` (or traps with a clear message \
         until the host body lands). See docs/BOT.md §3.1 (B1)."
    );
}