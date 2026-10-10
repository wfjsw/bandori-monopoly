//! The rules worker: one operation on one match, given the whole match.
//!
//! This is the library form -- [`handle`] is the whole protocol, so both the
//! `rules-worker` binary (stdio) and the server's in-process backend drive the
//! same code. Nothing is retained between calls; see the binary's docs for the
//! request shape.
//!
//! Recording (`docs/REPLAY.md` §4) rides on top of the same protocol: `info`
//! hands out the [`EngineStamp`], every **mutating** op carries a `"cp"` turn
//! boundary when the turn key moved, and `replay` re-runs a `.bdrec` and
//! reports the hash it ended on.

use game_core::data::{GameData, DATA_FILES};
use game_core::engine::{CardRules, Match, StubRules, SAVE_VERSION};
use game_core::net::{NetMessage, RoomMember};
use game_core::record::{decode_record, hash_save, EngineStamp, Replayer, RECORD_VERSION};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use game_rules::WasmRules;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Immutable content loaded once: game tables and card modules. Read-only
/// across requests -- this is the crate's only cross-request data, and it is
/// never written.
pub struct Ctx {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    /// Sha256 over the `DATA_FILES` contents in order (see [`Ctx::load`]).
    /// Empty when the caller never supplied one -- [`EngineStamp`] then leaves
    /// `data_sha256` unknown and [`game_core::record::compat`] skips it.
    pub data_sha256: String,
}

impl Ctx {
    /// Wrap already-loaded tables. Used by the server's in-process backend.
    pub fn new(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Self {
        Self {
            data,
            rules,
            data_sha256: String::new(),
        }
    }

    /// Record where the game tables came from, so the [`EngineStamp`] can name
    /// them. `Ctx::load` fills this in; callers of [`Ctx::new`] may too.
    pub fn with_data_sha(mut self, sha: impl Into<String>) -> Self {
        self.data_sha256 = sha.into();
        self
    }

    /// Load the game tables and card modules once. Read-only from here on.
    /// The tables' sha256 (in [`DATA_FILES`] order) is captured here too, so a
    /// record written by this worker says which board / card data it saw.
    pub fn load(data_dir: &std::path::Path, rules_dir: &std::path::Path) -> Self {
        let mut contents: Vec<(String, String)> = Vec::new();
        let data = Arc::new(
            GameData::load(|f| {
                let text =
                    std::fs::read_to_string(data_dir.join(f)).map_err(|e| e.to_string())?;
                contents.push((f.to_string(), text.clone()));
                Ok(text)
            })
            .unwrap_or_else(|e| panic!("cannot load game data from {}: {e}", data_dir.display())),
        );
        let mut hasher = Sha256::new();
        for name in DATA_FILES {
            // Same recipe as web-glue's `load_data`: every `DATA_FILES` entry
            // in order (missing entries are skipped), a leading BOM is not
            // content. `GameData::load` does not ask for all of them
            // (`skill_simple.json` is display-only and the engine never reads
            // it), so read the ones it skipped -- otherwise this hash disagrees
            // with the browser's and the record names an engine bundle the
            // replay loader has never seen.
            let owned;
            let text: Option<&str> = match contents.iter().find(|(f, _)| f == name) {
                Some((_, text)) => Some(text),
                None => match std::fs::read_to_string(data_dir.join(name)) {
                    Ok(t) => {
                        owned = t;
                        Some(owned.as_str())
                    }
                    Err(_) => None,
                },
            };
            if let Some(text) = text {
                hasher.update(text.strip_prefix('\u{feff}').unwrap_or(text).as_bytes());
            }
        }
        let data_sha256 = hex(&hasher.finalize());
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
        Self {
            data,
            rules,
            data_sha256,
        }
    }

    /// The [`EngineStamp`] of the engine this worker is running. `format` /
    /// `save_version` come from game-core, `abi` from `card-sdk`, the ruleset
    /// hash from the loaded card modules (`"stub"` without any) and the data
    /// hash from [`Ctx::load`] / [`Ctx::with_data_sha`].
    ///
    /// `bundle` names the engine bundle a browser can replay the record with
    /// (`docs/REPLAY.md` §9). The server binary is not the browser glue, so
    /// the glue identity comes from the build it was deployed with: the
    /// `BD_GLUE_SHA` env var, or the `glueSha256` in the file `BD_ENGINE_ID`
    /// (default `webui/src/wasm/engine_id.json`, written by
    /// `tools/build-glue.mjs`). Same deploy, same ruleset and data -- the
    /// same stamp a browser session of that build seals. With neither, the
    /// bundle is empty ("unknown") and the loader falls back to matching the
    /// other fields.
    pub fn stamp(&self) -> EngineStamp {
        let stamp = EngineStamp {
            format: RECORD_VERSION,
            save_version: SAVE_VERSION,
            abi: game_rules::ABI_VERSION as u32,
            ruleset_sha256: self
                .rules
                .ruleset_sha256()
                .unwrap_or("stub")
                .to_string(),
            data_sha256: self.data_sha256.clone(),
            engine: "game-core".into(),
            build: env!("CARGO_PKG_VERSION").into(),
            bundle: String::new(),
        };
        let bundle = game_core::record::bundle_id(&deployed_glue_sha(), &stamp);
        EngineStamp { bundle, ..stamp }
    }
}

/// The deployed build's glue identity (see [`Ctx::stamp`]). Cached: the
/// environment and the file do not move under a running server.
fn deployed_glue_sha() -> String {
    use std::sync::OnceLock;
    static SHA: OnceLock<String> = OnceLock::new();
    SHA.get_or_init(|| {
        if let Ok(s) = std::env::var("BD_GLUE_SHA") {
            let s = s.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        let path = std::env::var("BD_ENGINE_ID")
            .unwrap_or_else(|_| "webui/src/wasm/engine_id.json".to_string());
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!(
                "no BD_GLUE_SHA and cannot read {path} -- records get an empty engine bundle id"
            );
            return String::new();
        };
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{path}: {e} -- records get an empty engine bundle id");
                return String::new();
            }
        };
        v.get("glueSha256")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    })
    .clone()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
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

/// `MatchState`'s turn key `(round, turn)`. A change opens a checkpoint.
fn turn_key(m: &Match) -> (i32, i32) {
    let st = m.state();
    (st.round, st.turn)
}

fn run(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let op = req.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        // Liveness / observability. No match state is involved.
        "ping" => Ok(json!({"ok": true})),

        // The engine's identity, for a record's `EngineStamp` (docs/REPLAY.md §4).
        "info" => Ok(json!({"ok": true, "stamp": ctx.stamp()})),

        "new" => {
            let members: Vec<RoomMember> = parse(req, "members")?;
            let mode = req.get("mode").and_then(Value::as_i64).unwrap_or(0) as i32;
            let weights: ScoreWeights = match req.get("weights") {
                None | Some(Value::Null) => ScoreWeights::default(),
                Some(w) => {
                    serde_json::from_value(w.clone()).map_err(|e| format!("weights: {e}"))?
                }
            };
            // Prefer the derived 256-bit seed (every new match,
            // `docs/FAIRNESS.md`); a bare u64 `seed` is the legacy path, kept
            // for pre-fairness callers and tests only.
            let m = match req.get("seed256").and_then(Value::as_str) {
                Some(hex) => {
                    let key = game_core::fair::unhex32(hex)
                        .map_err(|e| format!("seed256: {e}"))?;
                    Match::new_seeded(
                        ctx.data.clone(),
                        ctx.rules.clone(),
                        &members,
                        game_core::rng::Seed256(key),
                        MatchMode::from_i32(mode).unwrap_or_default(),
                        weights,
                    )
                }
                None => {
                    let seed = req.get("seed").and_then(Value::as_u64).unwrap_or(1);
                    Match::new(
                        ctx.data.clone(),
                        ctx.rules.clone(),
                        &members,
                        seed,
                        MatchMode::from_i32(mode).unwrap_or_default(),
                        weights,
                    )
                }
            };
            Ok(json!({"ok": true, "state": m.save()}))
        }

        // Mutating ops: restore -> op -> save. The caller gets the new blob and
        // is responsible for keeping it; the worker forgets it here. Each one
        // also reports the turn-boundary checkpoint (`cp`) when the turn key
        // moved, so the server can append it to the record log
        // (`docs/REPLAY.md` §4).
        "act" | "tick" | "quick_start" | "finish" => {
            let mut m = restore(ctx, req)?;
            let before = turn_key(&m);
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
            let mut v = json!({"ok": true, "state": m.save(), "changed": true, "ended": m.ended()});
            if !out.is_null() {
                v["error"] = out;
            }
            if let Some(cp) = checkpoint(&m, before) {
                v["cp"] = cp;
            }
            Ok(v)
        }

        // Mutating: a member dropped or returned mid-match.
        "member_left" | "member_back" => {
            let mut m = restore(ctx, req)?;
            let before = turn_key(&m);
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
            let mut v = json!({"ok": true, "state": m.save(), "changed": true, "ended": m.ended()});
            if let Some(cp) = checkpoint(&m, before) {
                v["cp"] = cp;
            }
            Ok(v)
        }

        // Read-only ops: the blob comes back unchanged, so it is not echoed.
        // `needExtras` (default false): compute the per-viewer `view_extra`
        // fields (`aiAnswer` / `playable` / `estCost` / `skills`). A human
        // client derives those itself from the seat view (the seat-view
        // engine, `docs/BOT.md` §1) -- skipping here is pure saved work. The
        // bot service asks for them.
        "view" => {
            let m = restore(ctx, req)?;
            let member = req.get("member").and_then(Value::as_i64).unwrap_or(0) as i32;
            let need_extras = req
                .get("needExtras")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let state = m.state();
            let player_id = state.player_of(member);
            let extra = if need_extras {
                m.view_extra(member)
            } else {
                Value::Null
            };
            let mut view = json!({
                "state": state,
                "hand": m.hand_of(member),
                "handNotes": m.hand_notes_of(member),
                "draw": m.draw_of(member),
                "you": member,
                "playerId": player_id,
                "tileQuotes": m.tile_quotes(member),
            });
            if need_extras {
                view["aiAnswer"] = extra.get("aiAnswer").cloned().unwrap_or(Value::Null);
                view["playable"] = extra.get("playable").cloned().unwrap_or(Value::Null);
                view["estCost"] = extra.get("estCost").cloned().unwrap_or(Value::Null);
                view["skills"] = extra.get("skills").cloned().unwrap_or(Value::Null);
            }
            Ok(json!({ "ok": true, "view": view }))
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

        // Re-run a `.bdrec` and report where it ended. `force`: a stamp
        // mismatch is not this op's business -- the caller has already decided
        // to ask. `final_hash` is the hash of the **derived** final `save()`,
        // which is what the test compares against the blob the server kept.
        "replay" => Ok(replay(ctx, req)?),

        _ => Err(format!("unknown op {op:?}")),
    }
}

/// `{round, turn, hash}` when the turn key moved across `before`.
fn checkpoint(m: &Match, before: (i32, i32)) -> Option<Value> {
    let after = turn_key(m);
    if after == before {
        return None;
    }
    Some(json!({
        "round": after.0,
        "turn": after.1,
        "hash": hash_save(&m.save()),
    }))
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

/// The `record` argument of the `replay` op: raw `.bdrec` bytes as an array of
/// numbers, or the record JSON as a string. Both land in
/// [`game_core::record::decode_record`], which sniffs zstd / gzip / plain.
fn record_arg(req: &Value) -> Result<Vec<u8>, String> {
    match req.get("record") {
        Some(Value::String(s)) => Ok(s.as_bytes().to_vec()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(|| "record byte out of range".to_string())
            })
            .collect(),
        _ => Err("missing record".into()),
    }
}

/// `{record}` -> `{final_hash, diverged}`. Drives the log to its end and hashes
/// the `save()` it produced; `diverged` covers a checkpoint that disagreed, an
/// act whose Ok/Err bit differed, **and** a final hash that does not match the
/// one sealed into the record.
///
/// `record` is either the raw `.bdrec` bytes (an array of numbers -- zstd
/// today, plain JSON from an older store) or the record JSON as a string; both
/// are decoded by the shared [`game_core::record::decode_record`], which
/// sniffs the framing.
fn replay(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let bytes = record_arg(req)?;
    let rec = decode_record(&bytes).map_err(|e| e.to_string())?;
    let mut rp = Replayer::new_with_stamp(
        ctx.data.clone(),
        ctx.rules.clone(),
        &rec,
        &ctx.stamp(),
        true,
    )
    .map_err(|e| e.to_string())?;
    // The whole log. `step_ticks` applies the non-tick inputs at the current
    // position along the way, so this walks the body exactly as a player would.
    let mut guard = 0u32;
    let mut last = rp.status().tick;
    while !rp.status().ended {
        let st = rp.step_ticks(256);
        if st.tick == last {
            break; // no progress
        }
        last = st.tick;
        guard += 1;
        if guard > 200_000 {
            return Err("replay did not terminate".into());
        }
    }
    let final_hash = hash_save(&rp.match_ref().save());
    let mut diverged = rp.status().diverged;
    if !rec.body.final_hash.is_empty() && rec.body.final_hash != final_hash {
        diverged = true;
    }
    Ok(json!({
        "ok": true,
        "final_hash": final_hash,
        "diverged": diverged,
        "tick": rp.status().tick,
    }))
}