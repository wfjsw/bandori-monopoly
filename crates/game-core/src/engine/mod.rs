//! The match engine: the game shell that owns a live match.
//!
//! [`Match`] is the host. It owns the clocks, the live prompt (bot answers, time-outs,
//! auction bidding), the end-match vote, and at most one *pending routine*.
//!
//! Game flow runs as **routines** -- straight-line code over a [`Cx`] (opening hands,
//! a turn start, a player command, a bot decision, leftover auctions). A routine
//! that needs a player decision halts; the host shows the prompt, collects the
//! answer, then re-runs the routine from the snapshot taken when it started, with the
//! answer appended to its log. Because the world (including the RNG and the event /
//! prompt counters) is re-derived deterministically, the re-run reproduces everything
//! up to the prompt -- same dice, same event ids -- and continues past it.
//!
//! Host-side changes made while a routine is pending (vote messages, disconnects,
//! leaving) are deferred until it commits, so they can never collide with replayed
//! event ids.

mod ai;
mod cx;
// Console cheats: debug builds only (`cfg(debug_assertions)`), so a release
// binary / wasm carries no cheat handling at all and a `debug` act is just an
// unknown command there (`docs/SERVER.md`).
#[cfg(debug_assertions)]
mod debug;
mod move_ctx;
mod ops;
mod play;
pub mod rules;
mod setup;
mod tile_text;
mod work;
mod world;

pub use tile_text::compose as compose_tile_texts;

pub use ai::{
    bot_wants_build, bot_wants_buy, bot_wants_force_buy, bot_wants_play_card, bot_wants_redeem,
    wants_build, wants_buy, wants_redeem, BUILD_RESERVE, BUY_RESERVE, CHAOS_COUNTER_CHANCE,
    CHAOS_RESERVE, FORCE_BUY_RESERVE, MAX_PLAYS_PER_TURN, PLAY_CARD_CHANCE, REDEEM_RESERVE,
};
pub use cx::{AnswerProvider, Answered, Ask, Cx, Flow, Halt, HeuristicProvider, Reply, AI_UNSET};
pub use move_ctx::{MoveCtx, MoveKind, Roll};
pub use play::purchase;
pub use play::MAX_SETTLE_DEPTH;
pub use play::{Paid, Pay, PayEvent, PayKind};
pub use rules::{
    CardRules, Dest, Dir, Effect, ExclusiveAsk, Gate, Negation, Payee, RollSource, StubRules,
    TileTextDecl, Trigger, TriggerBuy, TriggerMove, TriggerPay,
};
pub use work::{
    AfterDriveFateWork, ContinueSettleWork, ResumeFnWork, SettleFrame, SettleStage, SettleWork,
    WorkItem,
};
pub use world::{Extreme, Hidden, Lingering, Scheduled, SharedWorld, TurnCtx, TurnSnap, World};

/// Measurement counters for `docs/BOT.md` §5 (B0). Compiled out unless the
/// `bot-cost` feature is on; nothing behavioural either way.
#[cfg(feature = "bot-cost")]
pub mod bot_cost {
    use std::sync::atomic::AtomicU64;
    /// [`Cx::world_copy`] calls -- the per-drive / per-probe `World` clones.
    /// Engine-side routine snapshots (`Match::execute` / `start`) clone the
    /// world too, 1-2 per routine (re)run; those are not counted here.
    pub static WORLD_CLONES: AtomicU64 = AtomicU64::new(0);
    /// Play-gate (`rules.cant_play`) evaluations that actually ran.
    pub static CANT_PLAY_CALLS: AtomicU64 = AtomicU64::new(0);
    /// Play-gate verdicts served from the per-world memo (fix A) instead of
    /// re-running the check.
    pub static CANT_PLAY_MEMO_HITS: AtomicU64 = AtomicU64::new(0);
    /// [`Cx::share_world`] calls -- pure checks that shared the live world
    /// instead of cloning it (fix B).
    pub static WORLD_SHARES: AtomicU64 = AtomicU64::new(0);
}

/// Always-on wall-time buckets for the bot's rollout-cost breakdown
/// (`target/scratch/profile/REPORT-2.md`). Two cheap `AtomicU64` bumps per
/// timed site; nothing behavioural. `reset` / `snapshot` from the harness.
pub mod rtimer {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static STATE_NS: AtomicU64 = AtomicU64::new(0);
    pub static VIEW_EXTRA_NS: AtomicU64 = AtomicU64::new(0);
    pub static LOG_NS: AtomicU64 = AtomicU64::new(0);
    pub static WORLD_COPY_NS: AtomicU64 = AtomicU64::new(0);
    pub static TICK_NS: AtomicU64 = AtomicU64::new(0);
    pub static ACT_NS: AtomicU64 = AtomicU64::new(0);

    /// A start time. `wasm32-unknown-unknown` has no clock -- `Instant::now()`
    /// panics there -- so the browser engine times nothing.
    #[cfg(not(target_arch = "wasm32"))]
    pub type Stamp = std::time::Instant;
    #[cfg(target_arch = "wasm32")]
    pub type Stamp = ();

    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> Stamp {
        std::time::Instant::now()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn now() -> Stamp {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn add(stat: &AtomicU64, t0: Stamp) {
        stat.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn add(_stat: &AtomicU64, _t0: Stamp) {}

    /// Drop-guard so early returns still record.
    pub struct Guard(pub &'static AtomicU64, pub Stamp);
    impl Drop for Guard {
        fn drop(&mut self) {
            add(self.0, self.1);
        }
    }
    pub fn guard(stat: &'static AtomicU64) -> Guard {
        Guard(stat, now())
    }

    pub fn reset() {
        for s in [
            &STATE_NS,
            &VIEW_EXTRA_NS,
            &LOG_NS,
            &WORLD_COPY_NS,
            &TICK_NS,
            &ACT_NS,
        ] {
            s.store(0, Ordering::Relaxed);
        }
    }

    pub fn snapshot() -> [(&'static str, u64); 6] {
        let g = |s: &AtomicU64| s.load(Ordering::Relaxed);
        [
            ("state()", g(&STATE_NS)),
            ("view_extra", g(&VIEW_EXTRA_NS)),
            ("log/event", g(&LOG_NS)),
            ("world_copy", g(&WORLD_COPY_NS)),
            ("tick", g(&TICK_NS)),
            ("act", g(&ACT_NS)),
        ]
    }
}

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// The engine's own AI answer for one seat's live prompt -- the typed form of
/// `view_extra`'s `aiAnswer` (`Ask::ai` / `ai_picked` / `worth`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RawAiAnswer {
    pub answer: i32,
    pub picked: Vec<String>,
    pub worth: i32,
}

/// Typed twin of [`Match::view_extra`] (same fields, no JSON).
#[derive(Debug, Clone, Default)]
pub struct ViewExtra {
    pub ai_answer: Option<RawAiAnswer>,
    pub playable: Vec<bool>,
    pub est_cost: Vec<i32>,
    /// The viewer's pressable skills (`skill:*` field instances with an
    /// `On::Play` entry), each gated through [`why_not_act`].
    pub skills: Vec<SkillAction>,
}

/// Cheap decision-surface probe: the fields the rollout loop needs, without
/// cloning [`MatchState`] or computing the buy/build previews.
#[derive(Debug, Clone, Copy, Default)]
pub struct SurfaceProbe {
    pub round: i32,
    pub turn: i32,
    pub step: i32,
    pub phase_is_play: bool,
    pub busy: bool,
    pub prompt_id: i32,
    /// `prompt.id > 0 && prompt.waiting(seat)`.
    pub prompt_waiting: bool,
    /// `step` is OPS/END and `turn == seat` (a turn surface).
    pub turn_surface: bool,
}

use crate::data::GameData;
use crate::msg::Msg;
use crate::net::{NetMessage, RoomMember};
use crate::rng::Rng;
use crate::scoring::ScoreWeights;
use crate::state::stage;
use crate::state::{MatchEvent, MatchPlayer, MatchState, MatchVote, SkillAction};
use crate::MatchMode;

use cx::HaltKind;
use world::Signal;

/// Longest pause the host keeps after a routine before the next automatic step.
const MAX_WAIT: f32 = 6.0;
/// Pause between pre-game choices (`StartChoice`).
const CHOICE_WAIT: f32 = 1.2;
/// Seconds before the first phase starts (`_wait` in the constructor).
const ORDER_WAIT: f32 = 4.0;
/// Turn-clock defaults. `BASE_TIME` is gone as a reset-to value: the per-turn
/// base refill is [`crate::clock::ClockSettings::base_refill`] (the
/// `turnBaseTime` hook's default), and the bonus cap is
/// [`crate::clock::ClockSettings::bonus_max`]. [`BONUS_REFILL`] remains the
/// `turnBonusTime` hook default -- it is a hook seed, not a pool knob.
const BONUS_REFILL: f32 = 10.0;
/// Hard cap on how long the acting seat's clock may remain held waiting for
/// that seat's `anim_done` ack, in seconds. Fed by ticks (never wall time).
const ANIM_HOLD_CAP: f32 = 10.0;
/// End-match vote duration and cool-down after a failed vote.
const VOTE_SECONDS: f32 = 30.0;
const VOTE_COOLDOWN: f32 = 30.0;
/// Grace period for remote humans after a prompt times out (`TickAsk`).
const REMOTE_GRACE: f32 = 1.5;
/// Minimum seconds left on an auction after a bid (`PlaceBid`).
const AUCTION_EXTEND: f32 = 6.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Routine {
    Opening,
    NextTurn,
    Ai(usize),
    Act(usize, Box<NetMessage>),
    Leftovers(Vec<usize>),
}

/// The acting seat's clock is held until this seat acks `anim_done` for
/// `seq` (or [`ANIM_HOLD_CAP`] seconds of ticks elapse). Only the acting
/// seat's ack matters; bots and Solo never hold. Recorded as
/// [`crate::record::Input::AnimDone`] so replays reproduce the same pauses.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClockHold {
    /// Room member id of the seat that must ack (the acting human).
    member: i32,
    /// State version the ack must name. A lower `seq` is stale and ignored.
    seq: i32,
    /// Seconds held so far (tick-fed; [`ANIM_HOLD_CAP`] releases).
    elapsed: f32,
}

/// The prompt players are currently answering.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Live {
    ask: Ask,
    answers: Vec<i32>,
    picked: Vec<String>,
    ai_at: Vec<Option<f32>>,
    time_left: f32,
    bid: i32,
    bidder: i32,
    /// Whether this prompt draws on the answering seats' time pools (ordinary
    /// prompts) or runs on its own fixed window (bankruptcy leftover auction).
    /// Mirrors [`Ask::use_pool`]; the copy is what `tick_live` reads so a
    /// future `Ask` mutation cannot silently change an in-flight prompt.
    #[serde(default = "default_use_pool")]
    use_pool: bool,
}

fn default_use_pool() -> bool {
    true
}

impl Live {
    fn new(ask: Ask) -> Self {
        let n = ask.view.players.len();
        let use_pool = ask.use_pool;
        Self {
            answers: vec![-1; n],
            picked: vec![],
            ai_at: vec![None; n],
            time_left: ask.view.time_left,
            bid: 0,
            bidder: -1,
            ask,
            use_pool,
        }
    }

    fn kind(&self) -> &str {
        &self.ask.view.kind
    }

    fn place_bid(&mut self, player_id: usize, amount: i32) {
        self.bid = amount;
        self.bidder = player_id as i32;
        self.time_left = self.time_left.max(AUCTION_EXTEND);
        self.ai_at.iter_mut().for_each(|a| *a = None);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pending {
    routine: Routine,
    snapshot: World,
    answers: Vec<Answered>,
    live: Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Deferred {
    Leave(usize),
    Left(usize, bool),
    Back(usize),
    Log(String, i32, Msg),
    Finish(String),
}

/// Everything in a [`Match`] except the shared data and card rules: what
/// [`Match::save`] writes and [`Match::restore`] reads back (refresh recovery).
#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    mode: MatchMode,
    world: World,
    pending: Option<Pending>,
    live_rng: Rng,
    wait: f32,
    bonus_time: Vec<f32>,
    base_time: Vec<f32>,
    /// Match clock / timer settings (see [`crate::clock`]). Constant for the
    /// life of the match; restored alongside the pools so a refresh keeps the
    /// same refill caps and setup windows.
    #[serde(default)]
    clock: crate::clock::ClockSettings,
    /// `true` when this match was built from a `MatchSetup` with `clock: None`
    /// (a pre-settings record). Legacy clock semantics: base-first drain,
    /// base reset-to-hook-value at turn start, prompts never touch a pool.
    /// New matches always write `false`.
    #[serde(default)]
    legacy_clock: bool,
    timed_out: bool,
    vote: MatchVote,
    vote_seq: i32,
    vote_cooldown: f32,
    deferred: Vec<Deferred>,
    seq: i32,
    /// Anim-ack clock hold. `None` when the clock is running.
    #[serde(default)]
    clock_hold: Option<ClockHold>,
}

/// Bumped whenever [`Saved`] changes shape; older saves are rejected.
/// v2: `FieldCard::hand_limit_delta` became the generic `FieldCard::props` map.
/// v5: `World::marker_owner` (marker ownership, user ruling 2026-10-07).
/// Public so a match record's [`crate::record::EngineStamp`] can name the save
/// format it was written against (see `docs/REPLAY.md`).
/// v6: every tile mark / player token carries its owning card instance
/// (`TileMark::instance` / `Counter::instance`), stamped **at creation time**.
/// `restore_saved` is exact (`version != SAVE_VERSION` is refused) and runs no
/// re-attach -- rewriting live state would break save → restore identity.
/// v7: `TurnCtx::plan` (the live `MoveCtx`) is serialized. It carries the roll
/// tables a card shaped for the pending move; skipping it made save/restore
/// lose those tables and the next roll diverged.
/// v8: turn clock renamed (`base_time` / `bonus_time`) and `clock_hold`
/// (anim-ack pause) is part of the saved shape. Older saves are refused.
/// v9: `base_time` is **per seat** (`Vec<f32>`), and the match carries its
/// [`crate::clock::ClockSettings`] plus a `legacy_clock` flag (see
/// [`crate::record::MatchSetup::clock`]). Older saves are refused -- the
/// archived engine bundle is what replays them (`docs/REPLAY.md`).
/// v10: roster/player wire names are neutral (`persona` / `skillOwner` /
/// `affiliations` / `groupSkill`). Older saves are not loaded (SAVE_VERSION).
/// v11: state key `"charge"`; `FieldCard` named counters only (`counters`
/// map); profile `charge*` fields; `StateVar.flags`.
/// v12: idiomatic engine sentinels. `MoveCtx` drops `ROLLER_UNSET` /
/// `-1` / `0 = none` for `roller: Option<Seat>`, `teleport_to` / `start` /
/// `stop_at: Option<Tile>`, `steps: Option<i32>`, `min_roll: Option<i32>`;
/// `Roll` carries `dice: Dice`. `World.recorded_roll` is `Option<i32>`
/// (the `no_recorded_roll` default is gone). `TriggerBuy.deal_owner` is
/// `Option<Payee>` and `deal_houses` is `Option<i32>`. No compat shims --
/// older saves are refused (replays ride archived engine bundles).
pub const SAVE_VERSION: u32 = 12;

/// A running match.
pub struct Match {
    data: Arc<GameData>,
    rules: Arc<dyn CardRules>,
    mode: MatchMode,
    world: World,
    pending: Option<Pending>,
    live_rng: Rng,
    wait: f32,
    /// Per-seat 免费时间 pool (drains first).
    bonus_time: Vec<f32>,
    /// Per-seat 恢复时间 pool (drains second).
    base_time: Vec<f32>,
    timed_out: bool,
    vote: MatchVote,
    vote_seq: i32,
    vote_cooldown: f32,
    deferred: Vec<Deferred>,
    /// Public state version: the identity of one [`MatchState`] frame.
    ///
    /// **Invariant: `seq` moves exactly when [`Self::changed`].** Three
    /// consumers key on it and would otherwise serve a stale frame after a
    /// change it did not observe:
    ///
    /// * the SSE `match` event (`server/src/sse.rs`) only emits when `seq`
    ///   differs from the last one sent -- a change that leaves `seq` alone is
    ///   never broadcast;
    /// * the client's `OnlineExtras` cache (`webui/src/game/viewExtras.ts`)
    ///   is keyed `state.seq:prompt.id`;
    /// * the clock hold is armed with the current `seq` and
    ///   [`Self::anim_done`] treats a lower one as a stale ack.
    ///
    /// The vote used to break this: casting / auto-filling / ending a vote
    /// set `changed` but left `seq` alone, so a vote never reached online
    /// clients. Every `changed = true` site must pair with `seq += 1`.
    ///
    /// Not a gameplay counter: `decision_seed` mixes it only to vary bot
    /// search samples ("never game state", `server/src/lib.rs`), and a record
    /// replays through the engine bundle that wrote it (`docs/REPLAY.md` §9),
    /// so changing where it bumps is not a rules or replay break.
    seq: i32,
    clock_hold: Option<ClockHold>,
    /// Set on every visible change; [`Self::take_changed`] drains it. Must
    /// move with [`Self::seq`] -- see the note there.
    changed: bool,
    /// STACK-01: true while [`Self::flush_deferred`] is draining. Nested
    /// `execute` (Leave → start(Act)) must not re-enter the drain.
    flushing_deferred: bool,
    /// Simulation-mode answers (`docs/BOT.md` §3.2). `None` on a live match:
    /// the halt/replay model is what recordings and the server protocol ride.
    /// A fork installs one so every prompt is answered inline -- one forward
    /// pass per routine instead of one per prompt.
    provider: Option<Box<dyn AnswerProvider>>,
    /// Every [`Answered`] [`Match::complete_prompt`] committed, in order. The
    /// simulation-equivalence tests replay these through an
    /// [`AnswerProvider`] and demand identical checkpoints (B2's gate);
    /// drained with [`Match::take_prompt_log`]. Small enough to keep live
    /// (one entry per prompt).
    prompt_log: Vec<Answered>,
}

impl Match {
    /// Serialize the whole match (not the shared data / rules) to JSON.
    pub fn save(&self) -> String {
        let saved = Saved {
            version: SAVE_VERSION,
            mode: self.mode,
            world: self.world.clone(),
            pending: self.pending.clone(),
            live_rng: self.live_rng.clone(),
            wait: self.wait,
            bonus_time: self.bonus_time.clone(),
            base_time: self.base_time.clone(),
            clock: self.world.clock,
            legacy_clock: self.world.legacy_clock,
            timed_out: self.timed_out,
            vote: self.vote.clone(),
            vote_seq: self.vote_seq,
            vote_cooldown: self.vote_cooldown,
            deferred: self.deferred.clone(),
            seq: self.seq,
            clock_hold: self.clock_hold.clone(),
        };
        serde_json::to_string(&saved).expect("match state serializes")
    }

    /// Rebuild a match written by [`Match::save`].
    pub fn restore(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        json: &str,
    ) -> Result<Self, Msg> {
        let s: Saved = serde_json::from_str(json)
            .map_err(|e| Msg::new("err.save_corrupt").text("detail", e.to_string()))?;
        Self::restore_saved(data, rules, s)
    }

    /// [`Match::restore`] over an already-parsed save document -- the bot's
    /// fork materialisation, which builds the value in memory and must not pay
    /// for a string round trip (`docs/BOT.md` B4).
    pub fn restore_value(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        v: serde_json::Value,
    ) -> Result<Self, Msg> {
        let s: Saved = serde_json::from_value(v)
            .map_err(|e| Msg::new("err.save_corrupt").text("detail", e.to_string()))?;
        Self::restore_saved(data, rules, s)
    }

    fn restore_saved(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        s: Saved,
    ) -> Result<Self, Msg> {
        if s.version != SAVE_VERSION {
            return Err(Msg::new("err.save_version"));
        }
        // `SAVE_VERSION` is exact (older saves are refused above), so every
        // mark / token row is already stamped with its owning `instance` at
        // creation time. No re-attach runs here: rewriting live v6 state
        // would break save → restore identity (fuzz_save_restore).
        let mut world = s.world;
        // v9: clock settings live on the World (so `Cx` sees them mid-routine)
        // and are mirrored at the Saved top level for the bot mirror / docs.
        world.clock = s.clock;
        world.legacy_clock = s.legacy_clock;
        // The record / save carries its own board when one was in force
        // (`MatchSetup::board`); fall back to the host's `data.tiles`.
        let data = match &world.board {
            Some(board) => {
                let mut d = (*data).clone();
                d.tiles = board.clone();
                Arc::new(d)
            }
            None => data,
        };
        Ok(Self {
            data,
            rules,
            mode: s.mode,
            world,
            pending: s.pending,
            live_rng: s.live_rng,
            wait: s.wait,
            bonus_time: s.bonus_time,
            base_time: s.base_time,
            timed_out: s.timed_out,
            vote: s.vote,
            vote_seq: s.vote_seq,
            vote_cooldown: s.vote_cooldown,
            deferred: s.deferred,
            seq: s.seq,
            clock_hold: s.clock_hold,
            changed: true,
            provider: None,
            flushing_deferred: false,
            prompt_log: Vec::new(),
        })
    }

    /// Apply the tunables a [`crate::record::MatchSetup`] records: the named
    /// rule parameters and (when present) the tile table. Call after any
    /// [`Match::new*`] constructor that did not take a setup, so the match
    /// runs on -- and `save()` carries -- exactly what the record names.
    /// Empty `rule_params` / `board: None` mean the code default and the
    /// host's `data.tiles` respectively.
    pub fn apply_setup_tunables(
        &mut self,
        rule_params: &std::collections::BTreeMap<String, i32>,
        board: Option<&[crate::data::TileData]>,
    ) {
        self.world.rule_params = rule_params.clone();
        if let Some(board) = board {
            self.set_board(board.to_vec());
        }
    }

    /// Force the tile table for this match (from [`crate::record::MatchSetup::board`]
    /// or a host that wants a non-default board). Replaces `data.tiles` on a
    /// private copy of the shared [`GameData`] and records the table on the
    /// world so `save()` / restore remain self-contained.
    pub fn set_board(&mut self, board: Vec<crate::data::TileData>) {
        let mut d = (*self.data).clone();
        d.tiles = board.clone();
        self.data = Arc::new(d);
        self.world.board = Some(board);
    }

    /// A match from `(members, seed, mode, weights)`. At most 10 players.
    ///
    /// **Legacy** seeding: xoshiro256\*\* keyed by a `u64`. This is the stream
    /// every record written before the commit-reveal switch runs on, and the
    /// only way to rebuild one byte-identically. New matches go through
    /// [`Match::new_seeded`] (`docs/FAIRNESS.md`). Runs on
    /// [`ClockSettings::default`] with **new** clock semantics; a pre-settings
    /// record rebuilds through [`Match::new_with_clock`] with `legacy = true`.
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        seed: u64,
        mode: MatchMode,
        weights: ScoreWeights,
    ) -> Self {
        Self::new_with_clock(
            data,
            rules,
            members,
            seed,
            mode,
            weights,
            crate::clock::ClockSettings::default(),
            false,
        )
    }

    /// [`Match::new`] with explicit clock settings and the legacy-semantics
    /// flag. `legacy = true` is only for restoring a pre-settings record
    /// (`MatchSetup { clock: None }`): base-first drain, base reset-to-hook
    /// value at turn start, prompts never draw on a pool.
    pub fn new_with_clock(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        seed: u64,
        mode: MatchMode,
        weights: ScoreWeights,
        clock: crate::clock::ClockSettings,
        legacy: bool,
    ) -> Self {
        Self::build(
            data,
            rules,
            members,
            ((seed as i32) & 0x7FFF_FFFF) | 1,
            mode,
            weights,
            Rng::new(seed),
            Rng::new(seed ^ 0x5EED_1A7E),
            clock,
            legacy,
        )
    }

    /// The constructor every **new** match uses: ChaCha12 keyed by the
    /// 256-bit derived seed of the commit-reveal scheme (`docs/FAIRNESS.md`),
    /// with the live-cosmetics stream keyed by [`crate::fair::live_seed`].
    /// Runs on [`ClockSettings::default`] with **new** clock semantics.
    pub fn new_seeded(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        seed: crate::rng::Seed256,
        mode: MatchMode,
        weights: ScoreWeights,
    ) -> Self {
        Self::new_seeded_with_clock(
            data,
            rules,
            members,
            seed,
            mode,
            weights,
            crate::clock::ClockSettings::default(),
            false,
        )
    }

    /// [`Match::new_seeded`] with explicit clock settings and the
    /// legacy-semantics flag. See [`Match::new_with_clock`].
    pub fn new_seeded_with_clock(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        seed: crate::rng::Seed256,
        mode: MatchMode,
        weights: ScoreWeights,
        clock: crate::clock::ClockSettings,
        legacy: bool,
    ) -> Self {
        let key = seed.0;
        // Not seed bits: a match id travels in every public view, so it is
        // hashed out of the derived seed (docs/FAIRNESS.md threat model).
        let id = crate::fair::match_id(&key);
        let live = crate::fair::live_seed(&key);
        Self::build(
            data,
            rules,
            members,
            ((id as i32) & 0x7FFF_FFFF) | 1,
            mode,
            weights,
            Rng::from_seed256(key),
            Rng::from_seed256(live),
            clock,
            legacy,
        )
    }

    /// Shared body of [`Match::new`] / [`Match::new_seeded`]: `match_id` is
    /// already derived from the caller's seed, and the two streams arrive
    /// fully built so the seeding recipe remains in exactly one place per
    /// algorithm.
    fn build(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        match_id: i32,
        mode: MatchMode,
        weights: ScoreWeights,
        world_rng: Rng,
        live_rng: Rng,
        clock: crate::clock::ClockSettings,
        legacy_clock: bool,
    ) -> Self {
        let members = &members[..members.len().min(10)];
        // G-FLOW-12 / G-FLOW-14: the book seats 娱乐 at 3-10 and 正规 at 5-6.
        // `MatchMode::check_seats` is the bound; the lobby (web-glue) is the
        // refusal point. `Match::new` remains a harness constructor so unit
        // tests and the sim can hand-build small tables. Solo is unbounded.
        let w = weights.sanitized(ScoreWeights::default());
        // Solo with a persona already on every seat (the solo setup screen's
        // picks, its randoms resolved before the match is built) skips the
        // timed ban / pick entirely and opens the deck phase instead. Online
        // never takes this path: there `RoomMember::persona` is the lobby
        // avatar, not a match pick, and the pick phase must remain. An unknown
        // or duplicated pick is not a preset -- fall through to the pick phase.
        let preset = mode == MatchMode::Solo
            && !members.is_empty()
            && members.iter().all(|m| data.persona(&m.persona).is_some())
            && {
                let mut names: Vec<&str> = members.iter().map(|m| m.persona.as_str()).collect();
                names.sort_unstable();
                names.dedup();
                names.len() == members.len()
            };
        let st = MatchState {
            match_id,
            phase: "order".into(),
            mode: mode as i32,
            players: members
                .iter()
                .map(|m| MatchPlayer {
                    member: m.id,
                    player: m.player.clone(),
                    bot: m.bot,
                    // `ai` = the engine drives this seat. An Advanced bot is
                    // driven by an external search -- the server's `bot-service`
                    // (`docs/BOT.md` B5) online, the browser worker pool
                    // (`docs/BOT.md` B6) in Solo -- and starts held. Setup
                    // (ban / pick / deck) remains engine-side
                    // (`MatchPlayer::auto_setup`). An online room with no
                    // service attached rewrites the mentality to Standard
                    // before `new_match`; Solo's driver answers through the
                    // ordinary `act` path and falls back to the heuristic, so
                    // a seat is never parked on answers nobody will send.
                    ai: m.bot && m.mentality != crate::state::BotMentality::Advanced,
                    mentality: m.mentality,
                    // Carries through `roll_order`, so the seat that rolls high
                    // keeps the persona its member was given.
                    persona: if preset {
                        m.persona.clone()
                    } else {
                        String::new()
                    },
                    ..MatchPlayer::default()
                })
                .collect(),
            score_money: w.money,
            score_property: w.property,
            score_houses: w.houses,
            time_left: ORDER_WAIT,
            ..MatchState::default()
        };
        let n = st.players.len();
        let mut world = World::new(st, n, world_rng);
        world.clock = clock;
        world.legacy_clock = legacy_clock;
        let mut m = Self {
            world,
            live_rng,
            data,
            rules,
            mode,
            pending: None,
            wait: ORDER_WAIT,
            // Pools start at the match settings, not at zero waiting for the
            // first `StartTimer` to fill them (v9). Legacy matches keep the
            // old empty-start: their first turn *sets* base to the hook value.
            bonus_time: if legacy_clock {
                vec![0.0; n]
            } else {
                vec![clock.bonus_time; n]
            },
            base_time: if legacy_clock {
                vec![0.0; n]
            } else {
                vec![clock.base_time; n]
            },
            timed_out: false,
            vote: MatchVote::default(),
            vote_seq: 0,
            vote_cooldown: 0.0,
            deferred: vec![],
            seq: 0,
            clock_hold: None,
            changed: true,
            provider: None,
            flushing_deferred: false,
            prompt_log: Vec::new(),
        };
        m.direct(|cx| cx.roll_order());
        if preset {
            // Seat order is rolled; take the picks in seat order the way the
            // pick phase would, which logs each one and opens the deck phase
            // (bots submit their decks at the end of `do_pick`).
            m.direct(|cx| {
                cx.w.st.phase = "pick".into();
                cx.w.st.turn = 0;
                for i in 0..cx.w.player_count() {
                    let c = cx.w.st.players[i].persona.clone();
                    cx.do_pick(i, &c);
                }
            });
            m.wait = 0.0;
            // A table of nothing but bots has every deck already; start now
            // rather than sitting in the deck phase until the first tick.
            if m.world.st.players.iter().all(|p| p.deck_ready) {
                m.begin_play();
            }
        }
        m
    }

    // ---------------------------------------------------------------- queries

    /// The public state, with the live prompt, vote and clocks filled in.
    pub fn state(&self) -> MatchState {
        let t0 = rtimer::now();
        let st = self.state_inner();
        rtimer::add(&rtimer::STATE_NS, t0);
        st
    }

    fn state_inner(&self) -> MatchState {
        let mut st = self.world.public_state();
        st.seq = self.seq;
        st.vote = self.vote.clone();
        st.busy = self.pending.is_some();
        if let Some(p) = &self.pending {
            let l = &p.live;
            st.prompt = l.ask.view.clone();
            st.prompt.answers = l.answers.clone();
            // Solo has no answer deadline (see `tick_live`), so don't show a
            // spent bar: report the ask's own allotment instead of the ticking
            // bot-schedule counter underneath it.
            //
            // Table-wide: the tightest pending **human** pool when the prompt
            // draws on pools (each client re-personalises via
            // [`Match::personalize_prompt_time`]); otherwise the soft window
            // `l.time_left` (bots-only, dedicated timers, Solo).
            st.prompt.time_left = self.prompt_time_left(l);
            if l.kind() == "auction" {
                st.prompt.bid = l.bid;
                st.prompt.bidder = l.bidder;
            }
        } else {
            st.prompt = Default::default();
        }
        if st.phase == "play" && st.turn >= 0 {
            // Buy / build are settle-time offers only (`tile:property` /
            // `tile:ring` → `ctx::offer_buy` / `ctx::offer_build`). There is
            // no END-step purchase surface and no purchase previews.
            let ti = st.turn as usize;
            st.can_roll_here = false;
            st.can_end_here = false;
            let bonus = self.bonus_time.get(ti).copied().unwrap_or(0.0);
            let base = self.base_time.get(ti).copied().unwrap_or(0.0);
            st.base_time = base;
            st.bonus_time = bonus;
            st.time_left = base + bonus;
            st.clock_paused = self.clock_hold.is_some();
            // `why_not_act`'s roll / end branches, from the **live** fields the
            // gate reads (`w.st.skip_move`'s latch, `w.turn.main_moved`, the
            // hidden hand). The public `skip_move` is that same live latch --
            // it used to be re-derived from the timed / off-board layers, which
            // disagrees with the gate (a latch that ignores layer expiry, a
            // mid-turn timed layer, an off-board grant this
            // turn by MyGO:无路矢): a policy that trusted it sent `end` into
            // `err.roll_first` forever.
            {
                let live_skip = self.world.st.skip_move;
                let main_moved = self.world.turn.main_moved;
                let held = self.world.hidden.get(ti).map_or(0, |h| h.hand.len());
                let over_hand = st.players.get(ti).is_some_and(|p| p.over_hand(held));
                let my_turn = st.turn == ti as i32;
                let owes_roll = st.step == stage::OPS && !live_skip && !main_moved;
                st.can_roll_here = my_turn
                    && st.step == stage::OPS
                    && !st.busy
                    && !live_skip
                    && !main_moved
                    && st.roller == st.turn;
                st.can_end_here =
                    my_turn && !st.busy && st.step != stage::MOVE && !owes_roll && !over_hand;
            }
        }
        // Deed-popup tile texts (`docs/TILES.md` 「Tile text」): composed from
        // the rule instances' declared texts, in body-precedence order. View
        // only -- never part of `Match::save`.
        st.tile_texts = tile_text::compose(&self.world, &self.data, self.rules.as_ref());
        st
    }

    /// Seat `i`'s pool remaining (bonus + base), clamped at 0 for display.
    fn pool_left(&self, i: usize) -> f32 {
        self.pool_total(i).max(0.0)
    }

    /// Seat `i`'s raw pool total, which may be slightly negative during the
    /// prompt grace window (see [`REMOTE_GRACE`]).
    fn pool_total(&self, i: usize) -> f32 {
        let b = self.bonus_time.get(i).copied().unwrap_or(0.0);
        let base = self.base_time.get(i).copied().unwrap_or(0.0);
        b + base
    }

    /// Drain seat `i`'s pool by `dt`: **bonus first, then base**. Does not
    /// clamp at zero -- the prompt path relies on a small negative total as
    /// the [`REMOTE_GRACE`] window before forcing a default answer.
    fn drain_pool(&mut self, i: usize, dt: f32) {
        if i >= self.bonus_time.len() || i >= self.base_time.len() {
            return;
        }
        if self.bonus_time[i] > 0.0 {
            self.bonus_time[i] -= dt;
            if self.bonus_time[i] < 0.0 {
                // Spill the overshoot into base so the total moves by exactly dt.
                self.base_time[i] += self.bonus_time[i];
                self.bonus_time[i] = 0.0;
            }
        } else {
            self.base_time[i] -= dt;
        }
    }

    /// True while the anim-ack hold is waiting on this seat's client -- their
    /// clock (and prompt pool) must not consume presentation time.
    fn clock_hold_covers(&self, seat: usize) -> bool {
        let Some(hold) = &self.clock_hold else {
            return false;
        };
        self.world
            .st
            .players
            .get(seat)
            .is_some_and(|p| p.member == hold.member)
    }

    /// AUCTION_EXTEND is a **grant**, not a borrow: a bid tops every
    /// still-pending human seat's total remaining up to at least
    /// [`AUCTION_EXTEND`] by adding the shortfall to that seat's `base_time`
    /// (may exceed the refill cap -- the point is to keep the table moving,
    /// not to respect a per-turn budget). `Live::place_bid` separately raises
    /// the soft window `l.time_left` so bots reschedule.
    fn grant_auction_extend(&mut self) {
        if !self
            .pending
            .as_ref()
            .is_some_and(|p| self.prompt_uses_pool(&p.live))
        {
            return;
        }
        let mut grants: Vec<(usize, f32)> = Vec::new();
        if let Some(p) = &self.pending {
            for (k, &s) in p.live.ask.view.players.iter().enumerate() {
                if p.live.answers[k] >= 0 {
                    continue;
                }
                let seat = s as usize;
                if self.auto_player(seat) {
                    continue;
                }
                let total = self.pool_total(seat);
                if total < AUCTION_EXTEND {
                    grants.push((seat, AUCTION_EXTEND - total));
                }
            }
        }
        for (seat, add) in grants {
            if seat < self.base_time.len() {
                self.base_time[seat] += add;
            }
        }
    }

    /// True when this prompt draws on the answering seats' pools (new clock
    /// semantics, ordinary prompts). Bankruptcy leftover auctions and every
    /// pre-settings (`legacy_clock`) prompt run on a fixed window instead.
    fn prompt_uses_pool(&self, l: &Live) -> bool {
        !self.world.legacy_clock && l.use_pool
    }

    /// The table-wide prompt countdown published on [`MatchState::prompt`]:
    /// the **tightest** pending human pool when the prompt draws on pools,
    /// otherwise the soft window `l.time_left` (bots-only prompts, dedicated
    /// timers, Solo). Each client re-personalises via
    /// [`Match::personalize_prompt_time`].
    fn prompt_time_left(&self, l: &Live) -> f32 {
        if self.mode == MatchMode::Solo {
            // Solo has no answer deadline (see `tick_live`), so don't show a
            // spent bar: report the ask's own allotment instead of the ticking
            // bot-schedule counter underneath it.
            return l.ask.view.time_left.max(l.time_left).max(0.0);
        }
        if self.prompt_uses_pool(l) {
            let mut tightest: Option<f32> = None;
            for (k, &s) in l.ask.view.players.iter().enumerate() {
                if l.answers[k] >= 0 {
                    continue;
                }
                let seat = s as usize;
                if self.auto_player(seat) {
                    continue;
                }
                let left = self.pool_left(seat);
                tightest = Some(match tightest {
                    None => left,
                    Some(t) => t.min(left),
                });
            }
            if let Some(t) = tightest {
                return t;
            }
        }
        l.time_left.max(0.0)
    }

    /// Personalise a [`MatchState`] built by [`Match::state`] for one viewer:
    /// when that member is a pending human on a pool-consuming prompt, their
    /// own pool remaining replaces the table-wide tightest. Rules-worker's
    /// `view` op calls this so each client sees its own countdown. No-op on
    /// dedicated timers / legacy prompts / non-pending seats.
    pub fn personalize_prompt_time(&self, st: &mut MatchState, member: i32) {
        let Some(p) = &self.pending else { return };
        let l = &p.live;
        if !self.prompt_uses_pool(l) {
            return;
        }
        let Some(seat) = self.player_index(member) else {
            return;
        };
        let Some(k) = l.ask.view.player_index(seat as i32) else {
            return;
        };
        if l.answers[k] >= 0 || self.auto_player(seat) {
            return;
        }
        st.prompt.time_left = self.pool_left(seat);
    }

    /// True once after every change; the server broadcasts on it.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn ended(&self) -> bool {
        self.world.st.phase == "ended"
    }

    /// Events with id greater than `last_id` (`EventsSince`).
    ///
    /// Clones each event (several heap strings per row) -- take
    /// [`Match::events_since_ref`] when the caller only reads them.
    pub fn events_since(&self, last_id: i32) -> Vec<MatchEvent> {
        self.world
            .recent
            .iter()
            .filter(|e| e.id > last_id)
            .cloned()
            .collect()
    }

    /// Events with id greater than `last_id`, borrowed -- no `MatchEvent`
    /// clone. Same shape as [`Match::hand_len_first`]: the non-cloning
    /// sibling of [`Match::events_since`] for callers that only scan, find,
    /// or map keys. The owned form stays for JSON / SSE / snapshot boundaries
    /// that need a `Vec<MatchEvent>`.
    pub fn events_since_ref(&self, last_id: i32) -> impl Iterator<Item = &MatchEvent> + '_ {
        self.world.recent.iter().filter(move |e| e.id > last_id)
    }

    fn player_index(&self, member: i32) -> Option<usize> {
        self.world
            .st
            .players
            .iter()
            .position(|s| s.member == member)
    }

    pub fn player_of_member(&self, member: i32) -> Option<&MatchPlayer> {
        self.player_index(member).map(|i| &self.world.st.players[i])
    }

    /// The member's hand -- private, never part of [`Match::state`].
    /// Test seam: set a player's persona (exclusive-card checks read it).
    #[doc(hidden)]
    pub fn set_persona(&mut self, member: i32, persona: &str) {
        if let Some(i) = self.player_index(member) {
            self.world.st.players[i].persona = persona.to_string();
            self.world
                .bind_skills(self.data.as_ref(), &*self.rules, i as i32);
        }
    }

    /// Test seam: put cards straight into a player's hand.
    #[doc(hidden)]
    pub fn give_cards(&mut self, member: i32, cards: &[&str]) {
        let Some(i) = self.player_index(member) else {
            return;
        };
        for c in cards {
            self.world.hidden[i].hand.push((*c).to_string());
        }
    }

    /// Test seam: the replayable world, for arranging a scenario (money,
    /// positions, deeds, piles, loaded dice). Only valid with no routine
    /// pending -- a pending routine re-runs from its own snapshot.
    #[doc(hidden)]
    pub fn world_mut(&mut self) -> &mut World {
        assert!(
            self.pending.is_none(),
            "world_mut while a routine is pending"
        );
        self.changed = true;
        self.seq += 1;
        &mut self.world
    }

    #[doc(hidden)]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The 256-bit keys of this match's two streams (`world.rng`, `live_rng`),
    /// when they are ChaCha streams. What `docs/FAIRNESS.md`'s verify step
    /// compares against the derived seed; `None` on the legacy xoshiro
    /// stream.
    pub fn seed256s(&self) -> (Option<[u8; 32]>, Option<[u8; 32]>) {
        (self.world.rng.seed256(), self.live_rng.seed256())
    }

    pub fn hand_of(&self, member: i32) -> Vec<String> {
        self.player_index(member)
            .map(|i| self.world.hidden[i].hand.clone())
            .unwrap_or_default()
    }

    /// Hand length and first card id, without cloning the hand -- the rollout
    /// heuristic's over-hand discard branch only needs these.
    pub fn hand_len_first(&self, member: i32) -> (usize, Option<String>) {
        match self.player_index(member) {
            Some(i) => {
                let h = &self.world.hidden[i].hand;
                (h.len(), h.first().cloned())
            }
            None => (0, None),
        }
    }

    /// The decision surface at `member`'s seat, without cloning [`MatchState`]
    /// and without the buy/build previews. The rollout loop calls this every
    /// tick; [`Match::state`] runs only when a surface is actually found.
    pub fn surface_probe(&self, member: i32) -> SurfaceProbe {
        let st = &self.world.st;
        let seat = self.player_index(member).map(|i| i as i32);
        let prompt = self.pending.as_ref().map(|p| &p.live);
        let (prompt_id, prompt_waiting) = match prompt {
            Some(l) => {
                let id = l.ask.view.id;
                let waiting = id > 0
                    && seat.is_some_and(|s| {
                        l.ask
                            .view
                            .player_index(s)
                            .is_some_and(|k| l.answers.get(k).copied().unwrap_or(-1) < 0)
                    });
                (id, waiting)
            }
            None => (0, false),
        };
        let my_turn = seat.is_some_and(|s| st.turn == s);
        let turn_surface = my_turn
            && self.pending.is_none()
            && (st.step == stage::OPS || st.step == stage::END)
            && st.phase == "play";
        SurfaceProbe {
            round: st.round,
            turn: st.turn,
            step: st.step,
            phase_is_play: st.phase == "play",
            busy: self.pending.is_some(),
            prompt_id,
            prompt_waiting,
            turn_surface,
        }
    }

    /// The member's remaining draw pile, **sorted by card id**.
    ///
    /// The pile's order is the shuffled one the engine draws from, so it never
    /// leaves the engine -- what a caller gets is the *contents*, which is what
    /// a deck list is. Sort the ids however you like for display; sorting here
    /// is what keeps the wire from carrying the draw order.
    pub fn draw_of(&self, member: i32) -> Vec<String> {
        let mut v = self
            .player_index(member)
            .map(|i| self.world.hidden[i].deck.clone())
            .unwrap_or_default();
        v.sort();
        v
    }

    /// Per-card notes for the member's hand.
    pub fn hand_notes_of(&self, member: i32) -> Vec<Msg> {
        let Some(i) = self.player_index(member) else {
            return vec![];
        };
        let cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        self.world.hidden[i]
            .hand
            .iter()
            .map(|c| self.rules.hand_note(&cx, i, c))
            .collect()
    }

    /// Per-tile "what to expect" quotes from `member`'s seat
    /// ([`purchase::tile_quotes`]), parallel to the board's tiles.
    ///
    /// An unknown member (or a spectator / replay viewer with no seat) gets
    /// the seat-independent reading: base price, base rent, force-buy.
    pub fn tile_quotes(&self, member: i32) -> Vec<Option<crate::state::TileQuote>> {
        purchase::tile_quotes(
            &self.data,
            &self.world,
            &*self.rules,
            self.player_index(member),
        )
    }

    /// Per-viewer extras the client's 托管 autopilot reads on top of [`Match::state`].
    ///
    /// Two things the shared [`MatchState`] deliberately does not carry:
    ///
    /// * `aiAnswer` -- the engine's own AI answer for **this** player's live
    ///   prompt (`Ask::ai` / `ai_picked` / `worth`), only while they are still
    ///   waiting on it. Never another seat's entry: an auction ceiling is
    ///   hidden information. That is data, not a takeover -- the seat still
    ///   belongs to the player, and a browser autopilot just relays it through
    ///   the ordinary `act` path.
    /// * `playable` -- parallel to [`Match::hand_of`]: would `cant_play` allow
    ///   each card right now? Recomputed from the world, so the client does not
    ///   have to re-derive the phase / exclusive / status gates.
    /// * `estCost` -- parallel to `playable`: the card's bot-only **estimated
    ///   execution cost** (`prop::EST_COST`, user ruling 2026-10-07). Bots and
    ///   the autopilot read it as a reserve check; it is never legality.
    ///
    /// Both view builders (`web-glue` and `rules-worker`) merge this into the
    /// match frame.
    pub fn view_extra(&self, member: i32) -> serde_json::Value {
        let t0 = rtimer::now();
        let v = self.view_extra_inner(member);
        rtimer::add(&rtimer::VIEW_EXTRA_NS, t0);
        v
    }

    /// Typed twin of [`Match::view_extra`] for in-process callers (the bot's
    /// rollout / action abstraction). Same fields, no JSON round trip.
    pub fn view_extra_typed(&self, member: i32) -> ViewExtra {
        let t0 = rtimer::now();
        let v = self.view_extra_parts(member);
        rtimer::add(&rtimer::VIEW_EXTRA_NS, t0);
        v
    }

    fn view_extra_inner(&self, member: i32) -> serde_json::Value {
        let p = self.view_extra_parts(member);
        let ai_answer = p.ai_answer.map(|a| {
            serde_json::json!({
                "answer": a.answer,
                "picked": a.picked,
                "worth": a.worth,
            })
        });
        serde_json::json!({
            "aiAnswer": ai_answer,
            "playable": p.playable,
            "estCost": p.est_cost,
            "skills": p.skills,
        })
    }

    fn view_extra_parts(&self, member: i32) -> ViewExtra {
        let Some(i) = self.player_index(member) else {
            return ViewExtra::default();
        };
        let ai_answer = self.pending.as_ref().and_then(|p| {
            let l = &p.live;
            let k = l.ask.view.player_index(i as i32)?;
            if l.answers.get(k).copied().unwrap_or(-1) >= 0 {
                return None;
            }
            Some(RawAiAnswer {
                answer: l.ask.ai.get(k).copied().unwrap_or(l.ask.view.fallback),
                picked: l.ask.ai_picked.get(k).cloned().unwrap_or_default(),
                worth: l.ask.worth.get(k).copied().unwrap_or(0),
            })
        });
        let cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        let mut playable = Vec::new();
        let mut est_cost = Vec::new();
        // One `cant_play` per distinct card id: a hand may hold duplicates and
        // the answer is a pure query of the (unchanged) state within this loop.
        let mut memo: std::collections::HashMap<&str, bool> = std::collections::HashMap::new();
        for c in &self.world.hidden[i].hand {
            let ok = *memo
                .entry(c.as_str())
                .or_insert_with(|| cx.cant_play(i, c, false).is_none());
            playable.push(ok);
            est_cost.push(self.rules.card_prop(c, crate::state::prop::EST_COST));
        }
        // The viewer's pressable skills: every `skill:*` field instance with an
        // activatable `On::Play`, gated through the same `why_not_act` the
        // `act: "skill"` request takes -- so "shown enabled" ⇔ "accepted".
        // Passive / hook-only skills (`has_play` = false) never appear.
        let busy = self.pending.is_some();
        let mut skills = Vec::new();
        for (_uid, id) in self.world.field_instances(i as i32) {
            if !id.starts_with("skill:") || !self.rules.has_play(&id) {
                continue;
            }
            let m = NetMessage {
                act: "skill".into(),
                card: id.clone(),
                ..Default::default()
            };
            let reason = why_not_act(&cx, i, &m, busy);
            // `skill:<owner>:<name>` -- the owner is the persona or group the
            // skill came from (granted copies keep the original owner).
            let source = id
                .strip_prefix("skill:")
                .and_then(|s| s.split(':').next())
                .unwrap_or_default()
                .to_string();
            skills.push(SkillAction {
                id: id.clone(),
                source,
                title: Msg::new("ask.cardOption").card("card", &id),
                text: Msg::new("ask.cardOption").card("card", &id),
                enabled: reason.is_none(),
                reason: reason.unwrap_or_default(),
            });
        }
        ViewExtra {
            ai_answer,
            playable,
            est_cost,
            skills,
        }
    }

    /// The engine's own answer for `member`'s live prompt, **computed** from
    /// this world -- the seat-view engine's twin of [`Self::view_extra_typed`]'s
    /// `ai_answer` (which reads the precomputed [`Ask`] fill the live match
    /// stamped at prompt-open).
    ///
    /// A determinized fork has no precomputed fill (`bot-core`'s `stub_pending`
    /// can only mirror the searching seat's public `aiAnswer`, which the online
    /// frame no longer carries). This entry runs the same choice functions the
    /// prompt-open fill ran:
    ///
    /// * [`Cx::ai_auction_worth`] for an auction ceiling,
    /// * [`Cx::auto_mortgage`] for a mortgage selection,
    /// * the `Ask::with_ai` policy tests the settle offers stamp -- "no"
    ///   only when the fill said no ([`Cx::ai_wants_buy`] /
    ///   [`Cx::ai_wants_build`] / [`Cx::ai_wants_force_buy`], 0 = accept),
    /// * [`Cx::ai_agent_choice`] for the agent landing's tile pick
    ///   (`rules/tiles`' `ask.agent.title`),
    /// * [`Cx::ai_answer_for_entry`] for every other prompt (the fallback, or
    ///   a counteraction propensity roll).
    ///
    /// The `with_ai` row matters: those fills are **not** the fallback. A
    /// recompute that only ran `ai_answer_for_entry` answered a fundable buy
    /// with the prompt's "decline" slot, and online 托管 (which trusts this
    /// number over its own policy table) never bought a tile. Keep the
    /// title-keyed rows in step with the `Ask::with_ai` sites in
    /// [`super::play`] (`offer_buy` / `offer_build` / `offer_force_buy`) and
    /// `rules/tiles` (`agent.rs`).
    ///
    /// `None` when the seat has no open prompt or has already answered. The
    /// result is **not** bit-identical to the live match's precomputed fill
    /// whenever that fill drew from the match RNG (auction ceilings, counteraction
    /// propensity) -- the fork owns a fresh stream. That is expected and
    /// listed in the hidden-state audit (`docs/BOT.md` §1). The rows above are
    /// deterministic and must match exactly.
    pub fn compute_ai_answer(&mut self, member: i32) -> Option<RawAiAnswer> {
        let i = self.player_index(member)?;
        let prompt = {
            let p = self.pending.as_ref()?;
            let l = &p.live;
            let k = l.ask.view.player_index(i as i32)?;
            if l.answers.get(k).copied().unwrap_or(-1) >= 0 {
                return None;
            }
            l.ask.view.clone()
        };
        let fallback = prompt.fallback;
        let mut cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        match prompt.kind.as_str() {
            "auction" => {
                let base = if prompt.tile >= 0 {
                    purchase::quote_native(&self.data, &self.world.st, prompt.tile as usize).max(0)
                } else {
                    0
                };
                let worth = cx.ai_auction_worth(i, base);
                Some(RawAiAnswer {
                    answer: 0,
                    picked: Vec::new(),
                    worth,
                })
            }
            "mortgage" => {
                let need = prompt.bid;
                let deeds: Vec<usize> =
                    prompt.items.iter().filter_map(|s| s.parse().ok()).collect();
                let picked = if deeds.is_empty() {
                    cx.auto_mortgage(i, need)
                } else {
                    cx.auto_mortgage_from(i, need, deeds)
                };
                Some(RawAiAnswer {
                    answer: 0,
                    picked,
                    worth: 0,
                })
            }
            _ => {
                // `Ask::with_ai` rows first: the settle offers stamp a policy
                // test, not the fallback, and `ai_answer_for_entry` would
                // return the prompt's "decline" / "none" slot for them. Title
                // key is the marker (the client's `autopilot.ts` reads the
                // same keys). 0 = accept the offer.
                let t = prompt.tile;
                let title = prompt.title.key();
                let policy: Option<i32> = match title {
                    "ask.buy.title" if t >= 0 => {
                        Some(if cx.ai_wants_buy(i, t as usize) { 0 } else { 1 })
                    }
                    "ask.build.title" if t >= 0 => Some(if cx.ai_wants_build(i, t as usize) {
                        0
                    } else {
                        1
                    }),
                    "ask.force_buy.title" if t >= 0 => {
                        let price = if prompt.price >= 0 {
                            prompt.price
                        } else {
                            purchase::quote_native(&self.data, &self.world.st, t as usize).max(0)
                        };
                        Some(if cx.ai_wants_force_buy(i, price) {
                            0
                        } else {
                            1
                        })
                    }
                    "ask.agent.title" => {
                        let tiles: Vec<usize> =
                            prompt.items.iter().filter_map(|s| s.parse().ok()).collect();
                        Some(cx.ai_agent_choice(i, &tiles))
                    }
                    _ => None,
                };
                if let Some(answer) = policy {
                    return Some(RawAiAnswer {
                        answer,
                        picked: Vec::new(),
                        worth: 0,
                    });
                }
                // `fill_ai`'s per-seat body, on a throwaway `Ask` view of the
                // same prompt (the fallback / counteraction propensity path). The
                // prompt's own `Ask` is already filled on a stubbed fork, so
                // read the entry through the shared helper rather than
                // `fill_ai` (which skips non-`AI_UNSET` slots).
                let ask = Ask {
                    view: prompt.clone(),
                    ai: vec![cx::AI_UNSET],
                    ai_picked: vec![Vec::new()],
                    worth: vec![0],
                    use_pool: true,
                };
                let answer = cx.ai_answer_for_entry(&ask, i);
                Some(RawAiAnswer {
                    answer: if answer == cx::AI_UNSET {
                        fallback
                    } else {
                        answer
                    },
                    picked: Vec::new(),
                    worth: 0,
                })
            }
        }
    }

    // ------------------------------------------------------- simulation fork

    /// Cheap in-memory fork of the whole match -- the simulation's unit of
    /// work (`docs/BOT.md` §3.2 / B4). A `World` clone is refcount bumps plus
    /// the players / piles / board (~6 µs mid-game), versus the ~10 ms a
    /// save-JSON -> restore round trip cost. The provider is **not** carried
    /// over: a fork is policy-free until [`Match::set_provider`] installs one.
    pub fn fork(&self) -> Self {
        Self {
            data: self.data.clone(),
            rules: self.rules.clone(),
            mode: self.mode,
            world: self.world.clone(),
            pending: self.pending.clone(),
            live_rng: self.live_rng.clone(),
            wait: self.wait,
            bonus_time: self.bonus_time.clone(),
            base_time: self.base_time.clone(),
            timed_out: self.timed_out,
            vote: self.vote.clone(),
            vote_seq: self.vote_seq,
            vote_cooldown: self.vote_cooldown,
            deferred: self.deferred.clone(),
            seq: self.seq,
            clock_hold: self.clock_hold.clone(),
            changed: self.changed,
            provider: None,
            flushing_deferred: false,
            prompt_log: Vec::new(),
        }
    }

    /// Install a simulation answer provider (`docs/BOT.md` §3.2). Every
    /// [`Cx::ask`] whose replay-log entry is missing calls it instead of
    /// halting, so a routine runs once per simulation. Returns the previous
    /// provider, if any. Never installed on a live match.
    pub fn set_provider(
        &mut self,
        p: Option<Box<dyn AnswerProvider>>,
    ) -> Option<Box<dyn AnswerProvider>> {
        std::mem::replace(&mut self.provider, p)
    }

    /// The shared game data a fork's caller already holds (cheap clone).
    pub fn data(&self) -> Arc<GameData> {
        self.data.clone()
    }

    /// The card rules a fork's caller already holds (cheap clone).
    pub fn rules(&self) -> Arc<dyn CardRules> {
        self.rules.clone()
    }

    // ---------------------------------------------------------------- running routines

    /// Run `f` directly on the world (no prompts possible). Only valid with no
    /// routine pending.
    fn direct<R>(&mut self, f: impl FnOnce(&mut Cx) -> R) -> R {
        debug_assert!(
            self.pending.is_none(),
            "direct world mutation while a routine is pending"
        );
        let (data, rules) = (self.data.clone(), self.rules.clone());
        let mut cx = Cx::new(self.world.clone(), &data, &*rules, &[]);
        let r = f(&mut cx);
        self.world = cx.into_world();
        self.changed = true;
        self.seq += 1;
        self.arm_clock_hold();
        r
    }

    fn start(&mut self, r: Routine) {
        let snapshot = self.world.clone();
        self.execute(r, snapshot, vec![]);
    }

    fn execute(&mut self, routine: Routine, snapshot: World, answers: Vec<Answered>) {
        let (data, rules) = (self.data.clone(), self.rules.clone());
        // Simulation mode: answer inline through the installed provider
        // (`docs/BOT.md` §3.2). With none installed this is the halt/replay
        // path, unchanged.
        let mut prov = self.provider.take();
        let (res, delay, mut w) = {
            let mut cx = Cx::new(snapshot.clone(), &data, &*rules, &answers);
            let mut cx = match prov.as_mut() {
                Some(p) => cx.with_provider(&mut **p),
                None => cx,
            };
            let res = run(&mut cx, &routine);
            let delay = cx.take_delay();
            (res, delay, cx.into_world())
        };
        self.provider = prov;
        self.changed = true;
        self.seq += 1;
        // Anim hold for the acting human: every commit that leaves the seat
        // something to watch (this one) re-arms against the new seq. The hold
        // release path (`anim_done` / cap) never comes through here, so it
        // cannot re-arm itself.
        self.arm_clock_hold();
        match res {
            Ok(()) | Err(Halt(HaltKind::Ended)) => {
                let signals = std::mem::take(&mut w.signals);
                // Once-per-flash (ABI v55): a prompt's re-run starts from a
                // snapshot and would drop the pre-prompt pass's flash events.
                // `FlashState::saved` carries every flash already posted;
                // re-insert any missing from the tail (renumbered) so a flash
                // shown before the prompt remains shown and is not replayed.
                if !answers.is_empty() {
                    let saved = w.flash.lock().saved.clone();
                    for e in saved {
                        if e.r#type != "flash" {
                            continue;
                        }
                        let dup = w.recent.iter().any(|x| {
                            x.r#type == "flash" && x.msg.key() == e.msg.key() && x.card == e.card
                        });
                        if !dup {
                            let mut e = e;
                            e.id = w.next_event;
                            w.next_event += 1;
                            w.recent.push_back(e);
                        }
                    }
                }
                self.world = w;
                self.pending = None;
                self.wait = delay.min(MAX_WAIT);
                for s in signals {
                    match s {
                        Signal::StartTimer(i, refill, base_refill) => {
                            // `prop::{BASE,BONUS}_TIME_SET` on a board-owned
                            // instance (元祖！邦多利酱) replace the hooked defaults.
                            let set = |key: &str| {
                                self.world
                                    .st
                                    .board_field
                                    .iter()
                                    .filter(|f| f.tile < 0)
                                    .filter_map(|f| f.props.get(key).copied())
                                    .filter(|&v| v > 0)
                                    .map(|v| (v - 1) as f32)
                                    .reduce(f32::min)
                            };
                            self.bonus_time[i] = set(crate::state::prop::BONUS_TIME_SET)
                                .unwrap_or_else(|| {
                                    (self.bonus_time[i] + refill).min(self.world.clock.bonus_max())
                                });
                            // `baseTimeRefillSet` swaps the hooked refill only.
                            let base_refill = set(crate::state::prop::BASE_TIME_REFILL_SET)
                                .unwrap_or(base_refill);
                            if self.world.legacy_clock {
                                // Pre-settings: 恢复时间 is *set* to the hooked
                                // value, not refilled into. Matches every
                                // recording written before `MatchSetup::clock`.
                                self.base_time[i] =
                                    set(crate::state::prop::BASE_TIME_SET).unwrap_or(base_refill);
                            } else {
                                // Refill + cap-lift: `settings.base_time.max(base_refill)`
                                // lets a hook that grants a larger refill (e.g.
                                // TEST:clockHook doubles 15 → 30) still show up as
                                // base 30, while the default path caps back at the
                                // configured initial pool.
                                self.base_time[i] = set(crate::state::prop::BASE_TIME_SET)
                                    .unwrap_or_else(|| {
                                        (self.base_time[i] + base_refill)
                                            .min(self.world.clock.base_time.max(base_refill))
                                    });
                            }
                        }
                        Signal::TurnBegan => self.timed_out = false,
                    }
                }
                // STACK-01: `flush_deferred` is the only drain. `execute` called
                // from inside it (Deferred::Leave → start(Act)) must not
                // re-enter the drain -- that was the second unbounded
                // host-stack shape (`flush_deferred → start → execute` → ...).
                if !self.flushing_deferred {
                    self.flush_deferred();
                }
            }
            Err(Halt(HaltKind::Ask(ask))) => {
                w.signals.clear();
                self.world = w;
                self.pending = Some(Pending {
                    routine,
                    snapshot,
                    answers,
                    live: Live::new(*ask),
                });
            }
            // STACK-01 safety net: a suspension that escaped the work-stack
            // pump (should not happen -- `drain_work` is the pump). Drain what
            // was queued and commit, rather than aborting the match.
            Err(Halt(HaltKind::Suspended)) => {
                self.world = w;
                self.pending = None;
                let mut cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
                let _ = cx.drain_work();
                self.world = cx.into_world();
                self.changed = true;
                self.seq += 1;
                if !self.flushing_deferred {
                    self.flush_deferred();
                }
            }
        }
    }

    fn complete_prompt(&mut self) {
        let Some(p) = self.pending.take() else { return };
        let fallback = p.live.ask.view.fallback;
        let a = Answered {
            answers: p
                .live
                .answers
                .iter()
                .map(|&x| if x < 0 { fallback } else { x })
                .collect(),
            picked: p.live.picked,
            bid: p.live.bid,
            bidder: p.live.bidder,
        };
        self.prompt_log.push(a.clone());
        let mut answers = p.answers;
        answers.push(a);
        self.execute(p.routine, p.snapshot, answers);
    }

    /// Every [`Answered`] committed so far, in order (`docs/BOT.md` §3.2's
    /// equivalence gate). Drains the log; a simulation replays these through
    /// an [`AnswerProvider`] and must reproduce the same checkpoints.
    #[doc(hidden)]
    pub fn take_prompt_log(&mut self) -> Vec<Answered> {
        std::mem::take(&mut self.prompt_log)
    }

    /// Host-side log line; deferred while a routine is pending.
    fn host_log(&mut self, kind: &str, player_id: i32, text: Msg) {
        if self.pending.is_some() {
            self.deferred
                .push(Deferred::Log(kind.into(), player_id, text));
        } else {
            self.direct(|cx| {
                cx.w.log(kind, player_id, text);
            });
        }
    }

    fn flush_deferred(&mut self) {
        // STACK-01: iterative drain. `start` → `execute` must not re-enter
        // this loop (Leave → Act → Leave → … used to recurse).
        if self.flushing_deferred {
            return;
        }
        self.flushing_deferred = true;
        while self.pending.is_none() && !self.deferred.is_empty() {
            match self.deferred.remove(0) {
                Deferred::Log(k, s, t) => self.host_log(&k, s, t),
                Deferred::Left(i, can_return) => self.apply_left(i, can_return),
                Deferred::Back(i) => self.apply_back(i),
                Deferred::Leave(i) => {
                    if self.world.st.phase == "play" && !self.world.out(i) {
                        self.start(Routine::Act(i, Box::new(NetMessage::act("leave"))));
                    }
                }
                Deferred::Finish(reason) => self.direct(|cx| cx.finish(&reason, None)),
            }
        }
        self.flushing_deferred = false;
    }

    /// Is this prompt player answered by the machine (bot, or a human who left)?
    ///
    /// This is the **prompt** auto-answer ([`Self::tick_live`]): `ai` seats
    /// (standard / chaos bots, and a human under 托管). An Advanced bot is
    /// `ai = false` on purpose -- its prompts wait for the external driver
    /// (`docs/BOT.md` B5). Votes are different; see [`Self::auto_vote`].
    fn auto_player(&self, player_id: usize) -> bool {
        self.world.st.players[player_id].ai
            || self
                .deferred
                .iter()
                .any(|d| matches!(d, Deferred::Leave(s) | Deferred::Left(s, _) if *s == player_id))
    }

    /// Is this seat's **vote** cast by the machine?
    ///
    /// Every bot seat -- standard, chaos, and **advanced** -- votes via the
    /// engine (user ruling 2026-10-09: "have the engine vote for advanced bot
    /// seats"). An advanced bot's play-phase decisions belong to the external
    /// driver, but the drivers have no vote path and a solo vote never
    /// expires, so the engine casts the seat's vote with the standard policy
    /// (yes) exactly as it always has for `ai` seats. A human under 托管 is
    /// likewise machine-answered, as today; an active human votes for
    /// themselves.
    fn auto_vote(&self, player_id: usize) -> bool {
        let p = &self.world.st.players[player_id];
        p.ai || p.bot
            || self
                .deferred
                .iter()
                .any(|d| matches!(d, Deferred::Leave(s) | Deferred::Left(s, _) if *s == player_id))
    }

    // ---------------------------------------------------------------- clock

    /// Feed an animation-done ack from seat `member` for state version `seq`.
    ///
    /// A matching ack (or one naming a newer seq -- the client is already
    /// current) releases the hold so the turn clock starts consuming. A stale
    /// ack (`seq` older than the hold) is ignored and returns `false`. Recorded
    /// as [`crate::record::Input::AnimDone`] so replays reproduce the pause.
    pub fn anim_done(&mut self, member: i32, seq: i32) -> bool {
        let Some(hold) = &self.clock_hold else {
            return false;
        };
        if hold.member != member || seq < hold.seq {
            return false;
        }
        self.clock_hold = None;
        self.changed = true;
        self.seq += 1;
        true
    }

    /// True while the acting seat's clock is held for an anim ack
    /// (published as [`MatchState::clock_paused`]).
    pub fn clock_paused(&self) -> bool {
        self.clock_hold.is_some()
    }

    /// Arm (or clear) the anim hold after a game commit. Only the acting
    /// human's client is waited on; Solo and machine seats never hold. The
    /// hold is keyed to the just-bumped [`Self::seq`] so a stale ack is
    /// ignored. Not called from [`Self::anim_done`] -- a hold release must
    /// not re-arm itself.
    fn arm_clock_hold(&mut self) {
        if self.mode == MatchMode::Solo {
            self.clock_hold = None;
            return;
        }
        let st = &self.world.st;
        if st.phase != "play" || st.turn < 0 {
            self.clock_hold = None;
            return;
        }
        let turn = st.turn as usize;
        let actor = if st.step == stage::OPS && !st.skip_move && st.roller >= 0 {
            st.roller as usize
        } else {
            turn
        };
        let Some(p) = st.players.get(actor) else {
            self.clock_hold = None;
            return;
        };
        if p.ai {
            self.clock_hold = None;
            return;
        }
        self.clock_hold = Some(ClockHold {
            // The room member id (what the ack route sends), not the seat index.
            member: p.member,
            seq: self.seq,
            elapsed: 0.0,
        });
    }

    /// Advance the match by `dt` seconds (`Tick`).
    pub fn tick(&mut self, dt: f32) {
        let _tg = rtimer::guard(&rtimer::TICK_NS);
        let phase = self.world.st.phase.clone();
        if phase.is_empty() || phase == "ended" {
            return;
        }
        if self.wait > 0.0 {
            self.wait -= dt;
        }
        match phase.as_str() {
            "order" => {
                self.world.st.time_left = self.wait.max(0.0);
                if self.wait <= 0.0 {
                    let ranked = self.mode == MatchMode::Ranked;
                    self.direct(|cx| {
                        if ranked {
                            cx.begin_ban()
                        } else {
                            cx.begin_pick()
                        }
                    });
                    self.wait = CHOICE_WAIT;
                }
            }
            "ban" | "pick" | "deck" => self.tick_choice(dt),
            "play" => {
                // Anim hold covers the whole play surface (prompts and wait
                // beats included) so the acting seat never loses clock time
                // to presentation. Tick-fed; 10s cap releases a stalled tab.
                if let Some(hold) = &mut self.clock_hold {
                    hold.elapsed += dt;
                    if hold.elapsed >= ANIM_HOLD_CAP {
                        self.clock_hold = None;
                        self.changed = true;
                        self.seq += 1;
                    }
                }
                self.tick_vote(dt);
                if self.world.st.phase == "play" {
                    self.tick_play(dt);
                }
            }
            _ => {}
        }
    }

    /// `TickChoice` -- ban/pick/deck time-outs and bot choices.
    fn tick_choice(&mut self, dt: f32) {
        // A solo match never expires a choice: nobody is waiting on the local
        // player, so there is nothing a timeout is protecting. Bots still act
        // on their own `wait` schedule below.
        let solo = self.mode == MatchMode::Solo;
        if !solo {
            self.world.st.time_left -= dt;
        }
        if self.world.st.phase == "deck" {
            if solo || self.world.st.time_left <= 0.0 {
                let round_up = self
                    .world
                    .st
                    .players
                    .iter()
                    .any(|p| !p.deck_ready && (!solo || p.auto_setup()));
                if round_up {
                    self.direct(|cx| {
                        for i in 0..cx.w.player_count() {
                            let p = &cx.w.st.players[i];
                            // Only machine seats are rounded up in solo -- the
                            // human is waited for, however long that takes.
                            if !p.deck_ready && (!solo || p.auto_setup()) {
                                cx.submit_deck(i, None);
                            }
                        }
                    });
                }
                if !solo || self.world.st.players.iter().all(|s| s.deck_ready) {
                    self.begin_play();
                }
            }
            return;
        }
        let Some(cur) = self.world.st.current() else {
            return;
        };
        // Setup is engine-side for every bot, Advanced included (B5): the
        // server only holds the seat during play.
        let due = if cur.auto_setup() {
            self.wait <= 0.0
        } else {
            !solo && self.world.st.time_left <= 0.0
        };
        if !due {
            return;
        }
        let turn = self.world.st.turn as usize;
        if self.world.st.phase == "ban" {
            self.direct(|cx| {
                // Standard: 50% a random persona, else no ban. Chaos always
                // bans something if anything is left. A human seat (time-out)
                // never bans.
                let chaos = cx.is_chaos(turn);
                let pick = if cx.w.st.players[turn].auto_setup() && (chaos || cx.w.rng.chance(0.5))
                {
                    cx.random_persona(false)
                } else {
                    String::new()
                };
                cx.do_ban(turn, &pick);
            });
        } else {
            // A seat's last click on the pick screen wins the timeout while
            // that persona is still free; an empty / stale intent falls back
            // to a random free one (the pre-timeout behaviour). Bots never
            // set an intent, so they keep the random path.
            let intent = self.world.hidden[turn].pick_intent.clone();
            self.direct(|cx| {
                let c = if !intent.is_empty() && cx.pickable(&intent) {
                    intent
                } else {
                    cx.random_persona(true)
                };
                cx.do_pick(turn, &c);
            });
        }
        self.after_choice();
    }

    fn after_choice(&mut self) {
        self.wait = CHOICE_WAIT;
        if self.world.st.phase == "deck" && self.world.st.players.iter().all(|s| s.deck_ready) {
            self.begin_play();
        }
    }

    fn begin_play(&mut self) {
        self.direct(|cx| cx.begin_play());
        self.start(Routine::Opening);
    }

    /// `TickPlay`
    fn tick_play(&mut self, dt: f32) {
        if self.pending.is_some() {
            self.tick_live(dt);
            return;
        }
        if self.world.next_turn_pending {
            if self.wait <= 0.0 {
                self.start(Routine::NextTurn);
            }
            return;
        }
        if let Some(deeds) = self.world.leftovers.front().cloned() {
            self.direct(|cx| {
                cx.w.leftovers.pop_front();
            });
            self.start(Routine::Leftovers(deeds));
            return;
        }
        let st = &self.world.st;
        let Some(cur) = st.current() else { return };
        let turn = st.turn as usize;
        if cur.out() {
            self.direct(|cx| cx.w.next_turn_pending = true);
            return;
        }
        if st.roller >= 0 && self.world.out(st.roller as usize) {
            // Reassign the roll to the seat now at the plate. Not through
            // `direct`: that would re-arm the clock hold on a tick. It is a
            // visible field (`MatchState::roller`), so it still owes the frame
            // identity its bump -- see the note on `seq`.
            self.world.st.roller = turn as i32;
            self.changed = true;
            self.seq += 1;
        }
        if self.wait > 0.0 {
            return;
        }
        // 开始 holds for a beat before 运营 opens -- the stage swap needs the
        // time to read, exactly as 结束 holds after the walk. `turn_start` parked
        // the turn here; this lifts it into 运营 once that beat is over.
        //
        // Through `direct`, not a bare write: `changed`/`seq` only move there
        // and in `execute`, so a raw assignment leaves `take_changed()` false
        // and the client keeps rendering 开始 for the whole turn.
        if self.world.st.step == stage::START {
            self.direct(|cx| cx.w.st.step = stage::OPS);
            self.wait = 1.2;
            return;
        }
        let st = &self.world.st;
        let actor = if st.step == stage::OPS && !st.skip_move && st.roller >= 0 {
            st.roller as usize
        } else {
            turn
        };
        if !st.players[actor].ai {
            // A solo match has nobody waiting on the local player: the turn
            // clock exists to keep a *networked* table moving, so here it never
            // expires and the AI never takes a human's turn over. Bots still
            // fall through below and act for themselves.
            if self.mode == MatchMode::Solo {
                return;
            }
            if !self.timed_out {
                // Still held (anim ack pending, cap not yet reached): do not
                // consume base/bonus.
                if self.clock_hold.is_some() {
                    return;
                }
                // Drain order is **bonus first, then base** (`docs/SERVER.md`
                // §Turn clock). Legacy matches keep the old base-first order so
                // a pre-settings recording's countdowns remain identical.
                if self.world.legacy_clock {
                    if self.base_time[turn] > 0.0 {
                        self.base_time[turn] = (self.base_time[turn] - dt).max(0.0);
                    } else {
                        self.bonus_time[turn] = (self.bonus_time[turn] - dt).max(0.0);
                    }
                } else if self.bonus_time[turn] > 0.0 {
                    self.bonus_time[turn] = (self.bonus_time[turn] - dt).max(0.0);
                } else {
                    self.base_time[turn] = (self.base_time[turn] - dt).max(0.0);
                }
                if self.base_time[turn] + self.bonus_time[turn] > 0.0 {
                    return;
                }
                self.timed_out = true;
                self.host_log(
                    "text",
                    turn as i32,
                    Msg::new("log.timeout").player_id("who", turn),
                );
            }
        }
        self.start(Routine::Ai(turn));
    }

    /// `TickAsk` / `TickAuction` -- bots answer after a short delay; time-outs
    /// fall back to the default answer.
    ///
    /// Under the post-settings clock an ordinary prompt (`use_pool`) draws on
    /// each pending **human** seat's own pool (bonus then base). `Live::time_left`
    /// remains the soft window bots schedule against and the all-bot fallback
    /// deadline; a human is no longer force-completed just because that window
    /// closed if their pool still has time. Bankruptcy leftover auctions
    /// (`use_pool = false`) and every pre-settings (`legacy_clock`) prompt keep
    /// the old fixed-window behaviour.
    // Indexing by prompt player walks several parallel vectors at once.
    #[allow(clippy::needless_range_loop)]
    fn tick_live(&mut self, dt: f32) {
        let players: Vec<usize> = match &self.pending {
            Some(p) => p
                .live
                .ask
                .view
                .players
                .iter()
                .map(|&s| s as usize)
                .collect(),
            None => return,
        };
        let auto: Vec<bool> = players.iter().map(|&s| self.auto_player(s)).collect();
        let money: Vec<i32> = players
            .iter()
            .map(|&s| self.world.st.players[s].money)
            .collect();
        let can_pay: Vec<bool> = players
            .iter()
            .map(|&s| {
                let x = &self.world.st.players[s];
                !x.out() && self.world.gate_open(&*self.rules, Gate::Pay, s)
            })
            .collect();
        // Standard's bid step / nudge read the seat's `StrategyParams`
        // (`docs/BOT.md` §3.8). Chaos keeps the literals. Resolved up front so
        // the `live` / `live_rng` borrows below remain disjoint.
        let strat: Vec<crate::strategy::StrategyParams> = players
            .iter()
            .map(|&s| {
                crate::strategy::for_seat_sha(
                    &self.data,
                    &self.world.st,
                    s,
                    Some(self.rules.ruleset_sha256().unwrap_or("stub")),
                )
            })
            .collect();
        let solo = self.mode == MatchMode::Solo;
        let uses_pool = self
            .pending
            .as_ref()
            .is_some_and(|p| self.prompt_uses_pool(&p.live));
        // Prompt pool drain (post-settings ordinary prompts only). Each
        // pending human seat spends its own pool; the anim-ack hold pauses
        // consumption for the held seat so presentation never costs clock.
        // Solo never forces *or drains* a local player's prompt -- nobody is
        // waiting on them.
        if uses_pool && !solo {
            let pending: Vec<bool> = self
                .pending
                .as_ref()
                .map(|p| (0..players.len()).map(|k| p.live.answers[k] < 0).collect())
                .unwrap_or_default();
            let held: Vec<bool> = players.iter().map(|&s| self.clock_hold_covers(s)).collect();
            for k in 0..players.len() {
                if pending[k] && !auto[k] && !held[k] {
                    self.drain_pool(players[k], dt);
                }
            }
        }
        // Snapshot every prompted seat's pool total *before* the `live` borrow
        // so the force-default pass below can read them without fighting it.
        let pool: Vec<f32> = players.iter().map(|&s| self.pool_total(s)).collect();
        let rng = &mut self.live_rng;
        let p = self.pending.as_mut().expect("checked above");
        let l = &mut p.live;
        l.time_left -= dt;
        let before = (l.answers.clone(), l.bid);
        // A solo match never forces a *local* player's answer -- nobody is
        // waiting on them. The countdown still runs, because the bots above
        // schedule their own answers against it. The mode is the predicate,
        // not the human count: an online room with one human still owes its
        // table an answer.
        let hold = solo && (0..players.len()).any(|k| l.answers[k] < 0 && !auto[k]);
        let mut bid_grant = false;
        // Force-default a pending human seat whose pool is exhausted past the
        // grace window. Shared by the auction and ordinary-prompt branches.
        let force_exhausted = |l: &mut Live| {
            if !uses_pool || hold {
                return;
            }
            for k in 0..players.len() {
                if l.answers[k] >= 0 || auto[k] {
                    continue;
                }
                if pool[k] <= -REMOTE_GRACE {
                    if matches!(l.kind(), "pick" | "mortgage") {
                        l.picked = l.ask.ai_picked[k].clone();
                        l.answers[k] = 0;
                    } else if l.kind() == "auction" {
                        l.answers[k] = 1; // pass
                    } else {
                        l.answers[k] = l.ask.view.fallback;
                    }
                }
            }
        };
        let done = if l.kind() == "auction" {
            for k in 0..players.len() {
                if l.answers[k] >= 0 || l.bidder == players[k] as i32 || !auto[k] {
                    continue;
                }
                match l.ai_at[k] {
                    None => l.ai_at[k] = Some(l.time_left - (0.6 + rng.f64() as f32 * 1.4)),
                    Some(at) if l.time_left <= at => {
                        l.ai_at[k] = None;
                        let seat = players[k];
                        let chaos = {
                            let x = &self.world.st.players[seat];
                            x.bot && x.mentality == crate::state::BotMentality::Chaos
                        };
                        // Standard's raise step / nudge come from the seat's
                        // `StrategyParams` (defaults == the old `100` /
                        // `below(3)` formula). Chaos keeps the literals.
                        let min = if chaos {
                            if l.bid <= 0 {
                                100
                            } else {
                                l.bid + 100
                            }
                        } else {
                            strat[k].bid_min(l.bid)
                        };
                        let cap = l.ask.worth[k].min(money[k]);
                        if min > cap || !can_pay[k] {
                            l.answers[k] = 1;
                        } else {
                            // Standard nudges the minimum (up to +200 at the
                            // default `bid_nudge_steps`) -- the original
                            // formula, untouched at the defaults. Chaos raises
                            // anywhere up to its ceiling, which `Ask::worth`
                            // already capped at money - `CHAOS_RESERVE`. A
                            // human seat is never chaos: a time-out keeps the
                            // standard policy.
                            let bid = if chaos {
                                let steps = ((cap - min) / 100 + 1).max(1) as usize;
                                min + 100 * rng.below(steps) as i32
                            } else {
                                let p = &strat[k];
                                let step = p.bid_step;
                                let nudge = p.bid_nudge_steps.max(1) as usize;
                                cap.min(min + step * rng.below(nudge) as i32)
                            };
                            l.place_bid(seat, bid);
                            bid_grant = true;
                        }
                    }
                    _ => {}
                }
            }
            force_exhausted(l);
            let open = (0..players.len())
                .filter(|&k| l.answers[k] < 0 && l.bidder != players[k] as i32)
                .count();
            if uses_pool {
                // Per-seat pool deadline (with grace); the soft window alone
                // must not force a human who still has pool left.
                let humans_pending = (0..players.len()).any(|k| l.answers[k] < 0 && !auto[k]);
                open == 0 || (!humans_pending && l.time_left <= 0.0 && !hold)
            } else {
                open == 0 || (l.time_left <= 0.0 && !hold)
            }
        } else {
            let picks = matches!(l.kind(), "pick" | "mortgage");
            let multi = l.kind() == "multi";
            let fallback = l.ask.view.fallback;
            let max = if l.kind() == "tile" {
                l.ask.view.items.len() as i32
            } else {
                l.ask.view.options.len() as i32 - 1
            };
            for k in 0..players.len() {
                if l.answers[k] >= 0 || !auto[k] {
                    continue;
                }
                if l.kind() == "mulligan" {
                    // Machine seats decide at once. The barrier waits on
                    // people; making a human sit out most of the window for a
                    // bot's simulated thinking time is pure friction, and the
                    // answer is already precomputed (`Ask::ai`).
                    l.answers[k] = l.ask.ai[k].clamp(0, max.max(0));
                    continue;
                }
                match l.ai_at[k] {
                    None => l.ai_at[k] = Some(l.time_left - (0.4 + rng.f64() as f32 * 0.6)),
                    Some(at) if l.time_left <= at => {
                        if picks {
                            l.picked = l.ask.ai_picked[k].clone();
                            l.answers[k] = 0;
                        } else if multi {
                            // A bitmask answer must not be clamped to an option
                            // index (that would destroy e.g. `0b0111`). Take
                            // the precomputed mask when it is already valid;
                            // otherwise the prompt's valid fallback mask.
                            let v = l.ask.ai[k];
                            let n = l.ask.view.options.len().min(31) as i32;
                            let ok = v >= 0
                                && n > 0
                                && (v >> n) == 0
                                && v.count_ones() as i32 == l.ask.view.count;
                            l.answers[k] = if ok { v } else { fallback };
                        } else {
                            l.answers[k] = l.ask.ai[k].clamp(0, max.max(0));
                        }
                    }
                    _ => {}
                }
            }
            if l.kind() == "mulligan" {
                // 初始重抽 is a hard barrier (rulebook 时点流程 step 6 before
                // 7/8): the game does not advance until every seat has an
                // answer, and the prompt's own window decides only for a seat
                // that never answers (keep -- the fallback). It is *not*, in
                // particular, cut short by the 恢复时间 pool (`use_pool` is
                // off), which is what used to end it after ~16 s while a
                // human was still deciding. Solo holds for its local player
                // like every other prompt.
                //
                // The window is still needed: an open tab keeps the seat's
                // presence alive, so a player who walks away is never taken
                // over and would otherwise wedge the whole table in the
                // opening forever.
                if l.answers.iter().all(|&a| a >= 0) {
                    true
                } else if hold || l.time_left > 0.0 {
                    false
                } else {
                    for k in 0..players.len() {
                        if l.answers[k] < 0 {
                            l.answers[k] = fallback;
                        }
                    }
                    true
                }
            } else if uses_pool {
                force_exhausted(l);
                let humans_pending = (0..players.len()).any(|k| l.answers[k] < 0 && !auto[k]);
                if l.answers.iter().all(|&a| a >= 0) {
                    true
                } else if hold || humans_pending {
                    // A human still has pool (or Solo is holding for them).
                    false
                } else if l.time_left > 0.0 {
                    // Only bots left on the soft window's schedule.
                    false
                } else {
                    for k in 0..players.len() {
                        if l.answers[k] < 0 {
                            if picks {
                                l.picked = l.ask.ai_picked[k].clone();
                                l.answers[k] = 0;
                            } else {
                                l.answers[k] = fallback;
                            }
                        }
                    }
                    true
                }
            } else {
                // Keep waiting while time is left, plus a short grace for remote humans.
                let humans_pending = (0..players.len()).any(|k| l.answers[k] < 0 && !auto[k]);
                if l.answers.iter().all(|&a| a >= 0) {
                    true
                } else if hold
                    || l.time_left > 0.0
                    || (l.time_left > -REMOTE_GRACE && !solo && humans_pending)
                {
                    false
                } else {
                    for k in 0..players.len() {
                        if l.answers[k] < 0 {
                            if picks {
                                l.picked = l.ask.ai_picked[k].clone();
                                l.answers[k] = 0;
                            } else {
                                l.answers[k] = fallback;
                            }
                        }
                    }
                    true
                }
            }
        };
        if (l.answers.clone(), l.bid) != before {
            self.changed = true;
            self.seq += 1;
        }
        if bid_grant {
            self.grant_auction_extend();
        }
        if done {
            self.complete_prompt();
        }
    }

    // ---------------------------------------------------------------- input

    /// A player command (`Act`). `Err` carries the message shown to the player.
    pub fn act(&mut self, member: i32, m: &NetMessage) -> Result<(), Msg> {
        let _tg = rtimer::guard(&rtimer::ACT_NS);
        let i = self
            .player_index(member)
            .ok_or_else(|| Msg::new("err.not_in_match"))?;
        let s = &self.world.st.players[i];
        if s.out() && m.act != "leave" {
            return Err(Msg::new(if s.bankrupt {
                "err.spectating"
            } else {
                "err.left_match"
            }));
        }
        let phase = self.world.st.phase.clone();
        let busy = self.pending.is_some();
        match m.act.as_str() {
            "ban" => {
                if phase != "ban" || self.world.st.turn != i as i32 {
                    return Err(Msg::new("err.not_your_ban"));
                }
                let c = m.persona.clone();
                if !c.is_empty() && !self.with_cx(|cx| cx.bannable(&c)) {
                    return Err(Msg::new("err.cannot_ban"));
                }
                self.direct(|cx| cx.do_ban(i, &c));
                self.after_choice();
            }
            "pick" => {
                if phase != "pick" || self.world.st.turn != i as i32 {
                    return Err(Msg::new("err.not_your_pick"));
                }
                let c = m.persona.clone();
                if !self.with_cx(|cx| cx.pickable(&c)) {
                    return Err(Msg::new("err.chara_taken"));
                }
                self.direct(|cx| cx.do_pick(i, &c));
                self.after_choice();
            }
            // Private pick-screen click: recorded (so a timeout in a replay
            // resolves the same way) but never broadcast. Legal any time
            // during the pick phase for a seat that has not picked yet -- the
            // player may mark a favourite while waiting. Does not advance the
            // turn or touch `time_left`.
            "preselect" => {
                if phase != "pick" || !self.world.st.players[i].persona.is_empty() {
                    return Err(Msg::new("err.no_preselect_now"));
                }
                let c = m.persona.clone();
                if c.is_empty() {
                    return Ok(());
                }
                if !self.with_cx(|cx| cx.pickable(&c)) {
                    return Err(Msg::new("err.chara_taken"));
                }
                self.world.hidden[i].pick_intent = c;
            }
            "deck" => {
                if phase != "deck" || self.world.st.players[i].deck_ready {
                    return Err(Msg::new("err.no_deck_now"));
                }
                let ok = self
                    .data
                    .persona(&self.world.st.players[i].persona)
                    .is_some_and(|c| crate::deck::is_complete(&self.data, c, &m.cards));
                if !ok {
                    return Err(Msg::new("err.deck_invalid"));
                }
                let cards = m.cards.clone();
                self.direct(|cx| cx.submit_deck(i, Some(&cards)));
                if self.world.st.players.iter().all(|s| s.deck_ready) {
                    self.begin_play();
                }
            }
            "answer" => return self.answer(i, m),
            "vote" => return self.vote(i, m.value == 1),
            "leave" => {
                let member = self.world.st.players[i].member;
                if phase != "play" {
                    self.member_left(member, false);
                } else if busy {
                    self.deferred.push(Deferred::Leave(i));
                    self.tick_live(0.0);
                } else {
                    self.start(Routine::Act(i, Box::new(m.clone())));
                }
            }
            "skill" => {
                if let Some(why) = self.with_cx(|cx| why_not_act(cx, i, m, busy)) {
                    return Err(why);
                }
                self.start(Routine::Act(i, Box::new(m.clone())));
            }
            // `debug` only exists in debug builds; in release it falls through
            // to `err.unknown_act` like any unrecognized command.
            #[cfg(debug_assertions)]
            "debug" => return self.debug_act(i, m),
            "roll" | "buy" | "build" | "mortgage" | "redeem" | "play" | "discard" | "end" => {
                if let Some(why) = self.with_cx(|cx| why_not_act(cx, i, m, busy)) {
                    return Err(why);
                }
                self.start(Routine::Act(i, Box::new(m.clone())));
            }
            _ => return Err(Msg::new("err.unknown_act")),
        }
        Ok(())
    }

    fn with_cx<R>(&self, f: impl FnOnce(&Cx) -> R) -> R {
        let cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        f(&cx)
    }

    /// `Answer` -- a human's reply to the live prompt.
    fn answer(&mut self, i: usize, m: &NetMessage) -> Result<(), Msg> {
        let money = self.world.st.players[i].money;
        let can_pay = self.with_cx(|cx| cx.can_pay(i));
        let data = self.data.clone();
        let p = self
            .pending
            .as_mut()
            .ok_or_else(|| Msg::new("err.prompt_over"))?;
        let l = &mut p.live;
        if m.prompt != l.ask.view.id {
            return Err(Msg::new("err.prompt_over"));
        }
        let k = l
            .ask
            .view
            .player_index(i as i32)
            .ok_or_else(|| Msg::new("err.prompt_not_yours"))?;
        if l.answers[k] >= 0 {
            return Err(Msg::new(if l.kind() == "auction" {
                "err.auction_passed"
            } else {
                "err.answered"
            }));
        }
        let picked = |items: &[String]| -> Vec<String> {
            let mut out: Vec<String> = vec![];
            for c in &m.cards {
                if items.contains(c) && !out.contains(c) {
                    out.push(c.clone());
                }
            }
            out
        };
        let mut bid_grant = false;
        match l.kind() {
            "auction" => {
                if m.value < 0 {
                    l.answers[k] = 1;
                } else {
                    let min = if l.bid <= 0 { 100 } else { l.bid + 100 };
                    if l.bidder == i as i32 {
                        return Err(Msg::new("err.already_top_bid"));
                    }
                    if m.value < min {
                        return Err(Msg::new("err.bid_min").n("min", min));
                    }
                    if !can_pay {
                        return Err(Msg::new("err.cannot_bid"));
                    }
                    // B4 (`PIPELINE-AUDIT` K10b) -- 规则书 L76 「需pay或spend
                    // 资金且资金不足时可以选择抵押拥有的地契」: a bid may be funded
                    // by 抵押, so eligibility is cash **plus** what the seat could
                    // raise, not cash alone. The old `m.value > money` gate let a
                    // short bidder not even bid. (`auction_tile` runs
                    // `raise_funds` on the winner and only voids if they go out.)
                    let raiseable: i32 = (0..self.data.tiles.len())
                        .filter(|&t| {
                            self.world.st.owners.get(t).copied().unwrap_or(-1) == i as i32
                                && !self.world.st.mortgaged.get(t).copied().unwrap_or(false)
                        })
                        .map(|t| self.data.tiles[t].price / 2)
                        .sum();
                    if m.value > money + raiseable {
                        return Err(Msg::new("err.poor"));
                    }
                    l.place_bid(i, m.value / 100 * 100);
                    bid_grant = true;
                }
            }
            "pick" => {
                let got = picked(&l.ask.view.items);
                if got.len() as i32 != l.ask.view.count {
                    return Err(Msg::new("err.pick_count").i("n", l.ask.view.count));
                }
                l.picked = got;
                l.answers[k] = 0;
            }
            "mortgage" => {
                if !l.ask.view.options.is_empty() && m.value == 1 {
                    l.picked.clear();
                    l.answers[k] = 1;
                } else {
                    let got = picked(&l.ask.view.items);
                    let sum: i32 = got
                        .iter()
                        .filter_map(|t| t.parse::<usize>().ok())
                        .map(|t| data.tiles[t].price / 2)
                        .sum();
                    if sum < l.ask.view.bid {
                        return Err(Msg::new("err.mortgage_short")
                            .n("need", l.ask.view.bid)
                            .n("sum", sum));
                    }
                    l.picked = got;
                    l.answers[k] = 0;
                }
            }
            "tile" => {
                let n = l.ask.view.items.len() as i32;
                let v = if m.value < 0 { n } else { m.value };
                if v > n {
                    return Err(Msg::new("err.no_option"));
                }
                l.answers[k] = v;
            }
            "multi" => {
                // Bitmask of exactly `count` distinct options. The popcount
                // check is the "cannot select one effect twice" guarantee.
                let n = l.ask.view.options.len() as i32;
                let bits = m.value;
                if n <= 0 || bits < 0 || (bits >> n) != 0 {
                    return Err(Msg::new("err.no_option"));
                }
                if bits.count_ones() as i32 != l.ask.view.count {
                    return Err(Msg::new("err.pick_count").i("n", l.ask.view.count));
                }
                l.answers[k] = bits;
            }
            _ => {
                if m.value < 0 || m.value as usize >= l.ask.view.options.len() {
                    return Err(Msg::new("err.no_option"));
                }
                l.answers[k] = m.value;
            }
        }
        let complete = l.kind() != "auction" && l.answers.iter().all(|&a| a >= 0);
        let is_auction = l.kind() == "auction";
        self.changed = true;
        self.seq += 1;
        if bid_grant {
            // AUCTION_EXTEND is a grant to every still-pending human pool (see
            // `grant_auction_extend`); `place_bid` already raised the soft window.
            self.grant_auction_extend();
        }
        if complete {
            self.complete_prompt();
        } else if is_auction {
            self.tick_live(0.0);
        }
        Ok(())
    }

    // ---------------------------------------------------------------- vote

    /// Every seat still in the match votes -- humans by hand, bot seats (any
    /// mentality) by the engine via [`Self::auto_player`] (user ruling
    /// 2026-10-09). An `ai` seat used to be dropped here, which left an
    /// advanced bot (`ai` off, held for its driver) as a voter nobody answered
    /// -- and a solo vote never expires.
    fn voters(&self) -> Vec<usize> {
        (0..self.world.player_count())
            .filter(|&p| !self.world.out(p))
            .collect()
    }

    /// `Vote` -- start or answer the vote to end the match by score.
    fn vote(&mut self, i: usize, yes: bool) -> Result<(), Msg> {
        if self.world.st.phase != "play" {
            return Err(Msg::new("err.not_playing"));
        }
        if self.vote.id == 0 {
            if !yes {
                return Err(Msg::new("err.no_vote"));
            }
            if self.vote_cooldown > 0.0 {
                return Err(Msg::new("err.vote_cooldown").i("s", self.vote_cooldown.ceil() as i64));
            }
            let voters = self.voters();
            if !voters.contains(&i) {
                return Err(Msg::new("err.vote_out"));
            }
            self.vote_seq += 1;
            self.vote = MatchVote {
                id: self.vote_seq,
                by: i as i32,
                players: voters.iter().map(|&p| p as i32).collect(),
                answers: voters
                    .iter()
                    .map(|&p| if p == i { 1 } else { -1 })
                    .collect(),
                time_left: VOTE_SECONDS,
            };
            self.host_log(
                "vote",
                i as i32,
                Msg::new("log.vote_start").player_id("who", i),
            );
        } else {
            let k = self
                .vote
                .players
                .iter()
                .position(|&s| s == i as i32)
                .ok_or_else(|| Msg::new("err.vote_not_yours"))?;
            if self.vote.answers[k] >= 0 {
                return Err(Msg::new("err.voted"));
            }
            self.vote.answers[k] = yes as i32;
            self.host_log(
                "vote",
                i as i32,
                Msg::new(if yes { "log.vote_yes" } else { "log.vote_no" }).player_id("who", i),
            );
        }
        self.changed = true;
        self.seq += 1;
        self.check_vote();
        Ok(())
    }

    fn tick_vote(&mut self, dt: f32) {
        if self.vote_cooldown > 0.0 {
            self.vote_cooldown -= dt;
        }
        if self.vote.id == 0 {
            return;
        }
        // Solo never expires a vote -- see `tick_choice`. The mode is the
        // predicate, not the human count: an online room with one human still
        // owes its table an answer.
        if self.mode != MatchMode::Solo {
            self.vote.time_left -= dt;
        }
        for k in 0..self.vote.players.len() {
            let s = self.vote.players[k] as usize;
            if self.vote.answers[k] < 0 && (self.world.out(s) || self.auto_vote(s)) {
                self.vote.answers[k] = 1;
                self.changed = true;
                self.seq += 1;
            }
        }
        if self.mode != MatchMode::Solo && self.vote.time_left <= 0.0 {
            self.end_vote(false, Msg::new("vote.timeout"));
        } else {
            self.check_vote();
        }
    }

    fn check_vote(&mut self) {
        if let Some(k) = self.vote.answers.iter().position(|&a| a == 0) {
            self.end_vote(
                false,
                Msg::new("vote.against").player_id("who", self.vote.players[k]),
            );
        } else if self.vote.id != 0 && self.vote.answers.iter().all(|&a| a == 1) {
            self.end_vote(true, Msg::default());
        }
    }

    fn end_vote(&mut self, pass: bool, why: Msg) {
        self.vote = MatchVote::default();
        self.changed = true;
        self.seq += 1;
        if pass {
            self.host_log("vote", -1, Msg::new("log.vote_passed"));
            self.finish_now("vote");
        } else {
            self.vote_cooldown = VOTE_COOLDOWN;
            self.host_log("vote", -1, Msg::new("log.vote_failed").msg("why", why));
        }
    }

    // ---------------------------------------------------------------- lifecycle

    /// End the match now and rank by score (`Finish()`).
    pub fn finish(&mut self) {
        self.finish_now("settle");
    }

    fn finish_now(&mut self, reason: &'static str) {
        if self.pending.is_some() {
            // Keep what players have seen; drop the unfinished routine.
            self.pending = None;
            self.flush_deferred();
        }
        if self.world.st.phase == "play" {
            self.direct(|cx| cx.finish(reason, None));
        } else {
            self.deferred.push(Deferred::Finish(reason.into()));
        }
    }

    /// Skip the remaining pre-game phases with random personas and preset decks.
    pub fn quick_start(&mut self) {
        if matches!(self.world.st.phase.as_str(), "play" | "ended") {
            return;
        }
        self.direct(|cx| {
            for i in 0..cx.w.player_count() {
                if cx.w.st.players[i].persona.is_empty() {
                    let c = cx.random_persona(true);
                    cx.w.st.players[i].persona = c;
                }
                let d = cx.data;
                cx.w.bind_skills(d, cx.rules, i as i32);
                if !cx.w.st.players[i].deck_ready {
                    cx.submit_deck(i, None);
                }
            }
        });
        self.begin_play();
    }

    /// A member disconnected or left a room before play (`MemberLeft`): the
    /// machine takes over the player.
    pub fn member_left(&mut self, member: i32, can_return: bool) {
        let Some(i) = self.player_index(member) else {
            return;
        };
        if self.pending.is_some() {
            self.deferred.push(Deferred::Left(i, can_return));
            self.tick_live(0.0);
        } else {
            self.apply_left(i, can_return);
        }
    }

    fn apply_left(&mut self, i: usize, can_return: bool) {
        if self.world.st.players[i].ai || self.ended() {
            return;
        }
        let playing = self.world.st.phase == "play";
        let turn = self.world.st.turn == i as i32;
        self.direct(|cx| {
            cx.w.st.players[i].ai = true;
            let key = match (playing, can_return) {
                (true, true) => "log.dropped_returnable",
                (true, false) => "log.dropped",
                (false, true) => "log.left_returnable",
                (false, false) => "log.left",
            };
            let text = Msg::new(key).player_id("who", i);
            cx.w.log("ai", i as i32, text);
        });
        if turn {
            self.wait = CHOICE_WAIT;
        }
    }

    /// The member reconnected (`MemberBack`).
    pub fn member_back(&mut self, member: i32) {
        let Some(i) = self.player_index(member) else {
            return;
        };
        if self.pending.is_some() {
            self.deferred
                .retain(|d| !matches!(d, Deferred::Left(s, _) if *s == i));
            self.deferred.push(Deferred::Back(i));
        } else {
            self.apply_back(i);
        }
    }

    fn apply_back(&mut self, i: usize) {
        let s = &self.world.st.players[i];
        if self.ended() || !s.ai || s.bot || s.out() {
            return;
        }
        self.direct(|cx| {
            cx.w.st.players[i].ai = false;
            cx.w.log(
                "ai",
                i as i32,
                Msg::new("log.reconnected").player_id("who", i),
            );
        });
    }
}

fn run(cx: &mut Cx, r: &Routine) -> Flow<()> {
    match r {
        Routine::Opening => cx.opening(),
        Routine::NextTurn => cx.next_turn(),
        Routine::Ai(i) => cx.ai_step(*i),
        Routine::Leftovers(d) => cx.auction_leftovers(d.clone()),
        Routine::Act(i, m) => {
            let i = *i;
            let pos = cx.w.st.players[i].pos.max(0) as usize;
            match m.act.as_str() {
                "roll" => {
                    let turn = cx.w.st.turn as usize;
                    cx.main_move(turn, i)
                }
                // Retired: buy / build are settle-time offers only. The parse
                // path still accepts these acts (replay / old clients) and
                // rejects them with a clear reason rather than failing to
                // deserialize the record.
                "buy" | "build" => {
                    cx.w.log("text", i as i32, Msg::new("err.buy_build_at_settle"));
                    Ok(())
                }
                "mortgage" => cx.deed_mortgage(i, m.value as usize, 0).map(|_| ()),

                "redeem" => cx.deed_redeem(i, m.value as usize).map(|_| ()),
                "play" => cx.play_from_hand(i, &m.card),
                "skill" => cx.use_skill(i, &m.card),
                "discard" => cx.discard(i, &m.card),
                "end" => cx.end_turn_cmd(i),
                "leave" => cx.forfeit(i),
                _ => Ok(()),
            }
        }
    }
}

/// `DoAct` checks for the turn commands.
fn why_not_act(cx: &Cx, i: usize, m: &NetMessage, busy: bool) -> Option<Msg> {
    let st = &cx.w.st;
    let my_turn = cx.playing() && st.turn == i as i32;
    let pos = st.players[i].pos;
    let moved_off = |key: &str| {
        (m.value > 0 && m.value != pos).then(|| Msg::new(key).tile("was", m.value).tile("now", pos))
    };
    match m.act.as_str() {
        "roll" => {
            if !cx.playing() || st.step != stage::OPS || busy {
                return Some(Msg::new("err.no_roll_now"));
            }
            if st.skip_move && st.turn == i as i32 {
                return Some(cx.rules.gate_refusal_msg(Gate::Move, i));
            }
            // The turn's main move is already spent (an off-board main-move expiry
            // teleport, or a `card_move` that ran it early): 「一回合只能触发
            // 一次［主要移动］效果,多次触发［主要移动］时无效」. Refuse the roll
            // and let `end` through below.
            if cx.w.turn.main_moved && st.turn == i as i32 {
                return Some(Msg::new("err.roll_main_used"));
            }
            if st.roller != i as i32 {
                if st.turn != i as i32 || st.roller < 0 {
                    return Some(Msg::new("err.not_your_roll"));
                }
                return Some(Msg::new("err.roller_other").player_id("who", st.roller));
            }
            None
        }
        "skill" => {
            // 「运营阶段」 -- the same window every other turn action gets.
            if !cx.playing() || st.step != stage::OPS || busy {
                return Some(Msg::new("err.skill_not_now"));
            }
            if m.card.is_empty() {
                return Some(Msg::new("err.skill_unknown"));
            }
            // The ruleset's act / on-board gates. A card the ruleset exempts
            // from the act gate (`exemption_prop`) skips that one check.
            // Mirrors `cannot_play` for hand plays.
            if !cx.w.gate_open(cx.rules, Gate::OnBoard, i) {
                return Some(cx.rules.gate_refusal_msg(Gate::OnBoard, i));
            }
            if !cx.w.gate_open(cx.rules, Gate::Act, i) && !cx.rules.card_act_exemption(&m.card) {
                return Some(cx.rules.gate_refusal_msg(Gate::Act, i));
            }
            // The rule's own gate (`On::Play`'s), so the cost check remains in the
            // rule body where the 规则书 clause lives.
            cx.rules_cant_play(i, &m.card)
        }
        // Retired: buy / build are settle-time offers only. The parse path
        // still accepts these acts (replay / old clients) and rejects them
        // with a clear reason.
        "buy" | "build" => Some(Msg::new("err.buy_build_at_settle")),
        "mortgage" => cx.why_not_mortgage(i, m.value, busy),
        "redeem" => cx.why_not_redeem(i, m.value, busy),
        "play" => cx.cant_play(i, &m.card, busy),
        "discard" => {
            if !cx.playing() || !cx.over_hand(i) {
                return Some(Msg::new("err.not_over_hand"));
            }
            if !cx.w.hidden[i].hand.contains(&m.card) {
                return Some(Msg::new("err.no_such_card"));
            }
            None
        }
        "end" => {
            if !my_turn {
                return Some(Msg::new("err.not_your_turn"));
            }
            if busy {
                return Some(Msg::new("err.busy"));
            }
            if st.step == stage::OPS && !st.skip_move && !cx.w.turn.main_moved {
                return Some(Msg::new("err.roll_first"));
            }
            if st.step == stage::MOVE {
                return Some(Msg::new("err.moving"));
            }
            if cx.over_hand(i) {
                return Some(Msg::new("err.over_hand").i(
                    "limit",
                    i64::from(cx.w.st.players[i].hand_limit().unwrap_or_default()),
                ));
            }
            None
        }
        _ => None,
    }
}
