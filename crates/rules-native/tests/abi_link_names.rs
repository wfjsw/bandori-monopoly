//! Guard: every `card-sdk` `mod sys` import must carry a linked-guest
//! `link_name` (native **and** the wasm32-linked monolith).
//!
//! On a pure wasm32 guest module the externs live in the `ruleset` import
//! module and the sandbox linker registers them by their bare names. On a
//! native build -- and on wasm32 with `card-sdk/native-rules` (the bot
//! monolith) -- there is no import module, so the bare names would collide
//! with libc (`log`, `draw`, `target`, ...) or fail to link. Each one is
//! renamed to `ruleset_<name>` via
//! `#[cfg_attr(any(not(target_arch = "wasm32"), feature = "native-rules"),
//! link_name = "ruleset_<name>")]`, and `game-rules/src/native_shims.rs`
//! defines that symbol (the crate every `card-sdk/guest` consumer links, so a
//! unified `cargo test` resolves them). On wasm32-linked, wasm-ld resolves the
//! plain symbol reference inside the one module.
//!
//! A new import added to `mod sys` without the attribute links as the bare
//! name and fails the native build with an opaque `LNK2019: unresolved
//! external symbol <name>` -- or, worse, silently binds to libc. This test
//! turns that into a clear failure at `cargo test -p rules-native` time.
//!
//! See docs/BOT.md §3.1 (B1) and §3.6 (the monolith).

use std::path::Path;

/// `card-sdk`'s `mod sys` (`rules/card-sdk/src/ctx/sys.rs`): the file is the
/// one extern block, so all of it is scanned.
fn sys_block() -> String {
    let sys = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/card-sdk/src/ctx/sys.rs");
    std::fs::read_to_string(&sys).unwrap_or_else(|e| panic!("read {}: {e}", sys.display()))
}

/// The cfg that turns on the `link_name = "ruleset_*"` rename: native, or
/// wasm32 with `native-rules` (the bot monolith). Must match `sys.rs`.
const LINKED_GUEST_CFG: &str = r#"any(not(target_arch = "wasm32"), feature = "native-rules")"#;

#[test]
fn every_sys_import_has_a_native_link_name() {
    let block = sys_block();
    let block = block.as_str();

    let mut missing: Vec<String> = vec![];
    let mut wrong_cfg: Vec<String> = vec![];
    let mut prev_has_attr = false;
    for line in block.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg_attr") && trimmed.contains("link_name") {
            // Must cover both the native and the wasm32-linked configs.
            if !trimmed.contains(LINKED_GUEST_CFG) {
                wrong_cfg.push(trimmed.to_string());
            }
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
        "rules/card-sdk/src/ctx/sys.rs imports missing \
         `#[cfg_attr({LINKED_GUEST_CFG}, link_name = \"ruleset_<name>\")]`: \
         {missing:?}.\n\
         Without it the native build (rules-native, docs/BOT.md B1) and the \
         wasm32-linked monolith (bot-glue, §3.6) link the bare name, which \
         collides with libc or fails as an unresolved external. \
         Add the attribute above each `pub fn` in the extern block; `ruleset_<name>` \
         must also exist in crates/game-rules/src/native_shims.rs."
    );
    assert!(
        wrong_cfg.is_empty(),
        "rules/card-sdk/src/ctx/sys.rs `link_name` attrs must gate on \
         `{LINKED_GUEST_CFG}` (native + wasm32-linked), not the old \
         `not(target_arch = \"wasm32\")` alone: {wrong_cfg:?}"
    );
}

#[test]
fn every_sys_import_has_a_native_symbol() {
    let syms = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/game-rules/src/native_shims.rs"),
    )
    .unwrap();

    let block = sys_block();
    let block = block.as_str();

    let mut missing: Vec<String> = vec![];
    for line in block.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let needle = format!("fn ruleset_{name}(");
            if !syms.contains(&needle) {
                missing.push(name);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "crates/game-rules/src/native_shims.rs has no `ruleset_<name>` shim for \
         these `mod sys` imports: {missing:?}.\n\
         Add a `#[no_mangle] pub unsafe extern \"C-unwind\" fn ruleset_<name>(...)` that \
         forwards to `game_rules::hostfns::<name>` (or traps with a clear message \
         until the host body lands). See docs/BOT.md §3.1 (B1)."
    );
}

/// The wasm32-linked monolith (`bot-glue` + `native-rules`) must not emit a
/// `ruleset` **import** module: the same `link_name` renames resolve to
/// `native_shims` inside the one module. This test pins the cfg seam that
/// makes that true (`sys.rs` drops `wasm_import_module` under `native-rules`).
#[test]
fn wasm32_linked_drops_the_ruleset_import_module() {
    let block = sys_block();
    assert!(
        block.contains(
            r#"all(target_arch = "wasm32", not(feature = "native-rules"))"#
        ),
        "rules/card-sdk/src/ctx/sys.rs must gate `link(wasm_import_module = \"ruleset\")` \
         on `all(target_arch = \"wasm32\", not(feature = \"native-rules\"))` so the \
         linked monolith has no `ruleset` import module (docs/BOT.md §3.6). \
         The block is:\n{block}"
    );
    assert!(
        !block.contains(
            r#"#[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "ruleset"))]"#
        ),
        "rules/card-sdk/src/ctx/sys.rs still uses the bare wasm32 import-module \
         attr -- that makes every wasm32 build (including the monolith) import \
         from `ruleset`."
    );
}