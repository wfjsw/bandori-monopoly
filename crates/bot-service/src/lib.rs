//! The advanced-bot decision service (`docs/BOT.md` B5).
//!
//! The game server asks this process for one seat's answer. The request
//! carries **exactly one seat's view** -- the same frame the client gets
//! (`MatchState` + own hand + own sorted draw composition + `view_extra` +
//! the open prompt) -- never a `World`, a match seed, or another seat's
//! hidden information (`docs/BOT.md` §1). The reply is a proposed
//! [`NetMessage`]; the server stays authoritative and applies it through the
//! ordinary `act` path. Timeout / crash / no service = the server answers
//! with the engine's own heuristic, so a match never stalls.
//!
//! Search runs over determinizations on forks (`bot-core`) and uses its own
//! RNG seeded per decision -- the live match's RNG is never touched.
//!
//! This is the library form -- [`handle`] is the whole protocol, so the
//! `bot-service` binary (stdio, line-delimited JSON like `rules-worker`) and
//! the server's in-process test backend drive the same code. The request's
//! view is the whole input; the only state kept between calls is the search
//! cache (trees reused across consecutive decisions of one seat, and ponder
//! results keyed by the decision's information-set hash).
//!
//! ## Protocol additions over B5 (BOT-RESEARCH #2/#5)
//!
//! * `op: "ponder"` -- speculative search on the seat's current view (the
//!   server may send this while other seats act). The result is cached by
//!   [`SeatView::decision_key`] and a later `op: "decide"` whose view maps to
//!   the same key reuses it (`"reused": true`) without spending its budget.
//! * `decide` may search on several threads per request
//!   ([`SearchOpts::search_threads`], root-parallel ISMCTS) and reuses the
//!   subtree under the action it just played for the seat's next decision.
//! * Optional request fields are all clamped / defaulted here -- nothing is
//!   trusted from the wire.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bot_core::{
    action, Ismcts, MatchSim, SearchConfig, SearchOutcome, SeatView,
};
use game_core::data::GameData;
use game_core::engine::{CardRules, StubRules};
use game_core::net::NetMessage;
#[cfg(not(feature = "native-rules"))]
use game_rules::WasmRules;
#[cfg(feature = "native-rules")]
use game_rules::CardModules;
use serde_json::{json, Value};

// Re-exported for the server's fallback path: it must be able to answer with
// the engine's own heuristic from the view alone when this service is slow or
// gone (`docs/BOT.md` §1).
pub use bot_core::action as bot_action;
pub use bot_core::heuristic_message_view;
pub use bot_core::{next_turn_near, predict_upcoming_view};
pub use bot_core::SeatView as BotSeatView;

/// Hard cap on one request's search budget, whatever the caller asks for.
/// The client has its own deadline; this keeps a wild `budget_ms` from
/// hogging a search thread.
pub const MAX_BUDGET_MS: u64 = 5_000;
/// Floor, so "answer now" still gets one ISMCTS iteration.
pub const MIN_BUDGET_MS: u64 = 1;

/// Process-level search settings (CLI / env in the binary, defaults here for
/// the server's in-process backend). The A/B flags: set `bias_weight` and
/// `eval_weight` to `0` (and `implicit_minimax` off) for the pre-#3 search.
#[derive(Debug, Clone)]
pub struct SearchOpts {
    /// Root-parallel searches per decision (BOT-RESEARCH #2). `1` = the
    /// plain single-threaded search. The standalone binary defaults this to
    /// `--threads`; [`Ctx::new`] keeps `1` so the in-process server path
    /// never fans out threads inside the room tick's process.
    pub search_threads: usize,
    /// Progressive-bias weight `c_h` (0 = off).
    pub bias_weight: f64,
    /// Heuristic-eval mix `α` (0 = pure Monte-Carlo).
    pub eval_weight: f64,
    /// Implicit-minimax backup of the heuristic statistic.
    pub implicit_minimax: bool,
    /// Rollout horizon in rounds (1–2 is the affordable default).
    pub horizon_rounds: u32,
    /// Keep the subtree under the played action across decisions of one seat.
    pub reuse_trees: bool,
    /// Accept `op: "ponder"` and answer `decide` from the ponder cache.
    pub accept_ponder: bool,
    /// Stop the search before the budget when the best root action cannot be
    /// overtaken (`docs/BOT.md` §3.4 "Early stopping"). On by default here --
    /// the service is the production path. `bot-service --no-early-stop`
    /// restores the full-budget search for A/B.
    pub early_stop: bool,
    /// Cap on cached decisions / trees (LRU-ish by insertion).
    pub cache_cap: usize,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            search_threads: 1,
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

impl SearchOpts {
    /// The pre-#3 search, single-threaded, no tree reuse, no early stop --
    /// for A/B.
    pub fn legacy() -> Self {
        Self {
            search_threads: 1,
            bias_weight: 0.0,
            eval_weight: 0.0,
            implicit_minimax: false,
            horizon_rounds: 2,
            reuse_trees: false,
            accept_ponder: false,
            early_stop: false,
            cache_cap: 0,
        }
    }
}

/// One cached decision (ponder result, or a decide that a later request can
/// reuse when the information-set key matches).
#[derive(Debug, Clone)]
struct CachedDecision {
    answer: NetMessage,
    iterations: u64,
    elapsed_ms: u64,
    heuristic: bool,
    at: Instant,
}

/// A pondered answer older than this is stale -- the public state has almost
/// certainly moved, and the key's clocks-stripped hash is not a timestamp.
const RESULT_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
struct Cache {
    results: HashMap<u64, CachedDecision>,
    result_order: VecDeque<u64>,
    trees: HashMap<(String, i32), Ismcts>,
    tree_order: VecDeque<(String, i32)>,
    /// The last public turn `(round, turn)` each seat ran a **speculative**
    /// (predicted) search for. One searching ponder per seat per turn -- the
    /// rest of the idle probes are cheap no-ops. Without this bound the
    /// "next turn is near" gate still lets one through every 200 ms and the
    /// speculative search would dominate the CPU it is meant to save.
    speculative_at: HashMap<(String, i32), (i32, i32)>,
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
                // A key re-inserted later may still sit in the order queue;
                // only drop it when the map's entry is the stale one (same
                // key re-inserted keeps the map entry, so this is a no-op
                // then and the queue drains).
                if self.results.len() > cap {
                    self.results.remove(&old);
                }
            } else {
                break;
            }
        }
    }

    fn put_tree(&mut self, key: (String, i32), t: Ismcts, cap: usize) {
        if cap == 0 {
            return;
        }
        if self.trees.insert(key.clone(), t).is_none() {
            self.tree_order.push_back(key);
        }
        while self.trees.len() > cap {
            if let Some(old) = self.tree_order.pop_front() {
                if self.trees.len() > cap {
                    self.trees.remove(&old);
                }
            } else {
                break;
            }
        }
    }
}

/// Immutable content loaded once: game tables and card modules. Read-only
/// across requests -- the search clones what it needs and never writes back.
/// The only mutable state is [`SearchOpts`] (process config) and the search
/// cache (trees / ponder results), both interior-mutable so [`handle`] keeps
/// its `(&Ctx, Value)` signature for the server's in-process backend.
pub struct Ctx {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    pub opts: SearchOpts,
    cache: Mutex<Cache>,
}

impl Ctx {
    /// Wrap already-loaded tables. Used by the server's in-process backend.
    /// [`SearchOpts::default`] keeps `search_threads = 1` here -- the server
    /// process must not fan out inside the room tick; the standalone binary
    /// raises it via [`Self::with_opts`].
    pub fn new(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Self {
        Self {
            data,
            rules,
            opts: SearchOpts::default(),
            cache: Mutex::new(Cache::default()),
        }
    }

    /// Replace the search options (the binary's CLI / env surface).
    pub fn with_opts(mut self, opts: SearchOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Load the game tables and card modules once, the same way
    /// `rules-worker` and the server do (`data/` + `dist/cards` via
    /// [`WasmRules`]). Falls back to [`StubRules`] (cards have no effect)
    /// when no modules load, exactly like the match shell.
    pub fn load(data_dir: &std::path::Path, rules_dir: &std::path::Path) -> Self {
        let data = Arc::new(
            GameData::load(|f| {
                std::fs::read_to_string(data_dir.join(f)).map_err(|e| e.to_string())
            })
            .unwrap_or_else(|e| panic!("cannot load game data from {}: {e}", data_dir.display())),
        );
        let rules: Arc<dyn CardRules> = {
            // B1 (`docs/BOT.md` §3.1): with `--features native-rules` the
            // search's simulations run the card rules as plain native Rust --
            // no wasmtime Store per run. Advisory only: the match shell keeps
            // running the sandboxed modules, so a drift makes the bot weaker,
            // never the match wrong. Off by default while the drift check has
            // an open divergence (§5 B1).
            #[cfg(feature = "native-rules")]
            {
                let n = rules_native::native_rules(data.clone());
                eprintln!(
                    "card rules: native (rules-native, {} cards) -- simulations only, not the match",
                    n.ruleset().cards().len()
                );
                return Self {
                    data,
                    rules: Arc::new(n),
                    opts: SearchOpts::default(),
                    cache: Mutex::new(Cache::default()),
                };
            }
            #[cfg(not(feature = "native-rules"))]
            match WasmRules::load_dir(data.clone(), rules_dir) {
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
                    "no card modules in {} -- search runs on no-effect cards",
                    rules_dir.display()
                );
                Arc::new(StubRules)
            }
            Err(e) => {
                eprintln!("card modules failed to load: {e:?} -- falling back to no effects");
                Arc::new(StubRules)
            }
            }
        };
        Self {
            data,
            rules,
            opts: SearchOpts::default(),
            cache: Mutex::new(Cache::default()),
        }
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
    let op = req.get("op").and_then(Value::as_str).unwrap_or("decide");
    match op {
        // Liveness. No view is involved.
        "ping" => Ok(json!({"ok": true})),

        // What this service is, for logs and tests.
        "info" => Ok(json!({
            "ok": true,
            "service": "bot-service",
            "modules": ctx.rules.ruleset_sha256().unwrap_or("stub"),
            "search_threads": ctx.opts.search_threads,
            "bias_weight": ctx.opts.bias_weight,
            "eval_weight": ctx.opts.eval_weight,
            "implicit_minimax": ctx.opts.implicit_minimax,
            "horizon_rounds": ctx.opts.horizon_rounds,
            "reuse_trees": ctx.opts.reuse_trees,
            "ponder": ctx.opts.accept_ponder,
            "early_stop": ctx.opts.early_stop,
        })),

        "decide" => decide(ctx, req),

        // Speculative search while other seats act (BOT-RESEARCH #5). The
        // result is cached by the view's decision key and reused by a later
        // `decide` that matches it.
        "ponder" => ponder(ctx, req),

        // Drop a cached answer after the engine refused it, so a refused one
        // is never replayed (`docs/BOT.md` §5 B6).
        "invalidate" => invalidate(ctx, req),

        _ => Err(format!("unknown op {op:?}")),
    }
}

fn parse_opts_budget(req: &Value) -> u64 {
    req.get("budget_ms")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(MIN_BUDGET_MS, MAX_BUDGET_MS)
}

/// `{room, seat, view, prompt_id, budget_ms, seed}` ->
/// `{answer, iterations, elapsed_ms, heuristic}`.
fn decide(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let started = Instant::now();
    let view: SeatView = parse(req, "view")?;
    let budget_ms = parse_opts_budget(req);
    let seed = req.get("seed").and_then(Value::as_u64).unwrap_or(0);
    // `seat` on the wire is the member id (the server's "seat" is the member);
    // the player index rides the view and is the one the engine indexes by.
    let member = view.you;
    let seat = view.player_id.max(0) as usize;
    if seat >= view.state.players.len() {
        return Err(format!("seat {seat} out of range"));
    }

    let st = view.state.clone();
    // Heuristic-delegated surface (roll / end / discard / trivial prompts):
    // the engine's own policy answers it -- same as the B4 harness.
    let searched =
        action::legal_actions_with_cost(&ctx.data, &st, &view.hand, &view.playable, &view.est_cost, seat);
    // Trivial root (`docs/BOT.md` §3.4): one forced action, or a free menu.
    // Answer without searching -- this was 27–34% of searched bot CPU.
    match action::trivial_decision(&searched) {
        Some(action::Trivial::Forced(a)) => {
            let msg = action::to_net_message(&a, &st, seat);
            return Ok(json!({
                "ok": true,
                "answer": msg,
                "iterations": 0,
                "elapsed_ms": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "reused": false,
            }));
        }
        Some(action::Trivial::LowStakes) => {
            let msg = heuristic_message_view(&ctx.data, &view);
            return Ok(json!({
                "ok": true,
                "answer": msg,
                "iterations": 0,
                "elapsed_ms": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "reused": false,
            }));
        }
        None => {}
    }

    // Ponder cache hit: the same information set was already searched while
    // the other seats acted -- answer from the cache without spending the
    // decision's budget (BOT-RESEARCH #5).
    let key = view.decision_key();
    if ctx.opts.accept_ponder {
        let cached = ctx.cache.lock().unwrap().results.get(&key).cloned();
        if let Some(c) = cached {
            if c.at.elapsed() < RESULT_TTL {
                return Ok(json!({
                    "ok": true,
                    "answer": c.answer,
                    "iterations": c.iterations,
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                    "heuristic": c.heuristic,
                    "reused": true,
                    "pondered_ms": c.elapsed_ms,
                    "room": req.get("room").cloned().unwrap_or(Value::Null),
                    "prompt_id": req.get("prompt_id").cloned().unwrap_or(Value::Null),
                }));
            }
        }
    }

    // A refusals-proof fallback message, computed before the search so a
    // refused `to_net_message` still answers.
    let fallback = heuristic_message_view(&ctx.data, &view);
    let room = req
        .get("room")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let out = run_search(ctx, &view, seat, member, budget_ms, seed, &room, false);
    let msg: NetMessage = if out.heuristic {
        fallback
    } else {
        action::to_net_message(&out.action, &st, seat)
    };
    // Cache the result (a ponder for this key, or a repeated decision, reuses
    // it) and keep the subtree under the played action for the next decision
    // of this seat (BOT-RESEARCH #5).
    if ctx.opts.accept_ponder {
        ctx.cache.lock().unwrap().put_result(
            key,
            CachedDecision {
                answer: msg.clone(),
                iterations: out.iterations,
                elapsed_ms: out.elapsed.as_millis() as u64,
                heuristic: out.heuristic,
                at: Instant::now(),
            },
            ctx.opts.cache_cap,
        );
    }
    Ok(json!({
        "ok": true,
        "answer": msg,
        "iterations": out.iterations,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "heuristic": out.heuristic,
        "reused": false,
        "room": req.get("room").cloned().unwrap_or(Value::Null),
        "prompt_id": req.get("prompt_id").cloned().unwrap_or(Value::Null),
    }))
}

/// `{room, seat, view, budget_ms, seed}` -> `{iterations, elapsed_ms,
/// answer?, decisionKey}`. Speculative: the answer is also cached, so a
/// following `decide` on the same information set is free.
///
/// **What gets pondered (2026-10-08, `docs/BOT.md` §3.5).** An idle view has
/// no searchable surface -- the pre-C1 idle polls searched 0 of ~14 k ponders
/// (`target/scratch/profile/REPORT.md` §6). When the incoming view is idle
/// but the seat's **next own decision is near**, this predicts the turn-start
/// 运营 view ([`predict_upcoming_view`]) and searches *that* instead: the
/// speculative answer is cached under the predicted decision key (a real
/// `decide` that lands on the same key answers `reused: true` without
/// spending its budget) and the tree is kept warm for the seat. Otherwise it
/// is a cheap no-op -- no search, nothing cached.
fn ponder(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let started = Instant::now();
    if !ctx.opts.accept_ponder {
        return Ok(json!({"ok": true, "disabled": true}));
    }
    let view: SeatView = parse(req, "view")?;
    let budget_ms = parse_opts_budget(req);
    let seed = req.get("seed").and_then(Value::as_u64).unwrap_or(0);
    let member = view.you;
    let seat = view.player_id.max(0) as usize;
    if seat >= view.state.players.len() {
        return Err(format!("seat {seat} out of range"));
    }
    // What to search: the view itself when it is already a decision the
    // search would branch on, else the predicted turn-start surface when the
    // seat's next turn is near, else nothing.
    let idle = action::surface(&view.state, seat).is_none();
    let target = if idle {
        match bot_core::predict_upcoming_view(&view) {
            Some(v) => v,
            None => {
                // Nothing searchable and nothing upcoming: a cheap no-op.
                // This is the path that replaces the old "ponder the dead
                // idle view" traffic.
                return Ok(json!({
                    "ok": true,
                    "reused": false,
                    "iterations": 0,
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                    "heuristic": true,
                    "noop": true,
                }));
            }
        }
    } else {
        view.clone()
    };
    let key = target.decision_key();
    {
        let cache = ctx.cache.lock().unwrap();
        if let Some(c) = cache.results.get(&key) {
            if c.at.elapsed() < RESULT_TTL {
                return Ok(json!({
                    "ok": true,
                    "reused": true,
                    "iterations": c.iterations,
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                    "heuristic": c.heuristic,
                    "decisionKey": format!("{key:016x}"),
                }));
            }
        }
    }
    // Only worth pondering a surface the search would branch on.
    let st = target.state.clone();
    let searched = action::legal_actions_with_cost(
        &ctx.data,
        &st,
        &target.hand,
        &target.playable,
        &target.est_cost,
        seat,
    );
    if action::trivial_decision(&searched).is_some() {
        return Ok(json!({
            "ok": true,
            "reused": false,
            "iterations": 0,
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "heuristic": true,
            "decisionKey": format!("{key:016x}"),
        }));
    }
    let room = req
        .get("room")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    // One **searching** ponder per seat per public turn. The "next turn is
    // near" gate alone still fires every 200 ms while it holds; without this
    // bound the speculative search would cost more CPU than the decide it
    // might save. Everything else on the idle probe is a cheap no-op.
    if idle {
        let turn_id = (view.state.round, view.state.turn);
        let mut cache = ctx.cache.lock().unwrap();
        if cache.speculative_at.get(&(room.clone(), member)) == Some(&turn_id) {
            return Ok(json!({
                "ok": true,
                "reused": false,
                "iterations": 0,
                "elapsed_ms": started.elapsed().as_millis() as u64,
                "heuristic": true,
                "noop": true,
                "reason": "already-pondered-this-turn",
            }));
        }
        cache.speculative_at.insert((room.clone(), member), turn_id);
    }
    // `keep_whole_tree`: a speculative search does not know which action the
    // real turn will play, so keep every explored node warm (the decide path
    // keeps only the subtree under the action it just played).
    let out = run_search(ctx, &target, seat, member, budget_ms, seed, &room, true);
    let msg = action::to_net_message(&out.action, &st, seat);
    ctx.cache.lock().unwrap().put_result(
        key,
        CachedDecision {
            answer: msg.clone(),
            iterations: out.iterations,
            elapsed_ms: out.elapsed.as_millis() as u64,
            heuristic: out.heuristic,
            at: Instant::now(),
        },
        ctx.opts.cache_cap,
    );
    Ok(json!({
        "ok": true,
        "reused": false,
        "iterations": out.iterations,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "heuristic": out.heuristic,
        "decisionKey": format!("{key:016x}"),
        "speculative": idle,
        // The speculative answer; the server may ignore it (the cached entry
        // is what a later `decide` reuses).
        "answer": msg,
    }))
}

/// `{decisionKey}` -> `{ok}`. Drop a cached answer after the engine refused
/// it, so the next probe re-decides instead of replaying the refused one
/// (`docs/BOT.md` §5 B6). The key is the reply's `decisionKey` (hex) or the
/// raw u64 a caller computed from the same view.
fn invalidate(ctx: &Ctx, req: &Value) -> Result<Value, String> {
    let key = req
        .get("decisionKey")
        .and_then(Value::as_u64)
        .or_else(|| {
            req.get("decisionKey")
                .and_then(Value::as_str)
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        })
        .or_else(|| req.get("key").and_then(Value::as_u64))
        .ok_or_else(|| "missing decisionKey".to_string())?;
    ctx.forget(key);
    Ok(json!({"ok": true, "dropped": true}))
}

impl Ctx {
    /// Drop any cached answer / nothing else for this information-set key.
    /// Called when the engine refused the answer, so a refused one is never
    /// replayed from the ponder/decide cache.
    pub fn forget(&self, decision_key: u64) {
        self.cache.lock().unwrap().results.remove(&decision_key);
    }
}

/// Run the (possibly root-parallel) search for one decision, reusing the
/// seat's kept subtree and storing the new one under the played action.
///
/// `keep_whole_tree` (a speculative `ponder`) keeps every explored node
/// instead of only the subtree under the action it would have played -- the
/// real turn has not chosen yet and a warm superset is what the later
/// `decide` re-attaches to.
fn run_search(
    ctx: &Ctx,
    view: &SeatView,
    seat: usize,
    member: i32,
    budget_ms: u64,
    seed: u64,
    room: &str,
    keep_whole_tree: bool,
) -> SearchOutcome {
    let cfg = SearchConfig {
        budget: Duration::from_millis(budget_ms),
        seed,
        horizon_rounds: ctx.opts.horizon_rounds,
        eval_weight: ctx.opts.eval_weight,
        implicit_minimax: ctx.opts.implicit_minimax,
        bias_weight: ctx.opts.bias_weight,
        early_stop: ctx.opts.early_stop,
        ..Default::default()
    };
    let threads = ctx.opts.search_threads.max(1);
    // Warm tree: the subtree kept under the action this seat last played.
    let mut tree = if ctx.opts.reuse_trees {
        ctx.cache
            .lock()
            .unwrap()
            .trees
            .get(&(room.to_string(), member))
            .cloned()
            .unwrap_or_default()
    } else {
        Ismcts::new()
    };
    let factory = {
        let data = ctx.data.clone();
        let rules = ctx.rules.clone();
        let view = view.clone();
        move || MatchSim::new(data.clone(), rules.clone(), view.clone(), seat, member)
    };
    let out = tree.search_root_parallel(factory, seat, cfg, threads);
    if ctx.opts.reuse_trees && !out.heuristic {
        if !keep_whole_tree {
            tree.retain_after(out.root_key, &out.action);
        }
        ctx.cache.lock().unwrap().put_tree(
            (room.to_string(), member),
            tree,
            ctx.opts.cache_cap.min(64),
        );
    }
    out
}

fn parse<T: serde::de::DeserializeOwned>(req: &Value, key: &str) -> Result<T, String> {
    let v = req.get(key).ok_or_else(|| format!("missing {key}"))?;
    serde_json::from_value(v.clone()).map_err(|e| format!("{key}: {e}"))
}

/// Build the request the server sends. Shared by the server client and the
/// tests so the wire shape lives in one place.
pub fn decide_request(
    id: u64,
    room: &str,
    seat: i32,
    view: &Value,
    prompt_id: i32,
    budget_ms: u64,
    seed: u64,
) -> Value {
    json!({
        "id": id,
        "op": "decide",
        "room": room,
        "seat": seat,
        "view": view,
        "prompt_id": prompt_id,
        "budget_ms": budget_ms,
        "seed": seed,
    })
}

/// Build a `ponder` request (BOT-RESEARCH #5). Same view frame as
/// [`decide_request`]; the server sends it while the other seats act.
pub fn ponder_request(id: u64, room: &str, seat: i32, view: &Value, budget_ms: u64, seed: u64) -> Value {
    json!({
        "id": id,
        "op": "ponder",
        "room": room,
        "seat": seat,
        "view": view,
        "budget_ms": budget_ms,
        "seed": seed,
    })
}

/// Build an `invalidate` request: drop the cached answer for one decision key
/// after the engine refused it (`docs/BOT.md` §5 B6).
pub fn invalidate_request(id: u64, decision_key: u64) -> Value {
    json!({
        "id": id,
        "op": "invalidate",
        "decisionKey": format!("{decision_key:016x}"),
    })
}

/// What one decision came back as. `answer` is the proposed command; the
/// server applies it through the normal `act` path (or falls back on error).
#[derive(Debug, Clone)]
pub struct BotAnswer {
    pub answer: NetMessage,
    pub iterations: u64,
    pub elapsed_ms: u64,
    pub heuristic: bool,
    /// True when the answer came from the ponder cache (a speculative search
    /// during another seat's turn matched this decision key).
    pub reused: bool,
}

impl BotAnswer {
    pub fn from_response(v: &Value) -> Result<Self, String> {
        if v.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(v
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("bot-service error")
                .to_string());
        }
        let answer: NetMessage = serde_json::from_value(
            v.get("answer")
                .cloned()
                .ok_or_else(|| "bot-service returned no answer".to_string())?,
        )
        .map_err(|e| format!("answer: {e}"))?;
        Ok(Self {
            answer,
            iterations: v.get("iterations").and_then(Value::as_u64).unwrap_or(0),
            elapsed_ms: v.get("elapsed_ms").and_then(Value::as_u64).unwrap_or(0),
            heuristic: v.get("heuristic").and_then(Value::as_bool).unwrap_or(false),
            reused: v.get("reused").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}