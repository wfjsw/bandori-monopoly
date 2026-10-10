//! Routine context, flow control, and prompts.
//!
//! A routine is ordinary straight-line code over a [`Cx`]. When it needs a player
//! decision it calls [`Cx::ask`]. If the answer is already in this routine's answer
//! log, `ask` returns it; otherwise it halts the routine with [`Halt::Ask`]. The host
//! then shows the prompt, collects the answer, and re-runs the routine from the same
//! snapshot with the longer log -- deterministic replay instead of coroutines.
//!
//! **Simulation mode** ([`AnswerProvider`]): with a provider installed and the
//! replay log exhausted, `ask` calls the provider and returns the answer at
//! once instead of halting -- one forward pass per routine, no replay
//! amplification (`docs/BOT.md` §3.2). The live match keeps the halt/replay
//! model; the default (no provider) path is byte-identical.

use serde::{Deserialize, Serialize};

use crate::data::{GameData, TileData};
use crate::msg::Msg;
use crate::state::MatchPrompt;

use super::rules::CardRules;
use super::world::World;

/// Why a routine stopped before finishing. Opaque to card rules: they only pass it
/// on with `?`.
#[derive(Debug)]
pub struct Halt(pub HaltKind);

#[derive(Debug)]
pub enum HaltKind {
    /// Needs a player decision. The partial world is shown; nothing is committed.
    Ask(Box<Ask>),
    /// The match ended mid-routine. Commit what happened so far.
    Ended,
    /// A nested host routine (a `[触发结算]` a card triggered) must run before
    /// this call returns. The caller's continuation is on
    /// [`Cx::work_stack`](super::work::WorkItem); the outermost
    /// [`Cx::drain_work`] pump runs the nested work then the resume.
    /// STACK-01: this is how the settle cycle stays on the heap, not the Rust stack.
    Suspended,
}

impl Halt {
    pub(crate) fn ended() -> Self {
        Halt(HaltKind::Ended)
    }

    /// A nested settle (or other work-stack job) must run before we continue.
    pub fn suspended() -> Self {
        Halt(HaltKind::Suspended)
    }

    /// Is this a work-stack suspension (not a prompt / match end)?
    pub fn is_suspended(&self) -> bool {
        matches!(self.0, HaltKind::Suspended)
    }
}

pub type Flow<T> = Result<T, Halt>;

/// Sentinel in [`Ask::ai`] for "no precomputed answer -- fill it in when the
/// prompt is raised, from the seat's [`crate::state::BotMentality`]". Never
/// survives into a saved [`Ask`]: [`Cx::ask`] replaces every entry.
pub const AI_UNSET: i32 = i32::MIN;

/// A prompt plus everything the host needs to answer it without the routine:
/// per-player AI answers (computed when the prompt was raised) and auction valuations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    pub view: MatchPrompt,
    /// AI answer for each entry in `view.players`.
    pub ai: Vec<i32>,
    /// AI card/deed selection for `pick` / `mortgage` prompts, per player.
    pub ai_picked: Vec<Vec<String>>,
    /// Auction: most each player's AI is willing to bid.
    pub worth: Vec<i32>,
}

impl Ask {
    fn new(kind: &str, players: Vec<usize>, title: Msg, text: Msg, time: f32) -> Self {
        let n = players.len();
        Self {
            view: MatchPrompt {
                kind: kind.into(),
                title,
                text,
                players: players.iter().map(|&s| s as i32).collect(),
                answers: vec![-1; n],
                time_left: time,
                ..MatchPrompt::default()
            },
            ai: vec![AI_UNSET; n],
            ai_picked: vec![vec![]; n],
            worth: vec![0; n],
        }
    }

    /// `Choice`: pick one of `options`. The AI answer is left [`AI_UNSET`];
    /// [`Cx::ask`] fills it per seat (standard: the fallback; chaos: a random
    /// non-default option) unless [`Self::with_ai`] overrides it.
    pub fn choice(
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        options: Vec<Msg>,
        fallback: i32,
        time: f32,
    ) -> Self {
        let mut a = Self::new("choice", players, title, text, time);
        a.view.options = options;
        a.view.fallback = fallback;
        a
    }

    /// `TileAsk`: pick one of `tiles` (answer == len means "none"). Same AI
    /// fill as [`Self::choice`]; chaos never takes "none" while a tile exists.
    pub fn tile(
        player_id: usize,
        title: Msg,
        text: Msg,
        tiles: &[usize],
        labels: Vec<Msg>,
    ) -> Self {
        let mut a = Self::new("tile", vec![player_id], title, text, 20.0);
        a.view.items = tiles.iter().map(|t| t.to_string()).collect();
        a.view.options = labels;
        a.view.fallback = tiles.len() as i32;
        a
    }

    /// `MortgageAsk`: choose deeds worth at least `need`.
    /// An optional mortgage supplies a cancel label in `view.options`; answer
    /// value 1 cancels it, while value 0 submits the selected deeds as usual.
    pub fn mortgage(
        player_id: usize,
        need: i32,
        text: Msg,
        deeds: &[usize],
        ai_pick: Vec<String>,
    ) -> Self {
        let mut a = Self::new(
            "mortgage",
            vec![player_id],
            Msg::new("ask.mortgage.title"),
            text,
            25.0,
        );
        a.view.items = deeds.iter().map(|t| t.to_string()).collect();
        a.view.bid = need;
        // The selection is `ai_picked` (already mentality-aware); `ai` is unused.
        a.ai = vec![0];
        a.ai_picked = vec![ai_pick];
        a
    }

    /// `Auction`: open bidding on a tile.
    pub fn auction(
        tile: usize,
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        worth: Vec<i32>,
    ) -> Self {
        let n = players.len();
        let mut a = Self::new("auction", players, title, text, 10.0);
        a.view.tile = tile as i32;
        a.view.bid = 0;
        a.view.bidder = -1;
        // Bidding reads `worth` (already mentality-aware); `ai` is unused.
        a.ai = vec![0; n];
        a.worth = worth;
        a
    }

    pub fn with_ai(mut self, ai: impl Fn(usize) -> i32) -> Self {
        self.ai = self.view.players.iter().map(|&s| ai(s as usize)).collect();
        self
    }

    pub fn with_tile(mut self, t: usize) -> Self {
        self.view.tile = t as i32;
        self
    }

    pub fn with_kind(mut self, kind: &str) -> Self {
        self.view.kind = kind.into();
        self
    }
}

/// A completed prompt, as stored in the answer log.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Answered {
    /// Parallel to the prompt's players; never negative once complete.
    pub answers: Vec<i32>,
    pub picked: Vec<String>,
    pub bid: i32,
    pub bidder: i32,
}

/// What a routine gets back from [`Cx::ask`].
#[derive(Debug, Clone)]
pub struct Reply {
    pub players: Vec<i32>,
    pub fallback: i32,
    pub a: Answered,
}

/// The card id a prompt option declares (`Arg::Card` on the option's label).
/// Used by the [反击] offer, whose options are `ask.counteract.play` messages
/// naming the card each would declare.
fn option_card(m: &Msg) -> Option<String> {
    m.a.get("card").and_then(|a| match a {
        crate::msg::Arg::Card(id) => Some(id.clone()),
        _ => None,
    })
}

impl Reply {
    /// This seat's recorded answer, or the prompt's fallback.
    pub fn of(&self, player_id: usize) -> i32 {
        match self.players.iter().position(|&s| s == player_id as i32) {
            Some(i) if self.a.answers.get(i).is_some_and(|&v| v >= 0) => self.a.answers[i],
            _ => self.fallback,
        }
    }
}

/// Simulation-mode answer source (`docs/BOT.md` §3.2). Installed on a [`Cx`];
/// every [`Cx::ask`] whose replay-log entry is missing calls it instead of
/// halting the routine.
///
/// The live match never installs one (the halt/replay model is what recordings
/// and the server protocol ride). A simulation installs the ISMCTS tree policy
/// during descent and the rollout policy afterwards; both answer inline, so a
/// routine runs once per simulation instead of once per prompt.
pub trait AnswerProvider {
    /// Answer `ask` inline. `ask.ai` / `ask.ai_picked` / `ask.worth` are
    /// already filled in ([`Cx::fill_ai`]), so the default answer is the seat
    /// heuristic -- exactly what the engine's bot schedule would have applied.
    ///
    /// Returning `None` keeps the default halt path (the host shows the prompt
    /// and the routine replays with the answer appended). Use it for the
    /// handful of prompts the tree wants to branch on; everything else should
    /// be answered inline.
    fn answer(&mut self, ask: &Ask) -> Option<Answered>;
}

/// A provider that always answers with the seat heuristic already on the
/// [`Ask`] (`ai` / `ai_picked` / `worth`, i.e. what `tick_live` would apply on
/// the bot schedule). The rollout policy; also the right default for every
/// seat the tree does not abstract.
///
/// `params` (indexed by player index) carries the seats' resolved
/// [`crate::strategy::StrategyParams`] (`docs/BOT.md` §3.8) so the one-shot
/// auction solver uses each seat's bid step; empty = every seat plays the
/// default parameters, which is the old behaviour exactly.
#[derive(Default, Clone)]
pub struct HeuristicProvider {
    pub params: Vec<crate::strategy::StrategyParams>,
}

impl HeuristicProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_params(params: Vec<crate::strategy::StrategyParams>) -> Self {
        Self { params }
    }

    /// Resolved params for player index `seat` (defaults when absent).
    pub fn params_of(&self, seat: usize) -> crate::strategy::StrategyParams {
        self.params.get(seat).cloned().unwrap_or_default()
    }
}

impl AnswerProvider for HeuristicProvider {
    fn answer(&mut self, ask: &Ask) -> Option<Answered> {
        let p = self.params.clone();
        Some(heuristic_answer_with(ask, &move |seat: usize| {
            p.get(seat).cloned().unwrap_or_default()
        }))
    }
}

/// The answer the engine's bot schedule would land on for `ask` -- the
/// standard seat's `tick_live` fill, with the auction's bidding loop resolved
/// in one shot (each seat nudges the minimum once; the last bidder wins).
/// Every seat plays the default [`crate::strategy::StrategyParams`].
pub fn heuristic_answer(ask: &Ask) -> Answered {
    heuristic_answer_with(ask, &|_seat| crate::strategy::StrategyParams::default())
}

/// [`heuristic_answer`] with a per-seat parameter lookup (player index →
/// [`crate::strategy::StrategyParams`]). Only the auction solver reads it
/// (the raise step); the other prompts just echo the precomputed `ai` fill.
pub fn heuristic_answer_with(
    ask: &Ask,
    params: &dyn Fn(usize) -> crate::strategy::StrategyParams,
) -> Answered {
    let players = &ask.view.players;
    let n = players.len();
    if ask.view.kind == "auction" {
        return heuristic_auction(ask, params);
    }
    let picks = matches!(ask.view.kind.as_str(), "pick" | "mortgage");
    let max = if ask.view.kind == "tile" {
        ask.view.items.len() as i32
    } else {
        ask.view.options.len() as i32 - 1
    };
    let mut answers = Vec::with_capacity(n);
    for k in 0..n {
        if picks {
            answers.push(0);
        } else {
            let v = ask.ai.get(k).copied().unwrap_or(AI_UNSET);
            answers.push(if v == AI_UNSET {
                ask.view.fallback
            } else {
                v.clamp(0, max.max(0))
            });
        }
    }
    let picked = ask
        .ai_picked
        .iter()
        .take(n)
        .find(|p| !p.is_empty())
        .cloned()
        .unwrap_or_default();
    Answered {
        answers,
        picked,
        bid: 0,
        bidder: -1,
    }
}

/// One-shot the auction's bid loop: each seat still in raises the minimum once
/// (the standard bot's `min + 100` middle of its `+0..=+200` nudge) while that
/// stays under its ceiling, and passes otherwise. The last bidder wins at the
/// bid it made -- the shape `auction_tile` reads (`r.a.bidder` / `r.a.bid`).
fn heuristic_auction(
    ask: &Ask,
    params: &dyn Fn(usize) -> crate::strategy::StrategyParams,
) -> Answered {
    let players = &ask.view.players;
    let n = players.len();
    let mut answers = vec![0i32; n];
    let mut bid = 0i32;
    let mut bidder = -1i32;
    let mut changed = true;
    while changed {
        changed = false;
        for k in 0..n {
            if answers[k] != 0 || bidder == players[k] {
                continue;
            }
            let p = params(players[k] as usize);
            let min = p.bid_min(bid);
            let cap = ask.worth.get(k).copied().unwrap_or(0);
            if min > cap {
                answers[k] = 1;
            } else {
                bid = cap.min(min);
                bidder = players[k];
            }
            changed = true;
        }
    }
    Answered {
        answers,
        picked: vec![],
        bid,
        bidder,
    }
}

/// Routine context: the world being mutated, read-only game data, card rules, and
/// the replay cursor.
pub struct Cx<'a> {
    pub(crate) w: crate::engine::world::SharedWorld,
    pub(crate) data: &'a GameData,
    pub(crate) rules: &'a dyn CardRules,
    answers: &'a [Answered],
    cursor: usize,
    /// Simulation-mode answers (see [`AnswerProvider`]). `None` = halt/replay.
    provider: Option<&'a mut dyn AnswerProvider>,
    /// Seconds of presentation time requested since the last answered prompt; the
    /// host waits this long before the next automatic step.
    pub(crate) delay: f32,
    /// How deep the `money()` pipeline is nested right now. A card hook that
    /// forces another money movement from inside a before/after money event
    /// re-enters `money()`; this bounds that, the way `MAX_COUNTERACT_DEPTH`
    /// bounds the chain. Transient (not serialized).
    pub(crate) money_depth: u32,
    /// Card uids whose hooks are currently running and must not re-trigger on
    /// their own movement (the termination argument for nested money). Transient.
    pub reentrant_hooks: Vec<i32>,
    /// `"card"` activation event ids currently open, innermost last. A nested
    /// drive groups under the top of this stack; `-1` at the bottom.
    pub activation: Vec<i32>,
    /// The 移动起点 of the move currently being walked / teleported, or `-1`
    /// when no move is in flight. 「[经过]CiRCLE且[移动起点]不为CiRCLE」 reads it
    /// (`circle_reward`). Transient (not serialized) -- the walk sets it as it
    /// starts and the reward step is the only reader.
    pub(crate) move_start: i32,
    /// Guest state writes overlaid on the live world for a host routine's
    /// duration (see [`Self::overlay_guest_state`]). A stack: nested drives push
    /// and pop their own. Transient (not serialized).
    pub(crate) guest_overlays: Vec<GuestOverlay>,
    /// Play-gate verdicts for the current world stamp (fix A,
    /// `docs/BOT.md` "engine efficiency A–C"). `rules.cant_play` is a pure
    /// query of the world; within one unchanged world state the same
    /// `(player, card)` always answers the same, so a scan of the hand (or a
    /// second look at the same card) reuses the first verdict. Dropped the
    /// moment [`Self::w`] is written (its stamp changes).
    play_memo: std::cell::RefCell<PlayMemo>,
    /// Explicit work stack for nested host routines (STACK-01). See
    /// [`crate::engine::work`]. Transient (not serialized).
    pub(crate) work_stack: Vec<Box<dyn crate::engine::work::WorkItem>>,
    /// True while [`Self::drain_work`] is pumping. A nested `card_settle_at`
    /// suspends instead of recursing.
    pub(crate) draining_work: bool,
    /// Index in [`Self::work_stack`] where the current suspension group starts.
    /// A caller that has work after a raise inserts its continuation **here**
    /// (so it runs after the nested settle and the drive resume).
    pub(crate) suspend_base: usize,
    /// Saved card-rules counteract continuation (STACK-01). When a raise
    /// suspends, the remaining drive jobs and the half-updated trigger live
    /// here; the re-entered `CardRules::counteract` adopts them instead of
    /// re-collecting from a world the nested settle already changed.
    pub(crate) counteract_resume: Option<Box<dyn std::any::Any + Send>>,
    /// Dest a just-resumed drive returned (STACK-01). The card-fate work
    /// item reads it after the resume completes.
    pub(crate) last_drive_dest: Option<i32>,
    /// Nested-settle depth (STACK-01 rulebook guard [`MAX_SETTLE_DEPTH`]).
    pub(crate) settle_depth: u32,
    /// STACK-01: skip this many `drive` calls on the next counteract
    /// walk (they already completed before a nested-settle suspend).
    pub(crate) drive_skip: u32,
    /// Settles started in the current `drain_work` pump (STACK-01
    /// rulebook cap [`MAX_SETTLE_DEPTH`]).
    pub(crate) settle_ops: u32,
}

/// The play-gate memo: verdicts keyed by `(player, card id)`, valid only for
/// the [`crate::engine::world::SharedWorld`] stamp they were computed under.
/// A small linear map (a hand scan has few unique ids) -- no hashing on the
/// hot path.
#[derive(Default)]
struct PlayMemo {
    stamp: u64,
    entries: Vec<((usize, String), Option<Msg>)>,
}

impl PlayMemo {
    fn clear(&mut self) {
        self.entries.clear();
    }
    fn get(&self, player: usize, card: &str) -> Option<&Option<Msg>> {
        self.entries
            .iter()
            .find(|((p, c), _)| *p == player && c == card)
            .map(|(_, v)| v)
    }
    fn insert(&mut self, player: usize, card: &str, why: Option<Msg>) {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|((p, c), _)| *p == player && c == card)
        {
            slot.1 = why;
            return;
        }
        // Cap the memo: a scan that runs away must not grow without bound
        // (the stamp resets it on the next write anyway).
        if self.entries.len() >= 64 {
            self.entries.clear();
        }
        self.entries.push(((player, card.to_string()), why));
    }
}

/// The guest's pending player-state **clears**, overlaid on the live world
/// while a host routine runs, and **not persisted**:
/// [`Cx::restore_guests_to`] drops the overlay, keeping only the routine's own
/// effects, and the replay re-applies the guest writes deterministically from
/// the top of the body.
///
/// This is the merged view for 「…时」 a status the guest just cleared (e.g.
/// 壱雫空 zeroing [晕眩] and then paying) must be visible to `can_pay` without
/// the clear landing twice -- the body replays from the top, so a *persisted*
/// clear would be re-applied on top of itself and an additive write would
/// double-count. Only decreases cross; see [`Cx::overlay_guest_state`].
#[derive(Debug, Default)]
pub struct GuestOverlay {
    /// `(player, key, base, wrote)` for each state item the guest decreased.
    /// `base` is `None` when the key was absent before the overlay.
    state: Vec<(usize, String, Option<crate::state::StateVar>, crate::state::StateVar)>,
}

impl<'a> Cx<'a> {
    /// Press a skill button: run the rule's `On::Play` entry under its own id.
    ///
    /// A skill is a card rule -- it just lives on the player's field rather than
    /// in a hand -- so the card's play entry answers for it. Nothing moves: there
    /// is no hand card to spend and no destination to resolve, unlike playing a
    /// card. The gate is not re-checked here; `why_not_act` has already asked the
    /// rule's `cant_play` and refused the press if it named a reason, which is
    /// the same shape as every other turn action.
    /// A copy of the replayable world (a rules host runs against one).
    pub fn world_copy(&self) -> World {
        #[cfg(feature = "bot-cost")]
        crate::engine::bot_cost::WORLD_CLONES
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.w.deep_clone()
    }

    /// Immutable view of the live world -- **no clone**. Read-only queries
    /// (`out`, `hand_of`, the counteract pre-filter's window snapshot) go
    /// through this instead of [`Self::world_copy`].
    pub fn world(&self) -> &World {
        &self.w
    }

    /// A shareable handle to the live world -- a refcount bump, **no copy**
    /// (fix B). A pure check (`cant_play` probe, guard body, buy quote) runs
    /// against it; the first write inside the check's sandbox detaches a
    /// private copy (`Arc::make_mut`) and the live world is untouched. Prefer
    /// this over [`Self::world_copy`] whenever the consumer only reads, or
    /// writes into a throwaway sandbox.
    pub fn share_world(&self) -> crate::engine::world::SharedWorld {
        #[cfg(feature = "bot-cost")]
        crate::engine::bot_cost::WORLD_SHARES
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.w.share()
    }

    /// Publish every pending walk segment (see [`crate::engine::world::World::flush_walk`]).
    /// A guest-side log's re-insert mark must sit **after** the approach it
    /// interrupted, so the drive flushes before reading [`Self::live_event_count`].
    pub fn flush_walk(&mut self) {
        self.w.flush_walk();
    }

    /// How many events the live tail holds right now (the re-insert mark).
    pub fn live_event_count(&self) -> usize {
        self.w.recent.len()
    }

    /// Post one event onto the **live** stream now (write-through). Flushes a
    /// pending walk first so the approach precedes the line. Stamps `parent`
    /// (`-1` = top-level) and an optional `card` tag (the `"effect"` ride).
    /// Returns the event id.
    pub fn log_event(
        &mut self,
        kind: &str,
        player_id: i32,
        msg: Msg,
        parent: i32,
        card: &str,
        value: i32,
    ) -> i32 {
        self.w.flush_walk();
        let e = self.w.log(kind, player_id, msg);
        e.parent = parent;
        if !card.is_empty() {
            e.card = card.to_string();
        }
        if value != 0 {
            e.value = value;
        }
        e.id
    }

    /// Post a `"card"` activation onto the **live** stream (write-through).
    /// See [`World::card_activation`]. Returns the event id.
    pub fn log_card_activation(
        &mut self,
        kind: &str,
        owner: i32,
        card: &str,
        target: i32,
        tile: i32,
        negated: bool,
        msg: Msg,
        parent: i32,
    ) -> i32 {
        self.w.flush_walk();
        let e = self.w.card_activation(kind, owner, card, target, tile, negated, msg);
        e.parent = parent;
        e.id
    }

    /// Replace the message of an already-posted event (a re-run's correction).
    /// `false` when the id is gone (truncated tail) -- the caller then appends.
    pub fn relog_event(&mut self, id: i32, msg: Msg) -> bool {
        self.w.recent.replace_msg(id, msg)
    }

    /// Set `value` on already-posted events (a `"dice"` face, a `"fire"` spend).
    pub fn replace_event_values(&mut self, pairs: &[(i32, i32)]) {
        for &(id, value) in pairs {
            self.w.recent.set_value(id, value);
        }
    }

    /// Drop events by id (a halted drive's write-through lines). Rebuilds the
    /// tail without them.
    pub fn drop_events(&mut self, ids: &[i32]) {
        self.w.recent.drop_ids(ids);
    }

    /// Stamp `parent` on live events from index `since` onward (the host-effect
    /// events a drive just caused, grouped under its activation).
    pub fn stamp_parent_since(&mut self, since: usize, parent: i32) {
        self.w.recent.stamp_parent_since(since, parent);
    }

    /// Attach outcome lines to a `"card"` activation (and drop a negated
    /// activation's would-be results). No-op when the id is gone.
    pub fn set_event_results(&mut self, id: i32, results: Vec<Msg>) {
        let mut evs: Vec<crate::state::MatchEvent> = self.w.recent.iter().cloned().collect();
        let Some(e) = evs.iter_mut().find(|e| e.id == id) else {
            return;
        };
        e.results = results;
        let mut tail = crate::engine::world::EventTail::default();
        for e in evs {
            tail.push_back(e);
        }
        self.w.recent = tail;
    }

    /// How many events carry `parent` (excluding the activation itself).
    pub fn child_event_count(&self, parent: i32, self_id: i32) -> usize {
        self.w
            .recent
            .iter()
            .filter(|e| e.parent == parent && e.id != self_id)
            .count()
    }

    /// The stamp of the live world right now (changes on every write).
    pub fn world_stamp(&self) -> u64 {
        self.w.stamp()
    }

    /// Swap in the world a rules host produced; returns the previous one.
    pub fn swap_world(&mut self, w: World) -> World {
        self.play_memo.borrow_mut().clear();
        std::mem::replace(&mut self.w, crate::engine::world::SharedWorld::new(w)).into_inner()
    }

    /// Swap in a shared handle (the commit half of a drive -- see
    /// [`Self::swap_world`]). Zero-copy when the handle is the only one left.
    pub fn swap_shared(
        &mut self,
        w: crate::engine::world::SharedWorld,
    ) -> crate::engine::world::SharedWorld {
        self.play_memo.borrow_mut().clear();
        std::mem::replace(&mut self.w, w)
    }

    /// Unwrap the live world out of this context (the host adopts it).
    pub fn into_world(self) -> World {
        self.w.into_inner()
    }

    /// Swap the live world with `other` in place -- zero-copy, for the
    /// card-rules inline host (`docs/BOT.md` §3.2) which runs a host request
    /// against the guest's own world copy and hands it back afterwards.
    pub fn swap_world_ref(&mut self, other: &mut World) {
        self.play_memo.borrow_mut().clear();
        std::mem::swap(&mut *self.w, other);
    }

    /// Adopt the turn-ctx **policy** another world set up (build/buy discounts,
    /// free buy, fixed roll, ...) without taking its progress counters. A host
    /// routine runs against the live world and is not replayed, so a card's
    /// `set_build_discount` has to cross before `card_build` reads it; the
    /// replay re-derives the progress counters from the drive's snapshot.
    pub fn adopt_turn_policy(&mut self, from: &World) {
        let f = &from.turn;
        let t = &mut self.w.turn;
        t.build_discount = f.build_discount;
        t.build_discount_layers = f.build_discount_layers;
        t.build_cost_pct = f.build_cost_pct;
        // `linger` must cross `adopt_turn_policy` (`docs/PURCHASE.md`).
        if !f.lingering.is_empty() {
            for l in &f.lingering {
                if !t.lingering.iter().any(|x| x == l) {
                    t.lingering.push(l.clone());
                }
            }
        }
        t.fixed_roll = f.fixed_roll;
        t.extreme = f.extreme;
        t.play_from_hand = f.play_from_hand;
        t.no_money_loss = f.no_money_loss.clone();
        // The movement plan a card shaped just before pausing for a host
        // routine (`plan::set_pay_factor` / `set_rent_factor` / `set_can_build`
        // ) lives on the card's world copy; the routine
        // runs against the live world, so the knobs have to cross too --
        // otherwise 练习室里的风暴's (4-X)/4 and Repaint's 「支付减半」 are
        // dropped at the `SettleAt` / `Move` boundary.
        t.plan.pay_factor = f.plan.pay_factor;
        t.plan.rent_factor = f.plan.rent_factor;
        t.plan.can_build = f.plan.can_build;
        t.plan.no_buy = f.plan.no_buy;
        // A card body may latch a decision across the pauses its host routines
        // cause -- e.g. 黑色生日 freezing 「资金在1000以下」 at resolution so the
        // replay's re-run takes the same branch across its two [支付] entries.
        // That write lives on the card's world copy and is dropped when the run
        // pauses. Carry it over -- but *only* for keys a body explicitly
        // registers under the `latch.` prefix. (A latch is "already taken", so
        // the replay's re-write is a no-op and nothing double-counts.)
        for (to, from_p) in self.w.st.players.iter_mut().zip(from.st.players.iter()) {
            for (k, v) in &from_p.state {
                if !k.starts_with("latch:") {
                    continue;
                }
                if !to.state.contains_key(k) {
                    to.state.insert(k.clone(), v.clone());
                }
            }
        }
    }

    /// Overlay the guest's pending **player state** (status keys + money) on
    /// the live world for a host routine's duration, and push the saved base on
    /// [`Self::guest_overlays`]. See [`GuestOverlay`].
    ///
    /// Called at the `NeedHost` boundary: the routine (`pay` / `gain` / `move` /
    /// ...) runs against the live world, so a status the guest just **cleared**
    /// (e.g. 壱雫空 zeroing [晕眩] and then paying) has to be visible to
    /// `can_pay` -- without being *persisted*, because the body replays from
    /// the top and would re-apply the write (double-counting an additive one).
    /// [`Self::restore_guests_to`] drops it again, keeping only the routine's
    /// own effects; the replay then re-applies the guest writes.
    ///
    /// Only **decreases** cross (「清除」). A status the body itself just
    /// *applied* (心の雨's fallback `give_stun` then `gain 1000`) must not gate
    /// its own follow-on money -- 「无法收付款」 blocking a card's own gain to a
    /// player it just stunned is an open ruling, and the pre-overlay behaviour
    /// stands until it is decided. Money is not overlaid at all: a `gain_fixed`
    /// is re-applied on the replay and the pipeline's own moves are routine
    /// effects, not guest writes.
    pub fn overlay_guest_state(&mut self, from: &World) {
        let mut o = GuestOverlay::default();
        let n = self.w.st.players.len().min(from.st.players.len());
        for i in 0..n {
            // The guest map's keys only: a key the guest does not carry was not
            // written this iteration (the guest's copy came from this world).
            let keys: Vec<String> = from.st.players[i].state.keys().cloned().collect();
            for k in keys {
                let wrote = from.st.players[i].state[&k];
                let base = self.w.st.players[i].state.get(&k).copied();
                if base == Some(wrote) {
                    continue;
                }
                // Decrease only (a clear). `base` absent reads as 0, so a key
                // the guest created is never a decrease.
                let base_v = base.map(|b| b.value).unwrap_or(0);
                if wrote.value >= base_v {
                    continue;
                }
                self.w.st.players[i].state.insert(k.clone(), wrote);
                o.state.push((i, k, base, wrote));
            }
        }
        self.guest_overlays.push(o);
    }

    /// How many guest overlays are currently on the stack. A drive records
    /// this at entry and calls [`Self::restore_guests_to`] with it, so nested
    /// drives never pop an outer one.
    pub fn guest_overlay_depth(&self) -> usize {
        self.guest_overlays.len()
    }

    /// Drop every [`GuestOverlay`] this drive pushed (down to `base`), keeping
    /// only the host routine's own effects.
    ///
    /// * a key the routine left alone goes back to its pre-overlay value (the
    ///   replay re-applies the guest write on top);
    /// * a key the routine also touched keeps the routine's **delta** on the
    ///   base value, so the replay's re-application composes rather than
    ///   double-counts.
    pub fn restore_guests_to(&mut self, base: usize) {
        while self.guest_overlays.len() > base {
            let o = self.guest_overlays.pop().expect("len > base");
            for (i, k, base_v, wrote_v) in o.state {
                let now = self
                    .w
                    .st
                    .players
                    .get(i)
                    .and_then(|p| p.state.get(&k).copied())
                    .unwrap_or_default();
                if now == wrote_v {
                    // The routine left it: restore the base.
                    if let Some(p) = self.w.st.players.get_mut(i) {
                        match base_v {
                            Some(v) => {
                                p.state.insert(k, v);
                            }
                            None => {
                                p.state.remove(&k);
                            }
                        }
                    }
                } else if let Some(p) = self.w.st.players.get_mut(i) {
                    // The routine changed it too: keep only its delta.
                    let mut v = now;
                    v.value = base_v.map(|b| b.value).unwrap_or(0) + (now.value - wrote_v.value);
                    p.state.insert(k, v);
                }
            }
        }
    }

    /// The shared game data (tile/card lookups).
    pub fn game_data(&self) -> &'a GameData {
        self.data
    }

    /// The rules in force (a nested run may call back into them).
    pub fn rules(&self) -> &'a dyn CardRules {
        self.rules
    }

    pub fn new(
        w: World,
        data: &'a GameData,
        rules: &'a dyn CardRules,
        answers: &'a [Answered],
    ) -> Self {
        Self {
            w: crate::engine::world::SharedWorld::new(w),
            data,
            rules,
            answers,
            cursor: 0,
            provider: None,
            delay: 0.0,
            money_depth: 0,
            reentrant_hooks: Vec::new(),
            activation: Vec::new(),
            move_start: -1,
            guest_overlays: Vec::new(),
            play_memo: std::cell::RefCell::new(PlayMemo::default()),
            work_stack: Vec::new(),
            draining_work: false,
            suspend_base: 0,
            counteract_resume: None,
            last_drive_dest: None,
            settle_depth: 0,
            drive_skip: 0,
            settle_ops: 0,
        }
    }

    /// `CardRules::cant_play` with the per-world memo (fix A). The rule's
    /// play gate is a pure query of the world; within one unchanged world
    /// stamp the same `(player, card)` always answers the same, so a hand scan
    /// (or a second look at the same card in one step) reuses the first
    /// verdict instead of re-running the gate body.
    ///
    /// This is the **rules** gate only -- the engine's own cheap gates (hand
    /// membership, phase, `CannotPlay`, exclusivity, `normal`) run every time
    /// in [`Self::cant_play`] and are pure field reads.
    pub fn rules_cant_play(&self, player: usize, card: &str) -> Option<Msg> {
        #[cfg(feature = "bot-cost")]
        use std::sync::atomic::Ordering::Relaxed;
        let stamp = self.w.stamp();
        {
            let mut memo = self.play_memo.borrow_mut();
            if memo.stamp != stamp {
                memo.clear();
                memo.stamp = stamp;
            }
            if let Some(v) = memo.get(player, card) {
                #[cfg(feature = "bot-cost")]
                crate::engine::bot_cost::CANT_PLAY_MEMO_HITS.fetch_add(1, Relaxed);
                return v.clone();
            }
        }
        #[cfg(feature = "bot-cost")]
        crate::engine::bot_cost::CANT_PLAY_CALLS.fetch_add(1, Relaxed);
        let why = self.rules.cant_play(self, player, card);
        self.play_memo.borrow_mut().insert(player, card, why.clone());
        why
    }

    /// Simulation mode: answer every prompt through `p` instead of halting
    /// (`docs/BOT.md` §3.2). The default path (no provider) is unchanged.
    pub fn with_provider(mut self, p: &'a mut dyn AnswerProvider) -> Self {
        self.provider = Some(p);
        self
    }

    /// Is a provider installed? The card-rules bridge only runs its inline
    /// ask / host-request path when this is true, so the live match's
    /// halt/replay is byte-identical.
    pub fn has_provider(&self) -> bool {
        self.provider.is_some()
    }

    /// Raise a prompt. Returns the logged answer, or halts the routine.
    ///
    /// Every [`AI_UNSET`] entry in `ask.ai` is filled here, from the seat's
    /// [`crate::state::BotMentality`] (see [`Self::fill_ai`]), before the
    /// prompt is shown -- so a saved `Ask` always carries concrete answers and
    /// the host's bot schedule (`tick_live`) just reads them. The fill runs on
    /// the replay too (the ask is rebuilt from the same snapshot), so the world
    /// RNG advances identically on both passes.
    ///
    /// Order: the replay log first (a re-run must reproduce the recorded
    /// answers exactly), then the [`AnswerProvider`] if one is installed, and
    /// only then the halt. A provider that returns `None` falls through to the
    /// halt, so the tree can still branch on the prompts it cares about.
    pub fn ask(&mut self, mut ask: Ask) -> Flow<Reply> {
        self.w.ask_seq += 1;
        ask.view.id = self.w.ask_seq;
        self.fill_ai(&mut ask);
        if let Some(a) = self.answers.get(self.cursor) {
            self.cursor += 1;
            self.delay = 0.0;
            self.w.take_walk_delay();
            return Ok(Reply {
                players: ask.view.players,
                fallback: ask.view.fallback,
                a: a.clone(),
            });
        }
        if let Some(p) = self.provider.as_mut() {
            if let Some(a) = p.answer(&ask) {
                self.delay = 0.0;
                self.w.take_walk_delay();
                return Ok(Reply {
                    players: ask.view.players,
                    fallback: ask.view.fallback,
                    a,
                });
            }
        }
        // Halt for player input: flush any pending walk segment first, so the
        // client has animated the approach to this point before the prompt
        // shows (a [反击] window mid-walk, a CiRCLE reward, …).
        self.w.flush_walk();
        self.w.st.prompt = ask.view.clone();
        Err(Halt(HaltKind::Ask(Box::new(ask))))
    }

    /// Replace every [`AI_UNSET`] entry of `ask.ai` with the answer this seat's
    /// policy would give. Standard (and every human seat, including one taken
    /// over after a time-out) takes the prompt's fallback. Chaos picks uniformly
    /// among the non-default options -- never the "none" / "skip" / "do nothing"
    /// while an alternative exists -- and a tile prompt gets a random target
    /// rather than "none".
    ///
    /// A [反击] offer is the one exception -- and for both policies it is a
    /// real decision, not a pass (user ruling 2026-10-08, "bots must be able
    /// to counteract"): chaos declares on
    /// [`CHAOS_COUNTER_CHANCE`](super::CHAOS_COUNTER_CHANCE) of the offers it
    /// gets (a random offered card) and passes the rest, rolled here from the
    /// world RNG so it replays; standard declares per
    /// [`crate::strategy::StrategyParams::counteract_propensity`] (default
    /// 600‰ per offered card). The prompt is identified by its title key --
    /// the same marker the client's 托管 policies read
    /// (`autopilot.ts`'s `ask.counteract.title`).
    fn fill_ai(&mut self, ask: &mut Ask) {
        for (k, &s) in ask.view.players.iter().enumerate() {
            if ask.ai.get(k).copied().unwrap_or(AI_UNSET) != AI_UNSET {
                continue;
            }
            let seat = s as usize;
            ask.ai[k] = self.ai_answer_for_entry(ask, seat);
        }
    }

    /// The answer `seat`'s policy would give for `ask`'s entry `k` -- the
    /// per-seat body of [`Self::fill_ai`], extracted so the seat-view engine
    /// (`bot-glue`'s `extras`) can run it on a determinized fork, which has
    /// no precomputed [`Ask`] fill. Same RNG draws as the prompt-open fill
    /// (so a live match that re-runs it replays), which is also why a fork's
    /// answer can differ from the live one: the fork owns a fresh stream
    /// (`docs/BOT.md` §1, the hidden-state audit).
    pub fn ai_answer_for_entry(&mut self, ask: &Ask, seat: usize) -> i32 {
        let fallback = ask.view.fallback;
        // A tile prompt carries its targets in `items` (answer == len is
        // "none"); every other prompt's answers index `options`.
        let n = if ask.view.items.is_empty() {
            ask.view.options.len() as i32
        } else {
            ask.view.items.len() as i32
        };
        let counteract = ask.view.title.key() == "ask.counteract.title";
        if self.is_chaos(seat) {
            if counteract && n > 1 && self.w.rng.f64() >= super::CHAOS_COUNTER_CHANCE {
                // This offer it passes (the fallback is the skip option).
                fallback
            } else {
                self.chaos_pick(fallback, n)
            }
        } else if counteract {
            // Standard: declare on the seat's per-card propensity
            // (`docs/BOT.md` §3.8 "counteraction"; default 600‰, user
            // ruling 2026-10-08). `CounterParams` 0 holds a card back.
            self.counteract_pick(seat, ask, fallback, n)
        } else {
            fallback
        }
    }

    /// Standard's [反击] answer: the first offered card whose
    /// [`crate::strategy::CounterParams`] propensity fires, else `fallback`
    /// (the skip). Each candidate draws from the world RNG (so it replays) --
    /// one draw per offered card until one fires, so with `k` offered cards
    /// the per-offer declare rate is `1 - (1 - p)^k` on a uniform per-card
    /// propensity `p` (600‰ default: 60 % for one card, 84 % for two). A
    /// zero propensity draws nothing and never fires, which is how a book
    /// holds a card back.
    fn counteract_pick(&mut self, seat: usize, ask: &Ask, fallback: i32, n: i32) -> i32 {
        let p = self.strategy_of(seat);
        for i in 0..n {
            if i == fallback {
                continue;
            }
            let Some(id) = ask.view.options.get(i as usize).and_then(option_card) else {
                continue;
            };
            let propensity = p.counteract_propensity(&id, None);
            if propensity <= 0 {
                continue;
            }
            let roll = self.w.rng.f64() * 1000.0;
            if roll < propensity as f64 {
                return i;
            }
        }
        fallback
    }

    /// Uniform among `0..n` except `fallback`, falling back to `fallback` when
    /// it is the only option (or `n` is empty). Allocation-free: one draw from
    /// the `n-1` non-default slots, shifted past `fallback`.
    fn chaos_pick(&mut self, fallback: i32, n: i32) -> i32 {
        if n <= 1 {
            return fallback;
        }
        let has_fallback = fallback >= 0 && fallback < n;
        let non_default = n - i32::from(has_fallback);
        if non_default <= 0 {
            return fallback;
        }
        let mut k = self.w.rng.below(non_default as usize) as i32;
        if has_fallback && k >= fallback {
            k += 1;
        }
        k
    }

    /// Presentation pause, accumulated for the host to drain.
    pub(crate) fn wait(&mut self, secs: f32) {
        self.delay += secs + self.w.take_walk_delay();
    }

    /// Total presentation time owed to the host, including the pacing a lazy
    /// walk flush accumulated (`World::flush_walk`) since the last drain.
    pub(crate) fn take_delay(&mut self) -> f32 {
        self.delay += self.w.take_walk_delay();
        std::mem::take(&mut self.delay)
    }

    // ---- public surface for card rules ------------------------------------------

    pub fn data(&self) -> &GameData {
        self.data
    }

    pub fn state(&self) -> &crate::state::MatchState {
        &self.w.st
    }

    /// Append a log line.
    pub fn log(&mut self, player_id: i32, msg: Msg) {
        self.w.log("text", player_id, msg);
    }

    /// A card's effect activated -- or a counteraction negated it before its
    /// body could run. One `"card"` event carries both the log line and the
    /// client's card flash. `kind` is a [`crate::state::card_trigger`]
    /// constant; `owner` is the card's player, `target` the affected player and
    /// `tile` where it fired (`-1` when not applicable). A negated activation
    /// still shows the card, marked 无效, and logs 「<卡名> 的效果被无效」.
    pub fn card_activated(
        &mut self,
        kind: &str,
        owner: i32,
        card: &str,
        target: i32,
        tile: i32,
        negated: bool,
    ) {
        let msg = if negated {
            Msg::new("log.card_negated").card("card", card)
        } else {
            // A genuine activation with no declaration line of its own (a
            // skill press): 「{{who}} 的「{{card}}」发动」.
            let who = if owner >= 0 { owner } else { target };
            Msg::new("log.card_activated")
                .player_id("who", who)
                .card("card", card)
        };
        self.w
            .card_activation(kind, owner, card, target, tile, negated, msg);
    }

    /// Roll `count`d`sides` and log a dice event.
    pub fn roll(&mut self, player_id: i32, count: i32, sides: i32, what: Option<Msg>) -> i32 {
        let faces: Vec<i32> = (0..count).map(|_| self.w.rng.d(sides)).collect();
        let sum = faces.iter().sum();
        let detail = (count > 1).then(|| {
            let joined = faces
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join("+");
            Msg::new("log.part.dice_faces").text("faces", joined)
        });
        let text = Msg::new("log.dice")
            .player_id("who", player_id)
            .i("count", count)
            .i("sides", sides)
            .opt("what", what.map(|w| Msg::new("log.part.why").msg("why", w)))
            .i("sum", sum)
            .opt("detail", detail);
        self.w.log("dice", player_id, text).value = sum;
        sum
    }

    pub fn tile_count(&self) -> usize {
        self.data.tiles.len()
    }

    // ---- shared queries ---------------------------------------------------------

    pub fn tile(&self, t: usize) -> &'a TileData {
        &self.data.tiles[t]
    }

    pub(crate) fn out(&self, i: usize) -> bool {
        self.w.out(i)
    }

    /// The match is in the play phase.
    pub fn playing(&self) -> bool {
        self.w.st.phase == "play"
    }

    /// `CanPay`: not out, not stunned, not exiled.
    pub(crate) fn can_pay(&self, i: usize) -> bool {
        let s = &self.w.st.players[i];
        !s.out() && !s.stunned() && s.exile() == 0
    }

    /// `Blocked(i)` -- why a player can't move money.
    pub(crate) fn blocked(&self, i: usize) -> &'static str {
        let s = &self.w.st.players[i];
        if s.exile() > 0 {
            "status.exiled"
        } else {
            "status.stunned"
        }
    }

    /// Players starting at `from`, wrapping, that are in the game and not exiled.
    pub fn present_from(&self, from: usize) -> Vec<usize> {
        let n = self.w.player_count();
        (0..n)
            .map(|k| (from + k) % n)
            .filter(|&i| !self.out(i) && self.w.st.players[i].exile() == 0)
            .collect()
    }
}
