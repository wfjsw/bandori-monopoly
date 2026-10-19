//! Browser worker bundle for the advanced (ISMCTS) bot (`docs/BOT.md` B6).
//!
//! One wasm32 module: `game-core` + `game-rules` + `bot-core`. Loaded lazily
//! into a Web Worker -- the main page's glue is untouched, so its size does not
//! grow.
//!
//! **Default (`native-rules`) is the monolith** (docs/BOT.md §3.6 follow-up):
//! the card ruleset is **linked** as rlibs via `rules-native` -- no wasmi
//! `Store`, no per-run instantiate, no fetch of `/assets/rules/*.wasm`. The
//! host-import seam is the same `ruleset_*` symbols `rules-native` already
//! ships natively; on wasm32 wasm-ld resolves them inside this one module.
//! Must be built from the same commit as the deployed `dist/cards`. The
//! bundled [`ruleset_sha256`] is compared to the page's `index.json` and the
//! worker refuses to answer on mismatch.
//!
//! `--no-default-features --features sandbox` keeps the old wasmi sandbox path
//! (fetch the card modules the page loads) for fallback and tests. Cargo cannot
//! express "sandbox iff not `native-rules`", so the fallback names the feature
//! explicitly; bare `--no-default-features` is a compile error.
//!
//! **Information boundary** (`docs/BOT.md` §1): the only input is one seat's
//! view -- the exact frame the page renders for that seat (`MatchState` + own
//! hand + own sorted draw + `view_extra`). Never a `World`, a match seed, or
//! another seat's hand / deck order / `aiAnswer`. The reply is a proposed
//! `NetMessage`; the engine (solo glue / server) remains authoritative and
//! applies it through the ordinary `act` path.
//!
//! Surface (JSON strings across the boundary, the same shapes the server's
//! `bot-service` speaks):
//!
//! * `load_data` / `data_files` / `ruleset_add` / `ruleset_precompiled` /
//!   `ruleset_build` -- the one ruleset load sequence
//!   (`webui/src/core/rulesetLoad.ts` works against this glue unchanged).
//!   With `native-rules` the ruleset_* calls are no-ops: the rules are
//!   compiled in.
//! * `decide(view_json, budget_ms, seed)` -> `{answer, iterations, elapsed_ms,
//!   heuristic, reused, decisionKey, rootStats}`.
//! * `ponder(view_json, budget_ms, seed)` -- speculative search, cached by
//!   [`SeatView::decision_key`]; a later `decide` on the same key answers
//!   `reused: true` without spending its budget (BOT-RESEARCH #5).
//! * `extras(view_json, seed)` -> `{playable, estCost, skills, aiAnswer}` --
//!   the per-viewer extras the server's `view_extra` computes, derived from
//!   the viewer's own frame (the seat-view engine). `seed` is the
//!   determinizer's sampling seed, never the match's.
//!
//! Root-parallel ISMCTS runs **across workers** (the page's pool), not inside
//! one: `std::thread` is unavailable on wasm32, and each worker is its own
//! wasm instance + determinization stream (`seed_for_thread(seed, index)`).
//! The page merges the per-action root stats this API returns. Tree reuse is
//! per worker per seat (each worker keeps its own subtree).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use bot_core::{
    action, heuristic_message_view, view_extras, Ismcts, MatchSim, SearchConfig, SearchOutcome,
    SeatView,
};

#[cfg(all(not(feature = "native-rules"), not(feature = "sandbox")))]
compile_error!(
    "bot-glue: enable `native-rules` (the default monolith) or `sandbox` \
     (`--no-default-features --features sandbox` for the old wasmi path)"
);

use game_core::data::{GameData, Roster};
use game_core::engine::{CardRules, StubRules};
use game_core::net::NetMessage;
#[cfg(all(not(feature = "native-rules"), feature = "sandbox"))]
use game_rules::{Ruleset, WasmRules};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Sandbox `Ruleset::sha256()` of the cards this monolith was built against
/// (`index.json`'s `ruleset_sha256`). Empty when the build ran without a
/// built ruleset -- the worker then refuses to answer.
mod ruleset_identity {
    include!(concat!(env!("OUT_DIR"), "/ruleset_identity.rs"));
}
use ruleset_identity::RULESET_SHA256;

/// The browser bot worker is long-lived and the search allocates on every
/// iteration. Same allocator choice as `web-glue` (see its note on talc 5.x).
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

/// Hard cap on one request's search budget, whatever the caller asks for.
pub const MAX_BUDGET_MS: u64 = 5_000;
/// Floor, so "answer now" still gets one ISMCTS iteration.
pub const MIN_BUDGET_MS: u64 = 1;

/// A pondered / decided answer older than this is stale (the public state has
/// almost certainly moved; the key's clocks-stripped hash is not a timestamp).
const RESULT_TTL_MS: u64 = 60_000;

thread_local! {
    static DATA: RefCell<Option<Arc<GameData>>> = const { RefCell::new(None) };
    static RULES: RefCell<Option<Arc<dyn CardRules>>> = const { RefCell::new(None) };
    #[cfg(not(feature = "native-rules"))]
    static PENDING: RefCell<Option<game_rules::RulesetBuilder>> = const { RefCell::new(None) };
    /// Built ruleset, waiting for (or already bound to) the game tables.
    /// Either load order works: `load_data` before `ruleset_build` or after.
    /// Monolith (`native-rules`): unused -- the rules are compiled in.
    #[cfg(not(feature = "native-rules"))]
    static RULESET: RefCell<Option<Ruleset>> = const { RefCell::new(None) };
    /// The `BOOT_FILES` contents [`load_data`] was given, kept so a later
    /// `ruleset_build` can rebuild the tables with the ruleset's roster.
    static FILES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static RULESET_SHA: RefCell<String> = const { RefCell::new(String::new()) };
    static OPTS: RefCell<SearchOpts> = RefCell::new(SearchOpts::default());
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// Process-level search settings. Mirrors `bot_service::SearchOpts` minus the
/// thread knob (root-parallel lives in the page's worker pool).
#[derive(Debug, Clone)]
struct SearchOpts {
    bias_weight: f64,
    eval_weight: f64,
    implicit_minimax: bool,
    horizon_rounds: u32,
    reuse_trees: bool,
    accept_ponder: bool,
    /// Stop before the budget when the best root action cannot be overtaken
    /// (`docs/BOT.md` §3.4). On by default -- the worker is a production path.
    early_stop: bool,
    cache_cap: usize,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            bias_weight: 0.4,
            eval_weight: 0.3,
            implicit_minimax: true,
            horizon_rounds: 2,
            reuse_trees: true,
            accept_ponder: true,
            early_stop: true,
            cache_cap: 256,
        }
    }
}

#[derive(Debug, Clone)]
struct CachedDecision {
    answer: NetMessage,
    iterations: u64,
    elapsed_ms: u64,
    heuristic: bool,
    at_ms: u64,
}

#[derive(Debug, Default)]
struct Cache {
    results: HashMap<u64, CachedDecision>,
    result_order: VecDeque<u64>,
    trees: HashMap<i32, Ismcts>,
    /// Last public turn `(round, turn)` each seat ran a speculative
    /// (predicted) search for -- one searching ponder per seat per turn
    /// (`docs/BOT.md` §3.5), so the idle probes remain cheap no-ops.
    speculative_at: HashMap<i32, (i32, i32)>,
}

impl Cache {
    fn put_result(&mut self, key: u64, d: CachedDecision, cap: usize) {
        if cap == 0 {
            return;
        }
        if self.results.insert(key, d).is_none() {
            self.result_order.push_back(key);
        }
        while self.results.len() > cap {
            if let Some(old) = self.result_order.pop_front() {
                if self.results.len() > cap {
                    self.results.remove(&old);
                }
            } else {
                break;
            }
        }
    }

    fn live(&self, key: u64) -> Option<&CachedDecision> {
        let c = self.results.get(&key)?;
        if bot_core::clock::now_ms().saturating_sub(c.at_ms) < RESULT_TTL_MS {
            Some(c)
        } else {
            None
        }
    }

    /// Drop a cached answer after the engine refused it, so a refused one is
    /// never replayed (`docs/BOT.md` §5 B6).
    fn forget(&mut self, key: u64) {
        self.results.remove(&key);
    }
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn parse<T: serde::de::DeserializeOwned>(what: &str, s: &str) -> Result<T, JsError> {
    serde_json::from_str(s).map_err(|e| JsError::new(&format!("{what}: {e}")))
}

fn data() -> Result<Arc<GameData>, JsError> {
    DATA.with(|d| d.borrow().clone())
        .ok_or_else(|| JsError::new("game data not loaded (call load_data first)"))
}

fn rules() -> Result<Arc<dyn CardRules>, JsError> {
    let r = RULES.with(|r| r.borrow().clone());
    match r {
        Some(r) => Ok(r),
        // Same contract as the match shell: no modules = StubRules (cards do
        // nothing). The search still runs; it is just weaker. `info` reports
        // `"stub"` so the caller can tell.
        None => Ok(Arc::new(StubRules)),
    }
}

// ------------------------------------------------------------------ load

/// Build the tables from the stored files + the ruleset roster, then bind the
/// card rules.
///
/// Monolith (`native-rules`): the rules are compiled in (`rules-native`); the
/// roster is [`rules_native::roster`], and [`ruleset_add`] / [`ruleset_build`]
/// are no-ops so the worker's load sequence is unchanged.
///
/// Sandbox fallback: either load order works --
///
/// * `ruleset_build` then `load_data` -- the tables take the ruleset's roster.
/// * `load_data` then `ruleset_build` -- the tables are rebuilt here with the
///   roster the ruleset brings (ABI v63), so they are never stuck empty.
#[cfg(feature = "native-rules")]
fn assemble() -> Result<(), JsError> {
    let files = FILES.with(|f| f.borrow().clone());
    if files.is_empty() {
        // `load_data` has not run yet; a lone `ruleset_build` just waits.
        return Ok(());
    }
    let roster: Roster = rules_native::roster();
    let d = GameData::load(|f| files.get(f).cloned().ok_or_else(|| "missing".to_string()), roster)
        .map_err(|e| JsError::new(&e))?;
    let data = Arc::new(d);
    // Build the linked rules once; a later `load_data` rebinds them to the new
    // tables the same way the sandbox rebinds `WasmRules`.
    let rules: Arc<dyn CardRules> = Arc::new(rules_native::native_rules(data.clone()));
    RULES.with(|slot| *slot.borrow_mut() = Some(rules));
    DATA.with(|slot| *slot.borrow_mut() = Some(data));
    RULESET_SHA.with(|s| *s.borrow_mut() = RULESET_SHA256.to_string());
    Ok(())
}

#[cfg(not(feature = "native-rules"))]
fn assemble() -> Result<(), JsError> {
    let files = FILES.with(|f| f.borrow().clone());
    if files.is_empty() {
        // `load_data` has not run yet; a lone `ruleset_build` just stores the
        // set and waits for the tables.
        return Ok(());
    }
    let roster: Roster = RULESET.with(|r| {
        r.borrow()
            .as_ref()
            .map(|s| s.roster())
            .unwrap_or_default()
    });
    let d = GameData::load(|f| files.get(f).cloned().ok_or_else(|| "missing".to_string()), roster)
        .map_err(|e| JsError::new(&e))?;
    let data = Arc::new(d);
    let rules = RULESET
        .with(|r| r.borrow().clone())
        .map(|set| Arc::new(WasmRules::new(set, data.clone())) as Arc<dyn CardRules>);
    RULES.with(|slot| *slot.borrow_mut() = rules);
    DATA.with(|slot| *slot.borrow_mut() = Some(data));
    Ok(())
}

/// `files_json`: `{"board.json": "<contents>", ...}` for every file in
/// `BOOT_FILES`, plus the optional `deck_book.json`. Same shape as
/// `web-glue::load_data`. The roster is **not** among them: it comes from the
/// ruleset ([`ruleset_build`] / the linked table), whichever order the two are
/// called in.
#[wasm_bindgen]
pub fn load_data(files_json: &str) -> Result<(), JsError> {
    let files: HashMap<String, String> = parse("files", files_json)?;
    FILES.with(|slot| *slot.borrow_mut() = files);
    assemble()
}

/// The file names `load_data` needs (the host boot tables).
#[wasm_bindgen]
pub fn data_files() -> String {
    json(&game_core::data::BOOT_FILES)
}

/// Feed one card module (the bytes of `<sha>.wasm`); call `ruleset_build` after.
/// Monolith: no-op -- the ruleset is compiled in.
#[wasm_bindgen]
#[cfg(not(feature = "native-rules"))]
pub fn ruleset_add(bytes: &[u8]) -> Result<(), JsError> {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let b = p.get_or_insert_with(game_rules::Ruleset::builder);
        b.add(bytes).map_err(|e| JsError::new(&format!("{e:?}")))?;
        Ok(())
    })
}

/// Monolith: no-op (the worker's load sequence is unchanged).
#[wasm_bindgen]
#[cfg(feature = "native-rules")]
pub fn ruleset_add(_bytes: &[u8]) -> Result<(), JsError> {
    Ok(())
}

/// Feed the precompiled-condition blob (`conds-<sha>.bin`, `docs/GUARDS.md`
/// §8.2). Required whenever any loaded card declares a `pre`.
/// Monolith: no-op -- conditions are compiled from the linked table.
#[wasm_bindgen]
#[cfg(not(feature = "native-rules"))]
pub fn ruleset_precompiled(bytes: &[u8]) -> Result<(), JsError> {
    let conds = game_rules::PrecompiledConds::from_bytes(bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let b = p.get_or_insert_with(game_rules::Ruleset::builder);
        for e in conds.entries {
            b.precompiled(&e.card, e.entry, e.blob);
        }
        Ok(())
    })
}

/// Monolith: no-op (the worker's load sequence is unchanged).
#[wasm_bindgen]
#[cfg(feature = "native-rules")]
pub fn ruleset_precompiled(_bytes: &[u8]) -> Result<(), JsError> {
    Ok(())
}

/// Build the ruleset from the modules added so far. Returns the module count.
/// Either load order works with [`load_data`] (see [`assemble`]).
#[wasm_bindgen]
#[cfg(not(feature = "native-rules"))]
pub fn ruleset_build() -> Result<usize, JsError> {
    let built = PENDING
        .with(|p| p.borrow_mut().take())
        .ok_or_else(|| JsError::new("ruleset_add was never called"))?;
    let set = built.build().map_err(|e| JsError::new(&format!("{e:?}")))?;
    let n = set.module_count();
    let sha = set.sha256().to_string();
    RULESET.with(|r| *r.borrow_mut() = Some(set));
    RULESET_SHA.with(|s| *s.borrow_mut() = sha);
    assemble()?;
    Ok(n)
}

/// Monolith: the ruleset is compiled in. Returns `1` (one linked image) and
/// binds the tables. The worker still calls this so its load sequence is
/// byte-identical.
#[wasm_bindgen]
#[cfg(feature = "native-rules")]
pub fn ruleset_build() -> Result<usize, JsError> {
    RULESET_SHA.with(|s| *s.borrow_mut() = RULESET_SHA256.to_string());
    assemble()?;
    Ok(1)
}

/// Identity of the **linked** ruleset: the sandbox `Ruleset::sha256()` of the
/// cards this monolith was built against (the same order-independent hash
/// `game-rules` `host.rs` writes into `index.json` as `ruleset_sha256`).
/// The worker compares this to the page's `index.json` and refuses to answer
/// on mismatch. Empty when the build ran without a built ruleset.
#[wasm_bindgen]
pub fn ruleset_sha256() -> String {
    if cfg!(feature = "native-rules") {
        RULESET_SHA256.to_string()
    } else {
        RULESET_SHA.with(|s| s.borrow().clone())
    }
}

/// `true` when the card rules are compiled into this module (the monolith).
/// The worker skips fetching `/assets/rules/*.wasm` when this is `true`.
#[wasm_bindgen]
pub fn rules_linked() -> bool {
    cfg!(feature = "native-rules")
}

// ------------------------------------------------------------------ config

/// `{"bias_weight":0.4,"eval_weight":0.3,"implicit_minimax":true,
///   "horizon_rounds":2,"reuse_trees":true,"accept_ponder":true,
///   "early_stop":true,"cache_cap":256}`. Missing fields keep their current
/// value.
#[wasm_bindgen]
pub fn set_search(opts_json: &str) -> Result<(), JsError> {
    #[derive(serde::Deserialize, Default)]
    #[serde(default, rename_all = "camelCase")]
    struct Patch {
        bias_weight: Option<f64>,
        eval_weight: Option<f64>,
        implicit_minimax: Option<bool>,
        horizon_rounds: Option<u32>,
        reuse_trees: Option<bool>,
        accept_ponder: Option<bool>,
        early_stop: Option<bool>,
        cache_cap: Option<usize>,
    }
    let p: Patch = parse("opts", opts_json)?;
    OPTS.with(|o| {
        let mut o = o.borrow_mut();
        if let Some(v) = p.bias_weight {
            o.bias_weight = v.max(0.0);
        }
        if let Some(v) = p.eval_weight {
            o.eval_weight = v.clamp(0.0, 1.0);
        }
        if let Some(v) = p.implicit_minimax {
            o.implicit_minimax = v;
        }
        if let Some(v) = p.horizon_rounds {
            o.horizon_rounds = v.clamp(1, 4);
        }
        if let Some(v) = p.reuse_trees {
            o.reuse_trees = v;
        }
        if let Some(v) = p.accept_ponder {
            o.accept_ponder = v;
        }
        if let Some(v) = p.early_stop {
            o.early_stop = v;
        }
        if let Some(v) = p.cache_cap {
            o.cache_cap = v;
        }
    });
    Ok(())
}

/// Drop every cached tree and pondered answer (tests / a new match).
#[wasm_bindgen]
pub fn reset_cache() {
    CACHE.with(|c| *c.borrow_mut() = Cache::default());
}

/// Drop the cached answer for one decision key after the engine refused it
/// (`docs/BOT.md` §5 B6): `decision_key` is the reply's `decisionKey` (hex).
#[wasm_bindgen]
pub fn invalidate(decision_key: &str) -> Result<bool, JsError> {
    let key = u64::from_str_radix(decision_key.trim_start_matches("0x"), 16)
        .map_err(|e| JsError::new(&format!("decisionKey: {e}")))?;
    let dropped = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let had = c.results.contains_key(&key);
        c.forget(key);
        had
    });
    Ok(dropped)
}

/// Deterministic per-worker search seed (`bot_core::seed_for_thread`): worker
/// 0 keeps `seed` exactly (so a 1-worker pool is bit-identical to a plain
/// search), the others are split-mixed away from it. The page derives each
/// worker's `decide` seed with this so root-parallel across the pool matches
/// `Ismcts::search_root_parallel`.
#[wasm_bindgen]
pub fn seed_for_thread(seed: u32, thread: u32) -> u32 {
    (bot_core::seed_for_thread(seed as u64, thread as usize) & 0xFFFF_FFFF) as u32
}

/// What this worker is running, for logs and tests.
#[wasm_bindgen]
pub fn info() -> String {
    let opts = OPTS.with(|o| o.borrow().clone());
    let sha = RULESET_SHA.with(|s| s.borrow().clone());
    let sha = if cfg!(feature = "native-rules") && sha.is_empty() {
        RULESET_SHA256.to_string()
    } else {
        sha
    };
    json(&serde_json::json!({
        "service": "bot-glue",
        "linked": cfg!(feature = "native-rules"),
        "modules": if sha.is_empty() { "stub".to_string() } else { sha.clone() },
        "ruleset_sha256": sha,
        "bias_weight": opts.bias_weight,
        "eval_weight": opts.eval_weight,
        "implicit_minimax": opts.implicit_minimax,
        "horizon_rounds": opts.horizon_rounds,
        "reuse_trees": opts.reuse_trees,
        "ponder": opts.accept_ponder,
        "early_stop": opts.early_stop,
    }))
}

// ------------------------------------------------------------------ extras

/// `{view, seed}` -> `{playable, estCost, skills, aiAnswer}`.
///
/// The per-viewer extras the server's `Match::view_extra` computes, derived
/// from the viewer's **own** seat frame (the same input `decide` takes). Builds
/// the determinized world once and runs the same engine gates (`cant_play` /
/// `why_not_act` / `card_prop`, and the `ai_*` choice functions for
/// `aiAnswer`). Information boundary unchanged: never a `World`, a match seed,
/// or another seat's hand / deck order / `aiAnswer`.
///
/// `seed` is the determinizer's sampling seed (hidden zones the frame does not
/// pin) -- derive it from the decision identity (`decisionSeed`), never the
/// match's.
#[wasm_bindgen]
pub fn extras(view_json: &str, seed: u32) -> Result<String, JsError> {
    let view: SeatView = parse("view", view_json)?;
    let d = data()?;
    let r = rules()?;
    let extras = view_extras(&view, &d, &r, seed as u64)
        .map_err(|e| JsError::new(&format!("extras: {e}")))?;
    Ok(json(&extras.to_json()))
}

// ------------------------------------------------------------------ decide

/// One root action's statistics, with the **public command** the page would
/// send (`answer`). The page merges these across the worker pool
/// (`search_root_parallel`'s `merge_stats`) and sends the winner through the
/// ordinary `act` path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RootStat {
    answer: NetMessage,
    visits: u64,
    value_sum: f64,
    mean: f64,
    eval_max: f64,
    eval_sum: f64,
    eval_visits: u64,
}

fn root_stats_json(out: &SearchOutcome, st: &game_core::state::MatchState, seat: usize) -> Vec<RootStat> {
    out.root_stats_full
        .iter()
        .map(|s| RootStat {
            answer: action::to_net_message(&s.action, st, seat),
            visits: s.visits,
            value_sum: s.value_sum,
            mean: s.mean(),
            eval_max: s.eval_max,
            eval_sum: s.eval_sum,
            eval_visits: s.eval_visits,
        })
        .collect()
}

/// `{view, budget_ms, seed}` -> `{answer, iterations, elapsed_ms, heuristic,
/// reused, decisionKey, rootStats}`.
///
/// `view` is the exact per-member frame the client gets (`docs/BOT.md` §1).
/// `seed` is the **search** RNG's (the page derives it from the public
/// decision identity and the worker index), never the match's.
#[wasm_bindgen]
pub fn decide(view_json: &str, budget_ms: u32, seed: u32) -> Result<String, JsError> {
    let started = bot_core::clock::Instant::now();
    let view: SeatView = parse("view", view_json)?;
    let budget = Duration::from_millis((budget_ms as u64).clamp(MIN_BUDGET_MS, MAX_BUDGET_MS));
    let seat = view.player_id.max(0) as usize;
    if seat >= view.state.players.len() {
        return Err(JsError::new(&format!("seat {seat} out of range")));
    }
    let member = view.you;

    // Heuristic-delegated surface (roll / end / discard / trivial prompts).
    let d = data()?;
    let searched = action::legal_actions_with_cost(
        &d,
        &view.state,
        &view.hand,
        &view.playable,
        &view.est_cost,
        seat,
    );
    // Trivial root (docs/BOT.md §3.4): one forced action, or a free menu.
    // Answer without searching.
    match action::trivial_decision(&searched) {
        Some(action::Trivial::Forced(a)) => {
            let msg = action::to_net_message(&a, &view.state, seat);
            return Ok(json(&serde_json::json!({
                "ok": true,
                "answer": msg,
                "iterations": 0,
                "elapsedMs": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "reused": false,
                "decisionKey": format!("{:016x}", view.decision_key()),
                "rootStats": [],
            })));
        }
        Some(action::Trivial::LowStakes) => {
            let msg = heuristic_message_view(&d, &view);
            return Ok(json(&serde_json::json!({
                "ok": true,
                "answer": msg,
                "iterations": 0,
                "elapsedMs": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "reused": false,
                "decisionKey": format!("{:016x}", view.decision_key()),
                "rootStats": [],
            })));
        }
        None => {}
    }

    let key = view.decision_key();
    let opts = OPTS.with(|o| o.borrow().clone());
    if opts.accept_ponder {
        if let Some(c) = CACHE.with(|c| c.borrow().live(key).cloned()) {
            return Ok(json(&serde_json::json!({
                "ok": true,
                "answer": c.answer,
                "iterations": c.iterations,
                "elapsedMs": started.elapsed().as_millis() as u64,
                "heuristic": c.heuristic,
                "reused": true,
                "ponderedMs": c.elapsed_ms,
                "decisionKey": format!("{key:016x}"),
                "rootStats": [],
            })));
        }
    }

    let st = view.state.clone();
    let fallback = heuristic_message_view(&d, &view);
    let out = run_search(&view, seat, member, budget, seed as u64, &opts, false);
    let msg: NetMessage = if out.heuristic {
        fallback
    } else {
        action::to_net_message(&out.action, &st, seat)
    };
    let root_stats = if out.heuristic {
        Vec::new()
    } else {
        root_stats_json(&out, &st, seat)
    };
    if opts.accept_ponder {
        CACHE.with(|c| {
            c.borrow_mut().put_result(
                key,
                CachedDecision {
                    answer: msg.clone(),
                    iterations: out.iterations,
                    elapsed_ms: out.elapsed.as_millis() as u64,
                    heuristic: out.heuristic,
                    at_ms: bot_core::clock::now_ms(),
                },
                opts.cache_cap,
            )
        });
    }
    Ok(json(&serde_json::json!({
        "ok": true,
        "answer": msg,
        "iterations": out.iterations,
        "elapsedMs": started.elapsed().as_millis() as u64,
        "heuristic": out.heuristic,
        "reused": false,
        "decisionKey": format!("{key:016x}"),
        "rootStats": root_stats,
    })))
}

/// Speculative search while the other seats act (BOT-RESEARCH #5). Same view
/// frame as [`decide`]; the result is cached by the information-set key.
///
/// When the incoming view is idle, the **next own decision** is predicted
/// (`bot_core::predict_upcoming_view`, `docs/BOT.md` §3.5) and that turn-start
/// 运营 surface is what gets searched -- an idle view has no searchable
/// surface. Nothing is searched (and nothing cached) when the next decision
/// is not near.
#[wasm_bindgen]
pub fn ponder(view_json: &str, budget_ms: u32, seed: u32) -> Result<String, JsError> {
    let started = bot_core::clock::Instant::now();
    let opts = OPTS.with(|o| o.borrow().clone());
    if !opts.accept_ponder {
        return Ok(json(&serde_json::json!({"ok": true, "disabled": true})));
    }
    let view: SeatView = parse("view", view_json)?;
    let budget = Duration::from_millis((budget_ms as u64).clamp(MIN_BUDGET_MS, MAX_BUDGET_MS));
    let seat = view.player_id.max(0) as usize;
    if seat >= view.state.players.len() {
        return Err(JsError::new(&format!("seat {seat} out of range")));
    }
    let member = view.you;
    // What to search: the view itself when it is already a decision the
    // search would branch on, else the predicted turn-start surface, else
    // nothing (a cheap no-op).
    let idle = action::surface(&view.state, seat).is_none();
    let target = if idle {
        match bot_core::predict_upcoming_view(&view) {
            Some(v) => v,
            None => {
                return Ok(json(&serde_json::json!({
                    "ok": true,
                    "reused": false,
                    "iterations": 0,
                    "elapsedMs": started.elapsed().as_millis() as u64,
                    "heuristic": true,
                    "noop": true,
                })));
            }
        }
    } else {
        view.clone()
    };
    let key = target.decision_key();
    if let Some(c) = CACHE.with(|c| c.borrow().live(key).cloned()) {
        return Ok(json(&serde_json::json!({
            "ok": true,
            "reused": true,
            "iterations": c.iterations,
            "elapsedMs": started.elapsed().as_millis() as u64,
            "heuristic": c.heuristic,
            "decisionKey": format!("{key:016x}"),
        })));
    }
    let d = data()?;
    let searched = action::legal_actions_with_cost(
        &d,
        &target.state,
        &target.hand,
        &target.playable,
        &target.est_cost,
        seat,
    );
    if action::trivial_decision(&searched).is_some() {
        return Ok(json(&serde_json::json!({
            "ok": true,
            "reused": false,
            "iterations": 0,
            "elapsedMs": started.elapsed().as_millis() as u64,
            "heuristic": true,
            "decisionKey": format!("{key:016x}"),
        })));
    }
    // One searching ponder per seat per public turn (the idle probe ticks
    // every 200 ms while the "next turn is near" gate holds).
    if idle {
        let turn_id = (view.state.round, view.state.turn);
        let mut skip = false;
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.speculative_at.get(&member) == Some(&turn_id) {
                skip = true;
            } else {
                c.speculative_at.insert(member, turn_id);
            }
        });
        if skip {
            return Ok(json(&serde_json::json!({
                "ok": true,
                "reused": false,
                "iterations": 0,
                "elapsedMs": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "noop": true,
                "reason": "already-pondered-this-turn",
            })));
        }
    }
    let st = target.state.clone();
    let out = run_search(&target, seat, member, budget, seed as u64, &opts, true);
    let msg = if out.heuristic {
        heuristic_message_view(&d, &target)
    } else {
        action::to_net_message(&out.action, &st, seat)
    };
    CACHE.with(|c| {
        c.borrow_mut().put_result(
            key,
            CachedDecision {
                answer: msg.clone(),
                iterations: out.iterations,
                elapsed_ms: out.elapsed.as_millis() as u64,
                heuristic: out.heuristic,
                at_ms: bot_core::clock::now_ms(),
            },
            opts.cache_cap,
        )
    });
    Ok(json(&serde_json::json!({
        "ok": true,
        "reused": false,
        "iterations": out.iterations,
        "elapsedMs": started.elapsed().as_millis() as u64,
        "heuristic": out.heuristic,
        "decisionKey": format!("{key:016x}"),
        "speculative": idle,
        "answer": msg,
    })))
}

/// One single-threaded ISMCTS search over this worker's determinization
/// stream, reusing the seat's kept subtree and storing the new one under the
/// played action. Root-parallel across workers is the page's job (`docs/BOT.md`
/// §3.6): `std::thread` is unavailable on wasm32, and each worker already owns
/// its own stream via `seed_for_thread(seed, index)`.
///
/// `keep_whole_tree` (a speculative `ponder`) keeps every explored node; a
/// decide keeps only the subtree under the action it just played.
fn run_search(
    view: &SeatView,
    seat: usize,
    member: i32,
    budget: Duration,
    seed: u64,
    opts: &SearchOpts,
    keep_whole_tree: bool,
) -> SearchOutcome {
    let cfg = SearchConfig {
        budget,
        seed,
        horizon_rounds: opts.horizon_rounds,
        eval_weight: opts.eval_weight,
        implicit_minimax: opts.implicit_minimax,
        bias_weight: opts.bias_weight,
        early_stop: opts.early_stop,
        ..Default::default()
    };
    let mut tree = if opts.reuse_trees {
        CACHE.with(|c| c.borrow().trees.get(&member).cloned()).unwrap_or_default()
    } else {
        Ismcts::new()
    };
    let mut sim = MatchSim::new(data().expect("load_data"), rules().expect("rules"), view.clone(), seat, member);
    let out = tree.search(&mut sim, seat, cfg);
    if opts.reuse_trees && !out.heuristic {
        if !keep_whole_tree {
            tree.retain_after(out.root_key, &out.action);
        }
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            c.trees.insert(member, tree);
            // Trees are small; the ponder-result cap is the one that matters.
            while c.trees.len() > opts.cache_cap.max(1).min(64) {
                if let Some((&old, _)) = c.trees.iter().next() {
                    c.trees.remove(&old);
                } else {
                    break;
                }
            }
        });
    }
    out
}