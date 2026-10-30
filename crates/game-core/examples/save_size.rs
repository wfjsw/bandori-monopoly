//! Play bot-only matches to the end and report how large the solo save grows:
//! `Match::save()` plus `RecordedMatch::recorder_json()` -- the pair the solo
//! session writes to `localStorage` as `bm.solo`.
//!
//!   cargo run -p game-core --release --example save_size -- [players] [games]

use std::sync::Arc;

use game_core::data::{GameData, Roster};
use game_core::engine::StubRules;
use game_core::net::RoomMember;
use game_core::record::{MatchSetup, RecordedMatch};
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;

fn main() {
    let args: Vec<u64> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let players = args.first().copied().unwrap_or(4) as i32;
    let games = args.get(1).copied().unwrap_or(1);

    eprintln!("loading data");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/roster.json"),
        )
        .expect("roster fixture"),
    )
    .expect("roster fixture parses");
    let data = Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .unwrap(),
    );

    for seed in 0..games {
        let members: Vec<RoomMember> = (1..=players)
            .map(|i| RoomMember {
                id: i,
                player: format!("Bot{i}"),
                bot: true,
                mentality: BotMentality::Standard,
                ..Default::default()
            })
            .collect();
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        let mut m = RecordedMatch::new(
            data.clone(),
            Arc::new(StubRules),
            MatchSetup {
                members,
                seed,
                weights: ScoreWeights::default(),
                seed256: Some(game_core::rng::Seed256(key)),
                clock: Some(Default::default()),
                rule_params: Default::default(),
                board: None,
            },
            MatchMode::Solo,
        );
        eprintln!("quick_start");
        m.quick_start();
        eprintln!("started: {:?}", m.inner().state().prompt.kind);
        let mut pops = 0u32;
        while !m.inner().ended() && pops < 400_000 {
            for _ in 0..5 {
                m.tick_steps(1);
                if m.inner().ended() {
                    break;
                }
            }
            // No client drives these seats: play them with the real bot policy,
            // through `act` so their commands land in the recorder too.
            let seats: Vec<i32> = m
                .inner()
                .state()
                .players
                .iter()
                .filter(|p| p.bot && !p.bankrupt && !p.left)
                .map(|p| p.member)
                .collect();
            for seat in seats {
                if let Some(cmd) = bot_core::policy::act_for(m.inner(), seat) {
                    let _ = m.act(seat, &cmd);
                }
            }
            pops += 1;
            let st = m.inner().state();
            if pops % 2_000 == 0 {
                eprintln!(
                    "  pop {pops}: phase {} t {} round {} turn {} save {} rec {}",
                    st.phase,
                    st.time_left,
                    st.round,
                    st.turn,
                    m.inner().save().len(),
                    m.recorder_json().len()
                );
            }
            if st.round > 200 {
                m.inner_mut().finish();
            }
        }
        let save = m.inner().save();
        let rec = m.recorder_json();
        let st = m.inner().state();
        println!(
            "game {seed}: {players}p, {} rounds, save {} bytes, rec {} bytes, total {} bytes",
            st.round,
            save.len(),
            rec.len(),
            save.len() + rec.len()
        );
    }
}
