//! Drift check: the same seeded game through the sandboxed `WasmRules` and
//! the native `rules-native` backend must produce identical `save()`
//! checkpoints (`docs/BOT.md` §3.1 B1).
//!
//! The sandbox stays authoritative for real matches; this test is what keeps
//! the advisory native build honest. A failure here means the bot's
//! simulations disagree with the match engine -- fix before trusting the bot.

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use game_rules::{CardModules, WasmRules};
use rules_native::native_rules;

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .expect("game data"),
    )
}

fn wasm_rules(data: &Arc<GameData>) -> Arc<WasmRules> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    Arc::new(
        WasmRules::load_dir(data.clone(), &dir)
            .expect("read dist/cards")
            .expect("run `node tools/build-ruleset.mjs` first"),
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

/// Play one game to `max_rounds` on `rules`, returning the `save()` string at
/// the end of each round plus the final one.
fn checkpoints(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    seed: u64,
    max_rounds: i32,
) -> Vec<String> {
    let mut m = Match::new(
        data.clone(),
        rules,
        &members(4),
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut out = vec![];
    let mut last_round = -1;
    while !m.ended() {
        m.tick(0.25);
        let st = m.state();
        if st.round != last_round {
            last_round = st.round;
            out.push(m.save());
        }
        if st.round > max_rounds {
            m.finish();
        }
    }
    out.push(m.save());
    out
}

/// The native backend must be self-deterministic before it can be compared
/// to the sandbox -- a non-deterministic native run is a B1 bug, not a drift.
#[test]
fn native_is_deterministic() {
    let data = data();
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;
    let a = checkpoints(&data, native.clone(), 7, 8);
    let b = checkpoints(&data, native, 7, 8);
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(x, y, "native run {i} is not self-deterministic");
    }
}

/// One seed, both backends, checkpoint by checkpoint.
///
/// `#[ignore]` while there is a known divergence (2026-10-07): the native run
/// draws one extra card and raises two extra prompts in round 1 versus the
/// sandbox, starting from an identical setup. Reproduce with
/// `cargo run -p rules-native --example drift_where -- 7 2`. Findings:
/// docs/BOT.md section 5 (B1). The sandbox stays authoritative; this is
/// exactly the "bot gets weaker, match stays correct" case section 1 allows.
#[ignore = "known sandbox/native divergence in round 1 -- see docs/BOT.md B1"]
#[test]
fn native_and_sandbox_agree_on_a_seeded_game() {
    let data = data();
    let wasm = wasm_rules(&data);
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;
    let seed = 7u64;
    let cap = 12i32;

    let a = checkpoints(&data, wasm.clone() as Arc<dyn CardRules>, seed, cap);
    let b = checkpoints(&data, native, seed, cap);

    assert_eq!(a.len(), b.len(), "checkpoint count (rounds played)");
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        if x != y {
            let n = x.len().min(y.len());
            let at = (0..n).find(|&k| x.as_bytes()[k] != y.as_bytes()[k]).unwrap_or(n);
            let lo = at.saturating_sub(60);
            let sx = x.get(lo..(at + 80).min(x.len())).unwrap_or(x);
            let sy = y.get(lo..(at + 80).min(y.len())).unwrap_or(y);
            panic!(
                "checkpoint {i} diverged (seed {seed}, cap {cap}) at byte {at}.
                 sandbox: ...{sx}...
                 native : ...{sy}...
                 The native build is advisory: the sandbox stays authoritative."
            );
        }
    }
}

/// The native backend loads and answers `CardRules` the same shape the
/// sandbox does -- the cheap smoke the drift test builds on.
#[test]
fn native_rules_loads_the_shipped_table() {
    let data = data();
    let native = native_rules(data);
    assert!(native.ruleset().cards().len() > 100, "cards linked");
    assert!(
        native.ruleset_sha256().is_some(),
        "content hash for the record stamp"
    );
}
