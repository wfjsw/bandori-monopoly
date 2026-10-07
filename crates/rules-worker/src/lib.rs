//! The rules worker: one operation on one match, given the whole match.
//!
//! This is the library form -- [`handle`] is the whole protocol, so both the
//! `rules-worker` binary (stdio) and the server's in-process backend drive the
//! same code. Nothing is retained between calls; see the binary's docs for the
//! request shape.

use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use game_rules::WasmRules;
use serde_json::{json, Value};
use std::sync::Arc;

/// Immutable content loaded once: game tables and card modules. Read-only
/// across requests -- this is the crate's only cross-request data, and it is
/// never written.
pub struct Ctx {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
}

impl Ctx {
    /// Wrap already-loaded tables. Used by the server's in-process backend.
    pub fn new(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Self {
        Self { data, rules }
    }

    /// Load the game tables and card modules once. Read-only from here on.
    pub fn load(data_dir: &std::path::Path, rules_dir: &std::path::Path) -> Self {
        let data = Arc::new(
            GameData::load(|f| {
                std::fs::read_to_string(data_dir.join(f)).map_err(|e| e.to_string())
            })
            .unwrap_or_else(|e| panic!("cannot load game data from {}: {e}", data_dir.display())),
        );
        let rules: Arc<dyn CardRules> = match WasmRules::load_dir(data.clone(), rules_dir) {
            Ok(Some(r)) => {
                eprintln!(
                    "card modules: {} loaded from {}",
                    r.ruleset().module_count(),
                    rules_dir.display()
                );
                Arc::new(r)
            }
            Ok(None) => {
                eprintln!(
                    "no card modules in {} -- cards have no effect",
                    rules_dir.display()
                );
                Arc::new(StubRules)
            }
            Err(e) => {
                eprintln!("card modules failed to load: {e:?} -- falling back to no effects");
                Arc::new(StubRules)
            }
        };
        Self { data, rules }
    }
}

/// One request. `req` is trusted only to be JSON -- every field is validated
/// here and a problem becomes an error response, never a panic.
pub fn handle(ctx: &Ctx, req: Value) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let reply = match run(ctx, &req) {
        Ok(v) => v,
        Err(e) => json!({"ok": false, "error": e}),
    };
    // Echo the caller's id so responses can be matched to requests out of order.
    let mut obj = reply.as_object().cloned().unwrap_or_default();
    obj.insert("id".into(), id);
    Value::Object(obj)
}

fn run(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let op = req.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        // Liveness / observability. No match state is involved.
        "ping" => Ok(json!({"ok": true})),

        "new" => {
            let members: Vec<RoomMember> = parse(req, "members")?;
            let seed = req.get("seed").and_then(Value::as_u64).unwrap_or(1);
            let mode = req.get("mode").and_then(Value::as_i64).unwrap_or(0) as i32;
            let weights: ScoreWeights = match req.get("weights") {
                None | Some(Value::Null) => ScoreWeights::default(),
                Some(w) => {
                    serde_json::from_value(w.clone()).map_err(|e| format!("weights: {e}"))?
                }
            };
            let m = Match::new(
                ctx.data.clone(),
                ctx.rules.clone(),
                &members,
                seed,
                MatchMode::from_i32(mode).unwrap_or_default(),
                weights,
            );
            Ok(json!({"ok": true, "state": m.save()}))
        }

        // Mutating ops: restore -> op -> save. The caller gets the new blob and
        // is responsible for keeping it; the worker forgets it here.
        "act" | "tick" | "quick_start" | "finish" => {
            let mut m = restore(ctx, req)?;
            let out = match op {
                "act" => {
                    let member = req.get("member").and_then(Value::as_i64).unwrap_or(0) as i32;
                    let cmd: NetMessage = parse(req, "cmd")?;
                    // "" on success, else the `Msg` JSON the client renders.
                    match m.act(member, &cmd) {
                        Ok(()) => Value::Null,
                        Err(e) => serde_json::to_value(&e).unwrap_or(Value::Null),
                    }
                }
                "tick" => {
                    let dt = req.get("dt").and_then(Value::as_f64).unwrap_or(0.05) as f32;
                    m.tick(dt);
                    Value::Null
                }
                "quick_start" => {
                    m.quick_start();
                    Value::Null
                }
                _ => {
                    m.finish();
                    Value::Null
                }
            };
            let mut v = json!({"ok": true, "state": m.save(), "changed": true});
            if !out.is_null() {
                v["error"] = out;
            }
            Ok(v)
        }

        // Mutating: a member dropped or returned mid-match.
        "member_left" | "member_back" => {
            let mut m = restore(ctx, req)?;
            let member = req.get("member").and_then(Value::as_i64).unwrap_or(0) as i32;
            if op == "member_left" {
                let can_return = req
                    .get("canReturn")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                m.member_left(member, can_return);
            } else {
                m.member_back(member);
            }
            Ok(json!({"ok": true, "state": m.save(), "changed": true}))
        }

        // Read-only ops: the blob comes back unchanged, so it is not echoed.
        "view" => {
            let m = restore(ctx, req)?;
            let member = req.get("member").and_then(Value::as_i64).unwrap_or(0) as i32;
            let state = m.state();
            let player_id = state.player_of(member);
            let extra = m.view_extra(member);
            Ok(json!({
                "ok": true,
                "view": {
                    "state": state,
                    "hand": m.hand_of(member),
                    "handNotes": m.hand_notes_of(member),
                    "draw": m.draw_of(member),
                    "you": member,
                    "playerId": player_id,
                    "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(Value::Null),
                    "playable": extra.get("playable").cloned().unwrap_or(Value::Null),
                }
            }))
        }
        "changed" => {
            let mut m = restore(ctx, req)?;
            Ok(json!({"ok": true, "changed": m.take_changed()}))
        }
        "events" => {
            let m = restore(ctx, req)?;
            let since = req.get("since").and_then(Value::as_i64).unwrap_or(0) as i32;
            Ok(json!({"ok": true, "events": m.events_since(since)}))
        }
        "ended" => {
            let m = restore(ctx, req)?;
            Ok(json!({"ok": true, "ended": m.ended()}))
        }

        _ => Err(format!("unknown op {op:?}")),
    }
}

/// Rebuild the match named by `req.state`. This is the whole statelessness
/// contract in one function: the blob is the match.
fn restore(ctx: &Ctx, req: &Value) -> Result<Match, String> {
    let s = req
        .get("state")
        .and_then(Value::as_str)
        .ok_or("missing state")?;
    Match::restore(ctx.data.clone(), ctx.rules.clone(), s).map_err(|e| format!("restore: {e}"))
}

fn parse<T: serde::de::DeserializeOwned>(req: &Value, key: &str) -> Result<T, String> {
    let v = req.get(key).ok_or_else(|| format!("missing {key}"))?;
    serde_json::from_value(v.clone()).map_err(|e| format!("{key}: {e}"))
}
