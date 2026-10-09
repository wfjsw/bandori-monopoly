//! Drift check: the same seeded game through the sandboxed `WasmRules` and
//! the native `rules-native` backend must produce identical `save()`
//! checkpoints, event streams and final saves (`docs/BOT.md` §3.1 B1).
//!
//! The sandbox stays authoritative for real matches; this test is what keeps
//! the advisory native build honest. A failure here means the bot's
//! simulations disagree with the match engine -- fix before trusting the bot.
//!
//! The suite covers several seeds at a short round cap (fast enough for
//! `cargo test`). `DRIFT_LONG=1` extends to full-length games over more seeds.

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchEvent};
use game_core::MatchMode;
use game_rules::CardModules;
use game_rules::WasmRules;
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

/// One seeded game's observables: the `save()` at every round boundary plus
/// the final one, and every `MatchEvent` in order.
struct Run {
    checkpoints: Vec<String>,
    events: Vec<MatchEvent>,
}

fn play(data: &Arc<GameData>, rules: Arc<dyn CardRules>, seed: u64, max_rounds: i32) -> Run {
    let mut m = Match::new(
        data.clone(),
        rules,
        &members(4),
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut checkpoints = vec![];
    let mut events = vec![];
    let mut last_round = -1;
    let mut last_event = 0i32;
    while !m.ended() {
        m.tick(0.25);
        for e in m.events_since(last_event) {
            last_event = e.id;
            events.push(e);
        }
        let st = m.state();
        if st.round != last_round {
            last_round = st.round;
            checkpoints.push(m.save());
        }
        if st.round > max_rounds {
            m.finish();
        }
    }
    for e in m.events_since(last_event) {
        events.push(e);
    }
    checkpoints.push(m.save());
    Run {
        checkpoints,
        events,
    }
}

/// Context around the first differing byte of two strings.
fn byte_ctx(x: &str, y: &str) -> String {
    let at = (0..x.len().min(y.len()))
        .find(|&k| x.as_bytes()[k] != y.as_bytes()[k])
        .unwrap_or(x.len().min(y.len()));
    let lo = at.saturating_sub(60);
    let sx = x.get(lo..(at + 80).min(x.len())).unwrap_or(x);
    let sy = y.get(lo..(at + 80).min(y.len())).unwrap_or(y);
    format!("at byte {at}\n  sandbox: ...{sx}...\n  native : ...{sy}...")
}

fn assert_same(seed: u64, cap: i32, a: &Run, b: &Run) {
    assert_eq!(
        a.checkpoints.len(),
        b.checkpoints.len(),
        "seed {seed} cap {cap}: checkpoint count (rounds played)"
    );
    for (i, (x, y)) in a.checkpoints.iter().zip(b.checkpoints.iter()).enumerate() {
        assert_eq!(
            x, y,
            "seed {seed} cap {cap}: checkpoint {i} diverged.\n{}\n\
             The native build is advisory: the sandbox stays authoritative.",
            byte_ctx(x, y)
        );
    }
    assert_eq!(
        a.events.len(),
        b.events.len(),
        "seed {seed} cap {cap}: event count"
    );
    for (i, (x, y)) in a.events.iter().zip(b.events.iter()).enumerate() {
        if x != y {
            panic!(
                "seed {seed} cap {cap}: event {i} diverged.\n  sandbox: {x:?}\n  native : {y:?}\n\
                 The native build is advisory: the sandbox stays authoritative."
            );
        }
    }
}

/// The native backend must be self-deterministic before it can be compared
/// to the sandbox -- a non-deterministic native run is a B1 bug, not a drift.
#[test]
fn native_is_deterministic() {
    let data = data();
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;
    let a = play(&data, native.clone(), 7, 8);
    let b = play(&data, native, 7, 8);
    assert_eq!(a.checkpoints.len(), b.checkpoints.len());
    for (i, (x, y)) in a.checkpoints.iter().zip(b.checkpoints.iter()).enumerate() {
        assert_eq!(x, y, "native run {i} is not self-deterministic");
    }
    assert_eq!(a.events.len(), b.events.len());
    for (i, (x, y)) in a.events.iter().zip(b.events.iter()).enumerate() {
        assert_eq!(x, y, "native event {i} is not self-deterministic");
    }
}

/// Several seeds, both backends: identical `save()` checkpoints, identical
/// event streams, identical final saves.
///
/// Root causes closed 2026-10-09 (docs/BOT.md §5 "native-rules drift"):
/// `RulesHandle::pre` defaulted to `None` on `NativeRulesHandle`, so every
/// CEL condition read as absent and G4-deleted guards fired unfiltered; and
/// `card-sdk`'s native ABI passed 64-bit guest buffers as truncated `i32`
/// pointers (`cards_in` &co. returned empty, `rt::leak` handed the host a
/// garbage handle). Both now match the sandbox byte for byte.
#[test]
fn native_and_sandbox_agree_on_a_seeded_game() {
    let data = data();
    let wasm = wasm_rules(&data);
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;

    let long = std::env::var_os("DRIFT_LONG").is_some();
    // 3 seeds x ~6 rounds is ~10 s (two full backends per seed). The long
    // variant plays full games over more seeds -- run it before a bot trust
    // bump, not in the normal suite.
    let (seeds, cap): (u64, i32) = if long { (8, 200) } else { (3, 6) };

    for seed in 0..seeds {
        let a = play(&data, wasm.clone() as Arc<dyn CardRules>, seed, cap);
        let b = play(&data, native.clone(), seed, cap);
        assert_same(seed, cap, &a, &b);
    }
}

/// The two backends must agree on the loaded set itself: same card ids in the
/// same order, same declared properties, same compiled guard conditions.
#[test]
fn native_and_sandbox_agree_on_the_manifest() {
    let data = data();
    let wasm = wasm_rules(&data);
    let native = rules_native::NativeModules::new();

    let wc = wasm.ruleset().cards();
    let nc = native.cards();
    assert_eq!(wc.len(), nc.len(), "card count");
    for (i, (a, b)) in wc.iter().zip(nc.iter()).enumerate() {
        assert_eq!(a.id, b.id, "card {i} id");
        assert_eq!(a.on.len(), b.on.len(), "card {} entry count", a.id);
        for (j, (oa, ob)) in a.on.iter().zip(b.on.iter()).enumerate() {
            assert_eq!(oa.kind, ob.kind, "card {} entry {j} kind", a.id);
            assert_eq!(oa.triggers, ob.triggers, "card {} entry {j} triggers", a.id);
            assert_eq!(oa.pre, ob.pre, "card {} entry {j} condition", a.id);
            assert_eq!(oa.has_guard, ob.has_guard, "card {} entry {j} guard", a.id);
            // The compiled condition must be present exactly when the source
            // string is -- the `RulesHandle::pre` default-`None` bug left
            // every condition silently uncompiled on native.
            let src = oa.pre.as_deref();
            let compiled = native.pre(i as i32, j as i32).is_some();
            assert_eq!(
                src.is_some_and(|s| !s.is_empty()),
                compiled,
                "card {} entry {j}: condition {src:?} compiled={compiled}",
                a.id
            );
        }
        assert_eq!(a.props, b.props, "card {} props", a.id);
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