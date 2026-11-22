//! The hash-only state protocol: a worker replays a request against the last
//! blob it was sent when the caller names it by `(fnv1a64, len)`. A hit is
//! equivalent to sending the body; a miss errors so the caller retries.

use std::sync::Arc;

use game_core::data::{GameData, Roster};
use game_core::engine::StubRules;
use game_core::record::fnv1a64;
use rules_worker::{handle, Ctx};
use serde_json::{json, Value};

fn ctx() -> Ctx {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../game-core/tests/fixtures/roster.json"),
        )
        .expect("roster fixture"),
    )
    .expect("roster fixture parses");
    let data = GameData::load(
        |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
        roster,
    )
    .expect("game data");
    Ctx::new(Arc::new(data), Arc::new(StubRules))
}

fn new_match(ctx: &Ctx, seed: u64) -> String {
    let members = json!([
        {"id": 1, "player": "P1", "bot": true},
        {"id": 2, "player": "P2", "bot": true},
        {"id": 3, "player": "P3", "bot": true},
    ]);
    let v = handle(ctx, json!({"op": "new", "members": members, "mode": 0, "seed": seed}));
    assert_eq!(v["ok"], json!(true), "new: {v}");
    v["state"].as_str().expect("state string").to_string()
}

fn pair(state: &str) -> (u64, u64) {
    (fnv1a64(state.as_bytes()), state.len() as u64)
}

fn view(ctx: &Ctx, state: &str, by_hash_only: bool) -> Value {
    let (h, l) = pair(state);
    let mut req = json!({
        "op": "view",
        "member": 1,
        "needExtras": true,
        "stateHash": h,
        "stateLen": l,
    });
    if !by_hash_only {
        req["state"] = json!(state);
    }
    handle(ctx, req)
}

#[test]
fn a_named_state_is_replayed_without_the_body() {
    let ctx = ctx();
    let state = new_match(&ctx, 7);
    // First call carries the body, which also seeds the cache.
    let full = view(&ctx, &state, false);
    assert_eq!(full["ok"], json!(true), "full: {full}");
    // Second call names the same blob: same frame, no body.
    let lean = view(&ctx, &state, true);
    assert_eq!(lean["ok"], json!(true), "lean: {lean}");
    assert_eq!(
        lean["view"]["state"]["players"], full["view"]["state"]["players"],
        "hash-only frame differs"
    );
    assert_eq!(lean["view"]["state"]["turn"], full["view"]["state"]["turn"]);
}

#[test]
fn an_unknown_or_mismatched_hash_is_a_miss() {
    let ctx = ctx();
    let state = new_match(&ctx, 7);
    let _ = view(&ctx, &state, false);

    // Unknown hash, no body.
    let miss = handle(
        &ctx,
        json!({"op": "view", "member": 1, "stateHash": 1u64, "stateLen": 1u64}),
    );
    assert_eq!(miss["ok"], json!(false));
    assert_eq!(miss["error"], json!("state cache miss"));

    // A body that does not match the pair it came with.
    let (h, l) = pair(&state);
    let bad = handle(
        &ctx,
        json!({"op": "view", "member": 1, "state": state, "stateHash": h, "stateLen": l + 1}),
    );
    assert_eq!(bad["ok"], json!(false));
    assert_eq!(bad["error"], json!("state hash mismatch"));
}

#[test]
fn a_newer_state_replaces_the_cached_one() {
    let ctx = ctx();
    let first = new_match(&ctx, 7);
    let second = new_match(&ctx, 8);
    assert_ne!(first, second);
    // Caching `second` must make the older hash miss again.
    let _ = view(&ctx, &second, false);
    let stale = view(&ctx, &first, true);
    assert_eq!(stale["ok"], json!(false), "stale hash hit");
}

#[test]
fn stateless_ops_still_work() {
    let ctx = ctx();
    let v = handle(&ctx, json!({"op": "info"}));
    assert_eq!(v["ok"], json!(true), "info: {v}");
}
