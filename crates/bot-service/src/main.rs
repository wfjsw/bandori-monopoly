//! Advanced-bot decision service -- one long-running process, many requests.
//!
//! Transport matches `rules-worker`: newline-delimited JSON on stdin/stdout,
//! one response per request, `id` echoed so replies can come back out of order.
//! Unlike the match worker this one **pipelines**: the reader hands each
//! request to a search thread and keeps reading, so N concurrent decisions
//! (one per room) run on N threads inside one process.
//!
//! ```json
//! {"id":1,"op":"ping"}
//! {"id":2,"op":"decide","room":"R","seat":1,"view":{...},"prompt_id":0,"budget_ms":200,"seed":7}
//! {"id":3,"op":"ponder","room":"R","seat":1,"view":{...},"budget_ms":300,"seed":7}
//! ```
//!
//! ```
//! cargo run -p bot-service -- --data data --rules dist/cards --threads 4
//! ```
//!
//! Environment: `BM_DATA`, `BM_RULES`, `BOT_THREADS`, `BOT_SEARCH_THREADS`,
//! `BOT_BIAS_WEIGHT`, `BOT_EVAL_WEIGHT`, `BOT_HORIZON`, `BOT_LEGACY`.
//!
//! Flags (each also has an env fallback):
//!
//! * `--threads N` / `BOT_THREADS` -- request-handling workers (default 4).
//! * `--search-threads N` / `BOT_SEARCH_THREADS` -- root-parallel searches
//!   per decision (default = `--threads`). One decision may use several
//!   threads; the per-request deadline is unchanged.
//! * `--bias-weight F` / `BOT_BIAS_WEIGHT` -- progressive bias `c_h` (0 = off).
//! * `--eval-weight F` / `BOT_EVAL_WEIGHT` -- heuristic-eval mix `α` (0 = off).
//! * `--horizon N` / `BOT_HORIZON` -- rollout horizon in rounds (default 2).
//! * `--legacy` / `BOT_LEGACY=1` -- the pre-#3 search (no bias / eval mix /
//!   tree reuse / ponder cache), for A/B.
//! * `--no-reuse` / `--no-ponder` -- disable tree reuse / the ponder cache.
//!
//! Per-request hard deadline: the search is anytime and stops at
//! `budget_ms` (capped at [`bot_service::MAX_BUDGET_MS`]); the server also
//! applies its own timeout and falls back to the heuristic when this process
//! is slow or gone. A wedged request therefore costs at most one thread for
//! the budget, and never stalls a match.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use bot_service::{handle, Ctx, SearchOpts};
use serde_json::Value;

fn arg(name: &str, env: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| std::env::var(env).ok())
        .unwrap_or_else(|| default.to_string())
}

fn flag(name: &str) -> bool {
    let args: Vec<String> = std::env::args().collect();
    args.iter().any(|a| a == name)
}

fn main() {
    let data_dir = PathBuf::from(arg("--data", "BM_DATA", "data"));
    let rules_dir = PathBuf::from(arg("--rules", "BM_RULES", "dist/cards"));
    let threads: usize = arg("--threads", "BOT_THREADS", "4")
        .parse()
        .unwrap_or(4)
        .max(1);
    let legacy = flag("--legacy") || arg("--legacy", "BOT_LEGACY", "0") == "1";

    let mut opts = if legacy {
        SearchOpts::legacy()
    } else {
        SearchOpts {
            bias_weight: arg("--bias-weight", "BOT_BIAS_WEIGHT", "0.4")
                .parse()
                .unwrap_or(0.4),
            eval_weight: arg("--eval-weight", "BOT_EVAL_WEIGHT", "0.3")
                .parse()
                .unwrap_or(0.3),
            implicit_minimax: arg("--implicit-minimax", "BOT_IMPLICIT_MINIMAX", "1") != "0",
            horizon_rounds: arg("--horizon", "BOT_HORIZON", "2").parse().unwrap_or(2),
            reuse_trees: !flag("--no-reuse"),
            accept_ponder: !flag("--no-ponder"),
            cache_cap: 256,
            ..SearchOpts::default()
        }
    };
    // Root-parallel is orthogonal to the selection flags: always honour the
    // thread count (default = the request-worker count). `--legacy` keeps it
    // at 1 unless the flag is given explicitly.
    opts.search_threads = if legacy {
        arg("--search-threads", "BOT_SEARCH_THREADS", "1")
            .parse()
            .unwrap_or(1)
            .max(1)
    } else {
        arg("--search-threads", "BOT_SEARCH_THREADS", &threads.to_string())
            .parse()
            .unwrap_or(threads)
            .max(1)
    };
    let ctx = Arc::new(Ctx::load(&data_dir, &rules_dir).with_opts(opts.clone()));
    eprintln!(
        "bot-service: {} request thread(s), {} search thread(s)/decision, data {}, rules {}, \
         bias {}, eval {}, horizon {}, reuse {}, ponder {}",
        threads,
        opts.search_threads,
        data_dir.display(),
        rules_dir.display(),
        opts.bias_weight,
        opts.eval_weight,
        opts.horizon_rounds,
        opts.reuse_trees,
        opts.accept_ponder,
    );

    // Responses are written in completion order, not request order; `id`
    // disambiguates. One writer lock keeps lines whole.
    let stdout = Arc::new(Mutex::new(io::stdout()));
    let (tx, rx) = mpsc::channel::<String>();
    // The receiver is shared by the worker threads via the channel's own
    // clone-free design: one consumer thread hands jobs out is wrong (that
    // serialises), so each worker pulls from an `mpsc::Receiver` behind a
    // mutex -- the standard fan-out pattern.
    let rx = Arc::new(Mutex::new(rx));

    let mut workers = Vec::with_capacity(threads);
    for _ in 0..threads {
        let ctx = ctx.clone();
        let rx = rx.clone();
        let stdout = stdout.clone();
        workers.push(std::thread::spawn(move || {
            loop {
                let line = {
                    let guard = rx.lock().unwrap();
                    guard.recv()
                };
                let Ok(line) = line else { break };
                let out = match serde_json::from_str::<Value>(&line) {
                    Ok(req) => handle(&ctx, req),
                    // A bad request is answered, not fatal: the process stays
                    // up and keeps serving the pool.
                    Err(e) => serde_json::json!({"ok": false, "error": format!("bad request json: {e}")}),
                };
                let text = out.to_string();
                let mut w = stdout.lock().unwrap();
                let _ = writeln!(w, "{text}");
                let _ = w.flush();
            }
        }));
    }

    // Reader: stdin closed = a clean exit, not a crash. Drop `tx` so the
    // workers drain and join.
    let handled = AtomicU64::new(0);
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        handled.fetch_add(1, Ordering::Relaxed);
        if tx.send(line).is_err() {
            break;
        }
    }
    drop(tx);
    for w in workers {
        let _ = w.join();
    }
    eprintln!(
        "bot-service: exit after {} request(s)",
        handled.load(Ordering::Relaxed)
    );
}