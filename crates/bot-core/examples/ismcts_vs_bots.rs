//! ISMCTS seat vs 3 standard bots on the built-in rules -- B4 harness,
//! extended for the BOT-RESEARCH #2/#3/#5 measurements.
//!
//! ```
//! # iterations/decision + strength at one setting:
//! cargo run -p bot-core --release --example ismcts_vs_bots -- \
//!     [games] [budget_ms] [baseline_games] [--threads T] [--horizon H] \
//!     [--bias F] [--eval F] [--legacy] [--profile] [--compare] [--fixed-iters K] \
//!     [--early-stop] [--ponder]
//! ```
//!
//! * `--threads T` -- root-parallel searches per decision (default 1).
//! * `--horizon H` -- rollout horizon in rounds (default 2).
//! * `--bias F` / `--eval F` -- progressive-bias weight `c_h` and the
//!   heuristic-eval mix `α` (0 = the pre-#3 selection behaviour).
//! * `--legacy` -- the pre-#3 search, single-threaded (for A/B).
//! * `--profile` -- per-iteration fork / descent / rollout / eval / key split
//!   (the B4 "14 ms per iteration" attribution).
//! * `--compare` -- run legacy and the new settings back to back at the same
//!   wall budget and print both (the strength gate).
//! * `--fixed-iters K` -- stop after K iterations instead of the wall budget
//!   (deterministic runs).
//!
//! Seat 0 is driven by ISMCTS over determinizations; seats 1-3 are the
//! engine's standard bots. Expect a weak absolute signal on StubRules (cards
//! do nothing) -- the numbers that matter are iterations/decision and the
//! old-vs-new comparison at a fixed budget.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bot_core::{next_decision, Ismcts, MatchSim, SearchConfig, SearchOutcome, SeatView};
use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

fn bot_members(n: i32, seat0_human: bool) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            // Seat 0 is driven by the harness (a human from the engine's
            // point of view) so the engine never auto-playes it; the rest are
            // ordinary standard bots.
            bot: if seat0_human && i == 1 { false } else { true },
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect()
}

fn end_summary(m: &Match) -> (i32, i32, i32) {
    // (seat0 rank, seat0 score, rounds)
    let st = m.state();
    let me = &st.players[0];
    (me.rank, me.score, st.round)
}

#[derive(Clone, Copy)]
struct Opts {
    games: usize,
    budget_ms: u64,
    baseline_games: usize,
    threads: usize,
    horizon: u32,
    bias: f64,
    eval_w: f64,
    legacy: bool,
    profile: bool,
    compare: bool,
    fixed_iters: u64,
    /// Time management: stop before the budget when the best root action
    /// cannot be overtaken (`docs/BOT.md` §3.4). Off by default so the
    /// harness's published cells stay comparable to the C1 record.
    early_stop: bool,
    /// Speculate on the seat's next own decision between its turns
    /// (`docs/BOT.md` §3.5). Off by default.
    ponder: bool,
}

fn parse_opts() -> Opts {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // Flags that take a value: their value must not also count as positional.
    let mut skip_next = false;
    let mut positional: Vec<u64> = Vec::new();
    for a in &raw {
        if skip_next {
            skip_next = false;
            continue;
        }
        match a.as_str() {
            "--threads" | "--horizon" | "--bias" | "--eval" | "--fixed-iters" => skip_next = true,
            _ if a.starts_with("--") => {}
            _ => {
                if let Ok(n) = a.parse::<u64>() {
                    positional.push(n);
                }
            }
        }
    }
    let mut o = Opts {
        games: positional.first().copied().unwrap_or(8) as usize,
        budget_ms: positional.get(1).copied().unwrap_or(200),
        baseline_games: positional.get(2).copied().unwrap_or(0) as usize,
        threads: 1,
        horizon: 2,
        bias: 0.4,
        eval_w: 0.3,
        legacy: false,
        profile: false,
        compare: false,
        fixed_iters: 0,
        early_stop: false,
        ponder: false,
    };
    let mut i = 0;
    while i < raw.len() {
        let a = raw[i].as_str();
        let mut next = |i: &mut usize| -> String {
            *i += 1;
            raw.get(*i).cloned().unwrap_or_default()
        };
        match a {
            "--threads" => o.threads = next(&mut i).parse().unwrap_or(1).max(1),
            "--horizon" => o.horizon = next(&mut i).parse().unwrap_or(2),
            "--bias" => o.bias = next(&mut i).parse().unwrap_or(0.4),
            "--eval" => o.eval_w = next(&mut i).parse().unwrap_or(0.3),
            "--fixed-iters" => o.fixed_iters = next(&mut i).parse().unwrap_or(0),
            "--legacy" => {
                o.legacy = true;
                o.threads = 1;
                o.bias = 0.0;
                o.eval_w = 0.0;
            }
            "--profile" => o.profile = true,
            "--compare" => o.compare = true,
            "--early-stop" => o.early_stop = true,
            "--ponder" => o.ponder = true,
            _ => {}
        }
        i += 1;
    }
    o
}

fn search_cfg(o: &Opts, seed: u64) -> SearchConfig {
    SearchConfig {
        budget: Duration::from_millis(o.budget_ms),
        max_iterations: if o.fixed_iters > 0 {
            o.fixed_iters
        } else {
            100_000
        },
        horizon_rounds: o.horizon,
        seed,
        eval_weight: o.eval_w,
        implicit_minimax: !o.legacy && o.eval_w > 0.0,
        bias_weight: o.bias,
        profile: o.profile,
        early_stop: o.early_stop,
        ..Default::default()
    }
}

struct RunStats {
    decisions: u64,
    iterations: u64,
    elapsed_ms: f64,
    /// Decides answered from a pondered cache entry (`--ponder`).
    ponder_hits: u64,
    /// Speculative searches actually run (`--ponder`).
    ponders: u64,
    failbacks: u64,
    /// Failback sources, split for the regression triage.
    fb_delegated: u64,
    fb_heuristic: u64,
    fb_refused: u64,
    delegated: u64,
    rank: f64,
    score: f64,
    rounds: f64,
    wins: usize,
    wall: f64,
    fork_ms: f64,
    descent_ms: f64,
    rollout_ms: f64,
    eval_ms: f64,
    key_ms: f64,
}

fn run_ismcts(data: &Arc<GameData>, rules: Arc<dyn CardRules>, o: &Opts) -> RunStats {
    let mut s = RunStats {
        decisions: 0,
        iterations: 0,
        elapsed_ms: 0.0,
        ponder_hits: 0,
        ponders: 0,
        failbacks: 0,
        fb_delegated: 0,
        fb_heuristic: 0,
        fb_refused: 0,
        delegated: 0,
        rank: 0.0,
        score: 0.0,
        rounds: 0.0,
        wins: 0,
        wall: 0.0,
        fork_ms: 0.0,
        descent_ms: 0.0,
        rollout_ms: 0.0,
        eval_ms: 0.0,
        key_ms: 0.0,
    };
    let started = Instant::now();
    for seed in 0..o.games as u64 {
        let mut m = Match::new(
            data.clone(),
            rules.clone(),
            &bot_members(4, true),
            seed.wrapping_mul(31) + 5,
            MatchMode::Solo,
            ScoreWeights::default(),
        );
        m.quick_start();
        let member = 1; // seat 0
        let seat = 0usize;
        // Speculative answers keyed by `decision_key` (`--ponder`, production's
        // `op: "ponder"` cache). Cleared per game.
        let mut ponder_cache: std::collections::HashMap<u64, bot_core::Action> =
            std::collections::HashMap::new();
        let mut steps = 0u32;
        // Consecutive unanswerable surfaces (the heuristic act and the prompt
        // fallback are both refused): give up on the game instead of spinning
        // the step budget -- the regression triage's 199k-step loop.
        let mut stuck = 0u32;
        while !m.ended() && steps < 200_000 {
            steps += 1;
            if !next_decision(&mut m, member, 5_000) {
                break;
            }
            if m.state().round > 200 {
                m.finish();
                break;
            }
            let view = SeatView::from_match(&m, member);
            let st = view.state.clone();
            // Heuristic-delegated surfaces: answer without searching.
            let searched =
                bot_core::action::legal_actions(&data, &st, &view.hand, &view.playable, seat);
            if searched.is_empty() {
                s.delegated += 1;
                let msg = bot_core::heuristic_message_with(&data, &m, member, &st);
                let ok = m.act(member, &msg).is_ok();
                let ok = ok
                    || (st.prompt.id > 0
                        && m.act(
                            member,
                            &game_core::net::NetMessage {
                                act: "answer".into(),
                                prompt: st.prompt.id,
                                value: st.prompt.fallback,
                                ..Default::default()
                            },
                        )
                        .is_ok());
                if !ok {
                    s.failbacks += 1;
                    s.fb_delegated += 1;
                    if s.fb_delegated <= 3 {
                        eprintln!(
                            "DBG unanswerable surface prompt={} kind={} step={} turn={} busy={}",
                            st.prompt.id, st.prompt.kind, st.step, st.turn, st.busy
                        );
                    }
                    stuck += 1;
                    if stuck >= 3 {
                        eprintln!("DBG giving up on game after {stuck} stuck surfaces");
                        break;
                    }
                } else {
                    stuck = 0;
                }
                m.tick(0.25);
                if o.ponder {
                    ponder_next(&data, &rules, &m, member, seat, o, &mut ponder_cache, &mut s);
                }
                continue;
            }
            // Ponder cache hit: the same information set was already searched
            // speculatively while the other seats acted -- answer from it
            // without spending the decision's budget (`docs/BOT.md` §3.5).
            let view_key = view.decision_key();
            if o.ponder {
                if let Some(a) = ponder_cache.remove(&view_key) {
                    s.decisions += 1;
                    s.ponder_hits += 1;
                    let msg = bot_core::action::to_net_message(&a, &st, seat);
                    if m.act(member, &msg).is_err() {
                        s.failbacks += 1;
                        s.fb_refused += 1;
                        let msg = bot_core::heuristic_message_with(&data, &m, member, &st);
                        if m.act(member, &msg).is_err() {
                            let _ = m.act(member, &game_core::net::NetMessage::act("end"));
                        }
                    }
                    m.tick(0.25);
                    if o.ponder {
                        ponder_next(&data, &rules, &m, member, seat, o, &mut ponder_cache, &mut s);
                    }
                    continue;
                }
            }
            let cfg = search_cfg(o, seed.wrapping_mul(1_000_003) + steps as u64);
            let out: SearchOutcome = {
                let mut tree = Ismcts::new();
                let rules = rules.clone();
                let data = data.clone();
                let view = view.clone();
                tree.search_root_parallel(
                    move || MatchSim::new(data.clone(), rules.clone(), view.clone(), seat, member),
                    seat,
                    cfg,
                    o.threads,
                )
            };
            s.decisions += 1;
            s.iterations += out.iterations;
            s.elapsed_ms += out.elapsed.as_secs_f64() * 1000.0;
            if out.heuristic {
                s.failbacks += 1;
                s.fb_heuristic += 1;
            }
            if let Some(t) = out.timings {
                s.fork_ms += t.fork.as_secs_f64() * 1000.0;
                s.descent_ms += t.descent.as_secs_f64() * 1000.0;
                s.rollout_ms += t.rollout.as_secs_f64() * 1000.0;
                s.eval_ms += t.evaluate.as_secs_f64() * 1000.0;
                s.key_ms += t.key.as_secs_f64() * 1000.0;
            }
            let msg = bot_core::action::to_net_message(&out.action, &st, seat);
            if m.act(member, &msg).is_err() {
                // The search produced a refused act (stale surface): fall back.
                s.failbacks += 1;
                s.fb_refused += 1;
                if s.fb_refused <= 3 {
                    eprintln!(
                        "DBG refused search act {:?} at prompt={} kind={} step={}",
                        out.action, st.prompt.id, st.prompt.kind, st.step
                    );
                }
                let msg = bot_core::heuristic_message_with(&data, &m, member, &st);
                if m.act(member, &msg).is_err() {
                    let _ = m.act(member, &game_core::net::NetMessage::act("end"));
                }
            }
            m.tick(0.25);
            if o.ponder {
                ponder_next(&data, &rules, &m, member, seat, o, &mut ponder_cache, &mut s);
            }
        }
        let (r, sc, rd) = end_summary(&m);
        s.rank += r.max(1) as f64;
        s.score += sc as f64;
        s.rounds += rd as f64;
        if r == 1 {
            s.wins += 1;
        }
    }
    s.wall = started.elapsed().as_secs_f64();
    let n = o.games.max(1) as f64;
    s.rank /= n;
    s.score /= n;
    s.rounds /= n;
    s
}

/// Speculate on `member`'s **next own decision** (`docs/BOT.md` §3.5): predict
/// the turn-start 运营 view and search it, caching the answer under the
/// predicted decision key. A later real `decide` whose key matches answers
/// from the cache without spending its budget. This is the harness's model of
/// production's `op: "ponder"` -- the same predicate, the same prediction, the
/// same cache contract.
fn ponder_next(
    data: &Arc<GameData>,
    rules: &Arc<dyn CardRules>,
    m: &Match,
    member: i32,
    seat: usize,
    o: &Opts,
    cache: &mut std::collections::HashMap<u64, bot_core::Action>,
    s: &mut RunStats,
) {
    let view = SeatView::from_match(m, member);
    let Some(pred) = bot_core::predict_upcoming_view(&view) else {
        return;
    };
    let key = pred.decision_key();
    if cache.contains_key(&key) {
        return;
    }
    let searched = bot_core::action::legal_actions(
        data,
        &pred.state,
        &pred.hand,
        &pred.playable,
        seat,
    );
    if bot_core::action::trivial_decision(&searched).is_some() {
        return;
    }
    let cfg = SearchConfig {
        // Distinct stream from the real decide's (which hashes the game seed
        // and step count).
        seed: key ^ 0x9E37_79B9_7F4A_7C15,
        ..search_cfg(o, key)
    };
    let out = {
        let mut tree = Ismcts::new();
        let rules = rules.clone();
        let data = data.clone();
        let pred = pred.clone();
        tree.search_root_parallel(
            move || MatchSim::new(data.clone(), rules.clone(), pred.clone(), seat, member),
            seat,
            cfg,
            o.threads,
        )
    };
    if out.heuristic {
        return;
    }
    s.ponders += 1;
    cache.insert(key, out.action);
}

fn print_run(label: &str, o: &Opts, s: &RunStats) {
    let n = o.games.max(1) as f64;
    println!(
        "{label}: {games} games (seat0 vs 3 standard bots) in {wall:.1}s",
        games = o.games,
        wall = s.wall
    );
    println!(
        "  decisions {dec}, {:.1} decisions/s overall, {:.1} ms/decision",
        s.decisions as f64 / s.wall.max(1e-9),
        if s.decisions > 0 {
            s.elapsed_ms / s.decisions as f64
        } else {
            0.0
        },
        dec = s.decisions
    );
    println!(
        "  iterations/decision at {budget} ms (threads {threads}, horizon {horizon}): {:.1} \
         (search time {:.1} ms/decision, fallbacks {})",
        if s.decisions > 0 {
            s.iterations as f64 / s.decisions as f64
        } else {
            0.0
        },
        if s.decisions > 0 {
            s.elapsed_ms / s.decisions as f64
        } else {
            0.0
        },
        s.failbacks,
        budget = o.budget_ms,
        threads = o.threads,
        horizon = o.horizon,
    );
    println!(
        "  seat0 rank {:.2}, score {:.0}, wins {}/{}, rounds {:.0}",
        s.rank,
        s.score,
        s.wins,
        o.games,
        s.rounds
    );
    println!(
        "  surfaces: delegated {}, searched {} | failbacks: delegated-act {}, search-heuristic {}, search-act-refused {}",
        s.delegated, s.decisions, s.fb_delegated, s.fb_heuristic, s.fb_refused
    );
    if o.ponder {
        println!(
            "  ponder: {} speculative searches, {} decides served from the cache ({:.1}%)",
            s.ponders,
            s.ponder_hits,
            if s.decisions > 0 {
                100.0 * s.ponder_hits as f64 / s.decisions as f64
            } else {
                0.0
            }
        );
    }
    if o.profile && s.decisions > 0 {
        let d = s.decisions as f64;
        let it = s.iterations.max(1) as f64;
        println!(
            "  per-iteration split: fork {:.2} ms, descent {:.2} ms, rollout {:.2} ms, \
             eval {:.2} ms, decision_key {:.2} ms (search total {:.2} ms/iter)",
            s.fork_ms / it,
            s.descent_ms / it,
            s.rollout_ms / it,
            s.eval_ms / it,
            s.key_ms / it,
            s.elapsed_ms / it
        );
        println!(
            "  per-decision split: fork {:.1} ms, descent {:.1} ms, rollout {:.1} ms, \
             eval {:.1} ms, key {:.1} ms",
            s.fork_ms / d,
            s.descent_ms / d,
            s.rollout_ms / d,
            s.eval_ms / d,
            s.key_ms / d
        );
    }
    let _ = n;
}

fn main() {
    let o = parse_opts();
    let data = data();
    let rules: Arc<dyn CardRules> = Arc::new(StubRules);

    if o.baseline_games > 0 {
        // ---- baseline: 4 standard bots --------------------------------
        let base_started = Instant::now();
        let (mut base_rank, mut base_score, mut base_rounds, mut base_wins) =
            (0f64, 0f64, 0f64, 0usize);
        for seed in 0..o.baseline_games as u64 {
            let mut m = Match::new(
                data.clone(),
                rules.clone(),
                &bot_members(4, false),
                seed.wrapping_mul(17) + 1,
                MatchMode::Solo,
                ScoreWeights::default(),
            );
            m.quick_start();
            while !m.ended() {
                m.tick(0.25);
                if m.state().round > 200 {
                    m.finish();
                }
            }
            let (rank, score, rounds) = end_summary(&m);
            base_rank += rank.max(1) as f64;
            base_score += score as f64;
            base_rounds += rounds as f64;
            if rank == 1 {
                base_wins += 1;
            }
        }
        let base_secs = base_started.elapsed().as_secs_f64();
        let n = o.baseline_games.max(1) as f64;
        println!(
            "baseline: {} games x 4 standard bots in {base_secs:.1}s \
             (seat0 stand-in rank {:.2}, score {:.0}, wins {base_wins}, rounds {:.0})",
            o.baseline_games,
            base_rank / n,
            base_score / n,
            base_rounds / n
        );
    }

    if o.compare {
        // Old vs new at the same wall budget (BOT-RESEARCH validation plan).
        let old = Opts {
            legacy: true,
            threads: 1,
            bias: 0.0,
            eval_w: 0.0,
            profile: o.profile,
            ..o
        };
        let new = Opts {
            legacy: false,
            ..o
        };
        let s_old = run_ismcts(&data, rules.clone(), &old);
        print_run("legacy", &old, &s_old);
        let s_new = run_ismcts(&data, rules.clone(), &new);
        print_run("new", &new, &s_new);
        let iters = |s: &RunStats| -> f64 {
            if s.decisions > 0 {
                s.iterations as f64 / s.decisions as f64
            } else {
                0.0
            }
        };
        println!(
            "delta (new - legacy): score {:+.0}, rank {:+.2}, wins {:+}/{}, \
             iterations/decision {:+.1}",
            s_new.score - s_old.score,
            s_new.rank - s_old.rank,
            s_new.wins as i64 - s_old.wins as i64,
            o.games,
            iters(&s_new) - iters(&s_old),
        );
        return;
    }

    let s = run_ismcts(&data, rules, &o);
    print_run("ismcts", &o, &s);
}