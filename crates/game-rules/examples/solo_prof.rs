//! Latency profile of a Solo match on the real ruleset: times every `tick` and
//! `act` call and reports the slowest ones plus the rtimer buckets.
//!   CARDS_DIR=/abs/dist/cards cargo run -p game-rules --release --example solo_prof -- [games] [max_rounds]
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use game_core::data::{GameData, Roster};
use game_core::engine::{rtimer, CardRules, Match};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality};
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

fn load_data(roster: Roster) -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), roster).expect("data"))
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn main() {
    let a: Vec<u64> = std::env::args().skip(1).filter_map(|x| x.parse().ok()).collect();
    let games = a.first().copied().unwrap_or(2);
    let max_rounds = a.get(1).copied().unwrap_or(30) as i32;
    let dir = std::env::var("CARDS_DIR").expect("CARDS_DIR=/abs/dist/cards");
    let ruleset = Ruleset::load_dir(Path::new(&dir)).expect("read").expect("build-ruleset first");
    let data = load_data(ruleset.roster());
    let rules: Arc<dyn CardRules> = Arc::new(WasmRules::new(ruleset, data.clone()));
    let mut ticks: Vec<(f64, String)> = vec![];
    let mut acts: Vec<(f64, String)> = vec![];
    let mut saves: Vec<f64> = vec![];
    let mut views: Vec<f64> = vec![];
    rtimer::reset();
    let first = a.get(2).copied().unwrap_or(0);
    for seed in first..first + games {
        // member 1 = the "human" (driven below with default legal acts); 2..4 bots
        let members: Vec<RoomMember> = (1..=4)
            .map(|i| RoomMember { id: i, player: format!("P{i}"), bot: i != 1, mentality: BotMentality::default(), ..Default::default() })
            .collect();
        let mut m = Match::new(data.clone(), rules.clone(), &members, seed, MatchMode::Solo, ScoreWeights::default());
        let mut n = 0u64;
        while !m.ended() && m.state().round <= max_rounds && n < 100000 {
            let st = m.state().clone();
            let me = st.player_of(1);
            let t0 = Instant::now();
            let mut did = String::new();
            if me >= 0 {
                let mi = me as usize;
                if st.phase == "ban" && st.turn == me {
                    let _ = m.act(1, &NetMessage::act("ban"));
                    did = "ban".into();
                } else if st.phase == "pick" && st.turn == me {
                    if let Some(c) = data.personas.iter().find(|c| st.players.iter().all(|p| p.persona != c.name)) {
                        let _ = m.act(1, &NetMessage { persona: c.name.clone(), ..NetMessage::act("pick") });
                        did = "pick".into();
                    }
                } else if st.phase == "deck" && !st.players[mi].deck_ready {
                    if let Some(c) = data.persona(&st.players[mi].persona) {
                        let cards = game_core::deck::preset(&data, c);
                        let _ = m.act(1, &NetMessage { cards, ..NetMessage::act("deck") });
                        did = "deck".into();
                    }
                } else if st.prompt.id != 0 && st.prompt.players.contains(&me) {
                    let r = m.act(1, &NetMessage { value: st.prompt.fallback, prompt: st.prompt.id, ..NetMessage::act("answer") }); if n < 60000 && r.is_err() { eprintln!("answer err {:?} id={} tl={} answers={:?} wait/phase={} n={n}", r.as_ref().err().map(|e| e.key().to_string()), st.prompt.id, st.prompt.time_left, st.prompt.answers, st.phase); }
                    did = format!("answer:{}", st.prompt.kind);
                } else if st.phase == "play" && st.turn == me && st.prompt.id == 0 {
                    let act = if st.step == stage::OPS { "roll" } else { "end" };
                    let _ = m.act(1, &NetMessage::act(act));
                    did = act.into();
                }
            }
            if !did.is_empty() {
                acts.push((ms(t0), format!("seed{seed} #{n} r{} {did}", st.round)));
            }
            let t = Instant::now();
            m.tick(0.25);
            ticks.push((ms(t), format!("seed{seed} #{n} r{} phase={} step={}", st.round, st.phase, st.step)));
            if n % 20 == 0 {
                let t = Instant::now();
                let _ = m.save();
                saves.push(ms(t));
                let t = Instant::now();
                let _ = serde_json::to_string(&m.view_extra(1));
                let _ = serde_json::to_string(&m.state());
                views.push(ms(t));
            }
            n += 1;
        }
        eprintln!("seed {seed} done round={} ticks={n}", m.state().round);
    }
    let rep = |name: &str, v: &mut Vec<(f64, String)>| {
        v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let tot: f64 = v.iter().map(|x| x.0).sum();
        let p = |q: f64| v.get(((v.len() as f64) * (1.0 - q)) as usize).map(|x| x.0).unwrap_or(0.0);
        println!("{name}: n={} total={tot:.0}ms p50={:.3} p90={:.3} p99={:.3} max={:.1}ms", v.len(), p(0.5), p(0.1), p(0.01), v.first().map(|x| x.0).unwrap_or(0.0));
        for x in v.iter().take(8) {
            println!("   {:8.1}ms  {}", x.0, x.1);
        }
    };
    rep("tick(0.25)", &mut ticks);
    rep("human act", &mut acts);
    let avg = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let mx = |v: &Vec<f64>| v.iter().cloned().fold(0.0, f64::max);
    println!("save avg {:.2}ms max {:.2}; state+view_extra json avg {:.2}ms max {:.2}", avg(&saves), mx(&saves), avg(&views), mx(&views));
    #[cfg(feature = "bot-cost")]
    {
        use std::sync::atomic::Ordering::Relaxed as R;
        use game_rules::bot_cost as b;
        println!("instantiations={} instantiate={:.0}ms store={:.0}ms guest={:.0}ms host_world_clones={} engine_world_clones={}",
            b::INSTANTIATIONS.load(R), b::INSTANTIATE_NS.load(R) as f64 / 1e6, b::STORE_NS.load(R) as f64 / 1e6, b::GUEST_NS.load(R) as f64 / 1e6,
            b::HOST_WORLD_CLONES.load(R), game_core::engine::bot_cost::WORLD_CLONES.load(R));
        println!("counteract windows={} skipped={} probes={} memo_hits={} declared={}", b::COUNTERACT_WINDOWS.load(R), b::COUNTERACT_WINDOWS_SKIPPED.load(R), b::COUNTERACT_PROBES.load(R), b::COUNTERACT_PROBE_MEMO_HITS.load(R), b::COUNTERACT_DECLARED.load(R));
    }
    for (k, v) in rtimer::snapshot() {
        println!("rtimer {k}: {:.0}ms", v as f64 / 1e6);
    }
}
