//! Write a long, real-ruleset `.bdrec` for browser timing checks:
//!
//!   OUT=/tmp/big.bdrec cargo test -p rules-native --test gen_record -- --ignored
//!
//! The game-core test fixture uses `StubRules` and a synthetic roster, so its
//! records end early against the real engine. This one plays a full bot match
//! on the linked ruleset and records every command (format 4), which is what a
//! browser replays.

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::{NetMessage, RoomMember};
use game_core::record::{EngineStamp, MatchSetup, RecordedMatch, RECORD_VERSION};
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), rules_native::roster()).expect("game data"))
}

fn stamp() -> EngineStamp {
    EngineStamp {
        format: RECORD_VERSION,
        save_version: game_core::engine::SAVE_VERSION,
        abi: game_rules::ABI_VERSION as u32,
        ruleset_sha256: "native".into(),
        data_sha256: "native".into(),
        engine: "game-core".into(),
        build: "test".into(),
        bundle: String::new(),
    }
}

fn act_for(m: &Match, seat: i32) -> Option<NetMessage> {
    let st = m.state();
    let me = st.player_of(seat);
    let asked = st.prompt.id > 0 && st.prompt.waiting(me);
    if !asked && (st.phase != "play" || st.busy || st.vote.id > 0 || st.turn != me) {
        return None;
    }
    bot_core::policy::act_for(m, seat)
}

fn pump(rm: &mut RecordedMatch) {
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        let st = rm.inner().state();
        if st.phase == "play" || st.prompt.id == 0 {
            break;
        }
        let p = st.prompt;
        let mut any = false;
        for (k, &s) in p.players.iter().enumerate() {
            if p.answers.get(k).copied().unwrap_or(0) >= 0 {
                continue;
            }
            let member = st.players[s as usize].member;
            let _ = rm.act(
                member,
                &NetMessage {
                    prompt: p.id,
                    value: p.fallback,
                    ..NetMessage::act("answer")
                },
            );
            any = true;
        }
        if !any {
            break;
        }
    }
    let st = rm.inner().state();
    let members: Vec<i32> = if !st.drive.is_empty() {
        st.drive.clone()
    } else {
        st.players.iter().filter(|p| p.bot && !p.bankrupt && !p.left).map(|p| p.member).collect()
    };
    for member in members {
        if let Some(msg) = act_for(rm.inner(), member) {
            if rm.act(member, &msg).is_ok() {
                return;
            }
        }
    }
}

#[test]
#[ignore]
fn write_big_record() {
    let data = data();
    let rules = Arc::new(rules_native::native_rules(data.clone())) as Arc<dyn CardRules>;
    let members: Vec<RoomMember> = (1..=4)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let setup = MatchSetup {
        members,
        seed256: None,
        seed: 33,
        weights: ScoreWeights::default(),
        clock: None,
        ..Default::default()
    };
    let mut rm = RecordedMatch::new(data.clone(), rules.clone(), setup, MatchMode::Casual);
    rm.quick_start();
    pump(&mut rm);
    let mut n = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(1 + (n % 3) as u8);
        pump(&mut rm);
        n += 1;
        if n % 2000 == 0 {
            eprintln!("round {} steps {}", rm.inner().state().round, n);
        }
        if rm.inner().state().round > 80 {
            rm.finish();
        }
        assert!(n < 2_000_000, "game did not progress");
    }
    let file = rm
        .export_with_events(data, rules, stamp(), "2026-10-31 00:00")
        .expect("export");
    let bytes = game_core::record::encode_record_zst(&file);
    std::fs::write(std::env::var("OUT").unwrap(), bytes).unwrap();
    eprintln!("ticks {}", file.header.total_ticks);
}
