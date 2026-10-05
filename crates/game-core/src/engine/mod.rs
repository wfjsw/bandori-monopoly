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
mod ops;
mod play;
pub mod rules;
mod setup;
mod world;

pub use cx::{Answered, Ask, Cx, Flow, Halt, Reply};
pub use rules::{CardRules, Dest, StubRules, Trigger};
pub use world::{Hidden, TurnCtx, World};

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::msg::Msg;
use crate::net::{NetMessage, RoomMember};
use crate::rng::Rng;
use crate::scoring::ScoreWeights;
use crate::state::{MatchEvent, MatchSeat, MatchState, MatchVote};
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
        let n = ask.view.seats.len();
        Self { answers: vec![-1; n], picked: vec![], ai_at: vec![None; n], time_left: ask.view.time_left, bid: 0, bidder: -1, ask }
    }

    fn kind(&self) -> &str {
        &self.ask.view.kind
    }

    fn place_bid(&mut self, seat: usize, amount: i32) {
        self.bid = amount;
        self.bidder = seat as i32;
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
const SAVE_VERSION: u32 = 1;

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
    pub fn restore(data: Arc<GameData>, rules: Arc<dyn CardRules>, json: &str) -> Result<Self, Msg> {
        let s: Saved = serde_json::from_str(json).map_err(|e| Msg::new("err.save_corrupt").text("detail", e.to_string()))?;
        if s.version != SAVE_VERSION {
            return Err(Msg::new("err.save_version"));
        }
        Ok(Self {
            data,
            rules,
            mode: s.mode,
            world: s.world,
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
        })
    }

    /// `MatchHost(members, seed, mode, weights)`. At most 10 seats.
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
        let st = MatchState {
            match_id: ((seed as i32) & 0x7FFF_FFFF) | 1,
            phase: "order".into(),
            mode: mode as i32,
            seats: members
                .iter()
                .map(|m| MatchSeat { member: m.id, player: m.player.clone(), bot: m.bot, ai: m.bot, ..MatchSeat::default() })
                .collect(),
            score_money: w.money,
            score_property: w.property,
            score_houses: w.houses,
            time_left: ORDER_WAIT,
            ..MatchState::default()
        };
        let n = st.seats.len();
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
        };
        m.direct(|cx| cx.roll_order());
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
            st.prompt.time_left = l.time_left.max(0.0);
            st.prompt.bid = l.bid;
            st.prompt.bidder = l.bidder;
        } else {
            st.prompt = Default::default();
        }
        if st.phase == "play" && st.turn >= 0 {
            let bank = self.bank.get(st.turn as usize).copied().unwrap_or(0.0);
            st.shield = self.shield;
            st.bank = bank;
            st.time_left = self.shield + bank;
            if st.step == 1 && !st.busy {
                if let Some(s) = st.current() {
                    st.skip_move = s.stay > 0 || s.exile > 0;
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
        self.world.recent.iter().filter(|e| e.id > last_id).cloned().collect()
    }

    fn seat_index(&self, member: i32) -> Option<usize> {
        self.world.st.seats.iter().position(|s| s.member == member)
    }

    pub fn seat_of_member(&self, member: i32) -> Option<&MatchSeat> {
        self.seat_index(member).map(|i| &self.world.st.seats[i])
    }

    /// The member's hand -- private, never part of [`Match::state`].
    /// Test seam: set a seat's character (exclusive-card checks read it).
    #[doc(hidden)]
    pub fn set_character(&mut self, member: i32, character: &str) {
        if let Some(i) = self.seat_index(member) {
            self.world.st.seats[i].character = character.to_string();
        }
    }

    /// Test seam: put cards straight into a seat's hand.
    #[doc(hidden)]
    pub fn give_cards(&mut self, member: i32, cards: &[&str]) {
        let Some(i) = self.seat_index(member) else { return };
        for c in cards {
            self.world.hidden[i].hand.push((*c).to_string());
        }
    }

    pub fn hand_of(&self, member: i32) -> Vec<String> {
        self.seat_index(member).map(|i| self.world.hidden[i].hand.clone()).unwrap_or_default()
    }

    /// Per-card notes for the member's hand.
    pub fn hand_notes_of(&self, member: i32) -> Vec<Msg> {
        let Some(i) = self.seat_index(member) else { return vec![] };
        let cx = Cx::new(self.world.clone(), &self.data, &*self.rules, &[]);
        self.world.hidden[i].hand.iter().map(|c| self.rules.hand_note(&cx, i, c)).collect()
    }

    // ---------------------------------------------------------------- running routines

    /// Run `f` directly on the world (no prompts possible). Only valid with no
    /// routine pending.
    fn direct<R>(&mut self, f: impl FnOnce(&mut Cx) -> R) -> R {
        debug_assert!(self.pending.is_none(), "direct world mutation while a routine is pending");
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
        let mut cx = Cx::new(snapshot.clone(), &data, &*rules, &answers);
        let res = run(&mut cx, &routine);
        let delay = cx.delay;
        let mut w = cx.w;
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
                self.pending = Some(Pending { routine, snapshot, answers, live: Live::new(*ask) });
            }
        }
    }

    fn complete_prompt(&mut self) {
        let Some(p) = self.pending.take() else { return };
        let fallback = p.live.ask.view.fallback;
        let a = Answered {
            answers: p.live.answers.iter().map(|&x| if x < 0 { fallback } else { x }).collect(),
            picked: p.live.picked,
            bid: p.live.bid,
            bidder: p.live.bidder,
        };
        let mut answers = p.answers;
        answers.push(a);
        self.execute(p.routine, p.snapshot, answers);
    }

    /// Host-side log line; deferred while a routine is pending.
    fn host_log(&mut self, kind: &str, seat: i32, text: Msg) {
        if self.pending.is_some() {
            self.deferred.push(Deferred::Log(kind.into(), seat, text));
        } else {
            self.direct(|cx| {
                cx.w.log(kind, seat, text);
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

    /// Is this prompt seat answered by the machine (bot, or a human who left)?
    fn auto_seat(&self, seat: usize) -> bool {
        self.world.st.seats[seat].ai
            || self.deferred.iter().any(|d| matches!(d, Deferred::Leave(s) | Deferred::Left(s, _) if *s == seat))
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
                    self.direct(|cx| if ranked { cx.begin_ban() } else { cx.begin_pick() });
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
        self.world.st.time_left -= dt;
        if self.world.st.phase == "deck" {
            if self.world.st.time_left <= 0.0 {
                self.direct(|cx| {
                    for i in 0..cx.w.seat_count() {
                        if !cx.w.st.seats[i].deck_ready {
                            cx.submit_deck(i, None);
                        }
                    }
                });
                self.begin_play();
            }
            return;
        }
        let Some(cur) = self.world.st.current() else { return };
        let due = if cur.ai { self.wait <= 0.0 } else { self.world.st.time_left <= 0.0 };
        if !due {
            return;
        }
        let turn = self.world.st.turn as usize;
        if self.world.st.phase == "ban" {
            self.direct(|cx| {
                let pick = if cx.w.st.seats[turn].ai && cx.w.rng.chance(0.5) { cx.random_character(false) } else { String::new() };
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
        if self.world.st.phase == "deck" && self.world.st.seats.iter().all(|s| s.deck_ready) {
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
        let st = &self.world.st;
        let actor = if st.step == 1 && !st.skip_move && st.roller >= 0 { st.roller as usize } else { turn };
        if !st.seats[actor].ai && !self.timed_out {
            if self.shield > 0.0 {
                self.shield = (self.shield - dt).max(0.0);
            } else {
                self.bank[turn] = (self.bank[turn] - dt).max(0.0);
            }
            if self.shield + self.bank[turn] > 0.0 {
                return;
            }
            self.timed_out = true;
            self.host_log("text", turn as i32, Msg::new("log.timeout").seat("who", turn));
        }
        self.start(Routine::Ai(turn));
    }

    /// `TickAsk` / `TickAuction` -- bots answer after a short delay; time-outs
    /// fall back to the default answer.
    // Indexing by prompt seat walks several parallel vectors at once.
    #[allow(clippy::needless_range_loop)]
    fn tick_live(&mut self, dt: f32) {
        let seats: Vec<usize> = match &self.pending {
            Some(p) => p.live.ask.view.seats.iter().map(|&s| s as usize).collect(),
            None => return,
        };
        let auto: Vec<bool> = seats.iter().map(|&s| self.auto_seat(s)).collect();
        let money: Vec<i32> = seats.iter().map(|&s| self.world.st.seats[s].money).collect();
        let can_pay: Vec<bool> = seats
            .iter()
            .map(|&s| {
                let x = &self.world.st.seats[s];
                !x.out() && !x.stunned() && x.exile == 0
            })
            .collect();
        let solo = self.mode == MatchMode::Solo;
        let rng = &mut self.live_rng;
        let p = self.pending.as_mut().expect("checked above");
        let l = &mut p.live;
        l.time_left -= dt;
        let before = (l.answers.clone(), l.bid);
        let done = if l.kind() == "auction" {
            for k in 0..seats.len() {
                if l.answers[k] >= 0 || l.bidder == seats[k] as i32 || !auto[k] {
                    continue;
                }
                match l.ai_at[k] {
                    None => l.ai_at[k] = Some(l.time_left - (0.6 + rng.f64() as f32 * 1.4)),
                    Some(at) if l.time_left <= at => {
                        l.ai_at[k] = None;
                        let min = if l.bid <= 0 { 100 } else { l.bid + 100 };
                        let cap = l.ask.worth[k].min(money[k]);
                        if min > cap || !can_pay[k] {
                            l.answers[k] = 1;
                        } else {
                            let bid = cap.min(min + 100 * rng.below(3) as i32);
                            l.place_bid(seats[k], bid);
                        }
                    }
                    _ => {}
                }
            }
            let open = (0..seats.len()).filter(|&k| l.answers[k] < 0 && l.bidder != seats[k] as i32).count();
            l.time_left <= 0.0 || open == 0
        } else {
            let picks = matches!(l.kind(), "pick" | "mortgage");
            let fallback = l.ask.view.fallback;
            let max = if l.kind() == "tile" { l.ask.view.items.len() as i32 } else { l.ask.view.options.len() as i32 - 1 };
            for k in 0..seats.len() {
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
            let humans_pending = (0..seats.len()).any(|k| l.answers[k] < 0 && !auto[k]);
            if l.answers.iter().all(|&a| a >= 0) {
                true
            } else if l.time_left > 0.0 || (l.time_left > -REMOTE_GRACE && !solo && humans_pending) {
                false
            } else {
                for k in 0..seats.len() {
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
        let i = self.seat_index(member).ok_or_else(|| Msg::new("err.not_in_match"))?;
        let s = &self.world.st.seats[i];
        if s.out() && m.act != "leave" {
            return Err(Msg::new(if s.bankrupt { "err.spectating" } else { "err.left_match" }));
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
                if phase != "deck" || self.world.st.seats[i].deck_ready {
                    return Err(Msg::new("err.no_deck_now"));
                }
                let ok = self
                    .data
                    .character(&self.world.st.seats[i].character)
                    .is_some_and(|c| crate::deck::is_complete(&self.data, c, &m.cards));
                if !ok {
                    return Err(Msg::new("err.deck_invalid"));
                }
                let cards = m.cards.clone();
                self.direct(|cx| cx.submit_deck(i, Some(&cards)));
                if self.world.st.seats.iter().all(|s| s.deck_ready) {
                    self.begin_play();
                }
            }
            "answer" => return self.answer(i, m),
            "vote" => return self.vote(i, m.value == 1),
            "leave" => {
                let member = self.world.st.seats[i].member;
                if phase != "play" {
                    self.member_left(member, false);
                } else if busy {
                    self.deferred.push(Deferred::Leave(i));
                    self.tick_live(0.0);
                } else {
                    self.start(Routine::Act(i, Box::new(m.clone())));
                }
            }
            "skill" => return Err(Msg::new("err.skill_not_ported")),
            "debug" => return Err(Msg::new("err.debug_not_ported")),
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
        let money = self.world.st.seats[i].money;
        let can_pay = self.with_cx(|cx| cx.can_pay(i));
        let data = self.data.clone();
        let p = self.pending.as_mut().ok_or_else(|| Msg::new("err.prompt_over"))?;
        let l = &mut p.live;
        if m.prompt != l.ask.view.id {
            return Err(Msg::new("err.prompt_over"));
        }
        let k = l.ask.view.seat_index(i as i32).ok_or_else(|| Msg::new("err.prompt_not_yours"))?;
        if l.answers[k] >= 0 {
            return Err(Msg::new(if l.kind() == "auction" { "err.auction_passed" } else { "err.answered" }));
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
                    if m.value > money {
                        return Err(Msg::new("err.poor"));
                    }
                    if !can_pay {
                        return Err(Msg::new("err.cannot_bid"));
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
                let sum: i32 = got.iter().filter_map(|t| t.parse::<usize>().ok()).map(|t| data.tiles[t].price / 2).sum();
                if sum < l.ask.view.bid {
                    return Err(Msg::new("err.mortgage_short").n("need", l.ask.view.bid).n("sum", sum));
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
        } else if self.pending.as_ref().is_some_and(|p| p.live.kind() == "auction") {
            self.tick_live(0.0);
        }
        Ok(())
    }

    // ---------------------------------------------------------------- vote

    fn voters(&self) -> Vec<usize> {
        (0..self.world.seat_count()).filter(|&p| !self.world.out(p) && !self.world.st.seats[p].ai).collect()
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
                seats: voters.iter().map(|&p| p as i32).collect(),
                answers: voters.iter().map(|&p| if p == i { 1 } else { -1 }).collect(),
                time_left: VOTE_SECONDS,
            };
            self.host_log("vote", i as i32, Msg::new("log.vote_start").seat("who", i));
        } else {
            let k = self.vote.seats.iter().position(|&s| s == i as i32).ok_or_else(|| Msg::new("err.vote_not_yours"))?;
            if self.vote.answers[k] >= 0 {
                return Err(Msg::new("err.voted"));
            }
            self.vote.answers[k] = yes as i32;
            self.host_log("vote", i as i32, Msg::new(if yes { "log.vote_yes" } else { "log.vote_no" }).seat("who", i));
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
        self.vote.time_left -= dt;
        for k in 0..self.vote.seats.len() {
            let s = self.vote.seats[k] as usize;
            if self.vote.answers[k] < 0 && (self.world.out(s) || self.auto_seat(s)) {
                self.vote.answers[k] = 1;
                self.changed = true;
            }
        }
        if self.vote.time_left <= 0.0 {
            self.end_vote(false, Msg::new("vote.timeout"));
        } else {
            self.check_vote();
        }
    }

    fn check_vote(&mut self) {
        if let Some(k) = self.vote.answers.iter().position(|&a| a == 0) {
            self.end_vote(false, Msg::new("vote.against").seat("who", self.vote.seats[k]));
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
            for i in 0..cx.w.seat_count() {
                if cx.w.st.seats[i].character.is_empty() {
                    let c = cx.random_character(true);
                    cx.w.st.seats[i].character = c;
                }
                if !cx.w.st.seats[i].deck_ready {
                    cx.submit_deck(i, None);
                }
            }
        });
        self.begin_play();
    }

    /// A member disconnected or left a room before play (`MemberLeft`): the
    /// machine takes over the seat.
    pub fn member_left(&mut self, member: i32, can_return: bool) {
        let Some(i) = self.seat_index(member) else { return };
        if self.pending.is_some() {
            self.deferred.push(Deferred::Left(i, can_return));
            self.tick_live(0.0);
        } else {
            self.apply_left(i, can_return);
        }
    }

    fn apply_left(&mut self, i: usize, can_return: bool) {
        if self.world.st.seats[i].ai || self.ended() {
            return;
        }
        let playing = self.world.st.phase == "play";
        let turn = self.world.st.turn == i as i32;
        self.direct(|cx| {
            cx.w.st.seats[i].ai = true;
            let key = match (playing, can_return) {
                (true, true) => "log.dropped_returnable",
                (true, false) => "log.dropped",
                (false, true) => "log.left_returnable",
                (false, false) => "log.left",
            };
            let text = Msg::new(key).seat("who", i);
            cx.w.log("ai", i as i32, text);
        });
        if turn {
            self.wait = CHOICE_WAIT;
        }
    }

    /// The member reconnected (`MemberBack`).
    pub fn member_back(&mut self, member: i32) {
        let Some(i) = self.seat_index(member) else { return };
        if self.pending.is_some() {
            self.deferred.retain(|d| !matches!(d, Deferred::Left(s, _) if *s == i));
            self.deferred.push(Deferred::Back(i));
        } else {
            self.apply_back(i);
        }
    }

    fn apply_back(&mut self, i: usize) {
        let s = &self.world.st.seats[i];
        if self.ended() || !s.ai || s.bot || s.out() {
            return;
        }
        self.direct(|cx| {
            cx.w.st.seats[i].ai = false;
            cx.w.log("ai", i as i32, Msg::new("log.reconnected").seat("who", i));
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
            let pos = cx.w.st.seats[i].pos.max(0) as usize;
            match m.act.as_str() {
                "roll" => {
                    let turn = cx.w.st.turn as usize;
                    cx.main_move(turn, i)
                }
                "buy" => {
                    cx.w.st.bought = true;
                    cx.buy(i, pos)
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
                "discard" => {
                    cx.discard(i, &m.card);
                    Ok(())
                }
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
    let pos = st.seats[i].pos;
    let moved_off = |key: &str| (m.value > 0 && m.value != pos).then(|| Msg::new(key).tile("was", m.value).tile("now", pos));
    match m.act.as_str() {
        "roll" => {
            if !cx.playing() || st.step != 1 || busy {
                return Some(Msg::new("err.no_roll_now"));
            }
            if st.skip_move && st.turn == i as i32 {
                return Some(Msg::new("err.roll_stay"));
            }
            if st.roller != i as i32 {
                if st.turn != i as i32 || st.roller < 0 {
                    return Some(Msg::new("err.not_your_roll"));
                }
                return Some(Msg::new("err.roller_other").seat("who", st.roller));
            }
            None
        }
        "buy" => {
            if !my_turn || st.step != 3 || busy {
                return Some(Msg::new("err.no_buy_now"));
            }
            if let Some(e) = moved_off("err.moved_off_buy") {
                return Some(e);
            }
            if !cx.buyable_here(i) {
                return Some(Msg::new("err.cannot_buy"));
            }
            if st.seats[i].money < cx.buy_price(pos as usize) {
                return Some(Msg::new("err.buy_poor"));
            }
            None
        }
        "build" => {
            if !my_turn || st.step != 3 || busy {
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
            if st.seats[i].money < cx.build_cost(pos as usize) {
                return Some(Msg::new("err.poor"));
            }
            None
        }
        "mortgage" => cx.why_not_mortgage(i, m.value, busy),
        "redeem" => cx.why_not_redeem(i, m.value, busy),
        "play" => cx.why_not_play(i, &m.card, busy),
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
            if st.step == 1 && !st.skip_move {
                return Some(Msg::new("err.roll_first"));
            }
            if st.step == 2 {
                return Some(Msg::new("err.moving"));
            }
            if cx.over_hand(i) {
                return Some(Msg::new("err.over_hand").i("limit", world::HAND_LIMIT as i64));
            }
            None
        }
        _ => None,
    }
}
