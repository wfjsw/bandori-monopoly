//! Simulate bot-only matches and report what happened.
//!
//!   cargo run -p game-core --release --example sim -- [games] [players] [max_rounds]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let games = args.first().copied().unwrap_or(50);
    let players = args.get(1).copied().unwrap_or(4) as i32;
    let max_rounds = args.get(2).copied().unwrap_or(200) as i32;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    );

    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut prompts: BTreeMap<String, usize> = BTreeMap::new();
    let (mut rounds, mut texts) = (0i64, BTreeMap::<&str, usize>::new());
    let started = Instant::now();
    for seed in 0..games {
        let members: Vec<RoomMember> = (1..=players)
            .map(|i| RoomMember {
                id: i,
                player: format!("Bot{i}"),
                bot: true,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(
            data.clone(),
            Arc::new(StubRules),
            &members,
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        let (mut last_prompt, mut last_event) = (0, 0);
        while !m.ended() {
            m.tick(0.25);
            // Count events as they happen: the engine only keeps the last 400.
            for e in m.events_since(last_event) {
                last_event = e.id;
                *kinds.entry(e.r#type.clone()).or_default() += 1;
                // Message keys, nested ones included (the debug form lists them all).
                let keys = e.msg.to_string();
                for (needle, label) in [
                    ("log.circle_money", "circle money"),
                    ("log.circle_card", "circle card"),
                    ("log.auction_won", "auction won"),
                    ("log.force_buy", "force buy"),
                    ("log.part.rent_half", "agent half rent"),
                ] {
                    if keys.contains(needle) {
                        *texts.entry(label).or_default() += 1;
                    }
                }
            }
            let st = m.state();
            if st.prompt.id != 0 && st.prompt.id != last_prompt {
                last_prompt = st.prompt.id;
                *prompts.entry(st.prompt.kind.clone()).or_default() += 1;
            }
            if st.round > max_rounds {
                m.finish();
            }
        }
        let st = m.state();
        rounds += st.round as i64;
        *reasons.entry(st.end_reason.clone()).or_default() += 1;
    }
    let secs = started.elapsed().as_secs_f64();
    println!(
        "{games} games x {players} bots, cap {max_rounds} rounds: {secs:.2}s ({:.1} ms/game)",
        secs * 1000.0 / games as f64
    );
    println!("avg rounds {:.1}", rounds as f64 / games as f64);
    println!("end reasons {reasons:?}");
    println!("prompts     {prompts:?}");
    println!("event kinds {kinds:?}");
    println!("details     {texts:?}");
}
