//! D2: offline deck-book derivation (`docs/BOT.md` §3.7).
//!
//! Fills `data/deck_book.json` (the D1 format, `game_core::deck_book`) with
//! per-character decks that beat their back-off parent, measured by the
//! engine's own end-of-match score at a round cap under the **real** card
//! rules (`WasmRules`, `dist/cards`) -- StubRules cards do nothing, so they
//! cannot tune a deck.
//!
//! ```text
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!   cargo run -p game-rules --release --example deckbook -- \
//!     --cap-rounds 40 --games-per-candidate 32 \
//!     --characters 户山香澄,美竹兰 --threads 16 --budget-minutes 20 \
//!     --levels me --margin 0 --seed 1 \
//!     --out target/scratch/deckbook/deck_book.json --resume
//! ```
//!
//! * **Candidates** per character: `deck::preset`, single-card drop / add
//!   ablations over `deck::pool`, then greedy one-card swaps from the best so
//!   far. Every candidate passes `deck::clean` / `deck::is_complete`.
//! * **Evaluation**: games with the candidate in the target seat against the
//!   field's current book (or preset) decks; metric = the engine's weighted
//!   score at the cap (`scoring.rs` / `MatchPlayer::score`), plus placement;
//!   common random numbers (the same seed set across candidates); mean ± CI.
//! * **Racing**: successive halving drops weak candidates early.
//! * **Fictitious play**: opponents' decks are rebuilt from the working book
//!   and the shortlist re-evaluated, `--fp-rounds` times.
//! * **Output**: only entries that beat their back-off parent by `--margin`.
//!   `--out` never defaults to `data/deck_book.json` -- a trial must not
//!   overwrite the shipped book.
//!
//! Deterministic given `--seed`; resumable via a checkpoint under
//! `target/scratch/deckbook/`. Each worker thread owns its own `WasmRules`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use game_core::data::{CharacterData, GameData};
use game_core::deck;
use game_core::deck_book::{
    DeckBook, DeckBookEntry, DECK_BOOK_VERSION, POLICY_STANDARD,
};
use game_core::engine::{CardRules, Match};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

// --------------------------------------------------------------- config

#[derive(Debug, Clone, PartialEq)]
enum Level {
    /// `(me)` -- across random tables.
    Me,
    /// `(me, seat, sorted multiset of opponents' bands)`.
    SeatBands,
    /// `(me, sorted multiset of opponents' bands)`.
    Bands,
    /// `(me, seat, opponents' characters in seat order)`.
    Exact,
}

impl Level {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "me" => Some(Self::Me),
            "seat_bands" | "seat-bands" => Some(Self::SeatBands),
            "bands" => Some(Self::Bands),
            "exact" => Some(Self::Exact),
            _ => None,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Me => "me",
            Self::SeatBands => "seat_bands",
            Self::Bands => "bands",
            Self::Exact => "exact",
        }
    }
}

#[derive(Debug, Clone)]
struct Config {
    cap_rounds: i32,
    games_per_candidate: usize,
    /// First successive-halving rung (also the greedy-neighbor rung).
    rung0: usize,
    characters: Vec<String>,
    threads: usize,
    budget: Option<Instant>,
    levels: Vec<Level>,
    margin: f64,
    seed: u64,
    out: PathBuf,
    checkpoint: PathBuf,
    resume: bool,
    fp_rounds: usize,
    greedy_rounds: usize,
    /// Game count for greedy *neighbours* (the winner is topped up to
    /// `games_per_candidate` in the final race). Ranking neighbours needs far
    /// fewer games than certifying a margin.
    greedy_games: usize,
    players: usize,
}

impl Config {
    fn scratch() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch/deckbook")
    }
    fn deadline(&self) -> bool {
        self.budget.is_some_and(|t| Instant::now() >= t)
    }
}

fn parse_args() -> Config {
    let mut cap_rounds = 40i32;
    let mut games_per_candidate = 32usize;
    let mut characters: Vec<String> = Vec::new();
    let mut threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut budget_minutes = 0.0f64;
    let mut levels = vec![Level::Me];
    let mut margin = 0.0f64;
    let mut seed = 1u64;
    let mut out = Config::scratch().join("deck_book.json");
    let mut checkpoint = Config::scratch().join("checkpoint.json");
    let mut resume = false;
    let mut fp_rounds = 1usize;
    let mut greedy_rounds = 2usize;
    let mut greedy_games = 64usize;
    let mut players = 4usize;
    let mut rung0 = 0usize;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut take = |name: &str| -> String {
            i += 1;
            args.get(i).cloned().unwrap_or_else(|| {
                eprintln!("deckbook: {name} needs a value");
                std::process::exit(2);
            })
        };
        match a {
            "--cap-rounds" => cap_rounds = take("--cap-rounds").parse().unwrap_or(40),
            "--games-per-candidate" => {
                games_per_candidate = take("--games-per-candidate").parse().unwrap_or(32)
            }
            "--characters" => {
                characters = take("--characters")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "--threads" => threads = take("--threads").parse().unwrap_or(4).max(1),
            "--budget-minutes" => budget_minutes = take("--budget-minutes").parse().unwrap_or(0.0),
            "--levels" => {
                levels = take("--levels")
                    .split(',')
                    .filter_map(|s| Level::parse(s))
                    .collect();
                if levels.is_empty() {
                    eprintln!("deckbook: --levels needs at least one of me,seat_bands,bands,exact");
                    std::process::exit(2);
                }
            }
            "--margin" => margin = take("--margin").parse().unwrap_or(0.0),
            "--seed" => seed = take("--seed").parse().unwrap_or(1),
            "--out" => out = PathBuf::from(take("--out")),
            "--checkpoint" => checkpoint = PathBuf::from(take("--checkpoint")),
            "--resume" => resume = true,
            "--fp-rounds" => fp_rounds = take("--fp-rounds").parse().unwrap_or(1),
            "--greedy-rounds" => greedy_rounds = take("--greedy-rounds").parse().unwrap_or(2),
            "--greedy-games" => greedy_games = take("--greedy-games").parse().unwrap_or(64),
            "--players" => players = take("--players").parse().unwrap_or(4).clamp(2, 10),
            "--rung0" => rung0 = take("--rung0").parse().unwrap_or(0),
            "--help" | "-h" => {
                println!(
                    "deckbook --cap-rounds N --games-per-candidate N --characters a,b \
                     --threads N --budget-minutes M --levels me|seat_bands|bands|exact \
                     --margin X --seed S --out PATH --resume --fp-rounds N \
                     --greedy-rounds N --greedy-games N --players N --rung0 N --checkpoint PATH"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("deckbook: unknown flag {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let rung0 = if rung0 == 0 {
        games_per_candidate.min(8).max(2)
    } else {
        rung0
    };
    let budget = (budget_minutes > 0.0).then(|| Instant::now() + std::time::Duration::from_secs_f64(budget_minutes * 60.0));
    Config {
        cap_rounds,
        games_per_candidate: games_per_candidate.max(rung0),
        characters,
        threads,
        budget,
        levels,
        margin,
        seed,
        out,
        checkpoint,
        resume,
        fp_rounds,
        greedy_rounds,
        greedy_games: greedy_games.max(rung0),
        players,
        rung0,
    }
}

// --------------------------------------------------------------- stats

#[derive(Debug, Clone, Default)]
struct Stats {
    n: usize,
    sum: f64,
    sumsq: f64,
    rank_sum: f64,
    secs: f64,
}

impl Stats {
    fn push(&mut self, score: f64, rank: f64, secs: f64) {
        self.n += 1;
        self.sum += score;
        self.sumsq += score * score;
        self.rank_sum += rank;
        self.secs += secs;
    }
    fn mean(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.sum / self.n as f64
        }
    }
    fn rank_mean(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.rank_sum / self.n as f64
        }
    }
    /// 95 % CI half-width of the mean (normal approx).
    fn ci95(&self) -> f64 {
        if self.n < 2 {
            return f64::INFINITY;
        }
        let n = self.n as f64;
        let mean = self.mean();
        let var = (self.sumsq - n * mean * mean).max(0.0) / (n - 1.0);
        1.96 * (var / n).sqrt()
    }
}

// --------------------------------------------------------------- candidates

#[derive(Debug, Clone)]
struct Cand {
    /// Stable label (preset / drop:x / add:x / swap:a->b / book).
    id: String,
    cards: Vec<String>,
}

fn cand_key(cards: &[String]) -> Vec<String> {
    let mut v = cards.to_vec();
    v.sort();
    v
}

/// preset + single-card drop + single-card add over `deck::pool`, then the
/// greedy loop adds one-card swaps. All complete legal decks, deduped.
///
/// * **drop:i** -- remove preset card *i* and refill from the pool with *i*
///   excluded (a plain `deck::fill` would just put *i* back).
/// * **add:j** -- add pool card *j* not in the preset, cutting the preset's
///   last card in pool order to stay at 10.
fn candidates(data: &GameData, c: &CharacterData) -> Vec<Cand> {
    let pool: Vec<String> = deck::pool(data, c).into_iter().map(|k| k.id.clone()).collect();
    let preset = deck::preset(data, c);
    let mut out: Vec<Cand> = Vec::new();
    let mut seen: Vec<Vec<String>> = Vec::new();
    let mut push = |id: String, cards: Vec<String>| {
        let k = cand_key(&cards);
        if cards.len() == deck::SIZE && deck::is_complete(data, c, &cards) && !seen.contains(&k) {
            seen.push(k);
            out.push(Cand { id, cards });
        }
    };
    push("preset".into(), preset.clone());
    for card in preset.iter() {
        let mut d: Vec<String> = preset.iter().filter(|x| *x != card).cloned().collect();
        for k in pool.iter() {
            if d.len() >= deck::SIZE {
                break;
            }
            if k != card && !d.contains(k) {
                d.push(k.clone());
            }
        }
        push(format!("drop:{card}"), deck::clean(data, c, &d));
    }
    // The preset card to cut for an `add`: the last one in pool order.
    let cut = pool.iter().rev().find(|p| preset.contains(*p)).cloned();
    for card in pool.iter() {
        if preset.contains(card) {
            continue;
        }
        let mut d: Vec<String> = preset
            .iter()
            .filter(|x| cut.as_deref() != Some(x.as_str()))
            .cloned()
            .collect();
        d.push(card.clone());
        push(format!("add:{card}"), deck::clean(data, c, &d));
    }
    out
}

/// One-card swaps of `base` against the pool (the greedy neighbourhood).
fn swaps(data: &GameData, c: &CharacterData, base: &[String]) -> Vec<Cand> {
    let pool: Vec<String> = deck::pool(data, c).into_iter().map(|k| k.id.clone()).collect();
    let mut out: Vec<Cand> = Vec::new();
    let mut seen: Vec<Vec<String>> = Vec::new();
    for (i, card) in base.iter().enumerate() {
        for other in pool.iter() {
            if base.contains(other) {
                continue;
            }
            let mut d = base.to_vec();
            d[i] = other.clone();
            let cleaned = deck::clean(data, c, &d);
            let k = cand_key(&cleaned);
            if cleaned.len() == deck::SIZE && !seen.contains(&k) {
                seen.push(k);
                out.push(Cand {
                    id: format!("swap:{card}->{other}"),
                    cards: cleaned,
                });
            }
        }
    }
    out
}

// --------------------------------------------------------------- tables

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Opponent characters for one game, deterministic in the seed. Distinct, and
/// never `me`.
fn sample_opponents(names: &[String], me: &str, seed: u64, n: usize) -> Vec<String> {
    let mut pool: Vec<&String> = names.iter().filter(|c| c.as_str() != me).collect();
    let mut out = Vec::new();
    let mut s = seed;
    for _ in 0..n.min(pool.len()) {
        s = splitmix64(s);
        let k = (s as usize) % pool.len();
        out.push(pool.remove(k).clone());
    }
    out
}

/// Opponents constrained to one character per band in `bands` (a seat_bands /
/// bands key), deterministic in the seed.
fn sample_opponents_by_bands(
    data: &GameData,
    me: &str,
    bands: &[String],
    seed: u64,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut s = seed;
    for band in bands {
        let mut pool: Vec<&String> = data
            .characters
            .iter()
            .filter(|c| c.name != me && &c.band == band)
            .map(|c| &c.name)
            .collect();
        if pool.is_empty() {
            continue;
        }
        s = splitmix64(s);
        let k = (s as usize) % pool.len();
        out.push(pool.remove(k).clone());
    }
    out
}

/// One derivation unit: a book key plus how to sample tables for it.
#[derive(Debug, Clone)]
struct Task {
    level: Level,
    me: String,
    seat: Option<u32>,
    opponent_bands: Vec<String>,
    opponents: Vec<String>,
    /// Back-off parent: the deck this entry must beat by `margin`. `None` = the
    /// designer preset (`deck::preset`), the level-5 back-off.
    parent: Option<Vec<String>>,
}

impl Task {
    fn id(&self) -> String {
        match self.level {
            Level::Me => format!("me:{}", self.me),
            Level::SeatBands => format!(
                "seat_bands:{}:{}:{}",
                self.me,
                self.seat.unwrap_or(0),
                self.opponent_bands.join("+")
            ),
            Level::Bands => format!("bands:{}:{}", self.me, self.opponent_bands.join("+")),
            Level::Exact => format!(
                "exact:{}:{}:{}",
                self.me,
                self.seat.unwrap_or(0),
                self.opponents.join("+")
            ),
        }
    }
}

/// The character names, in `characters.json` order.
fn char_names(data: &GameData) -> Vec<String> {
    data.characters.iter().map(|c| c.name.clone()).collect()
}

/// Build the task list for the requested levels.
fn build_tasks(data: &GameData, cfg: &Config) -> Vec<Task> {
    let names = char_names(data);
    let me_chars: Vec<String> = if cfg.characters.is_empty() {
        names.clone()
    } else {
        cfg.characters
            .iter()
            .filter(|n| data.character(n).is_some())
            .cloned()
            .collect()
    };
    let mut tasks = Vec::new();
    for me in &me_chars {
        for level in &cfg.levels {
            match level {
                Level::Me => tasks.push(Task {
                    level: Level::Me,
                    me: me.clone(),
                    seat: None,
                    opponent_bands: vec![],
                    opponents: vec![],
                    parent: None,
                }),
                Level::SeatBands => {
                    // One representative band multiset per seat: the bands of
                    // three seed-derived opponents, held fixed so the key is
                    // coherent across the task's games.
                    for seat in 0..cfg.players as u32 {
                        let s = splitmix64(cfg.seed ^ (seat as u64) ^ 0xBAAD_5EED);
                        let opp = sample_opponents(&names, me, s, cfg.players - 1);
                        let mut bands: Vec<String> = opp
                            .iter()
                            .map(|o| {
                                data.character(o).map(|c| c.band.clone()).unwrap_or_default()
                            })
                            .collect();
                        bands.sort();
                        tasks.push(Task {
                            level: Level::SeatBands,
                            me: me.clone(),
                            seat: Some(seat),
                            opponent_bands: bands,
                            opponents: vec![],
                            parent: None,
                        });
                    }
                }
                Level::Bands => {
                    let s = splitmix64(cfg.seed ^ 0xBAAD);
                    let opp = sample_opponents(&names, me, s, cfg.players - 1);
                    let mut bands: Vec<String> = opp
                        .iter()
                        .map(|o| data.character(o).map(|c| c.band.clone()).unwrap_or_default())
                        .collect();
                    bands.sort();
                    tasks.push(Task {
                        level: Level::Bands,
                        me: me.clone(),
                        seat: None,
                        opponent_bands: bands,
                        opponents: vec![],
                        parent: None,
                    });
                }
                Level::Exact => {
                    let s = splitmix64(cfg.seed ^ 0xE0AC);
                    let opp = sample_opponents(&names, me, s, cfg.players - 1);
                    tasks.push(Task {
                        level: Level::Exact,
                        me: me.clone(),
                        seat: Some(0),
                        opponent_bands: vec![],
                        opponents: opp,
                        parent: None,
                    });
                }
            }
        }
    }
    tasks
}

// --------------------------------------------------------------- one game

struct GameResult {
    score: f64,
    rank: f64,
    secs: f64,
}

/// Opponent deck from the working book, else the preset.
fn field_deck(
    data: &GameData,
    book: &DeckBook,
    sha: &str,
    seat: usize,
    character: &str,
    others: &[String],
) -> Vec<String> {
    let Some(c) = data.character(character) else {
        return vec![];
    };
    book.lookup(data, c, seat, others, sha)
        .unwrap_or_else(|| deck::preset(data, c))
}

/// One capped game with `target_deck` in the target seat. `None` when no seed
/// in the deterministic retry chain fits the task (seat constraint) or the
/// table cannot be built. Retrying `splitmix64(seed + attempt)` keeps common
/// random numbers across candidates: every candidate walks the same chain.
fn play_one(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    book: &DeckBook,
    sha: &str,
    task: &Task,
    target_deck: &[String],
    seed: u64,
    cfg: &Config,
) -> Option<GameResult> {
    for attempt in 0..32u64 {
        if let Some(r) = play_one_seed(
            data,
            rules.clone(),
            book,
            sha,
            task,
            target_deck,
            splitmix64(seed.wrapping_add(attempt)),
            cfg,
        ) {
            return Some(r);
        }
        if task.seat.is_none() && task.level == Level::Me {
            return None; // no constraint to retry against
        }
    }
    None
}

fn play_one_seed(
    data: &Arc<GameData>,
    rules: Arc<dyn CardRules>,
    book: &DeckBook,
    sha: &str,
    task: &Task,
    target_deck: &[String],
    seed: u64,
    cfg: &Config,
) -> Option<GameResult> {
    let names = char_names(data);
    let opps = match task.level {
        Level::Me => sample_opponents(&names, &task.me, seed, cfg.players - 1),
        Level::SeatBands | Level::Bands => {
            sample_opponents_by_bands(data, &task.me, &task.opponent_bands, seed)
        }
        Level::Exact => {
            if task.opponents.len() != cfg.players - 1 {
                return None;
            }
            task.opponents.clone()
        }
    };
    if opps.len() != cfg.players - 1 {
        return None;
    }
    let mut chars = vec![task.me.clone()];
    chars.extend(opps);
    // All seats start non-bot so nothing auto-submits a deck; each is handed
    // its list explicitly, then `member_left` hands it to the engine (the same
    // path a disconnected human takes). Mentality stays Standard.
    let members: Vec<RoomMember> = chars
        .iter()
        .enumerate()
        .map(|(i, c)| RoomMember {
            id: i as i32 + 1,
            player: format!("P{}", i + 1),
            character: c.clone(),
            bot: false,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data.clone(),
        rules,
        &members,
        seed,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    let st = m.state();
    if st.phase != "deck" {
        return None;
    }
    let Some(target_seat) = st.players.iter().position(|p| p.character == task.me) else {
        return None;
    };
    if let Some(want) = task.seat {
        if target_seat as u32 != want {
            return None;
        }
    }
    // Per-seat decks: the candidate for us, the working book for the field.
    let mut decks: Vec<(i32, Vec<String>)> = Vec::new();
    for (i, p) in st.players.iter().enumerate() {
        let list = if p.character == task.me {
            target_deck.to_vec()
        } else {
            let others: Vec<String> = st
                .players
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, q)| q.character.clone())
                .collect();
            field_deck(data, book, sha, i, &p.character, &others)
        };
        if list.len() != deck::SIZE {
            return None;
        }
        decks.push((p.member, list));
    }
    for (member, cards) in &decks {
        let msg = NetMessage {
            act: "deck".into(),
            cards: cards.clone(),
            ..NetMessage::default()
        };
        m.act(*member, &msg).ok()?;
    }
    for (member, _) in &decks {
        m.member_left(*member, false);
    }
    let t0 = Instant::now();
    // `world()` is the raw state; `state()` builds a full public snapshot and
    // is far too dear to call every tick (the B0 bench does, we must not).
    while !m.ended() {
        m.tick(1.0);
        let st = &m.world().st;
        if st.phase == "ended" {
            break;
        }
        if st.round > cfg.cap_rounds {
            m.finish();
            break;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let st = m.state();
    let me = st.players.iter().find(|p| p.character == task.me)?;
    Some(GameResult {
        score: me.score as f64,
        rank: me.rank as f64,
        secs,
    })
}

// --------------------------------------------------------------- worker pool

struct Pool {
    stop: AtomicBool,
    games: AtomicU64,
    game_secs: AtomicU64, // µs of single-game wall (the per-game cost)
}

impl Pool {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            games: AtomicU64::new(0),
            game_secs: AtomicU64::new(0),
        }
    }
}

/// Run `jobs` across `cfg.threads` workers. Returns (games played, summed
/// per-game seconds) -- the per-game cost, not the wall time of the rung.
fn run_jobs<J, F>(
    cfg: &Config,
    data: &Arc<GameData>,
    ruleset: &Ruleset,
    jobs: Vec<J>,
    f: F,
) -> (u64, f64)
where
    J: Send + Clone,
    F: Fn(&Arc<dyn CardRules>, &J) -> Option<f64> + Send + Sync,
{
    if jobs.is_empty() {
        return (0, 0.0);
    }
    let queue = Mutex::new(jobs.into_iter());
    let pool = Arc::new(Pool::new());
    std::thread::scope(|scope| {
        for _ in 0..cfg.threads.min(64) {
            let queue = &queue;
            let pool = &pool;
            let f = &f;
            let data = data.clone();
            let ruleset = ruleset.clone();
            scope.spawn(move || {
                let rules: Arc<dyn CardRules> = Arc::new(WasmRules::new(ruleset, data));
                loop {
                    if pool.stop.load(Ordering::Relaxed) || cfg.deadline() {
                        pool.stop.store(true, Ordering::Relaxed);
                        break;
                    }
                    let job = {
                        let mut q = queue.lock().unwrap();
                        q.next()
                    };
                    let Some(job) = job else { break };
                    if let Some(secs) = f(&rules, &job) {
                        pool.games.fetch_add(1, Ordering::Relaxed);
                        pool.game_secs
                            .fetch_add((secs * 1e6) as u64, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    (
        pool.games.load(Ordering::Relaxed),
        pool.game_secs.load(Ordering::Relaxed) as f64 / 1e6,
    )
}

// --------------------------------------------------------------- racing

struct RaceState {
    /// cand id -> running stats (checkpointable within a task).
    stats: BTreeMap<String, Stats>,
}

impl RaceState {
    fn new() -> Self {
        Self {
            stats: BTreeMap::new(),
        }
    }
}

/// One `(cand, seed_idx)` draw. The seed is a function of (cfg.seed, task,
/// seed_idx) alone -- common random numbers across candidates.
fn game_seed(cfg: &Config, task: &Task, seed_idx: usize) -> u64 {
    let mut h = cfg.seed;
    for b in task.id().bytes() {
        h = splitmix64(h ^ b as u64);
    }
    splitmix64(h ^ (seed_idx as u64).wrapping_mul(0x1000_0000_01B3))
}

/// What a race produced: the per-candidate stats plus the raw cost.
struct RaceOut {
    stats: BTreeMap<String, Stats>,
    games: u64,
    game_secs: f64,
}

/// Evaluate `cands` with successive halving. Returns the final stats (the
/// winner is the highest mean; the caller compares against the parent).
fn race(
    cfg: &Config,
    data: &Arc<GameData>,
    ruleset: &Ruleset,
    book: &DeckBook,
    sha: &str,
    task: &Task,
    cands: &[Cand],
    state: &mut RaceState,
    target_games: usize,
    label: &str,
) -> RaceOut {
    let mut alive: Vec<String> = cands.iter().map(|c| c.id.clone()).collect();
    // The parent (or preset) always survives to the full budget so the margin
    // comparison is measured on equal footing.
    let anchor = cands
        .iter()
        .find(|c| c.id == "preset")
        .or_else(|| cands.first())
        .map(|c| c.id.clone());
    let mut rung = cfg.rung0.min(target_games).max(2);
    let mut games_total = 0u64;
    let mut game_secs_total = 0.0f64;
    loop {
        let games_needed: Vec<(String, usize)> = alive
            .iter()
            .filter_map(|id| {
                let n = state.stats.get(id).map(|s| s.n).unwrap_or(0);
                if n < rung {
                    Some((id.clone(), n))
                } else {
                    None
                }
            })
            .collect();
        if !games_needed.is_empty() {
            let by_id: BTreeMap<String, &Cand> =
                cands.iter().map(|c| (c.id.clone(), c)).collect();
            let mut jobs: Vec<(String, usize)> = Vec::new();
            for (id, have) in &games_needed {
                for idx in *have..rung {
                    jobs.push((id.clone(), idx));
                }
            }
            let results: Arc<Mutex<Vec<(String, usize, GameResult)>>> =
                Arc::new(Mutex::new(Vec::new()));
            {
                let results = results.clone();
                let by_id = &by_id;
                let (ng, gs) = run_jobs(cfg, data, ruleset, jobs, move |rules, (id, idx)| {
                    let Some(cand) = by_id.get(id) else {
                        return None;
                    };
                    let seed = game_seed(cfg, task, *idx);
                    let r = play_one(
                        data,
                        rules.clone(),
                        book,
                        sha,
                        task,
                        &cand.cards,
                        seed,
                        cfg,
                    )?;
                    let secs = r.secs;
                    results.lock().unwrap().push((id.clone(), *idx, r));
                    Some(secs)
                });
                games_total += ng;
                game_secs_total += gs;
            }
            for (id, _idx, r) in results.lock().unwrap().drain(..) {
                state
                    .stats
                    .entry(id)
                    .or_default()
                    .push(r.score, r.rank, r.secs);
            }
        }
        eprintln!(
            "  [{label}] rung {rung}: {} cands alive, {} games played",
            alive.len(),
            state.stats.values().map(|s| s.n).sum::<usize>()
        );
        if rung >= target_games || cfg.deadline() {
            break;
        }
        // Prune only while there is a real field left to halve.
        if alive.len() <= 2 {
            rung = (rung * 2).min(target_games);
            continue;
        }
        // Keep the top half by mean, plus the anchor.
        let mut ranked: Vec<(f64, String)> = alive
            .iter()
            .map(|id| {
                let s = state.stats.get(id).cloned().unwrap_or_default();
                (s.mean(), id.clone())
            })
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let keep = (alive.len() + 1) / 2;
        alive = ranked.into_iter().take(keep).map(|(_, id)| id).collect();
        if let Some(a) = &anchor {
            if !alive.contains(a) {
                alive.push(a.clone());
            }
        }
        rung = (rung * 2).min(target_games);
    }
    RaceOut {
        stats: state.stats.clone(),
        games: games_total,
        game_secs: game_secs_total,
    }
}

/// Greedy one-card local search from `start`.
fn greedy(
    cfg: &Config,
    data: &Arc<GameData>,
    ruleset: &Ruleset,
    book: &DeckBook,
    sha: &str,
    task: &Task,
    me: &CharacterData,
    start: &Cand,
    state: &mut RaceState,
    target_games: usize,
) -> (Cand, u64, f64) {
    let mut best = start.clone();
    let mut games_total = 0u64;
    let mut game_secs_total = 0.0f64;
    for round in 0..cfg.greedy_rounds {
        if cfg.deadline() {
            break;
        }
        let mut nbrs = swaps(data, me, &best.cards);
        if nbrs.is_empty() {
            break;
        }
        nbrs.push(best.clone());
        let mut st = RaceState::new();
        // Share the parent's stats so the incumbent keeps its games.
        if let Some(s) = state.stats.get(&best.id) {
            st.stats.insert(best.id.clone(), s.clone());
        }
        let out = race(
            cfg,
            data,
            ruleset,
            book,
            sha,
            task,
            &nbrs,
            &mut st,
            target_games.min(cfg.greedy_games),
            &format!("{} g{}", task.id(), round),
        );
        games_total += out.games;
        game_secs_total += out.game_secs;
        for (k, v) in &st.stats {
            state.stats.insert(k.clone(), v.clone());
        }
        let (winner, ws) = out
            .stats
            .iter()
            .max_by(|a, b| {
                a.1.mean()
                    .partial_cmp(&b.1.mean())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap_or((best.id.clone(), Stats::default()));
        let incumbent = out.stats.get(&best.id).cloned().unwrap_or_default();
        eprintln!(
            "  [{} g{}] best {} mean {:.1} (n={}) vs incumbent {} {:.1}",
            task.id(),
            round,
            winner,
            ws.mean(),
            ws.n,
            best.id,
            incumbent.mean()
        );
        if winner != best.id && ws.mean() > incumbent.mean() {
            let Some(c) = nbrs.iter().find(|c| c.id == winner) else {
                break;
            };
            best = c.clone();
        } else {
            break;
        }
    }
    (best, games_total, game_secs_total)
}

// --------------------------------------------------------------- checkpoint

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct TaskRecord {
    best_id: String,
    best_cards: Vec<String>,
    best_mean: f64,
    best_ci: f64,
    best_n: usize,
    parent_mean: f64,
    parent_n: usize,
    emitted: bool,
    secs: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Checkpoint {
    seed: u64,
    ruleset_sha256: String,
    cap_rounds: i32,
    games_per_candidate: usize,
    fp_round: usize,
    /// task id -> record. A task present here is done (for the current fp round).
    done: BTreeMap<String, TaskRecord>,
    /// ms/game rolling average, for the time estimate.
    ms_per_game: f64,
}

fn load_checkpoint(cfg: &Config) -> Checkpoint {
    if !cfg.resume {
        return Checkpoint::default();
    }
    let Ok(text) = std::fs::read_to_string(&cfg.checkpoint) else {
        return Checkpoint::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_checkpoint(cfg: &Config, cp: &Checkpoint) {
    if let Some(dir) = cfg.checkpoint.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(cp) {
        let _ = std::fs::write(&cfg.checkpoint, text);
    }
}

// --------------------------------------------------------------- output

fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Civil date from days since 1970-01-01 (Hinnant).
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} deckbook-tool")
}

fn entry(task: &Task, cards: &[String]) -> DeckBookEntry {
    DeckBookEntry {
        me: task.me.clone(),
        seat: task.seat,
        opponents: task.opponents.clone(),
        opponent_bands: task.opponent_bands.clone(),
        cards: cards.to_vec(),
    }
}

// --------------------------------------------------------------- main

fn main() {
    let cfg = parse_args();
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let ruleset_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let data: Arc<GameData> = Arc::new(
        GameData::load(|f| std::fs::read_to_string(data_dir.join(f)).map_err(|e| e.to_string()))
            .expect("game data"),
    );
    // Compile the card modules once; workers wrap clones of this `Ruleset`.
    let warm = WasmRules::load_dir(data.clone(), &ruleset_dir)
        .expect("read dist/cards")
        .expect("run `node tools/build-ruleset.mjs` first");
    let sha = warm
        .ruleset_sha256()
        .unwrap_or("stub")
        .to_string();
    let ruleset = warm.ruleset().clone();
    println!(
        "deckbook: ruleset {sha}, cap {} rounds, {} games/candidate (rung0 {}), {} threads, levels {:?}",
        cfg.cap_rounds,
        cfg.games_per_candidate,
        cfg.rung0,
        cfg.threads,
        cfg.levels.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
    );
    if let Some(t) = cfg.budget {
        println!(
            "deckbook: budget {:?} from now",
            t.saturating_duration_since(Instant::now())
        );
    }

    let tasks = build_tasks(&data, &cfg);
    if tasks.is_empty() {
        eprintln!("deckbook: no tasks (bad --characters?)");
        std::process::exit(2);
    }
    println!("deckbook: {} tasks", tasks.len());

    let mut cp = load_checkpoint(&cfg);
    if cp.ruleset_sha256 != sha && !cp.done.is_empty() {
        eprintln!(
            "deckbook: checkpoint ruleset {} != {sha} -- starting fresh",
            cp.ruleset_sha256
        );
        cp = Checkpoint::default();
    }
    cp.seed = cfg.seed;
    cp.ruleset_sha256 = sha.clone();
    cp.cap_rounds = cfg.cap_rounds;
    cp.games_per_candidate = cfg.games_per_candidate;

    // Working book for the field (starts empty = preset). Rebuilt after each
    // derivation pass and between fictitious-play rounds.
    let mut book = DeckBook::default();
    book.version = DECK_BOOK_VERSION;
    book.ruleset_sha256 = sha.clone();
    book.policy = POLICY_STANDARD.into();
    book.generated_at = now_stamp();

    let mut game_ms = cp.ms_per_game;
    let mut total_games = 0u64;
    let mut total_secs = 0.0f64;

    // ---- pass 0: derive -------------------------------------------------
    for fp in 0..=cfg.fp_rounds {
        if fp > 0 {
            cp.fp_round = fp;
            // Rebuild the field from the entries emitted so far.
            book.me.clear();
            for (tid, rec) in &cp.done {
                if rec.emitted {
                    if let Some(t) = tasks.iter().find(|t| t.id() == *tid) {
                        book.me.push(entry(t, &rec.best_cards));
                    }
                }
            }
            eprintln!(
                "deckbook: fictitious play round {fp} -- field book has {} me entries",
                book.me.len()
            );
            // Re-derive against the new field. Entries are updated in place --
            // a budget stop must keep the previous round's results, not wipe
            // them. On FP rounds only the shortlist (current best + preset) is
            // re-evaluated; a full re-search per round multiplies the cost.
            if cfg.deadline() {
                eprintln!("deckbook: budget exhausted -- skipping fictitious play round {fp}");
                break;
            }
            let keep = cp.done.clone();
            for t in &tasks {
                let prior = keep.get(&t.id()).cloned();
                if cfg.deadline() {
                    break;
                }
                derive_one(
                    &cfg,
                    &data,
                    &ruleset,
                    &book,
                    &sha,
                    t,
                    &mut cp,
                    &mut game_ms,
                    &mut total_games,
                    &mut total_secs,
                    prior,
                );
                save_checkpoint(&cfg, &cp);
            }
            continue;
        }
        for t in &tasks {
            if cfg.deadline() {
                break;
            }
            if fp == 0 && cfg.resume && cp.done.contains_key(&t.id()) {
                continue;
            }
            derive_one(
                &cfg,
                &data,
                &ruleset,
                &book,
                &sha,
                t,
                &mut cp,
                &mut game_ms,
                &mut total_games,
                &mut total_secs,
                None,
            );
            save_checkpoint(&cfg, &cp);
        }
    }

    // ---- emit ----------------------------------------------------------
    let mut out = DeckBook::default();
    out.version = DECK_BOOK_VERSION;
    out.ruleset_sha256 = sha.clone();
    out.policy = POLICY_STANDARD.into();
    out.generated_at = now_stamp();
    for (tid, rec) in &cp.done {
        if !rec.emitted {
            continue;
        }
        let Some(t) = tasks.iter().find(|t| t.id() == *tid) else {
            continue;
        };
        let e = entry(t, &rec.best_cards);
        match t.level {
            Level::Me => out.me.push(e),
            Level::SeatBands => out.seat_bands.push(e),
            Level::Bands => out.bands.push(e),
            Level::Exact => out.exact.push(e),
        }
    }
    out.me.sort_by(|a, b| a.me.cmp(&b.me));
    out.seat_bands
        .sort_by(|a, b| (a.seat, a.me.clone()).cmp(&(b.seat, b.me.clone())));
    out.bands.sort_by(|a, b| a.me.cmp(&b.me));
    out.exact.sort_by(|a, b| (a.seat, a.me.clone()).cmp(&(b.seat, b.me.clone())));

    if let Some(dir) = cfg.out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(&out).expect("book serializes");
    std::fs::write(&cfg.out, text).expect("write book");
    cp.ms_per_game = game_ms;
    save_checkpoint(&cfg, &cp);

    println!(
        "deckbook: wrote {} ({} me / {} seat_bands / {} bands / {} exact entries)",
        cfg.out.display(),
        out.me.len(),
        out.seat_bands.len(),
        out.bands.len(),
        out.exact.len(),
    );
    if total_games > 0 {
        let ms = total_secs * 1e3 / total_games as f64;
        println!(
            "deckbook: {} games, {:.0} ms/game average, {:.1} game-s total",
            total_games, ms, total_secs
        );
    }
    // Full-run estimate from the measured ms/game.
    if game_ms > 0.0 {
        for gpc in [300usize, 1000usize] {
            let est = estimate(&tasks, &data, &cfg, gpc, game_ms);
            println!("deckbook: full-run estimate at {gpc} games/candidate: {est}");
        }
    }
}

/// Rough full-run wall time for every character at `gpc` games/candidate.
fn estimate(_tasks: &[Task], data: &GameData, cfg: &Config, gpc: usize, ms_per_game: f64) -> String {
    let n_chars = if cfg.characters.is_empty() {
        data.characters.len()
    } else {
        cfg.characters.len().max(1)
    };
    // Per character: 1 + 10 drops + (pool-10) adds, then `greedy_rounds` swap
    // rounds of ~(pool-10)*10 neighbours, each racing to gpc with halving
    // (≈ 1.33x the final rung across the ladder, plus the anchor's full run).
    let pool = 21usize; // measured median pool
    let ablations = 1 + 10 + (pool.saturating_sub(10));
    let swaps = (pool.saturating_sub(10)) * 10;
    let ladder = 1.4f64;
    let per_char = ablations as f64 * ladder * gpc as f64
        + cfg.greedy_rounds as f64 * swaps as f64 * ladder.min(1.0) * cfg.greedy_games as f64;
    let chars = n_chars.max(1) as f64;
    let games = per_char * chars * (1.0 + cfg.fp_rounds as f64);
    let serial_s = games * ms_per_game / 1e3;
    let wall_h = serial_s / cfg.threads.max(1) as f64 / 3600.0;
    format!(
        "~{:.0}k games/char x {} chars x {} fp-passes = {:.1}M games; {:.0} ms/game / {} threads = {:.1} h wall (serial {:.0} h)",
        per_char / 1e3,
        chars,
        1 + cfg.fp_rounds,
        games / 1e6,
        ms_per_game,
        cfg.threads,
        wall_h,
        serial_s / 3600.0,
    )
}

/// Derive one task: candidates -> race -> greedy -> margin vs parent.
#[allow(clippy::too_many_arguments)]
fn derive_one(
    cfg: &Config,
    data: &Arc<GameData>,
    ruleset: &Ruleset,
    book: &DeckBook,
    sha: &str,
    task: &Task,
    cp: &mut Checkpoint,
    game_ms: &mut f64,
    total_games: &mut u64,
    total_secs: &mut f64,
    prior: Option<TaskRecord>,
) {
    let t0 = Instant::now();
    if cfg.deadline() {
        // Out of budget: keep whatever the previous pass measured.
        if let Some(p) = prior {
            cp.done.entry(task.id()).or_insert(p);
        }
        return;
    }
    let Some(me) = data.character(&task.me) else {
        return;
    };
    let mut cands = candidates(data, me);
    // Fictitious-play rounds: re-evaluate the shortlist only.
    if let Some(p) = &prior {
        if !p.best_id.is_empty() {
            cands.retain(|c| c.id == "preset" || c.id == p.best_id || c.cards == p.best_cards);
            if cands.is_empty() {
                cands = candidates(data, me);
            }
        }
    }
    if cands.is_empty() {
        return;
    }
    let mut state = RaceState::new();
    let label = task.id();
    eprintln!(
        "deckbook: {} -- {} candidates, up to {} games",
        label,
        cands.len(),
        cfg.games_per_candidate
    );
    let out = race(
        cfg,
        data,
        ruleset,
        book,
        sha,
        task,
        &cands,
        &mut state,
        cfg.games_per_candidate,
        &label,
    );
    let mut games_total = out.games;
    let mut game_secs_total = out.game_secs;
    // Greedy local search from the best ablation (unless the budget is gone).
    let mut best = out
        .stats
        .iter()
        .filter(|(id, _)| cands.iter().any(|c| c.id == **id))
        .max_by(|a, b| {
            a.1.mean()
                .partial_cmp(&b.1.mean())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| cands.iter().find(|c| c.id == *id).unwrap().clone())
        .unwrap_or_else(|| cands[0].clone());
    if !cfg.deadline() && cfg.greedy_rounds > 0 {
        let (b, g, s) = greedy(
            cfg,
            data,
            ruleset,
            book,
            sha,
            task,
            me,
            &best,
            &mut state,
            cfg.greedy_games,
        );
        best = b;
        games_total += g;
        game_secs_total += s;
    }
    let parent_cards = task
        .parent
        .clone()
        .unwrap_or_else(|| deck::preset(data, me));
    let parent_id = if parent_cards == deck::preset(data, me) {
        "preset".to_string()
    } else {
        "parent".to_string()
    };
    // Ensure the parent is measured at the same depth as the winner.
    let mut final_cands = vec![best.clone()];
    if parent_id == "preset" {
        if let Some(p) = cands.iter().find(|c| c.id == "preset") {
            if p.cards != best.cards {
                final_cands.push(p.clone());
            }
        }
    } else {
        final_cands.push(Cand {
            id: parent_id.clone(),
            cards: parent_cards.clone(),
        });
    }
    let out = race(
        cfg,
        data,
        ruleset,
        book,
        sha,
        task,
        &final_cands,
        &mut state,
        cfg.games_per_candidate,
        &format!("{label} final"),
    );
    games_total += out.games;
    game_secs_total += out.game_secs;
    let stats = &out.stats;
    let bs = stats.get(&best.id).cloned().unwrap_or_default();
    let ps = stats
        .get(&parent_id)
        .cloned()
        .or_else(|| stats.get("preset").cloned())
        .unwrap_or_default();
    let gain = bs.mean() - ps.mean();
    let emitted = best.cards != parent_cards && gain > cfg.margin && bs.n >= 2;
    eprintln!(
        "deckbook: {} -> {} mean {:.1} ± {:.1} (n={}, rank {:.2}) vs {} {:.1} ± {:.1} (n={}) gain {:+.1} {}",
        label,
        best.id,
        bs.mean(),
        bs.ci95(),
        bs.n,
        bs.rank_mean(),
        parent_id,
        ps.mean(),
        ps.ci95(),
        ps.n,
        gain,
        if emitted { "EMIT" } else { "keep parent" },
    );
    let wall = t0.elapsed().as_secs_f64();
    // A stop with no new games must not overwrite a previous record.
    if games_total == 0 && cp.done.contains_key(&task.id()) {
        return;
    }
    *total_games += games_total;
    *total_secs += game_secs_total;
    if game_secs_total > 0.0 && games_total > 0 {
        let ms = game_secs_total * 1e3 / games_total as f64;
        *game_ms = if *game_ms <= 0.0 {
            ms
        } else {
            *game_ms * 0.7 + ms * 0.3
        };
    }
    eprintln!(
        "deckbook: {} done in {:.1}s wall, {} games, {:.0} ms/game",
        label, wall, games_total,
        if games_total > 0 {
            game_secs_total * 1e3 / games_total as f64
        } else {
            0.0
        }
    );
    cp.done.insert(
        task.id(),
        TaskRecord {
            best_id: best.id.clone(),
            best_cards: best.cards.clone(),
            best_mean: bs.mean(),
            best_ci: bs.ci95(),
            best_n: bs.n,
            parent_mean: ps.mean(),
            parent_n: ps.n,
            emitted,
            secs: wall,
        },
    );
}