//! Client for the `bot-service` process (`docs/BOT.md` B5).
//!
//! The game server asks for one seat's decision and applies the answer itself.
//! Transport is the same line-delimited JSON the `rules-worker` pool speaks
//! (stdin/stdout), but **pipelined**: many requests may be in flight and
//! replies are matched by `id`, because several rooms can ask at once and the
//! room tick loop must never wait on a search.
//!
//! Two backends, one interface:
//!
//! * [`BotService::start`] -- a child process (`--bot-service <path>` /
//!   `BM_BOT_SERVICE`), the production path. `--threads N` request workers
//!   and `--search-threads N` root-parallel searches per decision inside it.
//! * [`BotService::in_process`] -- `bot_service::handle` on this process's
//!   blocking pool, over one shared [`bot_service::Ctx`] so the ponder cache
//!   and tree reuse survive across calls. Tests use this; a *missing* binary
//!   is not this -- it is no service at all (advanced bots then play as
//!   standard, see [`crate::room::Room::start`]).
//!
//! Every call has a hard deadline. A slow or wedged service is treated as a
//! missing answer and the caller falls back to the engine's heuristic -- a
//! match never stalls on this (§1).
//!
//! Two request kinds ([`BotService::decide`] / [`BotService::ponder`]); the
//! latter is speculative (BOT-RESEARCH #5) and answers a later `decide` from
//! the service's cache when the information-set key matches. The drive loop
//! ([`Drive`]) keeps one probe and one ponder in flight per seat.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use game_core::data::GameData;
use game_core::engine::CardRules;
use serde_json::Value;
use tokio::sync::oneshot;

pub use bot_service::{decide_request, ponder_request, BotAnswer};

/// Extra wall-clock on top of the search budget, sized to absorb **one
/// iteration overrun** (`docs/BOT.md` §5 B7: the ISMCTS loop is anytime
/// *between* iterations and always finishes the one it is in, and on the real
/// ruleset one iteration costs 0.4–1.3 s). Without this margin the outer
/// deadline fires routinely on searched surfaces and every one of them falls
/// back to the heuristic.
pub const ITERATION_OVERRUN_MS: u64 = 1_500;
/// Hard ceiling on one ask's outer deadline, whatever the budget says.
pub const MAX_ASK_TIMEOUT: Duration = Duration::from_millis(3_000);
/// Turn-surface budget when the prompt clock is not running.
pub const DEFAULT_TURN_BUDGET_MS: u64 = 800;
/// Never spend more than this on one decision, whatever the clock says.
pub const MAX_ASK_BUDGET_MS: u64 = 1_000;
/// Keep this much of a prompt's clock out of the search budget (network +
/// `act` apply + scheduling).
pub const PROMPT_MARGIN_MS: u64 = 200;
/// Speculative search budget for an idle seat's `ponder` (BOT-RESEARCH #5;
/// the example wire frame in `docs/BOT.md` §3.5 uses 300 ms).
pub const PONDER_BUDGET_MS: u64 = 300;

/// Outer deadline for one ask: the search budget plus room for one iteration
/// overrun and queue / apply latency, capped at [`MAX_ASK_TIMEOUT`].
///
/// This is the policy `docs/BOT.md` §5 B7 recommends for the real ruleset
/// (`budget + 1.5 s`, the old `budget + 400 ms` fired on 4-thread searches).
pub fn ask_timeout(budget_ms: u64) -> Duration {
    Duration::from_millis(budget_ms.saturating_add(ITERATION_OVERRUN_MS)).min(MAX_ASK_TIMEOUT)
}

/// How a decision call failed. The caller falls back on every variant.
#[derive(Debug, Clone)]
pub enum BotError {
    /// No answer inside the deadline.
    Timeout,
    /// The process died, the pipe broke, or the reply was not usable.
    Down(String),
}

impl std::fmt::Display for BotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotError::Timeout => write!(f, "bot-service timed out"),
            BotError::Down(e) => write!(f, "bot-service: {e}"),
        }
    }
}

/// One in-flight request. `id` is the wire id; the oneshot carries the raw
/// response line or the failure that ended it.
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>;

enum Backend {
    /// Child process, one request line in / one response line out, `id` matched.
    Child {
        stdin: Mutex<ChildStdin>,
        child: Mutex<Child>,
        pending: Pending,
        next: AtomicU64,
    },
    /// In-process: `bot_service::handle` on the blocking pool. One [`bot_service::Ctx`]
    /// for the life of the backend -- its ponder cache / tree reuse must
    /// survive across calls, exactly like the child process's does.
    Local {
        ctx: Arc<bot_service::Ctx>,
        next: AtomicU64,
    },
}

/// Counters for the drive loop and its tests: how many decisions the service
/// answered (and how many of those came from a ponder) versus how many fell
/// back to the engine's heuristic, plus the speculative traffic.
#[derive(Debug, Default, Clone, Copy)]
pub struct BotStats {
    /// `decide` requests sent.
    pub decides: u64,
    /// Answered by the service (search or its own heuristic delegation).
    pub decide_ok: u64,
    /// Subset of `decide_ok` that came from the ponder cache (`reused: true`).
    pub decide_reused: u64,
    /// Timeout / crash / bad reply -- the server answered with the engine's
    /// heuristic instead.
    pub decide_fallback: u64,
    /// `ponder` requests sent.
    pub ponders: u64,
    /// Ponder replies that came back ok.
    pub ponder_ok: u64,
    /// Ponder replies that found the key already cached.
    pub ponder_reused: u64,
    /// High-water mark of ponders in flight at once (must stay at 1 per seat;
    /// the drive keeps one per seat, so this is the per-seat bound observed).
    pub ponder_in_flight_max: u64,
}

/// The optional advanced-bot decision service.
pub struct BotService {
    backend: Backend,
    exe: PathBuf,
    stats: BotStatsInner,
    /// Test knob: artificial latency on every request, so a test can observe
    /// an in-flight ponder / decide without relying on machine speed.
    latency: AtomicU64,
}

#[derive(Debug, Default)]
struct BotStatsInner {
    decides: AtomicU64,
    decide_ok: AtomicU64,
    decide_reused: AtomicU64,
    decide_fallback: AtomicU64,
    ponders: AtomicU64,
    ponder_ok: AtomicU64,
    ponder_reused: AtomicU64,
    ponder_in_flight: AtomicU64,
    ponder_in_flight_max: AtomicU64,
}

impl BotStatsInner {
    fn snapshot(&self) -> BotStats {
        BotStats {
            decides: self.decides.load(Ordering::Relaxed),
            decide_ok: self.decide_ok.load(Ordering::Relaxed),
            decide_reused: self.decide_reused.load(Ordering::Relaxed),
            decide_fallback: self.decide_fallback.load(Ordering::Relaxed),
            ponders: self.ponders.load(Ordering::Relaxed),
            ponder_ok: self.ponder_ok.load(Ordering::Relaxed),
            ponder_reused: self.ponder_reused.load(Ordering::Relaxed),
            ponder_in_flight_max: self.ponder_in_flight_max.load(Ordering::Relaxed),
        }
    }

    fn ponder_start(&self) {
        let n = self.ponder_in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        self.ponder_in_flight_max.fetch_max(n, Ordering::Relaxed);
    }

    fn ponder_end(&self) {
        self.ponder_in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

impl BotService {
    /// Start `n` request threads inside a `bot-service` child process. `exe`
    /// is the binary; `data`/`rules` are forwarded to it (the same immutable
    /// tables the match workers use, so the simulator agrees with the live
    /// ruleset). `search_threads` is the root-parallel fan-out **per decision**
    /// (`docs/BOT.md` §3.5); the outer deadline already leaves room for one
    /// iteration overrun at this fan-out ([`ask_timeout`]).
    pub fn start(
        exe: PathBuf,
        data: PathBuf,
        rules: PathBuf,
        threads: usize,
        search_threads: usize,
    ) -> Result<Arc<Self>, String> {
        let mut child = Command::new(&exe)
            .arg("--data")
            .arg(&data)
            .arg("--rules")
            .arg(&rules)
            .arg("--threads")
            .arg(threads.max(1).to_string())
            .arg("--search-threads")
            .arg(search_threads.max(1).to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", exe.display()))?;
        let stdin = child.stdin.take().ok_or("bot-service has no stdin")?;
        let stdout = child.stdout.take().ok_or("bot-service has no stdout")?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        // Reader thread owns stdout and completes the pending map. It exits on
        // EOF, which fails every waiter at once.
        {
            let pending = pending.clone();
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    let id = serde_json::from_str::<Value>(&line)
                        .ok()
                        .and_then(|v| v.get("id").and_then(Value::as_u64));
                    let Some(id) = id else { continue };
                    let tx = pending.lock().unwrap().remove(&id);
                    if let Some(tx) = tx {
                        let _ = tx.send(Ok(line));
                    }
                }
                let mut rest = pending.lock().unwrap();
                for (_, tx) in rest.drain() {
                    let _ = tx.send(Err("bot-service exited".into()));
                }
            });
        }
        eprintln!(
            "bot-service: {} (threads {threads}, search-threads {search_threads})",
            exe.display()
        );
        Ok(Arc::new(Self {
            backend: Backend::Child {
                stdin: Mutex::new(stdin),
                child: Mutex::new(child),
                pending,
                next: AtomicU64::new(1),
            },
            exe,
            stats: BotStatsInner::default(),
            latency: AtomicU64::new(0),
        }))
    }

    /// The engine in this process -- no child. Tests use this. One shared
    /// [`bot_service::Ctx`], so the ponder cache / tree reuse work exactly as
    /// they do in the child process.
    pub fn in_process(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Arc<Self> {
        Arc::new(Self {
            backend: Backend::Local {
                ctx: Arc::new(bot_service::Ctx::new(data, rules)),
                next: AtomicU64::new(1),
            },
            exe: PathBuf::new(),
            stats: BotStatsInner::default(),
            latency: AtomicU64::new(0),
        })
    }

    /// The `bot-service` binary next to this executable, or `BM_BOT_SERVICE`.
    pub fn default_exe() -> PathBuf {
        if let Ok(p) = std::env::var("BM_BOT_SERVICE") {
            return PathBuf::from(p);
        }
        let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        p.pop();
        p.push(if cfg!(windows) {
            "bot-service.exe"
        } else {
            "bot-service"
        });
        p
    }

    /// Ask for one decision. Async: the caller (the room tick) never blocks on
    /// the search. `timeout` is the outer deadline; on expiry the caller falls
    /// back to the heuristic.
    pub async fn decide(
        &self,
        room: &str,
        seat: i32,
        view: &Value,
        prompt_id: i32,
        budget_ms: u64,
        seed: u64,
        timeout: Duration,
    ) -> Result<BotAnswer, BotError> {
        self.stats.decides.fetch_add(1, Ordering::Relaxed);
        let id = match &self.backend {
            Backend::Child { next, .. } | Backend::Local { next, .. } => {
                next.fetch_add(1, Ordering::Relaxed)
            }
        };
        let req = decide_request(id, room, seat, view, prompt_id, budget_ms, seed);
        let deadline = Instant::now() + timeout;
        let raw = self.call(id, req, deadline).await;
        let raw = match raw {
            Ok(r) => r,
            Err(e) => {
                self.stats.decide_fallback.fetch_add(1, Ordering::Relaxed);
                return Err(e);
            }
        };
        let v: Value =
            serde_json::from_str(&raw).map_err(|e| BotError::Down(format!("bad reply: {e}")))?;
        match BotAnswer::from_response(&v) {
            Ok(a) => {
                self.stats.decide_ok.fetch_add(1, Ordering::Relaxed);
                if a.reused {
                    self.stats.decide_reused.fetch_add(1, Ordering::Relaxed);
                }
                Ok(a)
            }
            Err(e) => {
                self.stats.decide_fallback.fetch_add(1, Ordering::Relaxed);
                Err(BotError::Down(e))
            }
        }
    }

    /// Speculative search for an idle seat (BOT-RESEARCH #5). The caller must
    /// not block on this -- spawn it and move on; the result is cached inside
    /// the service and a later [`Self::decide`] for the same information-set
    /// key answers from that cache. Returns the reply's `reused` bit (whether
    /// the key was already cached) for stats; the answer itself is the
    /// service's business.
    pub async fn ponder(
        &self,
        room: &str,
        seat: i32,
        view: &Value,
        budget_ms: u64,
        seed: u64,
        timeout: Duration,
    ) -> Result<bool, BotError> {
        self.stats.ponders.fetch_add(1, Ordering::Relaxed);
        self.stats.ponder_start();
        let id = match &self.backend {
            Backend::Child { next, .. } | Backend::Local { next, .. } => {
                next.fetch_add(1, Ordering::Relaxed)
            }
        };
        let req = ponder_request(id, room, seat, view, budget_ms, seed);
        let deadline = Instant::now() + timeout;
        let out = self.call(id, req, deadline).await;
        self.stats.ponder_end();
        let raw = out?;
        let v: Value =
            serde_json::from_str(&raw).map_err(|e| BotError::Down(format!("bad reply: {e}")))?;
        if v.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(BotError::Down(
                v.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("bot-service error")
                    .to_string(),
            ));
        }
        self.stats.ponder_ok.fetch_add(1, Ordering::Relaxed);
        let reused = v.get("reused").and_then(Value::as_bool).unwrap_or(false);
        if reused {
            self.stats.ponder_reused.fetch_add(1, Ordering::Relaxed);
        }
        Ok(reused)
    }

    /// Counters for tests and the fallback-rate measurement.
    pub fn stats(&self) -> BotStats {
        self.stats.snapshot()
    }

    /// Test knob: sleep this long before dispatching every request, so a test
    /// can observe an in-flight call without relying on machine speed.
    pub fn set_latency(&self, d: Duration) {
        self.latency.store(d.as_millis() as u64, Ordering::Relaxed);
    }

    async fn call(&self, id: u64, req: Value, deadline: Instant) -> Result<String, BotError> {
        let lag = self.latency.load(Ordering::Relaxed);
        if lag > 0 {
            tokio::time::sleep(Duration::from_millis(lag)).await;
        }
        // A zero / past deadline is a hard timeout: nothing may be waited on,
        // not even a reply that would have come back for free.
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(BotError::Timeout);
        }
        match &self.backend {
            Backend::Child {
                stdin, pending, ..
            } => {
                let (tx, rx) = oneshot::channel();
                pending.lock().unwrap().insert(id, tx);
                let line = serde_json::to_string(&req).map_err(|e| BotError::Down(e.to_string()))?;
                {
                    let mut w = stdin.lock().unwrap();
                    writeln!(w, "{line}").map_err(|e| BotError::Down(format!("write: {e}")))?;
                    w.flush()
                        .map_err(|e| BotError::Down(format!("flush: {e}")))?;
                }
                let left = deadline.saturating_duration_since(Instant::now());
                match tokio::time::timeout(left, rx).await {
                    Err(_) => {
                        pending.lock().unwrap().remove(&id);
                        Err(BotError::Timeout)
                    }
                    Ok(Err(_)) => Err(BotError::Down("bot-service dropped the reply".into())),
                    Ok(Ok(Err(e))) => Err(BotError::Down(e)),
                    Ok(Ok(Ok(line))) => Ok(line),
                }
            }
            Backend::Local { ctx, .. } => {
                let ctx = ctx.clone();
                let raw = req.to_string();
                let left = deadline.saturating_duration_since(Instant::now());
                let job = tokio::task::spawn_blocking(move || {
                    let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
                    bot_service::handle(&ctx, v).to_string()
                });
                match tokio::time::timeout(left, job).await {
                    Err(_) => Err(BotError::Timeout),
                    Ok(Err(e)) => Err(BotError::Down(format!("task: {e}"))),
                    Ok(Ok(line)) => Ok(line),
                }
            }
        }
    }

    /// The binary this client would spawn (empty for the in-process backend).
    pub fn exe(&self) -> &std::path::Path {
        &self.exe
    }
}

/// Kill the child on drop so a server shutdown does not leave it behind.
impl Drop for BotService {
    fn drop(&mut self) {
        if let Backend::Child { child, .. } = &mut self.backend {
            if let Ok(mut c) = child.lock() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

/// What one seat is being asked right now, from the view frame alone.
///
/// `None` = nothing the server should ask about (setup phases are engine-side
/// for Advanced bots; the seat is mid-routine; or it is not its turn).
/// `Some(prompt_id)` = an open prompt waiting on the seat; `Some(0)` = the
/// turn surface (运营 card plays / 结束 buy-build-end).
pub fn decision_at(state: &game_core::state::MatchState, player_id: i32) -> Option<i32> {
    use game_core::state::stage;
    if state.phase != "play" {
        return None;
    }
    if state.prompt.id > 0 {
        return state
            .prompt
            .waiting(player_id)
            .then_some(state.prompt.id);
    }
    if state.busy || state.turn != player_id {
        return None;
    }
    match state.step {
        s if s == stage::OPS || s == stage::END => Some(0),
        _ => None,
    }
}

/// Budget for one decision, from the prompt clock when one is running.
pub fn budget_ms(state: &game_core::state::MatchState, prompt_id: i32) -> u64 {
    if prompt_id <= 0 {
        return DEFAULT_TURN_BUDGET_MS.min(MAX_ASK_BUDGET_MS);
    }
    let ms = (state.prompt.time_left.max(0.0) * 1000.0) as u64;
    ms.saturating_sub(PROMPT_MARGIN_MS)
        .clamp(50, MAX_ASK_BUDGET_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_timeout_leaves_room_for_one_iteration_overrun() {
        // The documented policy (`docs/BOT.md` §5 B7): `budget + 1.5 s`, so a
        // real-ruleset iteration (0.4–1.3 s) finishing after the budget still
        // lands inside the deadline and does not fall back.
        assert_eq!(ask_timeout(0), Duration::from_millis(ITERATION_OVERRUN_MS));
        assert_eq!(ask_timeout(800), Duration::from_millis(800 + ITERATION_OVERRUN_MS));
        assert_eq!(
            ask_timeout(1_000),
            Duration::from_millis(1_000 + ITERATION_OVERRUN_MS)
        );
        // Capped: a huge budget cannot park the probe for minutes.
        assert_eq!(ask_timeout(60_000), MAX_ASK_TIMEOUT);
        // And it is strictly wider than the old `budget + 400 ms` policy,
        // which routinely fired on 4-thread searches.
        assert!(ask_timeout(800) > Duration::from_millis(800 + 400));
    }

    #[test]
    fn drive_keeps_one_ponder_per_seat_and_cancels_on_decide() {
        let mut d = Drive::default();
        let t1 = d.claim_ponder("R", 1).expect("first ponder claims");
        assert!(d.claim_ponder("R", 1).is_none(), "one in flight per seat");
        assert!(d.claim_ponder("R", 2).is_some(), "other seats are independent");
        assert_eq!(d.ponders_in_flight(), 2);
        // The seat's own decision cancels / ignores the ponder.
        assert!(d.cancel_ponder("R", 1));
        assert_eq!(d.ponders_in_flight(), 1);
        // The cancelled reply must not free a newer ponder's slot.
        let t2 = d.claim_ponder("R", 1).expect("re-ponder after cancel");
        d.release_ponder("R", 1, t1);
        assert_eq!(d.ponders_in_flight(), 2, "stale release is ignored");
        d.release_ponder("R", 1, t2);
        assert_eq!(d.ponders_in_flight(), 1);
        d.forget_room("R");
        assert_eq!(d.ponders_in_flight(), 0);
    }
}

// ---------------------------------------------------------------- drive loop

/// Per-server bookkeeping for asking after advanced seats: which ones have a
/// request in flight, and when to next probe each (so an idle seat is not
/// re-viewed every 50 ms tick).
#[derive(Debug, Default)]
pub struct Drive {
    in_flight: std::collections::HashSet<(String, i32)>,
    next_probe: HashMap<(String, i32), Instant>,
    /// Speculative `ponder` outstanding, keyed `(room, member)` → token. One
    /// per seat: a second ponder for the same seat is skipped until the first
    /// replies (or is cancelled by that seat's own decision).
    pondering: HashMap<(String, i32), u64>,
    next_ponder_token: u64,
}

/// How soon to look again after a seat turned out to have nothing to do.
const IDLE_PROBE: Duration = Duration::from_millis(200);
/// How soon to look again right after applying an answer (the seat may have
/// another decision immediately -- play a card, then roll).
const BUSY_PROBE: Duration = Duration::from_millis(50);

impl Drive {
    /// Claim `seat` for a probe if it is idle and its probe time is due.
    /// Returns false when the tick should leave it alone.
    pub fn claim(&mut self, room: &str, member: i32, now: Instant) -> bool {
        let key = (room.to_string(), member);
        if self.in_flight.contains(&key) {
            return false;
        }
        if let Some(at) = self.next_probe.get(&key) {
            if now < *at {
                return false;
            }
        }
        self.in_flight.insert(key);
        true
    }

    /// Release `seat` after a probe. `acted` picks the next probe delay.
    pub fn release(&mut self, room: &str, member: i32, acted: bool) {
        let key = (room.to_string(), member);
        self.in_flight.remove(&key);
        self.next_probe.insert(
            key,
            Instant::now() + if acted { BUSY_PROBE } else { IDLE_PROBE },
        );
    }

    /// Claim the ponder slot for `seat`. False when one is already outstanding
    /// (one in flight per seat) or the room is gone.
    pub fn claim_ponder(&mut self, room: &str, member: i32) -> Option<u64> {
        let key = (room.to_string(), member);
        if self.pondering.contains_key(&key) {
            return None;
        }
        self.next_ponder_token += 1;
        let token = self.next_ponder_token;
        self.pondering.insert(key, token);
        Some(token)
    }

    /// Release the ponder slot, but only when `token` is still the owner (a
    /// cancelled ponder must not free a newer one).
    pub fn release_ponder(&mut self, room: &str, member: i32, token: u64) {
        let key = (room.to_string(), member);
        if self.pondering.get(&key) == Some(&token) {
            self.pondering.remove(&key);
        }
    }

    /// The seat's own decision arrived: cancel / ignore the in-flight ponder.
    /// Its reply is dropped (the decide supersedes it) and the slot frees up
    /// so the next idle probe can speculate on the new state.
    pub fn cancel_ponder(&mut self, room: &str, member: i32) -> bool {
        self.pondering.remove(&(room.to_string(), member)).is_some()
    }

    /// How many ponders are outstanding right now (tests: the per-seat bound
    /// is 1, so this is also the number of seats pondering).
    pub fn ponders_in_flight(&self) -> usize {
        self.pondering.len()
    }

    /// Drop a room's seats (the match ended or the room went away).
    pub fn forget_room(&mut self, room: &str) {
        self.in_flight.retain(|(r, _)| r != room);
        self.next_probe.retain(|(r, _), _| r != room);
        self.pondering.retain(|(r, _), _| r != room);
    }
}