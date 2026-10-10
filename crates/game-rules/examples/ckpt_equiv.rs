//! Real-ruleset seeded-game equivalence check: run N seeded bot-only games with
//! `WasmRules` and record a hash of `Match::save()` at every turn boundary (and
//! at the end of every game). Run it on the tree before a behaviour-sensitive
//! change and again after; the two files must be byte-identical.
//!
//!   CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!     cargo run -p game-rules --release --example ckpt_equiv -- \
//!       [out.txt] [games] [players] [max_rounds] [standard|chaos] [--state]
//!
//! Defaults: `target/scratch/ckpt.txt`, 8 games, 4 players, 120 rounds,
//! `standard`. The trailing word picks every bot's mentality.
//!
//! `--state` hashes the save with the **event tail excluded** (`world.recent`,
//! `world.next_event`, and every `parent` / `results` field) so a change that
//! only reshapes the log still shows identical game state (money, positions,
//! ownership, hands, field, turn). Compare a `--state` run before and after to
//! prove state equivalence; the full run's deltas should be events only.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use game_rules::WasmRules;

/// FNV-1a 64 -- deterministic across runs and platforms (unlike `DefaultHasher`).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Strip the event tail and its presentation fields from a save, so the hash
/// covers game state only. `--state` mode. Recurses into `pending.snapshot`
/// (a halted routine's world) as well as the top-level `world`.
fn strip_events(save: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(save) else {
        return save.to_string();
    };
    if let Some(obj) = v.as_object_mut() {
        // Presentation pacing (walk-flush delays and the like) is allowed to
        // differ; game state is not.
        obj.remove("wait");
        obj.remove("shield");
    }
    fn scrub(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(o) => {
                // Event tail (and its id counter) at every world level.
                if o.contains_key("recent") || o.contains_key("next_event") {
                    o.remove("recent");
                    o.remove("next_event");
                }
                o.remove("parent");
                o.remove("results");
                for (_, x) in o.iter_mut() {
                    scrub(x);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a.iter_mut() {
                    scrub(x);
                }
            }
            _ => {}
        }
    }
    scrub(&mut v);
    serde_json::to_string(&v).unwrap_or_else(|_| save.to_string())
}

fn load_data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .expect("game data"),
    )
}

fn load_rules(data: &Arc<GameData>) -> Arc<dyn CardRules> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let rules: Arc<WasmRules> = WasmRules::load_dir(data.clone(), &dir)
        .expect("read dist/cards")
        .expect("run `node tools/build-ruleset.mjs` first")
        .into();
    rules
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let state_only = raw.iter().any(|a| a == "--state");
    let dump_dir = raw
        .iter()
        .find_map(|a| a.strip_prefix("--dump=").map(|s| s.to_string()));
    let dump_events = raw
        .iter()
        .find_map(|a| a.strip_prefix("--dump-events=").map(|s| s.to_string()));
    let mentality = raw
        .iter()
        .find_map(|a| BotMentality::parse(a))
        .unwrap_or_default();
    let args: Vec<u64> = raw.iter().filter_map(|a| a.parse().ok()).collect();
    // First bare arg is the games count only when it is not the out path.
    let out = raw
        .iter()
        .find(|a| !a.starts_with("--") && (a.ends_with(".txt") || a.contains('/') || a.contains('\\')))
        .cloned()
        .unwrap_or_else(|| "target/scratch/ckpt.txt".to_string());
    let games = args.first().copied().unwrap_or(8) as u32;
    let players = args.get(1).copied().unwrap_or(4) as i32;
    let max_rounds = args.get(2).copied().unwrap_or(120) as i32;

    let data = load_data();
    let rules = load_rules(&data);

    let mut lines: Vec<String> = Vec::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut prompts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_turns = 0u64;

    for seed in 0..games as u64 {
        eprintln!("ckpt: start seed={seed}");
        let members: Vec<RoomMember> = (1..=players)
            .map(|i| RoomMember {
                id: i,
                player: format!("Bot{i}"),
                bot: true,
                mentality,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(
            data.clone(),
            rules.clone(),
            &members,
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        let (mut last_turn, mut last_prompt, mut last_event) = (-1i32, 0i32, 0i32);
        let mut game_hash: u64 = 0xcbf2_9ce4_8422_2325;
        while !m.ended() {
            m.tick(0.25);
            for e in m.events_since(last_event) {
                last_event = e.id;
                *kinds.entry(e.r#type.clone()).or_default() += 1;
            }
            let st = m.state();
            if st.prompt.id != 0 && st.prompt.id != last_prompt {
                last_prompt = st.prompt.id;
                *prompts.entry(st.prompt.kind.clone()).or_default() += 1;
            }
            // A checkpoint at every turn boundary and at the end.
            if st.turn != last_turn || st.round > max_rounds {
                if st.round > max_rounds {
                    m.finish();
                }
                let save = m.save();
                eprintln!("ckpt: seed={seed} turn={} save_len={}", st.turn, save.len());
                let save = if state_only {
                    strip_events(&save)
                } else {
                    save
                };
                if let Some(dir) = &dump_dir {
                    let _ = std::fs::create_dir_all(dir);
                    let _ = std::fs::write(
                        Path::new(dir).join(format!("seed{seed}-turn{}-round{}.json", st.turn, st.round)),
                        &save,
                    );
                }
                if let Some(dir) = &dump_events {
                    if seed == 0 {
                        let _ = std::fs::create_dir_all(dir);
                        let mut lines = Vec::new();
                        for e in m.events_since(-1) {
                            lines.push(format!(
                                "id={} type={} card={:?} value={} msg={} parent={}",
                                e.id, e.r#type, e.card, e.value, e.msg.key(), e.parent
                            ));
                        }
                        let _ = std::fs::write(Path::new(dir).join("seed0-events.txt"), lines.join("\n"));
                    }
                }
                let h = fnv1a64(save.as_bytes());
                game_hash = game_hash
                    .wrapping_mul(0x0000_0100_0000_01b3)
                    .wrapping_add(h);
                lines.push(format!(
                    "seed={seed} turn={} round={} turnHash={h:016x}",
                    st.turn, st.round
                ));
                if st.turn != last_turn {
                    total_turns += 1;
                }
                last_turn = st.turn;
                if st.round > max_rounds {
                    break;
                }
            }
        }
        let st = m.state();
        let save = m.save();
        let save = if state_only {
            strip_events(&save)
        } else {
            save
        };
        let final_h = fnv1a64(save.as_bytes());
        game_hash = game_hash
            .wrapping_mul(0x0000_0100_0000_01b3)
            .wrapping_add(final_h);
        lines.push(format!(
            "seed={seed} END round={} reason={} endHash={final_h:016x} gameHash={game_hash:016x}",
            st.round, st.end_reason
        ));
    }

    lines.push(format!("TOTAL games={games} players={players} max_rounds={max_rounds} mentality={}", mentality.as_str()));
    lines.push(format!("TOTAL turns={total_turns}"));
    lines.push(format!("TOTAL prompts={prompts:?}"));
    lines.push(format!("TOTAL kinds={kinds:?}"));

    let path = Path::new(&out);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = std::fs::File::create(path).expect("write checkpoint file");
    for l in &lines {
        writeln!(f, "{l}").unwrap();
    }
    println!("wrote {} checkpoint lines to {out}", lines.len());
    println!("turns={total_turns} kinds={kinds:?}");
}