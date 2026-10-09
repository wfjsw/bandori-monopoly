//! B0: what an ISMCTS bot's simulations cost on the current engine
//! (`docs/BOT.md` §5). Measures full bot-only games with the real ruleset
//! (`WasmRules`, dist/cards) against the `StubRules` baseline, the cost of a
//! `World` clone, and the cost of a depth-N rollout forked from a mid-game
//! state.
//!
//!   CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!     cargo run -p game-rules --release --features bot-cost --example bot_cost -- [games] [players] [max_rounds]
//!
//! Add `wasmi-native` to the features for the interpreter backend (the browser
//! path) instead of wasmtime. Without `bot-cost` the run still reports times,
//! just not the instantiation / clone counters.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use game_core::data::GameData;
use game_core::engine::{CardRules, Cx, Dest, Flow, Match, StubRules, Trigger, World};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use game_rules::WasmRules;

// --------------------------------------------------------------- counting

#[derive(Default)]
struct EntryCounts {
    play: AtomicU64,
    event: AtomicU64,
    counteract: AtomicU64,
    settle: AtomicU64,
    /// ns spent inside the four heavy entry points (includes instantiate+guest).
    entry_ns: AtomicU64,
}

/// A `CardRules` proxy that counts engine -> rules entry calls (including the
/// halt/replay re-runs). Example-local: no engine change needed for this one.
struct CountingRules {
    inner: WasmRules,
    c: Arc<EntryCounts>,
}

impl CardRules for CountingRules {
    fn ruleset_sha256(&self) -> Option<&str> {
        self.inner.ruleset_sha256()
    }
    fn normal(&self, card: &str) -> bool {
        self.inner.normal(card)
    }
    fn cant_play(&self, cx: &Cx, player: usize, card: &str) -> Option<game_core::msg::Msg> {
        self.inner.cant_play(cx, player, card)
    }
    fn card_props(&self, card: &str) -> BTreeMap<String, i32> {
        self.inner.card_props(card)
    }
    fn has_rule(&self, id: &str) -> bool {
        self.inner.has_rule(id)
    }
    fn settle_tile(&self, cx: &mut Cx, player: usize, tile: usize, main: bool) -> Flow<()> {
        self.c.settle.fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        let r = self.inner.settle_tile(cx, player, tile, main);
        self.c.entry_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
    fn ai_play(&self, cx: &Cx, player: usize, card: &str) -> bool {
        self.inner.ai_play(cx, player, card)
    }
    fn play(&self, cx: &mut Cx, player: usize, card: &str) -> Flow<Dest> {
        self.c.play.fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        let r = self.inner.play(cx, player, card);
        self.c.entry_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
    fn event(&self, cx: &mut Cx, player: usize, id: &str) -> Flow<bool> {
        self.c.event.fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        let r = self.inner.event(cx, player, id);
        self.c.entry_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
    fn counteract(&self, cx: &mut Cx, t: &mut Trigger) -> Flow<()> {
        self.c.counteract.fetch_add(1, Ordering::Relaxed);
        let t0 = Instant::now();
        let r = self.inner.counteract(cx, t);
        self.c.entry_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }
    fn hand_note(&self, cx: &Cx, player: usize, card: &str) -> game_core::msg::Msg {
        self.inner.hand_note(cx, player, card)
    }
}

fn load_data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .expect("game data"),
    )
}

/// The shipped ruleset, the same one the server loads (`dist/cards`).
fn load_rules(data: &Arc<GameData>) -> Arc<WasmRules> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    WasmRules::load_dir(data.clone(), &dir)
        .expect("read dist/cards")
        .expect("run `node tools/build-ruleset.mjs` first")
        .into()
}

fn members(n: i32, mentality: BotMentality) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality,
            ..Default::default()
        })
        .collect()
}

struct GameStats {
    rounds: i64,
    secs: f64,
    prompts: usize,
    end_reason: String,
    state_secs: f64,
    ticks: u64,
}

/// One bot-only game to completion (or `max_rounds`). Also counts the distinct
/// prompt ids it showed (logical prompts = engine halt/replay pairs).
fn play_game(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    members: &[RoomMember],
    seed: u64,
    max_rounds: i32,
) -> GameStats {
    let mut m = Match::new(
        data.clone(),
        rules,
        members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let (mut last_prompt, mut prompts) = (0i32, 0usize);
    let (mut state_ns, mut ticks) = (0u64, 0u64);
    let t0 = Instant::now();
    while !m.ended() {
        m.tick(0.25);
        ticks += 1;
        let s0 = Instant::now();
        let st = m.state();
        state_ns += s0.elapsed().as_nanos() as u64;
        if st.prompt.id != 0 && st.prompt.id != last_prompt {
            last_prompt = st.prompt.id;
            prompts += 1;
        }
        if st.round > max_rounds {
            m.finish();
        }
    }
    let st = m.state();
    GameStats {
        rounds: st.round as i64,
        secs: t0.elapsed().as_secs_f64(),
        prompts,
        end_reason: st.end_reason.clone(),
        state_secs: state_ns as f64 / 1e9,
        ticks,
    }
}

fn sum_games(games: &[GameStats]) -> (f64, f64, f64, BTreeMap<String, usize>, f64, f64) {
    let n = games.len().max(1) as f64;
    let secs: f64 = games.iter().map(|g| g.secs).sum();
    let rounds: f64 = games.iter().map(|g| g.rounds as f64).sum();
    let prompts: f64 = games.iter().map(|g| g.prompts as f64).sum();
    let state_secs: f64 = games.iter().map(|g| g.state_secs).sum();
    let ticks: f64 = games.iter().map(|g| g.ticks as f64).sum();
    let mut reasons = BTreeMap::new();
    for g in games {
        *reasons.entry(g.end_reason.clone()).or_default() += 1;
    }
    (
        secs * 1000.0 / n,
        rounds / n,
        prompts / n,
        reasons,
        state_secs / n,
        ticks / n,
    )
}

// --------------------------------------------------------------- microbenches

/// Clone a `World` in a tight loop; report µs/clone and an approximate deep
/// size (compact postcard encoding -- a lower bound on the heap footprint, the
/// `EventTail` prefix is shared so its messages are not re-counted per clone).
fn clone_bench(w: &World, label: &str) -> f64 {
    for _ in 0..200 {
        std::hint::black_box(w.clone());
    }
    let n = 20_000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        std::hint::black_box(w.clone());
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    // Compact-ish proxy for the deep heap size (JSON is verbose; the EventTail
    // prefix is Arc-shared and re-counted here even though a clone bumps it).
    let bytes = serde_json::to_vec(w).map_or(0, |v| v.len());
    println!(
        "  world clone @{label:5}: {us:.3} µs/clone; ~{} KiB json encoding (shallow {} B)",
        bytes / 1024,
        std::mem::size_of::<World>()
    );
    us
}

/// Fork the mid-game match (`save`/`restore`) and run bots `depth` more rounds.
/// Returns mean seconds per rollout (fork + play) and mean rounds actually run.
fn rollouts(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    snap: &str,
    start_round: i32,
    depth: i32,
    iters: u32,
) -> (f64, f64) {
    let mut secs = 0.0;
    let mut rounds = 0.0;
    for _ in 0..iters {
        let t0 = Instant::now();
        let mut m = Match::restore(data.clone(), rules.clone(), snap).expect("restore");
        let fork = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let mut played = 0.0;
        let mut last = m.world().st.round;
        while !m.ended() {
            m.tick(0.25);
            let r = m.world().st.round;
            if r != last {
                played += (r - last) as f64;
                last = r;
            }
            if m.world().st.round >= start_round + depth {
                m.finish();
            }
        }
        secs += fork + t1.elapsed().as_secs_f64();
        rounds += played;
    }
    (secs / iters as f64, rounds / iters as f64)
}

/// The B2 path (`docs/BOT.md` §3.2): fork the mid-game match with a cheap
/// [`Match::fork`] clone and run bots `depth` more rounds with the answer
/// provider installed, so every prompt is answered inline and each routine /
/// card body runs once instead of once per pause.
fn rollouts_inline(
    base: &Match,
    start_round: i32,
    depth: i32,
    iters: u32,
) -> (f64, f64) {
    use game_core::engine::HeuristicProvider;
    let mut secs = 0.0;
    let mut rounds = 0.0;
    for _ in 0..iters {
        let t0 = Instant::now();
        let mut m = base.fork();
        m.set_provider(Some(Box::new(HeuristicProvider::default())));
        let fork = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let mut played = 0.0;
        let mut last = m.world().st.round;
        while !m.ended() {
            m.tick(0.25);
            let r = m.world().st.round;
            if r != last {
                played += (r - last) as f64;
                last = r;
            }
            if m.world().st.round >= start_round + depth {
                m.finish();
            }
        }
        secs += fork + t1.elapsed().as_secs_f64();
        rounds += played;
    }
    (secs / iters as f64, rounds / iters as f64)
}

/// Run one game, cloning the world at rounds 10 / 50 / 150 on the way.
fn clone_at_stages(data: &Arc<GameData>, rules: Arc<dyn CardRules>, members: &[RoomMember], seed: u64) {
    let mut m = Match::new(
        data.clone(),
        rules,
        members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let marks = [(10i32, "early"), (50, "mid"), (150, "late")];
    let mut next = 0;
    while !m.ended() && next < marks.len() {
        m.tick(0.25);
        let r = m.world().st.round;
        while next < marks.len() && r >= marks[next].0 {
            clone_bench(m.world(), marks[next].1);
            next += 1;
        }
        if m.world().st.round > 200 {
            m.finish();
        }
    }
}

// --------------------------------------------------------------- main

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mentality = raw
        .iter()
        .find_map(|a| BotMentality::parse(a))
        .unwrap_or_default();
    let args: Vec<u64> = raw.iter().filter_map(|a| a.parse().ok()).collect();
    let games = args.first().copied().unwrap_or(20);
    let players = args.get(1).copied().unwrap_or(4) as i32;
    let max_rounds = args.get(2).copied().unwrap_or(200) as i32;
    let backend = if cfg!(feature = "wasmi-native") {
        "wasmi"
    } else {
        "wasmtime"
    };

    let data = load_data();
    let wasm: Arc<WasmRules> = load_rules(&data);
    let counts = Arc::new(EntryCounts::default());
    let counting: Arc<dyn CardRules> = Arc::new(CountingRules {
        inner: WasmRules::new(wasm.ruleset().clone(), data.clone()),
        c: counts.clone(),
    });
    let stub: Arc<dyn CardRules> = Arc::new(StubRules);
    let mem = members(players, mentality);

    println!(
        "backend {backend}, {} games x {players} bots, cap {max_rounds} rounds, mentality {}",
        games,
        mentality.as_str()
    );

    // ---- 1. StubRules baseline (same seeds as sim.rs) --------------------
    let t0 = Instant::now();
    let stub_games: Vec<GameStats> = (0..games.max(50))
        .map(|s| play_game(&data, stub.clone(), &mem, s, max_rounds))
        .collect();
    let (stub_ms, stub_rounds, stub_prompts, stub_reasons, stub_state_s, stub_ticks) =
        sum_games(&stub_games);
    println!(
        "StubRules  : {:.1} ms/game, {:.1} ms/round, {:.1} rounds, {:.1} prompts/game, {:.1}s wall",
        stub_ms,
        stub_ms / stub_rounds.max(1.0),
        stub_rounds,
        stub_prompts,
        t0.elapsed().as_secs_f64()
    );
    println!(
        "             {:.0} ticks/game, {:.1}s/game in m.state() (bench overhead)",
        stub_ticks, stub_state_s
    );
    println!("             end reasons {stub_reasons:?}");

    // ---- 2. WasmRules full games (the real ruleset) ----------------------
    #[cfg(feature = "bot-cost")]
    {
        game_rules::bot_cost::INSTANTIATIONS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::INSTANTIATE_NS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::STORE_NS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::GUEST_NS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::HOST_WORLD_CLONES.store(0, Ordering::Relaxed);
        game_rules::bot_cost::COUNTERACT_WINDOWS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::COUNTERACT_WINDOWS_SKIPPED.store(0, Ordering::Relaxed);
        game_rules::bot_cost::COUNTERACT_PROBES.store(0, Ordering::Relaxed);
        game_rules::bot_cost::COUNTERACT_PROBE_MEMO_HITS.store(0, Ordering::Relaxed);
        game_rules::bot_cost::COUNTERACT_DECLARED.store(0, Ordering::Relaxed);
        game_core::engine::bot_cost::WORLD_CLONES.store(0, Ordering::Relaxed);
    }
    counts.play.store(0, Ordering::Relaxed);
    counts.event.store(0, Ordering::Relaxed);
    counts.counteract.store(0, Ordering::Relaxed);
    counts.settle.store(0, Ordering::Relaxed);
    counts.entry_ns.store(0, Ordering::Relaxed);

    let t0 = Instant::now();
    let wasm_games: Vec<GameStats> = (0..games)
        .map(|s| play_game(&data, counting.clone(), &mem, s, max_rounds))
        .collect();
    let wall = t0.elapsed().as_secs_f64();
    let (wasm_ms, wasm_rounds, wasm_prompts, wasm_reasons, wasm_state_s, wasm_ticks) =
        sum_games(&wasm_games);
    println!(
        "WasmRules  : {:.1} ms/game, {:.1} ms/round, {:.1} rounds, {:.1} prompts/game, {:.1}s wall ({backend})",
        wasm_ms,
        wasm_ms / wasm_rounds.max(1.0),
        wasm_rounds,
        wasm_prompts,
        wall
    );
    println!(
        "             {:.0} ticks/game, {:.1}s/game in m.state() (bench overhead)",
        wasm_ticks, wasm_state_s
    );
    println!("             end reasons {wasm_reasons:?}");
    let (play, event, counteract, settle) = (
        counts.play.load(Ordering::Relaxed),
        counts.event.load(Ordering::Relaxed),
        counts.counteract.load(Ordering::Relaxed),
        counts.settle.load(Ordering::Relaxed),
    );
    let entries = play + event + counteract + settle;
    println!(
        "  entry calls/game: {} total (play {} / event {} / counteract {} / settle {})",
        entries as f64 / games as f64,
        play as f64 / games as f64,
        event as f64 / games as f64,
        counteract as f64 / games as f64,
        settle as f64 / games as f64,
    );

    #[cfg(feature = "bot-cost")]
    {
        use game_core::engine::bot_cost as core_c;
        use game_rules::bot_cost as rules_c;
        let n = games as f64;
        let inst = rules_c::INSTANTIATIONS.load(Ordering::Relaxed);
        let inst_ns = rules_c::INSTANTIATE_NS.load(Ordering::Relaxed);
        let store_ns = rules_c::STORE_NS.load(Ordering::Relaxed);
        let guest_ns = rules_c::GUEST_NS.load(Ordering::Relaxed);
        let host_clones = rules_c::HOST_WORLD_CLONES.load(Ordering::Relaxed);
        let cx_clones = core_c::WORLD_CLONES.load(Ordering::Relaxed);
        let inst_s = inst_ns as f64 / 1e9;
        let store_s = store_ns as f64 / 1e9;
        let guest_s = guest_ns as f64 / 1e9;
        println!(
            "  module runs/game: {:.0} instantiations; {:.0} per prompt; {:.2} per entry call",
            inst as f64 / n,
            inst as f64 / n / wasm_prompts.max(1.0),
            inst as f64 / entries.max(1) as f64,
        );
        println!(
            "  world clones/game: {:.0} cx.world_copy + {:.0} host-boundary ({} total)",
            cx_clones as f64 / n,
            host_clones as f64 / n,
            (cx_clones + host_clones) as f64 / n,
        );
        let wins = rules_c::COUNTERACT_WINDOWS.load(Ordering::Relaxed);
        let wins_skip = rules_c::COUNTERACT_WINDOWS_SKIPPED.load(Ordering::Relaxed);
        let probes = rules_c::COUNTERACT_PROBES.load(Ordering::Relaxed);
        let memo_hits = rules_c::COUNTERACT_PROBE_MEMO_HITS.load(Ordering::Relaxed);
        let declared = rules_c::COUNTERACT_DECLARED.load(Ordering::Relaxed);
        println!(
            "  [反击] windows/game: {:.0} opened, {:.0} skipped (valid-option-first); probes/game {:.0} ({:.0} memo hits); declared/game {:.1}",
            wins as f64 / n,
            wins_skip as f64 / n,
            probes as f64 / n,
            memo_hits as f64 / n,
            declared as f64 / n,
        );
        println!(
            "  time share of {:.1}s wall: fire-up {:.2}s ({:.1}%) [store {:.2}s + instantiate {:.2}s], guest call {:.2}s ({:.1}%), engine shell {:.2}s ({:.1}%)",
            wall,
            store_s + inst_s,
            100.0 * (store_s + inst_s) / wall.max(1e-9),
            store_s,
            inst_s,
            guest_s,
            100.0 * guest_s / wall.max(1e-9),
            wall - inst_s - store_s - guest_s,
            100.0 * (wall - inst_s - store_s - guest_s) / wall.max(1e-9),
        );
        println!(
            "  (guest call includes host import callbacks; per-run avg {:.0} µs fire-up + {:.0} µs guest)",
            (inst_ns + store_ns) as f64 / inst.max(1) as f64 / 1e3,
            guest_ns as f64 / inst.max(1) as f64 / 1e3,
        );
        let entry_s = counts.entry_ns.load(Ordering::Relaxed) as f64 / 1e9;
        println!(
            "  inside CardRules entries: {:.2}s ({:.0}% of wall; nested entries double-count) -- almost all wall time is inside an entry.",
            entry_s,
            100.0 * entry_s / wall.max(1e-9),
        );
    }

    // ---- 3. World clone cost at early / mid / late game ------------------
    println!("world clone (StubRules game, same World shape):");
    clone_at_stages(&data, stub.clone(), &mem, 7);

    // ---- 4. Depth-N rollouts forked from a mid-game state ---------------
    println!("rollouts (fork via save/restore + N rounds of bot play):");
    for (label, rules) in [("StubRules", stub.clone() as Arc<dyn CardRules>), ("WasmRules", counting.clone())] {
        // Find a seed whose game is still alive at round 30; snapshot there.
        let mut snap = None;
        for seed in 0..8u64 {
            let mut m = Match::new(
                data.clone(),
                rules.clone(),
                &mem,
                seed,
                MatchMode::Casual,
                ScoreWeights::default(),
            );
            m.quick_start();
            while !m.ended() && m.state().round < 30 {
                m.tick(0.25);
            }
            if !m.ended() && m.state().round >= 30 {
                snap = Some((m.save(), m.state().round, seed));
                break;
            }
        }
        let Some((snap, start_round, seed)) = snap else {
            println!("  {label}: no mid-game snapshot found");
            continue;
        };
        print!("  {label} (seed {seed}, round {start_round}): ");
        let iters = if label == "WasmRules" {
            if games <= 5 { 5 } else { 15 }
        } else {
            60
        };
        for depth in [1, 2, 3, 5, 10] {
            let (secs, played) = rollouts(
                &data,
                rules.clone(),
                &snap,
                start_round,
                depth,
                iters,
            );
            let us = secs * 1e6;
            print!(
                "N={depth}: {:.0} µs ({:.0}/s, {:.1} rounds) ",
                us,
                1.0 / secs.max(1e-9),
                played
            );
        }
        println!();
        // ---- 5. B2: the answer-provider path (`docs/BOT.md` §3.2) --------
        // Same mid-game state, but forked with `Match::fork` and played with
        // the provider installed -- every prompt answered inline, one forward
        // pass per routine instead of one per pause.
        let mut base = Match::restore(data.clone(), rules.clone(), &snap).expect("restore");
        base.set_provider(Some(Box::new(
            game_core::engine::HeuristicProvider::default(),
        )));
        // (the provider is installed per fork inside `rollouts_inline`; drop
        // the one on the base so the forks start clean)
        base.set_provider(None);
        print!("  {label} inline (fork + provider): ");
        for depth in [1, 2, 3, 5, 10] {
            let (secs, played) = rollouts_inline(&base, start_round, depth, iters);
            let us = secs * 1e6;
            print!(
                "N={depth}: {:.0} µs ({:.0}/s, {:.1} rounds) ",
                us,
                1.0 / secs.max(1e-9),
                played
            );
        }
        println!();
    }
}