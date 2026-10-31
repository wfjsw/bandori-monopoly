//! Short drift smoke: one seeded two-round game through the sandboxed
//! `WasmRules` and the native `rules-native` backend, compared exactly.
//!
//! `drift.rs` plays the rounds the bot's trust bar wants, over several seeds,
//! and a full-content game at six rounds takes hours on a loaded host. This
//! file is the same comparison at a cap small enough to run with the normal
//! suite, so a backend split shows up here first; the long file stays the
//! authority.

use std::path::Path;
use std::sync::Arc;

use game_core::MatchMode;
use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_rules::WasmRules;
use rules_native::native_rules;

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = rules_native::roster();
    Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .expect("game data"),
    )
}

fn members(n: i32) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect()
}

/// Round-boundary saves plus every event, in order.
fn play(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    seed: u64,
    cap: i32,
) -> (Vec<String>, Vec<String>) {
    let mut m = Match::new(
        data.clone(),
        rules,
        &members(4),
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut checkpoints = Vec::new();
    let mut events = Vec::new();
    let mut last_round = -1;
    let mut last_event = 0i32;
    while !m.ended() {
        m.tick(0.25);
        for e in m.events_since(last_event) {
            last_event = e.id;
            events.push(format!("{e:?}"));
        }
        let st = m.state();
        if st.round != last_round {
            last_round = st.round;
            checkpoints.push(m.save());
        }
        if st.round > cap {
            m.finish();
        }
    }
    (checkpoints, events)
}

#[test]
fn native_and_sandbox_agree_on_a_two_round_game() {
    let data = data();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let wasm = Arc::new(
        WasmRules::load_dir(data.clone(), &dir)
            .expect("read dist/cards")
            .expect("run `node tools/build-ruleset.mjs` first"),
    );
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;

    let seed = 3;
    let cap = 2;
    let (wc, we) = play(&data, wasm as Arc<dyn CardRules>, seed, cap);
    let (nc, ne) = play(&data, native, seed, cap);

    assert_eq!(wc.len(), nc.len(), "round count (checkpoints)");
    for (i, (x, y)) in wc.iter().zip(nc.iter()).enumerate() {
        assert!(
            x == y,
            "checkpoint {i} diverged\n  sandbox: {x}\n  native : {y}"
        );
    }
    assert_eq!(we.len(), ne.len(), "event count");
    for (i, (x, y)) in we.iter().zip(ne.iter()).enumerate() {
        assert!(x == y, "event {i} diverged\n  sandbox: {x}\n  native : {y}");
    }
}
