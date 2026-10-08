//! Decision latency and iterations under the **real** ruleset (`dist/cards`
//! via `WasmRules`), at the budgets the server actually sends (`docs/BOT.md`
//! B5 §5; BOT-RESEARCH #2 gate).
//!
//! ```
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!   cargo run -p bot-service --release --example latency -- \
//!     --data data --rules dist/cards --decisions 24 --budgets 200,1000 \
//!     --search-threads 1,4
//! ```
//!
//! Parks a 4-seat match at one seat's real decisions (the same view frame the
//! server sends), runs `decide` at each (budget × search-threads) point, and
//! reports mean / p95 latency and iterations. `--stub` forces the no-effect
//! [`StubRules`] backend even when modules are present; without it, StubRules
//! is the fallback when no modules load -- check the banner. `--legacy` turns
//! off the #3 selection flags for an A/B.

use std::time::Instant;

use bot_service::{decide_request, handle, ponder_request, BotAnswer, Ctx, SearchOpts};
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
    let args: Vec<String> = std::env::args().collect();
    args.iter().any(|a| a == name)
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

fn at_decision(m: &Match, member: i32) -> bool {
    let st = m.state();
    if st.phase != "play" {
        return false;
    }
    let me = st.player_of(member);
    if st.prompt.id > 0 {
        return st.prompt.waiting(me);
    }
    !st.busy && st.turn == me && (st.step == stage::OPS || st.step == stage::END)
}

/// Park one fresh match at member 1's decision (the view frame the server
/// would send), or `None` when the game ended first.
fn park(seed: u64, data: &std::sync::Arc<game_core::data::GameData>, rules: &std::sync::Arc<dyn game_core::engine::CardRules>) -> Option<(Match, Value, i32)> {
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
    let mut m = Match::new(
        data.clone(),
        rules.clone(),
        &members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    for _ in 0..8_000 {
        if m.ended() {
            return None;
        }
        if at_decision(&m, 1) {
            // Park only at a *searchable* surface (the abstracted action list
            // is non-empty); answer mulligan / roll / end with the heuristic
            // and keep going -- the B5 sample was 21 delegated / 3 searched.
            let view = bot_service::BotSeatView::from_match(&m, 1);
            let me = m.state().player_of(1).max(0) as usize;
            let acts = bot_service::bot_action::legal_actions(
                data,
                &view.state,
                &view.hand,
                &view.playable,
                me,
            );
            if !acts.is_empty() {
                let frame = view_frame(&m, 1);
                let st = m.state();
                let prompt_id = if st.prompt.id > 0 && st.prompt.waiting(st.player_of(1)) {
                    st.prompt.id
                } else {
                    0
                };
                return Some((m, frame, prompt_id));
            }
            let msg = bot_service::heuristic_message_view(data, &view);
            let _ = m.act(1, &msg);
        }
        m.tick(0.25);
    }
    None
}

struct Collected {
    lats: Vec<f64>,
    search_lats: Vec<f64>,
    search_iters: Vec<f64>,
    iters: Vec<u64>,
    searched: usize,
    delegated: usize,
    refused: usize,
    reused: usize,
    wall: f64,
}

fn main() {
    let data_dir = std::path::PathBuf::from(arg("--data", "data"));
    let rules_dir = std::path::PathBuf::from(arg("--rules", "dist/cards"));
    let decisions: usize = arg("--decisions", "8").parse().unwrap_or(8);
    let budgets: Vec<u64> = arg("--budgets", "200,1000")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let thread_counts: Vec<usize> = arg("--search-threads", "1")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .map(|n: usize| n.max(1))
        .collect();
    let seed: u64 = arg("--seed", "1").parse().unwrap_or(1);
    let legacy = flag("--legacy");
    let with_ponder = flag("--ponder");

    let mut ctx = Ctx::load(&data_dir, &rules_dir);
    if flag("--stub") {
        eprintln!("--stub: forcing StubRules (no card effects)");
        ctx = Ctx::new(
            ctx.data.clone(),
            std::sync::Arc::new(game_core::engine::StubRules),
        );
    }
    if legacy {
        ctx = ctx.with_opts(SearchOpts::legacy());
    }
    let real = ctx.rules.ruleset_sha256().is_some();
    println!(
        "ruleset: {} ({}), legacy {}",
        ctx.rules.ruleset_sha256().unwrap_or("stub"),
        if real { "real WasmRules" } else { "STUB -- no effects" },
        legacy
    );

    for budget in &budgets {
        for threads in &thread_counts {
            let mut opts = ctx.opts.clone();
            opts.search_threads = *threads;
            let ctx = Ctx::new(ctx.data.clone(), ctx.rules.clone()).with_opts(opts);
            let mut c = Collected {
                lats: Vec::new(),
                search_lats: Vec::new(),
                search_iters: Vec::new(),
                iters: Vec::new(),
                searched: 0,
                delegated: 0,
                refused: 0,
                reused: 0,
                wall: 0.0,
            };
            let wall = Instant::now();
            for g in 0..decisions {
                // A fresh match per decision: the park loop is cheap, and it
                // keeps every sample at a comparable mid-game shape.
                let Some((mut m, view, prompt_id)) =
                    park(seed.wrapping_add(g as u64 * 97), &ctx.data, &ctx.rules)
                else {
                    continue;
                };
                if with_ponder {
                    // Speculative search first (BOT-RESEARCH #5), then the
                    // real decision -- it must hit the ponder cache.
                    let _ = handle(
                        &ctx,
                        ponder_request(10_000 + g as u64, "bench", 1, &view, *budget, 7000 + g as u64),
                    );
                }
                let t0 = Instant::now();
                let resp = handle(
                    &ctx,
                    decide_request(g as u64, "bench", 1, &view, prompt_id, *budget, 1000 + g as u64),
                );
                let dt = t0.elapsed();
                let Ok(ans) = BotAnswer::from_response(&resp) else {
                    eprintln!("decide failed: {resp}");
                    continue;
                };
                c.lats.push(dt.as_secs_f64() * 1000.0);
                c.iters.push(ans.iterations);
                if ans.reused {
                    c.reused += 1;
                }
                if ans.heuristic {
                    c.delegated += 1;
                } else {
                    c.searched += 1;
                    c.search_lats.push(dt.as_secs_f64() * 1000.0);
                    c.search_iters.push(ans.iterations as f64);
                }
                if m.act(1, &ans.answer).is_err() {
                    c.refused += 1;
                }
            }
            c.wall = wall.elapsed().as_secs_f64();
            c.lats.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if c.lats.is_empty() {
                println!(
                    "budget {budget} ms, threads {threads}: no decisions parked"
                );
                continue;
            }
            let n = c.lats.len().max(1) as f64;
            let mean = c.lats.iter().sum::<f64>() / n;
            let p95_idx = (((c.lats.len() as f64) * 0.95) as usize).min(c.lats.len() - 1);
            let p95 = c.lats[p95_idx];
            let mean_i = c.iters.iter().map(|&x| x as f64).sum::<f64>() / n;
            let (s_mean, s_max, s_iter) = if c.search_lats.is_empty() {
                (0.0, 0.0, 0.0)
            } else {
                let n = c.search_lats.len() as f64;
                (
                    c.search_lats.iter().sum::<f64>() / n,
                    c.search_lats.iter().cloned().fold(0.0f64, f64::max),
                    c.search_iters.iter().sum::<f64>() / n,
                )
            };
            println!(
                "budget {budget} ms, threads {threads}: n={}  latency mean {:.0} ms / p95 {:.0} ms  \
                 iterations mean {:.1}  searched {} (mean {:.0} ms, max {:.0} ms, \
                 {:.1} iters) delegated {} reused {} refused {}  wall {:.1}s",
                c.lats.len(),
                mean,
                p95,
                mean_i,
                c.searched,
                s_mean,
                s_max,
                s_iter,
                c.delegated,
                c.reused,
                c.refused,
                c.wall
            );
        }
    }
}