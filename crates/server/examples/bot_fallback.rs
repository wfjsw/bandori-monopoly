//! Fallback-rate measurement for the advanced-bot decision path, on the
//! **real** ruleset (`dist/cards` via `WasmRules`) at the server's default
//! budget / deadline policy (`docs/BOT.md` §3.5, §5 "B7 server wiring").
//!
//! ```
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!   cargo run -p server --release --example bot_fallback -- \
//!     --data data --rules dist/cards --decisions 12 --search-threads 4
//! ```
//!
//! Parks 4-seat matches at every decision one seat faces (searchable and
//! heuristic-delegated), sends `decide` with the server's budget, and counts
//! how many answers arrive inside the outer deadline (the service answers)
//! versus outside it (the server would fall back to the engine heuristic).
//! Both deadline policies are scored off the same latencies: the shipped one
//! (`budget + 1.5 s`, `server::botsvc::ask_timeout`) and the old
//! `budget + 400 ms` capped at 1.5 s that fired routinely on 4-thread
//! searches. `--ponder` sends a speculative `ponder` before each decide.

use std::time::{Duration, Instant};

use bot_service::{decide_request, handle, ponder_request, BotAnswer, Ctx};
use game_core::engine::Match;
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality};
use game_core::MatchMode;
use serde_json::{json, Value};

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

/// The exact frame the server sends (`rules-worker`'s `view` op).
fn view_frame(m: &Match, member: i32) -> Value {
    let st = m.state();
    let player_id = st.player_of(member);
    let extra = m.view_extra(member);
    json!({
        "state": st,
        "hand": m.hand_of(member),
        "handNotes": m.hand_notes_of(member),
        "draw": m.draw_of(member),
        "you": member,
        "playerId": player_id,
        "aiAnswer": extra.get("aiAnswer").cloned().unwrap_or(Value::Null),
        "playable": extra.get("playable").cloned().unwrap_or(Value::Null),
        "estCost": extra.get("estCost").cloned().unwrap_or(Value::Null),
    })
}

fn at_decision(m: &Match, member: i32) -> Option<i32> {
    let st = m.state();
    if st.phase != "play" {
        return None;
    }
    let me = st.player_of(member);
    if st.prompt.id > 0 {
        return st.prompt.waiting(me).then_some(st.prompt.id);
    }
    if !st.busy && st.turn == me && (st.step == stage::OPS || st.step == stage::END) {
        return Some(0);
    }
    None
}

struct Sample {
    budget_ms: u64,
    latency_ms: u64,
    searched: bool,
    reused: bool,
}

fn main() {
    let data_dir = std::path::PathBuf::from(arg("--data", "data"));
    let rules_dir = std::path::PathBuf::from(arg("--rules", "dist/cards"));
    let decisions: usize = arg("--decisions", "12").parse().unwrap_or(12);
    let threads: usize = arg("--search-threads", "4").parse().unwrap_or(4).max(1);
    let seed: u64 = arg("--seed", "1").parse().unwrap_or(1);
    let with_ponder = flag("--ponder");

    let ctx = Ctx::load(&data_dir, &rules_dir);
    let mut opts = bot_service::SearchOpts::default();
    opts.search_threads = threads;
    let ctx = ctx.with_opts(opts);
    let real = ctx.rules.ruleset_sha256().is_some();
    println!(
        "ruleset: {} ({}), search-threads {threads}, ponder {with_ponder}",
        ctx.rules.ruleset_sha256().unwrap_or("stub"),
        if real { "real WasmRules" } else { "STUB -- no effects" },
    );

    // One seat's whole decision stream over a fresh match per sample: every
    // decision the server would probe, not only the searchable ones.
    let members = vec![
        RoomMember {
            id: 1,
            player: "Searcher".into(),
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "B".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "C".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 4,
            player: "D".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];

    let mut samples: Vec<Sample> = Vec::new();
    let mut g = 0u64;
    while samples.len() < decisions {
        g += 1;
        let mut m = Match::new(
            ctx.data.clone(),
            ctx.rules.clone(),
            &members,
            seed.wrapping_add(g * 97),
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        // Walk the match, sampling every decision member 1 faces.
        for _ in 0..8_000 {
            if m.ended() || samples.len() >= decisions {
                break;
            }
            let Some(prompt_id) = at_decision(&m, 1) else {
                m.tick(0.25);
                continue;
            };
            let view = view_frame(&m, 1);
            // The server's default budget (`botsvc::budget_ms`).
            let st = m.state();
            let budget_ms = if prompt_id <= 0 {
                server::botsvc::DEFAULT_TURN_BUDGET_MS
            } else {
                let ms = (st.prompt.time_left.max(0.0) * 1000.0) as u64;
                ms.saturating_sub(server::botsvc::PROMPT_MARGIN_MS)
                    .clamp(50, server::botsvc::MAX_ASK_BUDGET_MS)
            };
            let seed = 1000 + samples.len() as u64;
            if with_ponder {
                let _ = handle(
                    &ctx,
                    ponder_request(9_000 + samples.len() as u64, "bench", 1, &view, 300, seed),
                );
            }
            let t0 = Instant::now();
            let resp = handle(
                &ctx,
                decide_request(
                    samples.len() as u64,
                    "bench",
                    1,
                    &view,
                    prompt_id,
                    budget_ms,
                    seed,
                ),
            );
            let latency_ms = t0.elapsed().as_millis() as u64;
            let Ok(ans) = BotAnswer::from_response(&resp) else {
                eprintln!("decide failed: {resp}");
                m.tick(0.25);
                continue;
            };
            samples.push(Sample {
                budget_ms,
                latency_ms,
                searched: !ans.heuristic && !ans.reused,
                reused: ans.reused,
            });
            let _ = m.act(1, &ans.answer);
            m.tick(0.25);
        }
    }

    // Score each sample under both deadline policies. A sample "falls back"
    // exactly when its latency exceeds the outer deadline -- the server gives
    // up and answers with the engine heuristic instead.
    let old_timeout = |b: u64| Duration::from_millis(b.saturating_add(400)).min(Duration::from_millis(1_500));
    let new_timeout = |b: u64| server::botsvc::ask_timeout(b);
    let mut old_ok = 0usize;
    let mut new_ok = 0usize;
    let mut searched = 0usize;
    let mut delegated = 0usize;
    let mut reused = 0usize;
    let mut lat_searched: Vec<u64> = Vec::new();
    for s in &samples {
        if s.reused {
            reused += 1;
        } else if s.searched {
            searched += 1;
            lat_searched.push(s.latency_ms);
        } else {
            delegated += 1;
        }
        if s.latency_ms <= old_timeout(s.budget_ms).as_millis() as u64 {
            old_ok += 1;
        }
        if s.latency_ms <= new_timeout(s.budget_ms).as_millis() as u64 {
            new_ok += 1;
        }
    }
    lat_searched.sort_unstable();
    let n = samples.len().max(1);
    let pct = |k: usize| 100.0 * k as f64 / n as f64;
    let s_mean = if lat_searched.is_empty() {
        0.0
    } else {
        lat_searched.iter().sum::<u64>() as f64 / lat_searched.len() as f64
    };
    let s_max = lat_searched.last().copied().unwrap_or(0);
    println!(
        "decisions {n}  searched {searched}  delegated {delegated}  reused {reused}"
    );
    println!(
        "searched latency: mean {:.0} ms, max {s_max} ms",
        s_mean
    );
    println!(
        "shipped deadline (budget + {} ms capped {:?}): answered {} ({:.1}%), fell back {} ({:.1}%)",
        server::botsvc::ITERATION_OVERRUN_MS,
        server::botsvc::MAX_ASK_TIMEOUT,
        new_ok,
        pct(new_ok),
        n - new_ok,
        pct(n - new_ok),
    );
    println!(
        "old deadline (budget + 400 ms capped 1.5 s): answered {} ({:.1}%), fell back {} ({:.1}%)",
        old_ok,
        pct(old_ok),
        n - old_ok,
        pct(n - old_ok),
    );
}