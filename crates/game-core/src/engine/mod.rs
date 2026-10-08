//! The match engine: the port of `MatchHost`'s game shell.
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
mod debug;
mod move_ctx;
mod ops;
mod play;
pub mod rules;
mod setup;
mod world;

pub use ai::{
    bot_wants_build, bot_wants_buy, bot_wants_force_buy, bot_wants_play_card, bot_wants_redeem,
    wants_buy, wants_build, wants_redeem, BUY_RESERVE, BUILD_RESERVE, CHAOS_COUNTER_CHANCE,
    CHAOS_RESERVE, FORCE_BUY_RESERVE, MAX_PLAYS_PER_TURN, PLAY_CARD_CHANCE, REDEEM_RESERVE,
};
pub use cx::{Answered, AnswerProvider, Ask, Cx, Flow, Halt, HeuristicProvider, Reply, AI_UNSET};
pub use move_ctx::{MoveCtx, MoveKind, Roll};
pub use play::{Paid, Pay};
pub use play::purchase;
pub use rules::{CardRules, Dest, StubRules, Trigger};
pub use world::{Hidden, Scheduled, TurnCtx, World};

/// Measurement counters for `docs/BOT.md` §5 (B0). Compiled out unless the
/// `bot-cost` feature is on; nothing behavioural either way.
#[cfg(feature = "bot-cost")]
pub mod bot_cost {
    use std::sync::atomic::AtomicU64;
    /// [`Cx::world_copy`] calls -- the per-drive / per-probe `World` clones.
    /// Engine-side routine snapshots (`Match::execute` / `start`) clone the
    /// world too, 1-2 per routine (re)run; those are not counted here.
    pub static WORLD_CLONES: AtomicU64 = AtomicU64::new(0);
}

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::msg::Msg;
use crate::net::{NetMessage, RoomMember};
use crate::rng::Rng;
use crate::scoring::ScoreWeights;
use crate::state::stage;
use crate::state::{MatchEvent, MatchPlayer, MatchState, MatchVote};
use crate::MatchMode;

use cx::HaltKind;
use world::Signal;

/// Longest pause the host keeps after a routine before the next automatic step.
const MAX_WAIT: f32 = 6.0;
/// Pause between pre-game choices (`StartChoice`).
const CHOICE_WAIT: f32 = 1.2;
/// Seconds before the first phase starts (`_wait` in the constructor).
const ORDER_WAIT: f32 = 4.0;
/// Turn clock: shield per turn, bank refill per turn, bank cap (`StartTimer`).
const SHIELD: f32 = 15.0;
const BANK_REFILL: f32 = 10.0;
const BANK_MAX: f32 = 30.0;
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
}

impl Live {
    fn new(ask: Ask) -> Self {
        let n = ask.view.players.len();
        Self {
            answers: vec![-1; n],
            picked: vec![],
            ai_at: vec![None; n],
            time_left: ask.view.time_left,
            bid: 0,
            bidder: -1,
            ask,
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
    bank: Vec<f32>,
    shield: f32,
    timed_out: bool,
    vote: MatchVote,
    vote_seq: i32,
    vote_cooldown: f32,
    deferred: Vec<Deferred>,
    seq: i32,
}

/// Bumped whenever [`Saved`] changes shape; older saves are rejected.
/// v2: `FieldCard.hand_limit_delta` became the generic `FieldCard.props` map.
/// v5: `World.marker_owner` (marker ownership, user ruling 2026-10-07).
/// Public so a match record's [`crate::record::EngineStamp`] can name the save
/// format it was written against (see `docs/REPLAY.md`).
pub const SAVE_VERSION: u32 = 5;

/// The `TileMark.kind` a [CP点] wore before it had a category of its own
/// (`card-general`'s `key!("clear_cp_mark")`). [`Match::restore`] re-reads one
/// of these into [`crate::state::mark_category::CP`]; new writes use
/// [`crate::state::mark_kind::CP`] and never this.
const LEGACY_CP_KIND: &str = "cards:card-general.clear_cp_mark";

/// A running match.
pub struct Match {
    data: Arc<GameData>,
    rules: Arc<dyn CardRules>,
    mode: MatchMode,
    world: World,
    pending: Option<Pending>,
    live_rng: Rng,
    wait: f32,
    bank: Vec<f32>,
    shield: f32,
    timed_out: bool,
    vote: MatchVote,
    vote_seq: i32,
    vote_cooldown: f32,
    deferred: Vec<Deferred>,
    seq: i32,
    changed: bool,
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
            bank: self.bank.clone(),
            shield: self.shield,
            timed_out: self.timed_out,
            vote: self.vote.clone(),
            vote_seq: self.vote_seq,
            vote_cooldown: self.vote_cooldown,
            deferred: self.deferred.clone(),
            seq: self.seq,
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

    fn restore_saved(data: Arc<GameData>, rules: Arc<dyn CardRules>, s: Saved) -> Result<Self, Msg> {
        if s.version != SAVE_VERSION {
            return Err(Msg::new("err.save_version"));
        }
        // Serde defaults fill in `TileMark.category` / `TileMark.src` for older
        // saves, but a [CP点] written before the category existed only says so
        // through its `kind` (and was player-owned, which it must not be). Say
        // it through the category instead so the view and the rules see the
        // same thing a fresh game writes.
        let mut world = s.world;
        for m in &mut world.st.marks {
            if m.category.is_empty() && m.kind == LEGACY_CP_KIND {
                m.category = crate::state::mark_category::CP.to_string();
                m.kind = crate::state::mark_kind::CP.to_string();
                m.owner = crate::state::BOARD_OWNER;
                // No instance provenance survived the old format; these marks
                // belong to no live card instance.
                m.src = -1;
            }
        }
        Ok(Self {
            data,
            rules,
            mode: s.mode,
            world,
            pending: s.pending,
            live_rng: s.live_rng,
            wait: s.wait,
            bank: s.bank,
            shield: s.shield,
            timed_out: s.timed_out,
            vote: s.vote,
            vote_seq: s.vote_seq,
            vote_cooldown: s.vote_cooldown,
            deferred: s.deferred,
            seq: s.seq,
            changed: true,
            provider: None,
            prompt_log: Vec::new(),
        })
    }

    /// `MatchHost(members, seed, mode, weights)`. At most 10 players.
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        members: &[RoomMember],
        seed: u64,
        mode: MatchMode,
        weights: ScoreWeights,
    ) -> Self {
        let members = &members[..members.len().min(10)];
        let w = weights.sanitized(ScoreWeights::from_rules(&data.match_rules));
        // Solo with a character already on every seat (the solo setup screen's
        // picks, its randoms resolved before the match is built) skips the
        // timed ban / pick entirely and opens the deck phase instead. Online
        // never takes this path: there `RoomMember.character` is the lobby
        // avatar, not a match pick, and the pick phase must stay. An unknown
        // or duplicated pick is not a preset -- fall through to the pick phase.
        let preset = mode == MatchMode::Solo
            && !members.is_empty()
            && members.iter().all(|m| data.character(&m.character).is_some())
            && {
                let mut names: Vec<&str> = members.iter().map(|m| m.character.as_str()).collect();
                names.sort_unstable();
                names.dedup();
                names.len() == members.len()
            };
        let st = MatchState {
            match_id: ((seed as i32) & 0x7FFF_FFFF) | 1,
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
                    // (ban / pick / deck) stays engine-side
                    // (`MatchPlayer::auto_setup`). An online room with no
                    // service attached rewrites the mentality to Standard
                    // before `new_match`; Solo's driver answers through the
                    // ordinary `act` path and falls back to the heuristic, so
                    // a seat is never parked on answers nobody will send.
                    ai: m.bot && m.mentality != crate::state::BotMentality::Advanced,
                    mentality: m.mentality,
                    // Carries through `roll_order`, so the seat that rolls high
                    // keeps the character its member was given.
                    character: if preset {
                        m.character.clone()
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
        let mut m = Self {
            world: World::new(st, n, seed),
            live_rng: Rng::new(seed ^ 0x5EED_1A7E),
            data,
            rules,
            mode,
            pending: None,
            wait: ORDER_WAIT,
            bank: vec![0.0; n],
            shield: 0.0,
            timed_out: false,
            vote: MatchVote::default(),
            vote_seq: 0,
            vote_cooldown: 0.0,
            deferred: vec![],
            seq: 0,
            changed: true,
            provider: None,
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
                    let c = cx.w.st.players[i].character.clone();
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
            st.prompt.time_left = if self.mode == MatchMode::Solo {
                l.ask.view.time_left.max(l.time_left).max(0.0)
            } else {
                l.time_left.max(0.0)
            };
            st.prompt.bid = l.bid;
            st.prompt.bidder = l.bidder;
        } else {
            st.prompt = Default::default();
        }
        if st.phase == "play" && st.turn >= 0 {
            // `st.buy_price` / `st.build_cost` view previews (`docs/PURCHASE.md`):
            // the quoted price at the player's position when a buy/build is on
            // the table, `-1` otherwise. `st.can_buy_here` / `st.can_build_here`
            // are the matching act-legality flags (`why_not_act`), so a client
            // -- or the bot's action abstraction -- never offers a command the
            // engine would refuse (`err.buy_poor` / `err.cannot_buy` / `err.poor`).
            let ti = st.turn as usize;
            let pos = st.players.get(ti).map(|p| p.pos).unwrap_or(-1);
            st.buy_price = -1;
            st.build_cost = -1;
            st.can_buy_here = false;
            st.can_build_here = false;
            st.can_roll_here = false;
            st.can_end_here = false;
            let money = st.players.get(ti).map(|p| p.money).unwrap_or(0);
            // `Cx::can_pay` (not out / stunned / exiled), without building a Cx
            // (which would clone the world -- this runs inside rollouts).
            let can_pay = st
                .players
                .get(ti)
                .map(|p| !p.out() && !p.stunned() && p.exile() == 0)
                .unwrap_or(false);
            if st.step == stage::END
                && !st.bought
                && pos >= 0
                && st.landed == pos
                && st.owners.get(pos as usize).copied().unwrap_or(-1) < 0
            {
                // The **quoted** price (`docs/PURCHASE.md`) -- the rules crate's
                // `buy_quote`, so a hook-aware discount shows in the view -- and
                // `-1` when the gate refuses the buy outright.
                let q = purchase::quote_for(
                    self.rules.as_ref(),
                    &self.world,
                    &self.data,
                    ti,
                    purchase::BuyKind::Land,
                    &[(pos as usize, -1)],
                );
                st.buy_price = q.first().filter(|q| q.eligible).map_or(-1, |q| q.price);
                // `why_not_act`'s buy branch: `buyable_here` (shape + the
                // plan's no-buy flag + `eligible`) plus the quoted funds check.
                // The shape / `eligible` halves are what `buy_price >= 0`
                // already answered above.
                st.can_buy_here = st.buy_price >= 0
                    && self
                        .data
                        .tiles
                        .get(pos as usize)
                        .is_some_and(|t| t.is_buyable())
                    && !self.world.turn.plan.no_buy
                    && can_pay
                    && money >= st.buy_price.max(0);
            }
            if st.step == stage::END && !st.built && !st.bought && pos >= 0 && st.landed == pos {
                // Only preview a build cost when a build is actually possible
                // (owned, not mortgaged, under the cap).
                let t = pos as usize;
                if st.owners.get(t).copied().unwrap_or(-1) == st.turn
                    && !st.mortgaged.get(t).copied().unwrap_or(false)
                {
                    if let Some(tile) = self.data.tiles.get(t) {
                        let houses = st.houses.get(t).copied().unwrap_or(0);
                        if houses < tile.rent.len().saturating_sub(1) as i32 {
                            st.build_cost = tile.house.max(0);
                        }
                    }
                }
                // `why_not_act`'s build branch: `why_not_build` (the plan's
                // `can_build` flag and `why_not_build_on`'s NO_BUILD vetoes,
                // the halves the cost preview does not carry) plus the funds.
                st.can_build_here = st.build_cost >= 0
                    && st.plan.can_build
                    && self
                        .world
                        .why_not_build_on(&self.data, ti as i32, pos)
                        .is_none()
                    && money >= st.build_cost.max(0);
            }
            let bank = self.bank.get(st.turn as usize).copied().unwrap_or(0.0);
            st.shield = self.shield;
            st.bank = bank;
            st.time_left = self.shield + bank;
            // `why_not_act`'s roll / end branches, from the **live** fields the
            // gate reads (`w.st.skip_move`'s latch, `w.turn.main_moved`, the
            // hidden hand). The public `skip_move` below re-derives from
            // stay / exile and can disagree (unstoppable, mid-turn [停留],
            // [除外]); a bot that trusted it sent `end` into `err.roll_first`.
            {
                let live_skip = self.world.st.skip_move;
                let main_moved = self.world.turn.main_moved;
                let over_hand = self
                    .world
                    .hidden
                    .get(ti)
                    .map(|h| h.hand.len() as i32)
                    .unwrap_or(0)
                    > st.players.get(ti).map(|p| p.hand_limit()).unwrap_or(5);
                let my_turn = st.turn == ti as i32;
                let owes_roll = st.step == stage::OPS && !live_skip && !main_moved;
                st.can_roll_here = my_turn
                    && st.step == stage::OPS
                    && !st.busy
                    && !live_skip
                    && !main_moved
                    && st.roller == st.turn;
                st.can_end_here = my_turn
                    && !st.busy
                    && st.step != stage::MOVE
                    && !owes_roll
                    && !over_hand;
            }
            if st.step == stage::OPS && !st.busy {
                if let Some(s) = st.current() {
                    st.skip_move = s.stay() > 0 || s.exile() > 0;
                }
            }
        }
        st
    }

    /// True once after every change; the server broadcasts on it.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn ended(&self) -> bool {
        self.world.st.phase == "ended"
    }

    /// Events with id greater than `last_id` (`EventsSince`).
    pub fn events_since(&self, last_id: i32) -> Vec<MatchEvent> {
        self.world
            .recent
            .iter()
            .filter(|e| e.id > last_id)
            .cloned()
            .collect()
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
    /// Test seam: set a player's character (exclusive-card checks read it).
    #[doc(hidden)]
    pub fn set_character(&mut self, member: i32, character: &str) {
        if let Some(i) = self.player_index(member) {
            self.world.st.players[i].character = character.to_string();
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

    pub fn hand_of(&self, member: i32) -> Vec<String> {
        self.player_index(member)
            .map(|i| self.world.hidden[i].hand.clone())
            .unwrap_or_default()
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
            .map(|i| self.world.hidden[i].draw.clone())
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
        let Some(i) = self.player_index(member) else {
            return serde_json::json!({ "aiAnswer": null, "playable": [], "estCost": [] });
        };
        let ai_answer = self.pending.as_ref().and_then(|p| {
            let l = &p.live;
            let k = l.ask.view.player_index(i as i32)?;
            if l.answers.get(k).copied().unwrap_or(-1) >= 0 {
                return None;
            }
            Some(serde_json::json!({
                "answer": l.ask.ai.get(k).copied().unwrap_or(l.ask.view.fallback),
                "picked": l.ask.ai_picked.get(k).cloned().unwrap_or_default(),
                "worth": l.ask.worth.get(k).copied().unwrap_or(0),
            }))
        });
        let cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        let mut playable = Vec::new();
        let mut est_cost = Vec::new();
        for c in &self.world.hidden[i].hand {
            playable.push(cx.cant_play(i, c, false).is_none());
            est_cost.push(self.rules.card_prop(c, crate::state::prop::EST_COST));
        }
        serde_json::json!({ "aiAnswer": ai_answer, "playable": playable, "estCost": est_cost })
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
            bank: self.bank.clone(),
            shield: self.shield,
            timed_out: self.timed_out,
            vote: self.vote.clone(),
            vote_seq: self.vote_seq,
            vote_cooldown: self.vote_cooldown,
            deferred: self.deferred.clone(),
            seq: self.seq,
            changed: self.changed,
            provider: None,
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
        self.world = cx.w;
        self.changed = true;
        self.seq += 1;
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
            (res, cx.delay, cx.w)
        };
        self.provider = prov;
        self.changed = true;
        self.seq += 1;
        match res {
            Ok(()) | Err(Halt(HaltKind::Ended)) => {
                let signals = std::mem::take(&mut w.signals);
                self.world = w;
                self.pending = None;
                self.wait = delay.min(MAX_WAIT);
                for s in signals {
                    match s {
                        Signal::StartTimer(i) => {
                            self.bank[i] = (self.bank[i] + BANK_REFILL).min(BANK_MAX);
                            self.shield = SHIELD;
                        }
                        Signal::TurnBegan => self.timed_out = false,
                    }
                }
                self.flush_deferred();
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
    }

    /// Is this prompt player answered by the machine (bot, or a human who left)?
    fn auto_player(&self, player_id: usize) -> bool {
        self.world.st.players[player_id].ai
            || self
                .deferred
                .iter()
                .any(|d| matches!(d, Deferred::Leave(s) | Deferred::Left(s, _) if *s == player_id))
    }

    // ---------------------------------------------------------------- clock

    /// Advance the match by `dt` seconds (`Tick`).
    pub fn tick(&mut self, dt: f32) {
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
                // Standard: 50% a random character, else no ban. Chaos always
                // bans something if anything is left. A human seat (time-out)
                // never bans.
                let chaos = cx.is_chaos(turn);
                let pick = if cx.w.st.players[turn].auto_setup()
                    && (chaos || cx.w.rng.chance(0.5))
                {
                    cx.random_character(false)
                } else {
                    String::new()
                };
                cx.do_ban(turn, &pick);
            });
        } else {
            self.direct(|cx| {
                let c = cx.random_character(true);
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
            self.world.st.roller = turn as i32;
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
                if self.shield > 0.0 {
                    self.shield = (self.shield - dt).max(0.0);
                } else {
                    self.bank[turn] = (self.bank[turn] - dt).max(0.0);
                }
                if self.shield + self.bank[turn] > 0.0 {
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
                !x.out() && !x.stunned() && x.exile() == 0
            })
            .collect();
        // Standard's bid step / nudge read the seat's `StrategyParams`
        // (`docs/BOT.md` §3.8). Chaos keeps the literals. Resolved up front so
        // the `live` / `live_rng` borrows below stay disjoint.
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
                        }
                    }
                    _ => {}
                }
            }
            let open = (0..players.len())
                .filter(|&k| l.answers[k] < 0 && l.bidder != players[k] as i32)
                .count();
            open == 0 || (l.time_left <= 0.0 && !hold)
        } else {
            let picks = matches!(l.kind(), "pick" | "mortgage");
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
                match l.ai_at[k] {
                    None => l.ai_at[k] = Some(l.time_left - (0.4 + rng.f64() as f32 * 0.6)),
                    Some(at) if l.time_left <= at => {
                        if picks {
                            l.picked = l.ask.ai_picked[k].clone();
                            l.answers[k] = 0;
                        } else {
                            l.answers[k] = l.ask.ai[k].clamp(0, max.max(0));
                        }
                    }
                    _ => {}
                }
            }
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
        };
        if (l.answers.clone(), l.bid) != before {
            self.changed = true;
            self.seq += 1;
        }
        if done {
            self.complete_prompt();
        }
    }

    // ---------------------------------------------------------------- input

    /// A player command (`Act`). `Err` carries the message shown to the player.
    pub fn act(&mut self, member: i32, m: &NetMessage) -> Result<(), Msg> {
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
                let c = m.character.clone();
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
                let c = m.character.clone();
                if !self.with_cx(|cx| cx.pickable(&c)) {
                    return Err(Msg::new("err.chara_taken"));
                }
                self.direct(|cx| cx.do_pick(i, &c));
                self.after_choice();
            }
            "deck" => {
                if phase != "deck" || self.world.st.players[i].deck_ready {
                    return Err(Msg::new("err.no_deck_now"));
                }
                let ok = self
                    .data
                    .character(&self.world.st.players[i].character)
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
                    // B4 (`PIPELINE-AUDIT` K10b) -- 规则书 L76 「需[支付]或[消耗]
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
            "tile" => {
                let n = l.ask.view.items.len() as i32;
                let v = if m.value < 0 { n } else { m.value };
                if v > n {
                    return Err(Msg::new("err.no_option"));
                }
                l.answers[k] = v;
            }
            _ => {
                if m.value < 0 || m.value as usize >= l.ask.view.options.len() {
                    return Err(Msg::new("err.no_option"));
                }
                l.answers[k] = m.value;
            }
        }
        let complete = l.kind() != "auction" && l.answers.iter().all(|&a| a >= 0);
        self.changed = true;
        self.seq += 1;
        if complete {
            self.complete_prompt();
        } else if self
            .pending
            .as_ref()
            .is_some_and(|p| p.live.kind() == "auction")
        {
            self.tick_live(0.0);
        }
        Ok(())
    }

    // ---------------------------------------------------------------- vote

    fn voters(&self) -> Vec<usize> {
        (0..self.world.player_count())
            .filter(|&p| !self.world.out(p) && !self.world.st.players[p].ai)
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
            if self.vote.answers[k] < 0 && (self.world.out(s) || self.auto_player(s)) {
                self.vote.answers[k] = 1;
                self.changed = true;
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

    /// Skip the remaining pre-game phases with random characters and preset decks.
    pub fn quick_start(&mut self) {
        if matches!(self.world.st.phase.as_str(), "play" | "ended") {
            return;
        }
        self.direct(|cx| {
            for i in 0..cx.w.player_count() {
                if cx.w.st.players[i].character.is_empty() {
                    let c = cx.random_character(true);
                    cx.w.st.players[i].character = c;
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
                "buy" => {
                    cx.w.st.bought = true;
                    cx.buy(i, pos, purchase::BuyKind::Land)
                }
                "build" => {
                    cx.w.st.built = true;
                    cx.build(i, pos)
                }
                "mortgage" => cx.mortgage(i, m.value as usize, None),
                "redeem" => {
                    cx.redeem(i, m.value as usize);
                    Ok(())
                }
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
                return Some(Msg::new("err.roll_stay"));
            }
            // The turn's main move is already spent (an `exileMain` expiry
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
            // The rule's own gate (`On::Play`'s), so the cost check stays in the
            // rule body where the 规则书 clause lives.
            cx.rules.cant_play(cx, i, &m.card)
        }
        "buy" => {
            if !my_turn || st.step != stage::END || busy {
                return Some(Msg::new("err.no_buy_now"));
            }
            if let Some(e) = moved_off("err.moved_off_buy") {
                return Some(e);
            }
            if !cx.buyable_here(i) {
                return Some(Msg::new("err.cannot_buy"));
            }
            // The quoted price, not the base (`docs/PURCHASE.md`): a player who
            // can afford only the discounted price is no longer refused.
            let quote = cx.buy_quote_for(i, pos as usize, purchase::BuyKind::Land);
            if quote.price < 0 {
                return Some(Msg::new("err.cannot_buy"));
            }
            if st.players[i].money < quote.price.max(0) {
                return Some(Msg::new("err.buy_poor"));
            }
            None
        }
        "build" => {
            if !my_turn || st.step != stage::END || busy {
                return Some(Msg::new("err.no_build_now"));
            }
            if let Some(e) = moved_off("err.moved_off_build") {
                return Some(e);
            }
            if st.built {
                return Some(Msg::new("err.built_already"));
            }
            if let Some(e) = cx.why_not_build(i, pos as usize) {
                return Some(e);
            }
            if st.players[i].money < cx.build_cost(pos as usize) {
                return Some(Msg::new("err.poor"));
            }
            None
        }
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
                return Some(Msg::new("err.over_hand").i("limit", cx.w.st.players[i].hand_limit() as i64));
            }
            None
        }
        _ => None,
    }
}
