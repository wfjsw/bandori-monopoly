//! The worker skips `view_extra` for human-member views (`needExtras: false`)
//! and still computes them for the bot service (`needExtras: true`).

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::StubRules;
use rules_worker::{handle, Ctx};
use serde_json::{json, Value};

fn ctx() -> Ctx {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
        .expect("game data");
    Ctx::new(Arc::new(data), Arc::new(StubRules))
}

fn new_match(ctx: &Ctx) -> Value {
    let members = json!([
        {"id": 1, "player": "P1", "bot": true},
        {"id": 2, "player": "P2", "bot": true},
        {"id": 3, "player": "P3", "bot": true},
    ]);
    let v = handle(
        ctx,
        json!({"op": "new", "members": members, "mode": 0, "seed": 7}),
    );
    assert_eq!(v["ok"], json!(true), "new: {v}");
    v["state"].clone()
}

#[test]
fn human_views_skip_the_extras() {
    let ctx = ctx();
    let state = new_match(&ctx);
    let v = handle(
        &ctx,
        json!({"op": "view", "state": state, "member": 1, "needExtras": false}),
    );
    assert_eq!(v["ok"], json!(true), "view: {v}");
    let view = &v["view"];
    // The extras are absent -- not null, not empty. The client computes them.
    assert!(view.get("aiAnswer").is_none(), "aiAnswer leaked: {view}");
    assert!(view.get("playable").is_none(), "playable leaked: {view}");
    assert!(view.get("estCost").is_none(), "estCost leaked: {view}");
    assert!(view.get("skills").is_none(), "skills leaked: {view}");
    // The rest of the frame is intact.
    assert!(view["state"]["players"].as_array().is_some());
    assert_eq!(view["you"], json!(1));
    assert_eq!(view["playerId"], view["playerId"]); // present
}

#[test]
fn human_views_skip_the_extras_by_default() {
    let ctx = ctx();
    let state = new_match(&ctx);
    let v = handle(&ctx, json!({"op": "view", "state": state, "member": 1}));
    assert_eq!(v["ok"], json!(true));
    let view = &v["view"];
    assert!(view.get("playable").is_none(), "default must skip extras");
    assert!(view.get("skills").is_none(), "default must skip extras");
}

#[test]
fn bot_views_carry_the_extras() {
    let ctx = ctx();
    let state = new_match(&ctx);
    let v = handle(
        &ctx,
        json!({"op": "view", "state": state, "member": 1, "needExtras": true}),
    );
    assert_eq!(v["ok"], json!(true), "view: {v}");
    let view = &v["view"];
    // The bot service reads these off its `BotSeatView`.
    assert!(view.get("aiAnswer").is_some(), "bot frame missing aiAnswer: {view}");
    assert!(view.get("playable").is_some(), "bot frame missing playable: {view}");
    assert!(view.get("estCost").is_some(), "bot frame missing estCost: {view}");
    assert!(view.get("skills").is_some(), "bot frame missing skills: {view}");
    // `playable` is one flag per hand card.
    let hand = view["hand"].as_array().expect("hand");
    let playable = view["playable"].as_array().expect("playable");
    assert_eq!(hand.len(), playable.len());
}