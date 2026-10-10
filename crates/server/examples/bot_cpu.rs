//! CPU profile of the advanced-bot decision service over **full matches**
//! (`docs/BOT.md` §3.5 / §5). Measurement only -- it drives the same code the
//! live server drives, at the live defaults, and reports where the bot
//! service's CPU goes. No behaviour is changed anywhere.
//!
//! ```text
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 cargo build -p server --release --example bot_cpu -j 2
//! ./target/coord/release/examples/bot_cpu --data data --rules dist/cards --matches 1
//! ```
//!
//! The harness plays 4-seat matches (3 **Advanced** bots + 1 Standard, like a
//! live table) and reproduces the server's drive loop in **game time** (which
//! equals wall time at `time_scale = 1.0`):
//!
//! * every idle advanced seat is probed every 200 ms (`botsvc::Drive` cadence),
//!   busy ones every 50 ms after an applied answer;
//! * a probe that finds a decision sends `decide` at the server's budget policy
//!   (`botsvc::budget_ms`: 800 ms turn / `min(1000, prompt left - 200)`);
//! * a probe that finds nothing to decide sends `ponder` at
//!   [`botsvc::PONDER_BUDGET_MS`] (the live `spawn_ponder` path);
//! * while one request runs, the match clock keeps advancing (the live ticker
//!   does not wait on a search) -- request wall time is replayed as engine
//!   ticks before the answer is applied, so prompt expiry races behave like
//!   production.
//!
//! Every request goes through the **wire form** of the `bot-service` protocol
//! (`serde_json` line -> [`bot_service::handle`] -> reply line), and the
//! process CPU time around that window is the per-request bot-service CPU
//! (the same work the child process does per request: line parse + search +
//! reply serialise). Engine-side work (view build, tick, act) is measured
//! separately so the live "bot-service vs server" split is visible.
//!
//! Reports, per match and in total: bot-service CPU-seconds and % of one core
//! over match time; decide vs ponder x searched/delegated/reused; per surface
//! kind; per-decision CPU and iterations; and a search-anatomy pass
//! (`SearchConfig::profile` on a sample of recorded views) that splits one
//! search into fork materialisation / descent / rollout / evaluation / key.

#[path = "shared/alloc_hook.rs"]
mod alloc_hook;

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOC: alloc_hook::ProfAlloc = alloc_hook::ProfAlloc;

use bot_service::{
    decide_request, handle, heuristic_message_view, ponder_request, BotAnswer, BotSeatView, Ctx,
    SearchOpts,
};
use game_core::engine::Match;
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality};
use game_core::MatchMode;
use serde_json::Value;
use server::botsvc;

// ---------------------------------------------------------------- process CPU

/// Process CPU time (user + kernel, all threads) -- the metric the live
/// `bot-service` process was measured in. Windows: `GetProcessTimes`; Linux:
/// `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)`; anything else falls back to wall
/// time (noted in the report).
#[cfg(windows)]
fn process_cpu() -> Duration {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    unsafe {
        let mut creation = FileTime { low: 0, high: 0 };
        let mut exit = FileTime { low: 0, high: 0 };
        let mut kernel = FileTime { low: 0, high: 0 };
        let mut user = FileTime { low: 0, high: 0 };
        let ok = GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        );
        if ok == 0 {
            return Duration::ZERO;
        }
        // FILETIME is 100 ns ticks.
        let ticks = |ft: FileTime| ((ft.high as u64) << 32 | ft.low as u64) * 100;
        Duration::from_nanos(ticks(kernel) + ticks(user))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn process_cpu() -> Duration {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        nsec: i64,
    }
    extern "C" {
        fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
    }
    const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
    unsafe {
        let mut ts = Timespec { sec: 0, nsec: 0 };
        if clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut ts) != 0 {
            return Duration::ZERO;
        }
        Duration::new(ts.sec.max(0) as u64, ts.nsec.max(0) as u32)
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn process_cpu() -> Duration {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("bot_cpu: no process-CPU clock on this target -- using wall time");
    }
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

// ------------------------------------------------------------------- helpers

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| default.to_string())
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// The exact frame the server sends (`MatchHandle::view` / `rules-worker`'s
/// `view` op) -- kept verbatim from `examples/bot_fallback.rs`.
fn view_frame(m: &Match, member: i32) -> Value {
    let st = m.state();
    let player_id = st.player_of(member);
    let extra = m.view_extra(member);
    serde_json::json!({
        "state": st,
        "hand": m.hand_of(member),
        "handNotes": m.hand_notes_of(member),
        "draw": m.draw_of(member),
        "you": member,
        "playerId": player_id,
        "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(Value::Null),
        "playable": extra.get("playable").cloned().unwrap_or(Value::Null),
        "estCost": extra.get("estCost").cloned().unwrap_or(Value::Null),
        "skills": extra.get("skills").cloned().unwrap_or(Value::Null),
    })
}

/// Why a request is being made, from the view alone. `idle/*` = the live
/// `spawn_ponder` path (nothing to decide right now).
fn surface_label(v: &BotSeatView) -> String {
    let st = &v.state;
    if st.phase != "play" {
        return format!("phase/{}", st.phase);
    }
    if st.prompt.id > 0 {
        if !st.prompt.waiting(v.player_id) {
            return format!("idle/other-prompt:{}", st.prompt.kind);
        }
        let p = &st.prompt;
        return match p.kind.as_str() {
            "choice" => {
                if p.title.k.as_ref() == "ask.counteract.title" {
                    "prompt/counteract".into()
                } else {
                    format!("prompt/choice:{}", p.title.k.as_ref())
                }
            }
            k => format!("prompt/{k}"),
        };
    }
    if st.busy {
        return "idle/busy".into();
    }
    if st.turn == v.player_id {
        return match st.step {
            s if s == stage::OPS => "turn/ops".into(),
            s if s == stage::END => "turn/end".into(),
            s if s == stage::MOVE => "turn/move".into(),
            s if s == stage::START => "turn/start".into(),
            other => format!("turn/step{other}"),
        };
    }
    "idle/other-turn".into()
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn ms(d: Duration) -> u64 {
    d.as_millis() as u64
}

// ------------------------------------------------------------------ records

#[derive(Debug, Clone)]
struct ReqRec {
    ponder: bool,
    surface: String,
    searched: bool,
    delegated: bool,
    reused: bool,
    iterations: u64,
    budget_ms: u64,
    wall_ms: u64,
    cpu_ms: u64,
    root_actions: usize,
    agrees_heuristic: bool,
    view_bytes: usize,
    /// `ask_timeout(budget)` would have given up on this reply.
    would_fallback: bool,
    /// The engine refused the applied answer (state moved on).
    act_refused: bool,
    /// Why the engine refused (i18n key), when it did.
    refuse_reason: String,
}

#[derive(Debug, Default, Clone)]
struct Agg {
    calls: usize,
    cpu_ms: u64,
    wall_ms: u64,
    iterations: u64,
    cpu_samples: Vec<u64>,
}

impl Agg {
    fn add(&mut self, r: &ReqRec) {
        self.calls += 1;
        self.cpu_ms += r.cpu_ms;
        self.wall_ms += r.wall_ms;
        self.iterations += r.iterations;
        self.cpu_samples.push(r.cpu_ms);
    }
    fn mean_cpu(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.cpu_ms as f64 / self.calls as f64
        }
    }
    fn mean_iters(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.iterations as f64 / self.calls as f64
        }
    }
}

#[derive(Debug, Default)]
struct MatchStats {
    rounds: i32,
    game_seconds: f64,
    capped: bool,
    engine_cpu_ms: u64,
    setup_cpu_ms: u64,
    /// Game seconds spent re-probing a refused answer (each refusal costs one
    /// 200 ms re-probe cycle of a stalled seat).
    stall_seconds: f64,
    reqs: Vec<ReqRec>,
}

/// One recorded view, re-searched afterwards with `profile: true` for the
/// inside-the-search split.
struct AnatomySample {
    view: BotSeatView,
    seat: usize,
    member: i32,
    budget_ms: u64,
    seed: u64,
}

// ------------------------------------------------------------- wire + clock

/// One request exactly as the `bot-service` child sees it: parse the line,
/// run `handle`, serialise the reply. The CPU window covers only this -- the
/// request's own `to_string` is the *server's* CPU in production.
fn wire_call(ctx: &Ctx, line: &str) -> (Value, Duration, Duration, String) {
    let c0 = process_cpu();
    let w0 = Instant::now();
    let req: Value = serde_json::from_str(line).expect("request line parses");
    let out = handle(ctx, req);
    let out_line = out.to_string();
    let wall = w0.elapsed();
    let cpu = process_cpu() - c0;
    (out, cpu, wall, out_line)
}

/// Advance the match clock (and engine) by `secs`, in the ticker's quanta --
/// the live ticker keeps running while a search is in flight.
fn catch_up(m: &mut Match, secs: f64, t: &mut f64, engine_cpu_ms: &mut u64) {
    let mut left = secs;
    while left > 1e-9 {
        let dt = left.min(0.5) as f32;
        let c0 = process_cpu();
        m.tick(dt);
        *engine_cpu_ms += ms(process_cpu().saturating_sub(c0));
        *t += dt as f64;
        left -= dt as f64;
    }
}

fn engine_tick(m: &mut Match, dt: f32, t: &mut f64, engine_cpu_ms: &mut u64) {
    let c0 = process_cpu();
    m.tick(dt);
    *engine_cpu_ms += ms(process_cpu().saturating_sub(c0));
    *t += dt as f64;
}

// ------------------------------------------------------------------- driver

fn run_match(
    ctx: &Ctx,
    seed: u64,
    max_rounds: i32,
    max_game_seconds: f64,
    budget_override: Option<u64>,
    ponder_budget_ms: u64,
    with_ponder: bool,
    ponder_all: bool,
    anatomy: &mut Vec<AnatomySample>,
    anatomy_cap: usize,
) -> MatchStats {
    let members = vec![
        RoomMember {
            id: 1,
            player: "Adv1".into(),
            bot: true,
            mentality: BotMentality::Advanced,
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "Adv2".into(),
            bot: true,
            mentality: BotMentality::Advanced,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "Adv3".into(),
            bot: true,
            mentality: BotMentality::Advanced,
            ..Default::default()
        },
        RoomMember {
            id: 4,
            player: "Std".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];
    let mut m = Match::new(
        ctx.data.clone(),
        ctx.rules.clone(),
        &members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    let c0 = process_cpu();
    m.quick_start();
    let mut st = MatchStats {
        setup_cpu_ms: ms(process_cpu().saturating_sub(c0)),
        ..Default::default()
    };

    let advanced: Vec<i32> = vec![1, 2, 3];
    let n = advanced.len();
    let mut next_probe = vec![0.0f64; n];
    let mut t = 0.0f64;
    let mut req_id = 1u64;
    let mut last_round_check = -1.0f64;
    let mut rng = seed ^ 0xB07_5EED;

    loop {
        if m.ended() {
            break;
        }
        if t >= max_game_seconds {
            st.capped = true;
            break;
        }
        if t - last_round_check >= 1.0 {
            last_round_check = t;
            let s = m.state();
            st.rounds = s.round;
            if s.round > max_rounds {
                st.capped = true;
                break;
            }
        }

        // One probe pass per due seat (a long request makes the other seats
        // overdue; they drain one per pass exactly like the live 200 ms
        // cadence would have fired them).
        let mut fired = false;
        for si in 0..n {
            if t + 1e-9 < next_probe[si] {
                continue;
            }
            fired = true;
            let member = advanced[si];
            let c0 = process_cpu();
            let view = view_frame(&m, member);
            let view_cpu = ms(process_cpu().saturating_sub(c0));
            st.engine_cpu_ms += view_cpu;
            let Ok(seat) = serde_json::from_value::<BotSeatView>(view.clone()) else {
                next_probe[si] = t + 0.2;
                continue;
            };
            let surface = surface_label(&seat);
            let prompt_id = botsvc::decision_at(&seat.state, seat.player_id);

            let Some(pid) = prompt_id else {
                // Nothing to decide: the live `spawn_ponder` path. Gated on
                // the seat's next own decision being near (`docs/BOT.md`
                // §3.5) -- an idle view has no searchable surface, and the
                // pre-C1 polls searched 0 of ~14 k ponders (`REPORT.md` §6).
                // `--ponder-all` restores the old "every idle probe" traffic
                // for a before/after from one binary.
                let near = bot_service::next_turn_near(&seat.state, seat.player_id);
                if seat.state.phase == "play" && with_ponder && (near || ponder_all) {
                    let req = ponder_request(req_id, "bench", member, &view, ponder_budget_ms, rng);
                    req_id += 1;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let line = req.to_string();
                    let view_bytes = line.len();
                    let (out, cpu, wall, _line) = wire_call(ctx, &line);
                    let iterations = out
                        .get("iterations")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let reused = out.get("reused").and_then(Value::as_bool).unwrap_or(false);
                    let heuristic =
                        out.get("heuristic").and_then(Value::as_bool).unwrap_or(false);
                    st.reqs.push(ReqRec {
                        ponder: true,
                        surface,
                        searched: !heuristic && !reused,
                        delegated: heuristic && !reused,
                        reused,
                        iterations,
                        budget_ms: ponder_budget_ms,
                        wall_ms: ms(wall),
                        cpu_ms: ms(cpu),
                        root_actions: 0,
                        agrees_heuristic: false,
                        view_bytes,
                        would_fallback: false,
                        act_refused: false,
                        refuse_reason: String::new(),
                    });
                    catch_up(&mut m, wall.as_secs_f64(), &mut t, &mut st.engine_cpu_ms);
                    // Phase-preserving cadence: overdue slots drain over the
                    // next passes (the live probe fires every 200 ms).
                    next_probe[si] += 0.2;
                    if next_probe[si] < t - 4.0 {
                        next_probe[si] = t + 0.2;
                    }
                } else {
                    next_probe[si] = t + 0.2;
                }
                continue;
            };

            // The seat's own decision: the live `decide` path.
            // The turn bank is gone: `tick_play`'s `timed_out` path hands the
            // seat to the engine's own `Routine::Ai`. Do not double-drive it.
            if pid == 0 && seat.state.time_left <= 0.0 {
                next_probe[si] = t + 0.2;
                continue;
            }
            let budget = budget_override.unwrap_or_else(|| botsvc::budget_ms(&seat.state, pid));
            let me = seat.player_id.max(0) as usize;
            let root_actions = bot_service::bot_action::legal_actions_with_cost(
                &ctx.data,
                &seat.state,
                &seat.hand,
                &seat.playable,
                &seat.est_cost,
                me,
            )
            .len();
            let heuristic_msg = heuristic_message_view(&ctx.data, &seat);
            let req = decide_request(req_id, "bench", member, &view, pid, budget, rng);
            req_id += 1;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let line = req.to_string();
            let view_bytes = line.len();
            let (out, cpu, wall, _line) = wire_call(ctx, &line);
            let parsed = BotAnswer::from_response(&out);
            let mut act_refused = false;
            let (iterations, searched, delegated, reused, answer) = match &parsed {
                Ok(a) => (
                    a.iterations,
                    !a.heuristic && !a.reused,
                    a.heuristic && !a.reused,
                    a.reused,
                    Some(a.answer.clone()),
                ),
                Err(_) => (0, false, true, false, None),
            };
            st.reqs.push(ReqRec {
                ponder: false,
                surface: surface.clone(),
                searched,
                delegated,
                reused,
                iterations,
                budget_ms: budget,
                wall_ms: ms(wall),
                cpu_ms: ms(cpu),
                root_actions,
                agrees_heuristic: answer
                    .as_ref()
                    .is_some_and(|a| *a == heuristic_msg),
                view_bytes,
                would_fallback: wall > botsvc::ask_timeout(budget),
                act_refused: false,
                refuse_reason: String::new(),
            });
            // The live ticker ran during the search; replay it before applying
            // so prompt-expiry races behave the same.
            catch_up(&mut m, wall.as_secs_f64(), &mut t, &mut st.engine_cpu_ms);

            let apply = match &parsed {
                Ok(a) => a.answer.clone(),
                Err(_) => heuristic_msg.clone(),
            };
            let c0 = process_cpu();
            let applied = m.act(member, &apply);
            st.engine_cpu_ms += ms(process_cpu().saturating_sub(c0));
            match applied {
                Ok(()) => {
                    if let Some(last) = st.reqs.last_mut() {
                        last.act_refused = false;
                    }
                }
                Err(e) => {
                    // Mirror the live drive (`server::apply_bot_answer`,
                    // docs/BOT.md §5 B6): never re-ask -- drop the cached
                    // answer and answer with the heuristic right now.
                    act_refused = true;
                    if let Some(last) = st.reqs.last_mut() {
                        last.act_refused = true;
                        last.refuse_reason = e.k.to_string();
                    }
                    let c0 = process_cpu();
                    let fallback = m.act(member, &heuristic_msg);
                    st.engine_cpu_ms += ms(process_cpu().saturating_sub(c0));
                    if fallback.is_err() {
                        // Even the heuristic was refused: the seat is stuck
                        // for one re-probe cycle.
                        st.stall_seconds += 0.2;
                        act_refused = true;
                    } else {
                        act_refused = false;
                    }
                }
            }

            // Record an anatomy sample for the searched decide.
            if searched && anatomy.len() < anatomy_cap {
                anatomy.push(AnatomySample {
                    view: seat,
                    seat: me,
                    member,
                    budget_ms: budget,
                    seed: rng,
                });
            }

            next_probe[si] = t + if act_refused { 0.2 } else { 0.05 };
        }
        if fired {
            continue;
        }
        // Nothing due: advance the engine to the next probe deadline.
        let next = next_probe
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(t + 0.05);
        let dt = ((next - t) as f32).clamp(0.05, 0.2);
        engine_tick(&mut m, dt, &mut t, &mut st.engine_cpu_ms);
    }

    let s = m.state();
    st.rounds = s.round;
    st.game_seconds = t;
    st
}

// ------------------------------------------------------------------- report

fn agg_by<F: Fn(&ReqRec) -> bool>(reqs: &[ReqRec], f: F) -> Agg {
    let mut a = Agg::default();
    for r in reqs {
        if f(r) {
            a.add(r);
        }
    }
    a
}

fn print_agg(label: &str, a: &Agg) {
    println!(
        "  {:<22} {:>5} calls  {:>9.1} CPU-s  {:>8.1} ms/call  iters {:>5.2}",
        label,
        a.calls,
        a.cpu_ms as f64 / 1000.0,
        a.mean_cpu(),
        a.mean_iters()
    );
}

fn main() {
    alloc_hook::init();
    let data_dir = std::path::PathBuf::from(arg("--data", "data"));
    let rules_dir = std::path::PathBuf::from(arg("--rules", "dist/cards"));
    let matches: usize = arg("--matches", "1").parse().unwrap_or(1).max(1);
    let seed0: u64 = arg("--seed", "1").parse().unwrap_or(1);
    let search_threads: usize = arg("--search-threads", "4")
        .parse()
        .unwrap_or(4)
        .max(1);
    let max_rounds: i32 = arg("--max-rounds", "30").parse().unwrap_or(30);
    let max_game_seconds: f64 = arg("--max-game-seconds", "1800")
        .parse()
        .unwrap_or(1800.0);
    let budget_override: Option<u64> = arg("--budget-ms", "").parse().ok().filter(|&b| b > 0);
    let ponder_budget_ms: u64 = arg("--ponder-budget-ms", "300")
        .parse()
        .unwrap_or(botsvc::PONDER_BUDGET_MS);
    let with_ponder = !flag("--no-ponder");
    // `--ponder-all`: the pre-C1 behaviour -- a `ponder` on every idle probe,
    // not only when the seat's next own decision is near. For a before/after
    // from one binary.
    let ponder_all = flag("--ponder-all");
    // `--no-early-stop`: the pre-C2 search, always the full budget. For a
    // before/after of the time-management rule from one binary.
    let early_stop = !flag("--no-early-stop");
    let anatomy_cap: usize = arg("--anatomy", "8").parse().unwrap_or(8);

    let ctx = Ctx::load(&data_dir, &rules_dir).with_opts(SearchOpts {
        search_threads,
        early_stop,
        ..SearchOpts::default()
    });
    let real = ctx.rules.ruleset_sha256().is_some();
    println!("=== bot-service CPU profile (measurement only) ===");
    println!(
        "ruleset: {} ({}), search-threads {}, ponder {}, ponder-budget {} ms, early-stop {}",
        ctx.rules.ruleset_sha256().unwrap_or("stub"),
        if real { "real WasmRules" } else { "STUB -- no effects" },
        search_threads,
        if with_ponder { "on" } else { "off" },
        ponder_budget_ms,
        if early_stop { "on" } else { "off" },
    );
    println!(
        "table: 3 advanced + 1 standard bot, MatchMode::Casual; budget policy {}",
        if let Some(b) = budget_override {
            format!("override {b} ms")
        } else {
            "server defaults (turn 800 ms / prompt min(1000, left-200))".to_string()
        }
    );
    println!(
        "caps: {} match(es), {} rounds, {} game-seconds; anatomy sample {}",
        matches, max_rounds, max_game_seconds, anatomy_cap
    );

    let mut all: Vec<ReqRec> = Vec::new();
    let mut per_match: Vec<MatchStats> = Vec::new();
    let mut anatomy: Vec<AnatomySample> = Vec::new();
    let wall0 = Instant::now();

    for g in 0..matches {
        let seed = seed0.wrapping_add(g as u64 * 97);
        eprintln!("--- match {} (seed {seed}) ---", g + 1);
        let st = run_match(
            &ctx,
            seed,
            max_rounds,
            max_game_seconds,
            budget_override,
            ponder_budget_ms,
            with_ponder,
            ponder_all,
            &mut anatomy,
            anatomy_cap,
        );
        eprintln!(
            "--- match {}: rounds {}, game-time {:.0}s, {} requests, {:.1} bot CPU-s ---",
            g + 1,
            st.rounds,
            st.game_seconds,
            st.reqs.len(),
            st.reqs.iter().map(|r| r.cpu_ms).sum::<u64>() as f64 / 1000.0
        );
        all.extend(st.reqs.iter().cloned());
        per_match.push(st);
    }
    let harness_wall = wall0.elapsed();

    // ------------------------------------------------------------- totals
    let game_seconds: f64 = per_match.iter().map(|m| m.game_seconds).sum();
    let stall_seconds: f64 = per_match.iter().map(|m| m.stall_seconds).sum();
    let engine_cpu_ms: u64 = per_match.iter().map(|m| m.engine_cpu_ms).sum();
    let setup_cpu_ms: u64 = per_match.iter().map(|m| m.setup_cpu_ms).sum();
    let bot_cpu_ms: u64 = all.iter().map(|r| r.cpu_ms).sum();

    println!();
    println!("-- per match --");
    for (i, m) in per_match.iter().enumerate() {
        let cpu: u64 = m.reqs.iter().map(|r| r.cpu_ms).sum();
        println!(
            "  match {}: rounds {}{}, game-time {:.0}s (stalled {:.0}s), bot CPU {:.1} s = {:.1}% of one core; engine CPU {:.1} s; requests {}",
            i + 1,
            m.rounds,
            if m.capped { " (capped)" } else { " (ended)" },
            m.game_seconds,
            m.stall_seconds,
            cpu as f64 / 1000.0,
            if m.game_seconds > 0.0 {
                100.0 * cpu as f64 / 1000.0 / m.game_seconds
            } else {
                0.0
            },
            m.engine_cpu_ms as f64 / 1000.0,
            m.reqs.len(),
        );
    }

    println!();
    println!("-- totals over {} match(es) --", matches);
    println!(
        "  match time (game = wall at time_scale 1): {:.0} s   harness wall {:.0} s",
        game_seconds,
        harness_wall.as_secs_f64()
    );
    println!(
        "  bot-service CPU: {:.1} CPU-s  = {:.1}% of one core over match time",
        bot_cpu_ms as f64 / 1000.0,
        if game_seconds > 0.0 {
            100.0 * bot_cpu_ms as f64 / 1000.0 / game_seconds
        } else {
            0.0
        }
    );
    println!(
        "  stalled game time (refused answers re-probed): {:.0} s = {:.0}% of match time",
        stall_seconds,
        if game_seconds > 0.0 {
            100.0 * stall_seconds / game_seconds
        } else {
            0.0
        }
    );
    if game_seconds - stall_seconds > 1.0 {
        println!(
            "  = {:.1}% of one core over non-stalled match time",
            100.0 * bot_cpu_ms as f64 / 1000.0 / (game_seconds - stall_seconds)
        );
    }
    println!(
        "  engine/harness CPU: {:.1} CPU-s (view+tick+act)   setup {:.1} CPU-s",
        engine_cpu_ms as f64 / 1000.0,
        setup_cpu_ms as f64 / 1000.0
    );
    println!("  requests: {}", all.len());

    let decide = agg_by(&all, |r| !r.ponder);
    let ponder = agg_by(&all, |r| r.ponder);
    let searched = agg_by(&all, |r| r.searched);
    let delegated = agg_by(&all, |r| r.delegated && !r.ponder);
    let reused = agg_by(&all, |r| r.reused && !r.ponder);
    let ponder_delegated = agg_by(&all, |r| r.ponder && r.delegated);

    println!();
    println!("-- decide vs ponder --");
    print_agg("decide (all)", &decide);
    print_agg("  searched", &searched);
    print_agg("  delegated", &delegated);
    print_agg("  reused (cache)", &reused);
    print_agg("ponder (all)", &ponder);
    print_agg("  (of which no-op)", &ponder_delegated);
    let ponder_searched = agg_by(&all, |r| r.ponder && r.searched);
    println!(
        "  ponder actually searched: {} of {} (rest are parse+hash no-ops)",
        ponder_searched.calls, ponder.calls
    );

    // ------------------------------------------------------- per surface
    let mut surfaces: HashMap<String, Agg> = HashMap::new();
    for r in &all {
        surfaces.entry(r.surface.clone()).or_default().add(r);
    }
    let mut rows: Vec<(String, Agg)> = surfaces.into_iter().collect();
    rows.sort_by(|a, b| b.1.cpu_ms.cmp(&a.1.cpu_ms));
    println!();
    println!("-- per surface kind (all requests) --");
    println!(
        "  {:<34} {:>5} {:>9} {:>9} {:>6} {:>6} {:>7}",
        "surface", "calls", "CPU-s", "ms/call", "iters", "roots", "agree%"
    );
    for (name, a) in &rows {
        let roots: f64 = {
            let rs: Vec<_> = all
                .iter()
                .filter(|r| &r.surface == name && r.searched)
                .collect();
            if rs.is_empty() {
                0.0
            } else {
                rs.iter().map(|r| r.root_actions as f64).sum::<f64>() / rs.len() as f64
            }
        };
        let agree: f64 = {
            let rs: Vec<_> = all
                .iter()
                .filter(|r| &r.surface == name && r.searched)
                .collect();
            if rs.is_empty() {
                0.0
            } else {
                100.0
                    * rs.iter().filter(|r| r.agrees_heuristic).count() as f64
                    / rs.len() as f64
            }
        };
        println!(
            "  {:<34} {:>5} {:>9.1} {:>9.1} {:>6.2} {:>6.1} {:>6.0}%",
            name,
            a.calls,
            a.cpu_ms as f64 / 1000.0,
            a.mean_cpu(),
            a.mean_iters(),
            roots,
            agree
        );
    }

    // ------------------------------------------------------ distributions
    let mut searched_cpu: Vec<u64> = all
        .iter()
        .filter(|r| r.searched)
        .map(|r| r.cpu_ms)
        .collect();
    searched_cpu.sort_unstable();
    println!();
    println!("-- searched decide CPU distribution --");
    if searched_cpu.is_empty() {
        println!("  (none)");
    } else {
        println!(
            "  n {}  mean {:.0} ms  p50 {}  p90 {}  p99 {}  max {} ms",
            searched_cpu.len(),
            searched_cpu.iter().sum::<u64>() as f64 / searched_cpu.len() as f64,
            percentile(&searched_cpu, 0.5),
            percentile(&searched_cpu, 0.9),
            percentile(&searched_cpu, 0.99),
            searched_cpu.last().unwrap()
        );
        println!(
            "  CPU/wall ratio (thread fan-out): {:.2}x  (search_threads = {})",
            searched_cpu.iter().sum::<u64>() as f64
                / all
                    .iter()
                    .filter(|r| r.searched)
                    .map(|r| r.wall_ms.max(1))
                    .sum::<u64>() as f64,
            search_threads
        );
    }

    // --------------------------------------------------------- suspects
    let one_action = agg_by(&all, |r| r.searched && r.root_actions <= 1);
    let obvious = agg_by(&all, |r| r.searched && r.agrees_heuristic);
    let fallback_risk = all.iter().filter(|r| r.would_fallback).count();
    let refused = all.iter().filter(|r| r.act_refused).count();
    let view_bytes_mean = if all.is_empty() {
        0.0
    } else {
        all.iter().map(|r| r.view_bytes).sum::<usize>() as f64 / all.len() as f64
    };

    println!();
    println!("-- where the CPU goes (suspects, quantified) --");
    println!(
        "  1. searched decides: {:.1} CPU-s = {:.1}% of bot CPU ({} calls, {:.0} ms CPU each)",
        searched.cpu_ms as f64 / 1000.0,
        pct(searched.cpu_ms, bot_cpu_ms),
        searched.calls,
        searched.mean_cpu()
    );
    println!(
        "     thread fan-out: {:.1} CPU-s per decide at {} search threads x ~{:.0} ms wall",
        searched.mean_cpu() / 1000.0,
        search_threads,
        if searched.calls == 0 {
            0.0
        } else {
            searched.wall_ms as f64 / searched.calls as f64
        }
    );
    println!(
        "  2. idle ponders: {} calls, {:.1} CPU-s = {:.1}% of bot CPU ({:.2} ms each, {} searched)",
        ponder.calls,
        ponder.cpu_ms as f64 / 1000.0,
        pct(ponder.cpu_ms, bot_cpu_ms),
        ponder.mean_cpu(),
        ponder_searched.calls
    );
    println!(
        "     (the live spawn_ponder path fires every 200 ms per idle advanced seat)"
    );
    println!(
        "  3. obvious searched decides (1 legal action): {} calls, {:.1} CPU-s = {:.1}%",
        one_action.calls,
        one_action.cpu_ms as f64 / 1000.0,
        pct(one_action.cpu_ms, bot_cpu_ms)
    );
    println!(
        "  4. searched decides where the pick == the heuristic pick: {} of {} ({:.0}%), {:.1} CPU-s = {:.1}%",
        obvious.calls,
        searched.calls,
        if searched.calls > 0 {
            100.0 * obvious.calls as f64 / searched.calls as f64
        } else {
            0.0
        },
        obvious.cpu_ms as f64 / 1000.0,
        pct(obvious.cpu_ms, bot_cpu_ms)
    );
    println!(
        "  5. budget policy: mean budget {:.0} ms vs mean wall {:.0} ms for searched ({:.0}% of budget spent); mean {} iterations",
        if searched.calls == 0 {
            0.0
        } else {
            all.iter()
                .filter(|r| r.searched)
                .map(|r| r.budget_ms as f64)
                .sum::<f64>()
                / searched.calls as f64
        },
        if searched.calls == 0 {
            0.0
        } else {
            searched.wall_ms as f64 / searched.calls as f64
        },
        if searched.calls == 0 {
            0.0
        } else {
            100.0
                * (searched.wall_ms as f64 / searched.calls as f64)
                / (all
                    .iter()
                    .filter(|r| r.searched)
                    .map(|r| r.budget_ms as f64)
                    .sum::<f64>()
                    / searched.calls as f64)
        },
        searched.mean_iters()
    );
    println!(
        "  6. deadline risk (wall > ask_timeout): {} of {} would fall back; act refused {}",
        fallback_risk,
        all.len(),
        refused
    );
    if refused > 0 {
        let mut reasons: HashMap<String, usize> = HashMap::new();
        for r in all.iter().filter(|r| r.act_refused) {
            *reasons
                .entry(format!("{} [{}]", r.refuse_reason, r.surface))
                .or_default() += 1;
        }
        let mut reasons: Vec<(String, usize)> = reasons.into_iter().collect();
        reasons.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in reasons.iter().take(12) {
            println!("       refused x{n}: {k}");
        }
    }
    println!(
        "  7. view wire size: mean {} bytes/request (parse+hash inside bot-service)",
        view_bytes_mean
    );

    // ---------------------------------------------------------- anatomy
    println!();
    println!(
        "-- search anatomy ({} sampled searched decides, re-run with profile) --",
        anatomy.len()
    );
    if anatomy.is_empty() {
        println!("  (no searched decide sampled)");
    } else {
        use bot_core::{Ismcts, MatchSim, SearchConfig};
        let mut fork = Duration::ZERO;
        let mut descent = Duration::ZERO;
        let mut rollout = Duration::ZERO;
        let mut evaluate = Duration::ZERO;
        let mut key = Duration::ZERO;
        let mut total_wall = Duration::ZERO;
        let mut total_iters = 0u64;
        let c0 = process_cpu();
        for s in &anatomy {
            let cfg = SearchConfig {
                budget: Duration::from_millis(s.budget_ms),
                seed: s.seed,
                horizon_rounds: ctx.opts.horizon_rounds,
                eval_weight: ctx.opts.eval_weight,
                implicit_minimax: ctx.opts.implicit_minimax,
                bias_weight: ctx.opts.bias_weight,
                profile: true,
                ..Default::default()
            };
            let mut tree = Ismcts::new();
            let data = ctx.data.clone();
            let rules = ctx.rules.clone();
            let v = s.view.clone();
            let (seat, member) = (s.seat, s.member);
            let factory = move || MatchSim::new(data.clone(), rules.clone(), v.clone(), seat, member);
            let out = tree.search_root_parallel(factory, seat, cfg, search_threads);
            total_wall += out.elapsed;
            total_iters += out.iterations;
            if let Some(tm) = out.timings {
                fork += tm.fork;
                descent += tm.descent;
                rollout += tm.rollout;
                evaluate += tm.evaluate;
                key += tm.key;
            }
        }
        let cpu_anatomy = ms(process_cpu().saturating_sub(c0));
        let sum = (fork + descent + rollout + evaluate + key).as_secs_f64().max(1e-9);
        let row = |name: &str, d: Duration| {
            println!(
                "  {:<10} {:>8.2} CPU-s  {:>5.1}%   ({:.0} ms/sample)",
                name,
                d.as_secs_f64(),
                100.0 * d.as_secs_f64() / sum,
                d.as_secs_f64() * 1000.0 / anatomy.len() as f64
            );
        };
        row("fork", fork);
        row("descent", descent);
        row("rollout", rollout);
        row("evaluate", evaluate);
        row("key", key);
        println!(
            "  total {:.1} CPU-s over {} samples ({} iterations, {:.0} ms wall each); harness CPU for the pass {:.1} s",
            sum,
            anatomy.len(),
            total_iters,
            total_wall.as_secs_f64() * 1000.0 / anatomy.len() as f64,
            cpu_anatomy as f64 / 1000.0
        );
    }

    // ------------------------------------------------------- summary json
    let summary = serde_json::json!({
        "matches": matches,
        "game_seconds": game_seconds,
        "stall_seconds": stall_seconds,
        "bot_cpu_s": bot_cpu_ms as f64 / 1000.0,
        "bot_pct_of_core": if game_seconds > 0.0 { 100.0 * bot_cpu_ms as f64 / 1000.0 / game_seconds } else { 0.0 },
        "engine_cpu_s": engine_cpu_ms as f64 / 1000.0,
        "requests": all.len(),
        "decides": decide.calls,
        "decide_searched": searched.calls,
        "decide_delegated": delegated.calls,
        "decide_reused": reused.calls,
        "ponder_calls": ponder.calls,
        "ponder_noop": ponder_delegated.calls,
        "ponder_searched": ponder_searched.calls,
        "searched_cpu_s": searched.cpu_ms as f64 / 1000.0,
        "ponder_cpu_s": ponder.cpu_ms as f64 / 1000.0,
        "searched_mean_cpu_ms": searched.mean_cpu(),
        "searched_mean_iters": searched.mean_iters(),
        "one_action_calls": one_action.calls,
        "obvious_calls": obvious.calls,
        "would_fallback": fallback_risk,
        "act_refused": refused,
    });
    println!();
    println!("SUMMARY {summary}");
    {
        let rows = game_core::engine::rtimer::snapshot();
        println!("-- engine rtimer buckets (whole run) --");
        for (k, ns) in rows {
            println!("  {k:<12} {:>8.2} s", ns as f64 / 1e9);
        }
    }
    dump_bot_cost();
    alloc_hook::dump();
}

/// `--features bot-cost`: the engine/rules measurement counters over the whole
/// run (`docs/BOT.md` §5). Compiled out otherwise.
#[cfg(feature = "bot-cost")]
fn dump_bot_cost() {
    use game_core::engine::bot_cost as gc;
    use game_rules::bot_cost as gr;
    use game_rules::cond_pre::guard_cost as cp;
    let g = |v: &std::sync::atomic::AtomicU64| v.load(std::sync::atomic::Ordering::Relaxed);
    println!("-- bot-cost counters (whole run) --");
    println!(
        "  world copies {}  shares {}  cant_play {} (memo hits {})",
        g(&gc::WORLD_CLONES),
        g(&gc::WORLD_SHARES),
        g(&gc::CANT_PLAY_CALLS),
        g(&gc::CANT_PLAY_MEMO_HITS)
    );
    println!(
        "  wasm: instantiations {}  instantiate {:.1}s  store {:.1}s  guest {:.1}s  host-world-clones {}",
        g(&gr::INSTANTIATIONS),
        g(&gr::INSTANTIATE_NS) as f64 / 1e9,
        g(&gr::STORE_NS) as f64 / 1e9,
        g(&gr::GUEST_NS) as f64 / 1e9,
        g(&gr::HOST_WORLD_CLONES)
    );
    println!(
        "  counteract: windows {} (skipped {})  probes {} (memo {})  declared {}",
        g(&gr::COUNTERACT_WINDOWS),
        g(&gr::COUNTERACT_WINDOWS_SKIPPED),
        g(&gr::COUNTERACT_PROBES),
        g(&gr::COUNTERACT_PROBE_MEMO_HITS),
        g(&gr::COUNTERACT_DECLARED)
    );
    println!(
        "  cond_pre: evals {}  skipped-by-condition {}  guard-asked {}  cond-eval {:.1}s",
        g(&cp::COND_EVALS),
        g(&cp::SKIPPED_BY_CONDITION),
        g(&cp::GUARD_ASKED),
        g(&cp::COND_EVAL_NS) as f64 / 1e9
    );
}

#[cfg(not(feature = "bot-cost"))]
fn dump_bot_cost() {}

fn pct(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}