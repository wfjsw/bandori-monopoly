//! Routine context, flow control, and prompts.
//!
//! A routine is ordinary straight-line code over a [`PlayCtx`]. When it needs a player
//! decision it calls [`PlayCtx::ask`]. If the answer is already in this routine's answer
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

use super::rules::{raise, CardRules, ExclusiveAsk, Trigger};
use super::world::{Extreme, World};

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
    /// [`PlayCtx::work_stack`](super::work::WorkItem); the outermost
    /// [`PlayCtx::drain_work`] pump runs the nested work then the resume.
    /// STACK-01: this is how the settle cycle remains on the heap, not the Rust stack.
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

    /// Is this a player prompt (as opposed to a suspension or the match end)?
    pub fn is_ask(&self) -> bool {
        matches!(self.0, HaltKind::Ask(_))
    }
}

pub type Flow<T> = Result<T, Halt>;

/// Sentinel in [`Ask::ai`] for "no precomputed answer -- fill it in when the
/// prompt is raised, from the seat's [`crate::state::BotMentality`]". Never
/// survives into a saved [`Ask`]: [`PlayCtx::ask`] replaces every entry.
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
    /// Whether answering this prompt draws on the answering seats' time pools
    /// (ordinary prompts under the post-settings clock) or runs on its own
    /// fixed window (`view.time_left`, e.g. the bankruptcy leftover auction --
    /// see [`crate::clock::ClockSettings::bankrupt_auction_time`]). Serde
    /// default `true` so an `Ask` written before this field remains ordinary.
    #[serde(default = "default_use_pool")]
    pub use_pool: bool,
}

fn default_use_pool() -> bool {
    true
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
            use_pool: true,
        }
    }

    /// `Choice`: pick one of `options`. The AI answer is left [`AI_UNSET`];
    /// [`PlayCtx::ask`] fills it per seat (standard: the fallback; chaos: a random
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

    /// `Multi`: pick exactly `count` of `options`. The answer is a **bitmask**
    /// (bit *i* = option *i*) in the same `i32` the answer log already carries.
    /// `fallback` is a valid mask -- the first `count` options -- so a timeout
    /// lands on something the body can apply.
    pub fn multi(
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        options: Vec<Msg>,
        count: i32,
        time: f32,
    ) -> Self {
        let n = options.len().min(31) as i32;
        let want = count.clamp(1, n.max(1));
        let mut a = Self::new("multi", players, title, text, time);
        a.view.options = options;
        a.view.count = want;
        a.view.fallback = if want >= 32 { -1 } else { (1i32 << want) - 1 };
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

    /// `Auction`: open bidding on a tile. `time` is the prompt's own window
    /// (the AI-schedule / all-bot fallback deadline); `use_pool` selects
    /// whether the answering seats also spend their time pools (ordinary
    /// auctions) or not (the bankruptcy leftover auction's dedicated timer --
    /// [`crate::clock::ClockSettings::bankrupt_auction_time`]).
    pub fn auction(
        tile: usize,
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        worth: Vec<i32>,
        time: f32,
        use_pool: bool,
    ) -> Self {
        let n = players.len();
        let mut a = Self::new("auction", players, title, text, time);
        a.view.tile = tile as i32;
        a.view.bid = 0;
        a.view.bidder = -1;
        // Bidding reads `worth` (already mentality-aware); `ai` is unused.
        a.ai = vec![0; n];
        a.worth = worth;
        a.use_pool = use_pool;
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

/// What a routine gets back from [`PlayCtx::ask`].
#[derive(Debug, Clone)]
pub struct Reply {
    pub players: Vec<i32>,
    pub fallback: i32,
    pub answered: Answered,
}

/// The card id a prompt option declares (`Arg::Card` on the option's label).
/// Used by the counteraction offer, whose options are `ask.counteract.play` messages
/// naming the card each would declare.
fn option_card(m: &Msg) -> Option<String> {
    m.args.get("card").and_then(|a| match a {
        crate::msg::Arg::Card(id) => Some(id.clone()),
        _ => None,
    })
}

impl Reply {
    /// This seat's recorded answer, or the prompt's fallback.
    pub fn of(&self, player_id: usize) -> i32 {
        match self.players.iter().position(|&s| s == player_id as i32) {
            Some(i) if self.answered.answers.get(i).is_some_and(|&v| v >= 0) => {
                self.answered.answers[i]
            }
            _ => self.fallback,
        }
    }
}

/// Simulation-mode answer source (`docs/BOT.md` §3.2). Installed on a [`PlayCtx`];
/// every [`PlayCtx::ask`] whose replay-log entry is missing calls it instead of
/// halting the routine.
///
/// The live match never installs one (the halt/replay model is what recordings
/// and the server protocol ride). A simulation installs the ISMCTS tree policy
/// during descent and the rollout policy afterwards; both answer inline, so a
/// routine runs once per simulation instead of once per prompt.
pub trait AnswerProvider {
    /// Answer `ask` inline. `ask.ai` / `ask.ai_picked` / `ask.worth` are
    /// already filled in ([`PlayCtx::fill_ai`]), so the default answer is the seat
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

/// A multi-select answer is a bitmask naming exactly `view.count` options out
/// of `0..options.len()`. Anything else is not a usable answer.
fn valid_multi_mask(ask: &Ask, v: i32) -> bool {
    if v < 0 {
        return false;
    }
    let n = ask.view.options.len().min(31) as i32;
    if n <= 0 || (v >> n) != 0 {
        return false;
    }
    v.count_ones() as i32 == ask.view.count
}

/// The answer the engine's bot schedule would land on for `ask`, with a
/// per-seat parameter lookup (player index → [`crate::strategy::StrategyParams`]).
/// Only the auction solver reads it (the raise step); the other prompts just
/// echo the precomputed `ai` fill.
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
    let multi = ask.view.kind == "multi";
    let max = if ask.view.kind == "tile" {
        ask.view.items.len() as i32
    } else {
        ask.view.options.len() as i32 - 1
    };
    let mut answers = Vec::with_capacity(n);
    for k in 0..n {
        if picks {
            answers.push(0);
        } else if multi {
            // Bitmask answers must not be clamped to an option index -- the
            // fallback is already a valid `count`-bit mask.
            let v = ask.ai.get(k).copied().unwrap_or(AI_UNSET);
            answers.push(if valid_multi_mask(ask, v) {
                v
            } else {
                ask.view.fallback
            });
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
/// remains under its ceiling, and passes otherwise. The last bidder wins at the
/// bid it made -- the shape `auction_tile` reads (`r.args.bidder` / `r.args.bid`).
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

/// The terminal outcome of a [触发结算], written as the frame passes its last
/// terminal stage. Distinguishes "the settle played out" from a cancel and from
/// the payer leaving, so a reader after the fact (council's refund, the
/// `settle.outcome` pre field) can tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettleOutcome {
    /// The settle ran to `tileResolved` with no earlier terminal outcome.
    #[default]
    Settled,
    /// Cancelled at `settle` or `settleBody`.
    Cancelled,
    /// The payer left the board (`out`) at `settle` or `settleAfter`.
    PayerOut,
    /// The match stopped playing before `tileResolved`.
    Aborted,
}

impl SettleOutcome {
    /// Decode the guest / CEL encoding; an unknown code is `Settled`.
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => SettleOutcome::Cancelled,
            2 => SettleOutcome::PayerOut,
            3 => SettleOutcome::Aborted,
            _ => SettleOutcome::Settled,
        }
    }

    /// The guest / CEL encoding.
    pub const fn as_i32(self) -> i32 {
        match self {
            SettleOutcome::Settled => 0,
            SettleOutcome::Cancelled => 1,
            SettleOutcome::PayerOut => 2,
            SettleOutcome::Aborted => 3,
        }
    }
}

/// One open [触发结算] on [`PlayCtx::settle_stack`]. The tile and payer are the
/// frame's; the `base*` figures are the *outer* settle figures at push time so
/// the entry's own settle-scoped writes compose on top without re-reading the
/// stack. `outcome` starts
/// [`SettleOutcome::Settled`] and is only lowered by a terminal branch.

/// A card-issued **house write** (设计 5.4): the effective change is computed
/// against the tile's cap first, and an unchanged count opens no window.
#[derive(Clone, Copy, Debug)]
pub enum HouseWrite {
    Set(i32),
    Add(i32),
}

/// A settle entry.
#[derive(Debug, Clone)]
pub struct SettleEntry {
    pub player: usize,
    /// The **governing tile** of the settle (the frame's `tile`).
    pub tile: i32,
    /// Owner of `tile` when the settle opened -- the payee a rent payment
    /// must reach to count as this settle's `paid`.
    pub target: i32,
    /// Outer pay product / rent product and the outer `settleAdd` /
    /// `settleRentAdd` sums at push time. The entry's own settle-scoped writes
    /// compose on top, and a nested settle restores the outer figures on its
    /// way out by copying these back.
    pub base: f64,
    pub base_rent: f64,
    pub base_add: i32,
    pub base_rent_add: i32,
    pub outcome: SettleOutcome,
    /// The `settleBody` link was negated: another card replaced the body
    /// (`Cancelled` covers both, so the two are split here). G10 §6.1.
    pub body_replaced: bool,
    /// A rent payment to this settle's owner on this settle's tile committed,
    /// and what it moved.
    pub paid: bool,
    pub paid_value: i32,
    /// The settle's move, stamped onto raises inside the settle so a payment
    /// listener can see which move it belongs to (and its path).
    pub move_context: Option<crate::engine::move_ctx::MoveCtx>,
    /// The settle's move view. A raise inside the settle inherits it when it
    /// carries none of its own (see `PlayCtx::raise`), so a payment hook can
    /// tell a card-forced settle (`settle_redirect`) from the walk's and reads
    /// the main-move flag the settle belongs to.
    pub mv: super::rules::TriggerMove,
    /// The settle's main-move flag, stamped on inheriting raises.
    pub is_main_move: bool,
}

/// Routine context: the world being mutated, read-only game data, card rules, and
/// the replay cursor.
pub struct PlayCtx<'a> {
    pub(crate) w: crate::engine::world::SharedWorld,
    pub(crate) data: &'a GameData,
    pub(crate) rules: &'a dyn CardRules,
    answers: &'a [Answered],
    cursor: usize,
    /// The guest answer log (G10 §6.3.5). A parking spot between drive
    /// starts: the running pass keeps the live copy on its guest store, the
    /// drive takes this one at its start and puts it back on a halt. A body
    /// resume carries the whole log in its work item; an ordinary prompt halt
    /// drops it with the context (the routine re-runs from its snapshot).
    /// Empty while a drive runs.
    pub guest_log: crate::engine::GuestLog,
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
    /// The open [触发结算]s, innermost last. An entry is pushed **once** per
    /// frame (at its first step, not on every resume) and popped on completion,
    /// so a suspended settle never inflates the depth. Non-empty is 「结算中」
    /// for the payments the settle carries -- RAS:游击演出's
    /// 「本回合的[结算]向其他玩家支付了至少1000资金」 tally -- and the
    /// innermost entry stamps every raise with its tile and outcome. The entry
    /// also carries the settle's 支付价格 base and addend: the `settleAdd` /
    /// `settleMul` stages raise once per settle, so every payment it carries
    /// folds the same addend / factor, and a nested settle restores the
    /// outer's figures on its way out. Transient.
    pub(crate) settle_stack: Vec<SettleEntry>,
    /// The execution id [`Self::reset_factor_ledger`] handed this drive. Held
    /// on the context (a resume keeps it) and drawn from the world's
    /// `factor_gen`, so parallel fresh contexts cannot collide. Transient.
    pub(crate) ledger_exec: u32,
    /// Nested `state` [反击] settlement depth. A counter's own skill-state
    /// toggles must not open another window (ping-pong). Transient.
    pub state_raise_depth: u32,
    /// A nested skill teleport's tail is raising (see
    /// `PlayCtx::teleport_tail_only`). Transient.
    pub(crate) in_teleport_tail: bool,
    /// `"card"` activation event ids currently open, innermost last. A nested
    /// drive groups under the top of this stack; `-1` at the bottom.
    pub activation: Vec<i32>,
    /// The move start of the move currently being walked / teleported, or `-1`
    /// when no move is in flight. 「[pass]the start square且[move start]不为the start square」 reads it
    /// (the built-in pass reward). Transient (not serialized) -- the walk sets it as it
    /// starts and the reward step is the only reader.
    pub(crate) move_start: i32,
    /// How deep the move pipeline (`walk_inner` / `teleport_as`) is nested.
    /// A `passTile` (or other) hook that calls `ctx::teleport_to` re-enters
    /// `card_teleport`; at depth > 0 that remains a bare position write (no
    /// second move tail) so the hook cannot recurse. Transient (not serialized).
    pub(crate) move_depth: u32,
    /// Guest state writes overlaid on the live world for a host routine's
    /// duration (see [`Self::overlay_guest_state`]). A stack: nested drives push
    /// and pop their own. The number is the pushing drive's id, so a pass
    /// restart reverts only its own crossings -- an in-flight nested drive's
    /// writes were not re-derived by the restart and must stay. Transient
    /// (not serialized).
    pub(crate) guest_overlays: Vec<(u32, GuestOverlay)>,
    /// `houseAdded` / `houseRemoved` rows owed by host tile writes this drive
    /// has applied (`(owner, tile, now, delta)`). A guest run is rebuilt on
    /// every pause pass, so a write applied on the live context cannot live on
    /// the guest `Run`; `commit_after` drains this into the committed run and
    /// raises there (design G09 5.4/5.5). Transient.
    pending_house_log: Vec<(i32, i32, i32, i32)>,
    /// Uids a [`Self::restore_guests_to`] put back into the live world for a
    /// re-run's reads. A drive drains the set at its loop top and hands it to
    /// every pass; a uid its replay never takes out again must not survive the
    /// commit (see `commit_after`).
    pub(crate) reinserted: Vec<i32>,
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
    /// True while [`Self::drain_work`] is pumping. A nested `settle_tile_at`
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
    /// The live counteraction chain (a type-erased `CounteractChainCursor`),
    /// installed around every counter body and every offer so the guests can
    /// read the pending answers (design G06 §6.2). Valid only for the duration
    /// of one drive / one offer.
    pub(crate) counteract_chain: Option<Box<dyn std::any::Any + Send>>,
    /// Counteraction chains whose address a parked continuation still
    /// references (see [`Self::keep_chain`]). Transient (not serialized).
    pub(crate) chain_keepalive: Vec<Box<dyn std::any::Any + Send>>,
    /// Dest a just-resumed drive returned (STACK-01). The card-dest work
    /// item reads it after the resume completes.
    pub(crate) last_drive_fate: Option<i32>,
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
/// hot path. Two verdicts ride along: the ruleset's `cant_play` and the
/// `ExclusiveUse` gate denial (both pure within one stamp).
#[derive(Default)]
struct PlayMemo {
    stamp: u64,
    entries: Vec<((usize, String), Option<Msg>)>,
    exclusive_stamp: u64,
    exclusive: Vec<((usize, String), bool)>,
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
    fn insert(&mut self, player: usize, card: &str, reason: Option<Msg>) {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|((p, c), _)| *p == player && c == card)
        {
            slot.1 = reason;
            return;
        }
        // Cap the memo: a scan that runs away must not grow without bound
        // (the stamp resets it on the next write anyway).
        if self.entries.len() >= 64 {
            self.entries.clear();
        }
        self.entries.push(((player, card.to_string()), reason));
    }
    fn clear_exclusive(&mut self) {
        self.exclusive.clear();
    }
    fn get_exclusive(&self, player: usize, card: &str) -> Option<bool> {
        self.exclusive
            .iter()
            .find(|((p, c), _)| *p == player && c == card)
            .map(|(_, v)| *v)
    }
    fn insert_exclusive(&mut self, player: usize, card: &str, denied: bool) {
        if let Some(slot) = self
            .exclusive
            .iter_mut()
            .find(|((p, c), _)| *p == player && c == card)
        {
            slot.1 = denied;
            return;
        }
        if self.exclusive.len() >= 64 {
            self.exclusive.clear();
        }
        self.exclusive.push(((player, card.to_string()), denied));
    }
}

/// The guest's pending player-state **clears**, overlaid on the live world
/// while a host routine runs, and **not persisted**:
/// [`PlayCtx::restore_guests_to`] drops the overlay, keeping only the routine's own
/// effects, and the replay re-applies the guest writes deterministically from
/// the top of the body.
///
/// This is the merged view for 「…时」 a status the guest just cleared (e.g.
/// 壱雫空 zeroing a layer and then paying) must be visible to `can_pay` without
/// the clear landing twice -- the body replays from the top, so a *persisted*
/// clear would be re-applied on top of itself and an additive write would
/// double-count. Only decreases cross; see [`PlayCtx::overlay_guest_state`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GuestOverlay {
    /// `(player, key, base, wrote)` for each state item the guest decreased.
    /// `base` is `None` when the key was absent before the overlay. `wrote` is
    /// always the value the replay's copy wants (a routine's own move
    /// composed on it); `crossed` says whether it also landed on **live**. A
    /// non-crossing increase at a pause boundary did not, so the revert must
    /// leave live to the routine instead of subtracting a delta it never
    /// carried.
    pub state: Vec<(
        usize,
        String,
        Option<crate::state::StateVar>,
        crate::state::StateVar,
        bool,
    )>,
    /// Field instances the guest placed, rewrote or took off the field in the
    /// same body ([`FieldDelta`]).
    #[serde(default)]
    pub field: Box<FieldDelta>,
    /// `(player, name, base, wrote)` held-counter rows the guest changed; both
    /// sides are `None` for an absent row.
    #[serde(default)]
    pub tokens: Vec<(
        usize,
        String,
        Option<crate::state::Counter>,
        Option<crate::state::Counter>,
    )>,
    /// `(player, base, wrote)` money the guest moved with a raw write
    /// (`ctx::adjust_money` outside the inline host). A money move through the
    /// pipeline applies on live instead and leaves no entry here.
    #[serde(default)]
    pub money: Vec<(usize, i32, i32)>,
    /// `(player, base, wrote)` persona writes (`ctx::set_persona`, 二人一体's
    /// swap). Absolute per-seat faces: `base` is the pass-start value and the
    /// write crosses a pause like a money row, or a request between the write
    /// and the body's end would rebuild the copy from live without it.
    #[serde(default)]
    pub persona: Vec<(usize, String, String)>,
    /// Board-row writes (`ctx::set_owner` / `ctx::set_houses` /
    /// `ctx::set_mortgaged`, embers, tile marks).
    #[serde(default)]
    pub board: Box<BoardDelta>,
    /// `(key, uid)` world-snapshot owners the guest added (a card a body
    /// places adopts the body's keys -- see `adopt_snapshot_keys`). Ownership
    /// is cleanup bookkeeping on the host world, and a nested drive forked
    /// from it before the adoption must see the copy on its row.
    #[serde(default)]
    pub owners: Vec<(i32, i32)>,
    /// `(player, delta)` private-pile writes (`ctx::send_to_dest`,
    /// `ctx::take_card`): hand / deck / graveyard lists the body moved cards
    /// in. Each list is `None` when the body left it alone.
    #[serde(default)]
    pub piles: Vec<(usize, PileDelta)>,
    /// Pick-field writes (`ctx::roll_pick`) on open roll transactions. The
    /// transaction itself is host-created, so only the pick columns cross.
    #[serde(default)]
    pub roll_picks: Vec<RollPickDelta>,
    /// Turn-end callback list (`ctx::at_turn_end` / `ctx::before_turn_end` and
    /// kin) against the pass base: `(base, wrote)` when the guest scheduled or
    /// consumed one. The guest is the only scheduler; the list crosses whole
    /// so an arm and a consumed `skip` both survive the boundary.
    #[serde(default)]
    pub scheduled: Option<(Vec<crate::engine::Scheduled>, Vec<crate::engine::Scheduled>)>,
    /// The public event surfaces the guest writes raw (`ctx::event_deck_push`,
    /// `ctx::event_active_push` / `_pop`): deck, public top view, active list,
    /// each against the pass base. They are append/remove lists, not keyed
    /// writes, so they cross as a delta like [`ScheduledDelta`].
    #[serde(default)]
    pub events: Option<EventDelta>,
    /// `(name, base, wrote)` rows of the rule-parameter bonus map
    /// (`ctx::add_ring_bonus`). A keyed additive row like a token counter:
    /// `base`/`wrote` are `None` for an absent key, the base is live's value
    /// at this boundary, and the rows are not deduped.
    #[serde(default)]
    pub rule_bonus: Vec<(String, Option<i32>, Option<i32>)>,
    /// Inspection-frame card lists the guest edited directly (`ctx::take_card`
    /// / `ctx::add_to_dest` on `CardPile::Inspection`). Opening and closing a
    /// frame are host requests, so only the listed cards cross -- the looker
    /// takes a card out before placing or discarding it.
    #[serde(default)]
    pub frames: Vec<FrameDelta>,
}

/// One inspection frame's card list against the pass base. The frame itself
/// (open/close) is host-owned; the body takes cards out of the look
/// (`ctx::take_card` with `CardPile::Inspection`) before it places or discards
/// them, and that take has to cross like any other guest write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameDelta {
    /// The frame id.
    pub id: i32,
    /// The list at the pass start.
    pub base: Vec<crate::engine::world::InspectionCard>,
    /// The list the body left.
    pub wrote: Vec<crate::engine::world::InspectionCard>,
}

/// One open roll transaction's pick columns against the pass base (design G10
/// §6.3.3: a direct guest write crosses the boundary like any other). The keep
/// point is read by the settle, a host routine that runs after the body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RollPickDelta {
    /// The transaction id.
    pub id: i32,
    /// `(base, wrote)` chosen cast index.
    pub chosen_index: (usize, usize),
    /// `(base, wrote)` unkept raw face.
    pub unused_face: (i32, i32),
    /// `(base, wrote)` recompute flag.
    pub dirty: (bool, bool),
}

/// A guest body's turn-end callback list against the pass base (design G10
/// §6.3.3: a direct guest write crosses the boundary like any other). Only the
/// entries the guest added or removed cross, so a callback the host scheduled
/// meanwhile is kept.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduledDelta {
    /// The list at the pass start.
    pub base: Vec<crate::engine::Scheduled>,
    /// The list the body left.
    pub wrote: Vec<crate::engine::Scheduled>,
}

/// The identity a scheduled entry is matched on across passes (the `skip`
/// flag is body-written state and may flip).
fn scheduled_identity(s: &crate::engine::Scheduled) -> (&str, i32, usize, i32, &str) {
    (&s.card, s.owner, s.target, s.uid, &s.entry)
}

/// The event-deck / public-top / active-event lists against the pass base
/// (design G10 §6.3.3: a direct guest write crosses the boundary like any
/// other). The event stack is not a keyed row, so the three lists cross whole
/// and a row another writer added meanwhile stays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDelta {
    /// The deck at the pass start.
    pub base_deck: Vec<String>,
    /// The deck the body left.
    pub wrote_deck: Vec<String>,
    /// The public face-down top view at the pass start.
    pub base_top: Vec<String>,
    /// The public top view the body left.
    pub wrote_top: Vec<String>,
    /// The event discard at the pass start.
    pub base_discard: Vec<String>,
    /// The event discard the body left.
    pub wrote_discard: Vec<String>,
    /// The removed-event list at the pass start.
    pub base_removed: Vec<String>,
    /// The removed-event list the body left.
    pub wrote_removed: Vec<String>,
    /// The active event rows at the pass start.
    pub base_active: Vec<crate::state::ActiveEvent>,
    /// The active event rows the body left.
    pub wrote_active: Vec<crate::state::ActiveEvent>,
}

/// Merge one append/remove list: a row the body removed goes, a row it added
/// comes, and a row only the live world has stays. Idempotent -- a replayed
/// delta whose live already equals the write is a no-op.
///
/// Counts, not membership: a deck legitimately holds the same row twice
/// (`event_deck_push` appends what it is handed), and a `contains` would read
/// the second copy's add (or removal) as one the base already carries.
fn merge_list<T: PartialEq + Clone>(live: &mut Vec<T>, base: &[T], wrote: &[T]) {
    if *live == *wrote {
        return;
    }
    if *live == *base {
        *live = wrote.to_vec();
        return;
    }
    let count = |list: &[T], x: &T| list.iter().filter(|y| *y == x).count();
    let mut handled: Vec<&T> = Vec::new();
    for x in base.iter().chain(wrote.iter()) {
        if handled.contains(&x) {
            continue;
        }
        handled.push(x);
        let base_n = count(base, x);
        let wrote_n = count(wrote, x);
        let mut live_n = count(live, x);
        // The removal: take it off live, but only as far down as the write --
        // a row the routine already removed, or a delta applied a second time,
        // must not take another copy.
        let mut doomed = base_n.saturating_sub(wrote_n);
        while doomed > 0 && live_n > wrote_n {
            let Some(i) = live.iter().position(|y| y == x) else {
                break;
            };
            live.remove(i);
            live_n -= 1;
            doomed -= 1;
        }
        // The addition: bring live up to the written count.
        while live_n < wrote_n {
            live.push((*x).clone());
            live_n += 1;
        }
    }
}

impl EventDelta {
    fn apply(&self, w: &mut World) {
        merge_list(&mut w.event_deck, &self.base_deck, &self.wrote_deck);
        merge_list(&mut w.st.event_top, &self.base_top, &self.wrote_top);
        merge_list(
            &mut w.event_discard,
            &self.base_discard,
            &self.wrote_discard,
        );
        merge_list(
            &mut w.event_removed,
            &self.base_removed,
            &self.wrote_removed,
        );
        merge_active(
            &mut w.st.event_active,
            &self.base_active,
            &self.wrote_active,
        );
    }

    fn restore(&self, w: &mut World) {
        merge_list(&mut w.event_deck, &self.wrote_deck, &self.base_deck);
        merge_list(&mut w.st.event_top, &self.wrote_top, &self.base_top);
        merge_list(
            &mut w.event_discard,
            &self.wrote_discard,
            &self.base_discard,
        );
        merge_list(
            &mut w.event_removed,
            &self.wrote_removed,
            &self.base_removed,
        );
        merge_active(
            &mut w.st.event_active,
            &self.wrote_active,
            &self.base_active,
        );
    }
}

/// [`merge_list`] keyed on `(id, player_id)` -- an active row's counters are
/// moved by other families, so a whole-row equality could miss the merge.
fn merge_active(
    live: &mut Vec<crate::state::ActiveEvent>,
    base: &[crate::state::ActiveEvent],
    wrote: &[crate::state::ActiveEvent],
) {
    let key = |e: &crate::state::ActiveEvent| (e.id.clone(), e.player_id);
    let live_keys: Vec<(String, i32)> = live.iter().map(key).collect();
    let wrote_keys: Vec<(String, i32)> = wrote.iter().map(key).collect();
    if live_keys == wrote_keys {
        return;
    }
    let base_keys: Vec<(String, i32)> = base.iter().map(key).collect();
    live.retain(|e| !base_keys.contains(&key(e)) || wrote_keys.contains(&key(e)));
    for (w, wk) in wrote.iter().zip(wrote_keys) {
        if base_keys.contains(&wk) {
            continue;
        }
        match live.iter().position(|e| key(e) == wk) {
            Some(i) => live[i] = w.clone(),
            None => live.push(w.clone()),
        }
    }
}

/// The guest's board-row writes against the pass base (design G10 §6.3.3).
/// The per-tile arrays are index-aligned, so they diff row by row; the tile
/// mark list is keyed by uid like [`ScheduledDelta`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardDelta {
    /// `(tile, base, wrote)` deed owners.
    pub owners: Vec<(usize, i32, i32)>,
    /// `(tile, base, wrote)` house counts.
    pub houses: Vec<(usize, i32, i32)>,
    /// `(tile, base, wrote)` mortgage flags.
    pub mortgaged: Vec<(usize, bool, bool)>,
    /// `(tile, base, wrote)` ember counts.
    pub embers: Vec<(usize, i32, i32)>,
    /// The tile mark list: `(base, wrote)`.
    pub marks: Option<(Vec<crate::state::TileMark>, Vec<crate::state::TileMark>)>,
}

impl BoardDelta {
    /// A scalar row write. Live still at the base takes the guest's value; a
    /// row the host moved takes the guest's *delta* -- unless live already
    /// sits on the guest's value (the write crossed at a pause), which stays.
    fn apply_scalar(rows: &[(usize, i32, i32)], live: &mut [i32]) {
        for (i, b, w) in rows {
            if let Some(v) = live.get_mut(*i) {
                if *v == *b {
                    *v = *w;
                } else if *v != *w {
                    *v = *v + (*w - *b);
                }
            }
        }
    }

    fn apply(&self, w: &mut World) {
        Self::apply_scalar(&self.owners, &mut w.st.owners);
        Self::apply_scalar(&self.houses, &mut w.st.houses);
        Self::apply_scalar(&self.embers, &mut w.st.embers);
        for (i, b, wrote) in &self.mortgaged {
            if let Some(v) = w.st.mortgaged.get_mut(*i) {
                if *v == *b {
                    *v = *wrote;
                }
            }
        }
        if let Some((base, wrote)) = &self.marks {
            for b in base {
                if !wrote.iter().any(|x| x.uid == b.uid) {
                    w.st.marks.retain(|m| m.uid != b.uid);
                }
            }
            for wr in wrote {
                match w.st.marks.iter().position(|m| m.uid == wr.uid) {
                    Some(i) => {
                        if !base.iter().any(|b| *b == *wr) {
                            w.st.marks[i] = wr.clone();
                        }
                    }
                    None => w.st.marks.push(wr.clone()),
                }
            }
            // Mark uids are monotonic and never reused (a dead handle must not
            // alias a later mark). The guest copy allocates them, so the
            // counter has to ride the overlay the way `next_card_uid` rides
            // `FieldDelta`; `restore` bumps it too, since giving the rows back
            // must not give the allocator back with them.
            let next = wrote
                .iter()
                .chain(base.iter())
                .map(|m| m.uid.saturating_add(1))
                .max()
                .unwrap_or(0);
            w.st.next_mark_uid = w.st.next_mark_uid.max(next);
        }
    }

    /// Undo where the guest left its own value in place (a host write since
    /// stays, as with every other family).
    fn restore(&self, w: &mut World) {
        for (i, b, wrote) in &self.owners {
            if let Some(v) = w.st.owners.get_mut(*i) {
                if *v == *wrote {
                    *v = *b;
                }
            }
        }
        for (i, b, wrote) in &self.houses {
            if let Some(v) = w.st.houses.get_mut(*i) {
                if *v == *wrote {
                    *v = *b;
                }
            }
        }
        for (i, b, wrote) in &self.embers {
            if let Some(v) = w.st.embers.get_mut(*i) {
                if *v == *wrote {
                    *v = *b;
                }
            }
        }
        for (i, b, wrote) in &self.mortgaged {
            if let Some(v) = w.st.mortgaged.get_mut(*i) {
                if *v == *wrote {
                    *v = *b;
                }
            }
        }
        if let Some((base, wrote)) = &self.marks {
            for wr in wrote {
                if !base.iter().any(|b| *b == *wr) {
                    if let Some(i) = w.st.marks.iter().position(|m| *m == *wr) {
                        w.st.marks.remove(i);
                    }
                }
            }
            for b in base {
                match w.st.marks.iter().position(|m| m.uid == b.uid) {
                    Some(i) => {
                        if !wrote.iter().any(|wr| *wr == *b) {
                            w.st.marks[i] = b.clone();
                        }
                    }
                    None => w.st.marks.push(b.clone()),
                }
            }
            // See `apply`: the allocator is monotonic across the rewind.
            let next = wrote
                .iter()
                .chain(base.iter())
                .map(|m| m.uid.saturating_add(1))
                .max()
                .unwrap_or(0);
            w.st.next_mark_uid = w.st.next_mark_uid.max(next);
        }
    }
}

impl ScheduledDelta {
    fn apply(&self, list: &mut Vec<crate::engine::Scheduled>) {
        // Idempotent: a stored overlay is replayed at the commit after its
        // crossing already put the list on live. The other families write keys
        // absolutely; this one adds and removes rows, so a second pass would
        // append the same callback again (the counter card's `at_end` fired
        // twice). Counts, not membership -- a body may legitimately schedule
        // two identical callbacks.
        fn counts(
            v: &[crate::engine::Scheduled],
        ) -> std::collections::BTreeMap<(String, i32, usize, i32, String), usize> {
            let mut m = std::collections::BTreeMap::new();
            for s in v {
                let (card, owner, target, uid, entry) = scheduled_identity(s);
                *m.entry((card.to_string(), owner, target, uid, entry.to_string()))
                    .or_insert(0usize) += 1;
            }
            m
        }
        if counts(list) == counts(&self.wrote) {
            return;
        }
        // Entries the body removed.
        for b in &self.base {
            if !self.wrote.contains(b) {
                if let Some(i) = list
                    .iter()
                    .position(|s| scheduled_identity(s) == scheduled_identity(b))
                {
                    list.remove(i);
                }
            }
        }
        for w in &self.wrote {
            if self.base.contains(w) {
                continue;
            }
            match self
                .base
                .iter()
                .find(|b| scheduled_identity(b) == scheduled_identity(w))
            {
                // A `skip` flip: the entry is known, only the flag moved.
                Some(b) => {
                    if let Some(s) = list.iter_mut().find(|s| {
                        s.skip == b.skip && scheduled_identity(s) == scheduled_identity(b)
                    }) {
                        s.skip = w.skip;
                    }
                }
                None => list.push(w.clone()),
            }
        }
    }

    /// Undo the guest's writes so the replay produces them again.
    fn restore(&self, list: &mut Vec<crate::engine::Scheduled>) {
        for w in &self.wrote {
            if !self.base.contains(w) {
                if let Some(i) = list.iter().position(|s| *s == *w) {
                    list.remove(i);
                }
            }
        }
        for b in &self.base {
            if self.wrote.contains(b) {
                continue;
            }
            match self
                .wrote
                .iter()
                .find(|w| scheduled_identity(w) == scheduled_identity(b))
            {
                Some(w) => {
                    if let Some(s) = list.iter_mut().find(|s| {
                        s.skip == w.skip && scheduled_identity(s) == scheduled_identity(w)
                    }) {
                        s.skip = b.skip;
                    }
                }
                None => list.push(b.clone()),
            }
        }
    }
}

impl RollPickDelta {
    fn apply(&self, r: &mut crate::engine::roll::RollTransaction) {
        r.chosen_index = self.chosen_index.1;
        r.unused_face = self.unused_face.1;
        r.dirty = self.dirty.1;
    }

    /// Undo where the guest left its own value in place (a host write since
    /// stays, as with every other family).
    fn restore(&self, r: &mut crate::engine::roll::RollTransaction) {
        if r.chosen_index == self.chosen_index.1 {
            r.chosen_index = self.chosen_index.0;
        }
        if r.unused_face == self.unused_face.1 {
            r.unused_face = self.unused_face.0;
        }
        if r.dirty == self.dirty.1 {
            r.dirty = self.dirty.0;
        }
    }
}

/// One player's private-pile delta against the pass base (design G10 §6.3.3:
/// a direct guest write crosses the boundary like any other).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PileDelta {
    /// `(base, wrote)` per list; `None` when the body left the list alone.
    #[serde(default)]
    pub hand: Option<(Vec<String>, Vec<String>)>,
    #[serde(default)]
    pub deck: Option<(Vec<String>, Vec<String>)>,
    #[serde(default)]
    pub grave: Option<(Vec<String>, Vec<String>)>,
    /// `(base, wrote)` of the seat's stored next-main-move step count
    /// (`ctx::set_next_steps`, 普通与理所当然) -- a guest write like the lists.
    #[serde(default)]
    pub next_steps: Option<(Option<i32>, Option<i32>)>,
}

/// Apply the guest's edit to a card list onto `live`: the span between the
/// common prefix and the common suffix of `base`/`wrote` is the guest's own
/// change, and it is patched into `live` where `live` still has the base's
/// cards. A card list is a whole-list row (order matters), but the host
/// routine or a nested drive may have moved it too while the body was paused;
/// replacing `live` with `wrote` would drop that move (the counter's card
/// leaving the hand, the draw taken meanwhile).
fn apply_pile_edit(live: &mut Vec<String>, base: &[String], wrote: &[String]) {
    if live == wrote {
        return;
    }
    if live == base {
        *live = wrote.to_vec();
        return;
    }
    let mut p = 0;
    while p < base.len().min(wrote.len()) && base[p] == wrote[p] {
        p += 1;
    }
    let mut s = 0;
    while s < base.len() - p
        && s < wrote.len() - p
        && base[base.len() - 1 - s] == wrote[wrote.len() - 1 - s]
    {
        s += 1;
    }
    let mut at = p.min(live.len());
    for card in &base[p..base.len() - s] {
        if let Some(j) = live[at.min(live.len())..].iter().position(|c| c == card) {
            live.remove(at + j);
        } else if let Some(j) = live[..at.min(live.len())].iter().position(|c| c == card) {
            live.remove(j);
            at = at.saturating_sub(1);
        }
    }
    // The re-run / the crossing may already have applied part of the insert
    // (a write reached again after a pause records the same delta twice). A
    // card `live` already carries is moved rather than inserted a second
    // time, one occurrence per copy `live` really had.
    let mut present: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in live.iter() {
        *present.entry(c.clone()).or_default() += 1;
    }
    for card in &wrote[p..wrote.len() - s] {
        if let Some(n) = present.get_mut(card.as_str()).filter(|n| **n > 0) {
            *n -= 1;
            if let Some(j) = live.iter().position(|c| c == card) {
                live.remove(j);
                if j < at {
                    at = at.saturating_sub(1);
                }
            }
        }
        let at2 = at.min(live.len());
        live.insert(at2, card.clone());
        at += 1;
    }
}

/// Move one rule-parameter bonus key from `from` to `to` only where the world
/// still holds `from` (the three-way guard of every keyed row). `None` means
/// the key is absent.
fn set_rule_bonus(w: &mut World, name: &str, from: Option<i32>, to: Option<i32>) {
    if w.rule_param_bonus.get(name).copied() != from {
        return;
    }
    match to {
        Some(v) => {
            w.rule_param_bonus.insert(name.to_string(), v);
        }
        None => {
            w.rule_param_bonus.remove(name);
        }
    }
}

impl PileDelta {
    fn apply(&self, h: &mut crate::engine::Hidden) {
        if let Some((base, wrote)) = &self.hand {
            apply_pile_edit(&mut h.hand, base, wrote);
        }
        if let Some((base, wrote)) = &self.deck {
            apply_pile_edit(&mut h.deck, base, wrote);
        }
        if let Some((base, wrote)) = &self.grave {
            apply_pile_edit(&mut h.graveyard, base, wrote);
        }
        if let Some((_, wrote)) = &self.next_steps {
            h.next_steps = *wrote;
        }
    }

    /// Undo the body's own edit (the host may have written the list since;
    /// that write stays, as with every other family).
    fn restore(&self, h: &mut crate::engine::Hidden) {
        if let Some((base, wrote)) = &self.hand {
            if &h.hand == wrote {
                h.hand = base.clone();
            } else {
                apply_pile_edit(&mut h.hand, wrote, base);
            }
        }
        if let Some((base, wrote)) = &self.deck {
            if &h.deck == wrote {
                h.deck = base.clone();
            } else {
                apply_pile_edit(&mut h.deck, wrote, base);
            }
        }
        if let Some((base, wrote)) = &self.grave {
            if &h.graveyard == wrote {
                h.graveyard = base.clone();
            } else {
                apply_pile_edit(&mut h.graveyard, wrote, base);
            }
        }
    }
}

/// The pass-start world surfaces the guest world is diffed against
/// ([`PlayCtx::capture_guest_state_from`] / [`PlayCtx::capture_guest_field_from`]).
#[derive(Clone, Debug, Default)]
pub struct PassBase {
    /// Every player's state map.
    pub state: Vec<std::collections::BTreeMap<String, crate::state::StateVar>>,
    /// Every player's field list.
    pub field: Vec<Vec<crate::state::FieldCard>>,
    /// The board field list.
    pub board_field: Vec<crate::state::FieldCard>,
    /// Every player's held-counter rows.
    pub tokens: Vec<Vec<crate::state::Counter>>,
    /// The world-snapshot owner rows.
    pub owners: Vec<(i32, Vec<i32>)>,
    /// Every player's money.
    pub money: Vec<i32>,
    /// Every player's persona face.
    pub persona: Vec<String>,
    /// Every player's private piles (hand / deck / graveyard).
    pub piles: Vec<(Vec<String>, Vec<String>, Vec<String>)>,
    /// Every player's stored next-main-move step count.
    pub next_steps: Vec<Option<i32>>,
    /// `(id, chosen_index, unused_face, dirty)` of every open roll transaction.
    pub rolls: Vec<(i32, usize, i32, bool)>,
    /// The turn-end callback list.
    pub scheduled: Vec<crate::engine::Scheduled>,
    /// The event deck.
    pub event_deck: Vec<String>,
    /// The public face-down top view.
    pub event_top: Vec<String>,
    /// The event discard.
    pub event_discard: Vec<String>,
    /// Removed (banished) events.
    pub event_removed: Vec<String>,
    /// The active event rows.
    pub event_active: Vec<crate::state::ActiveEvent>,
    /// The board rows a body writes raw (`ctx::set_owner` / `ctx::set_houses`
    /// / `ctx::set_mortgaged`, embers).
    pub board_owners: Vec<i32>,
    pub board_houses: Vec<i32>,
    pub board_mortgaged: Vec<bool>,
    pub board_embers: Vec<i32>,
    /// The tile mark list.
    pub marks: Vec<crate::state::TileMark>,
}

impl PassBase {
    /// Undo `own`'s rows on a base just snapshotted from `live`.
    ///
    /// A pass that paused for a host request replays the body after the
    /// request was applied, so the pass's base is the live world **minus** the
    /// guest's own writes: those crossed at the pause that led here and the
    /// commit's revert puts them back through the diff. A row the host
    /// overwrote since keeps the live value -- the reloaded copy carries it
    /// too, so it is the host's, not the guest's (same guard as
    /// [`PlayCtx::restore_guests_to`]). Snapshot owners are additive and stay
    /// on the row.
    ///
    /// Currently unwired: the pass top inlines these rules in
    /// `Run::after_host_inner` (it needs the live base there, not a probe), and
    /// no caller builds a base this way. Keep the arms here in step with
    /// [`GuestOverlay::restore`] -- they are the documented revert for a new
    /// surface either way.
    pub fn revert_guest_overlay(&mut self, own: &GuestOverlay, live: &World) {
        for (i, k, base_v, wrote_v, crossed) in &own.state {
            // A non-crossing increase never sat on live; overriding the
            // routine's value with the base would undo the routine too.
            if !*crossed {
                continue;
            }
            let now = live
                .st
                .players
                .get(*i)
                .and_then(|p| p.state.get(k))
                .copied()
                .unwrap_or_default();
            if now != *wrote_v {
                continue;
            }
            let Some(map) = self.state.get_mut(*i) else {
                continue;
            };
            match base_v {
                Some(v) => {
                    map.insert(k.clone(), *v);
                }
                None => {
                    map.remove(k);
                }
            }
        }
        for (owner, _i, inst) in &own.field.added {
            if let Some(l) = base_field_mut(self, *owner) {
                if let Some(j) = l.iter().position(|f| f.uid == inst.uid) {
                    l.remove(j);
                }
            }
        }
        for (owner, i, inst) in &own.field.removed {
            let Some(l) = base_field_mut(self, *owner) else {
                continue;
            };
            if l.iter().any(|f| f.uid == inst.uid) {
                continue;
            }
            let at = (*i).min(l.len());
            l.insert(at, inst.clone());
        }
        for (uid, base_w, wrote_w) in &own.field.changed {
            for owner in std::iter::once(None).chain((0..live.st.players.len()).map(Some)) {
                let lw = field_list(live, owner);
                let Some(lf) = lw.iter().find(|f| f.uid == *uid) else {
                    continue;
                };
                if !wrote_w.matches(lf) {
                    break;
                }
                if let Some(l) = base_field_mut(self, owner) {
                    if let Some(bf) = l.iter_mut().find(|f| f.uid == *uid) {
                        base_w.write_into(bf);
                    }
                }
                break;
            }
        }
        for (i, name, base_v, wrote_v) in &own.tokens {
            let now = live
                .st
                .players
                .get(*i)
                .and_then(|p| p.tokens.iter().find(|c| c.name == *name));
            if !token_matches(now, wrote_v.as_ref()) {
                continue;
            }
            let Some(l) = self.tokens.get_mut(*i) else {
                continue;
            };
            match base_v {
                Some(v) => match l.iter_mut().find(|c| c.name == *name) {
                    Some(c) => *c = v.clone(),
                    None => l.push(v.clone()),
                },
                None => l.retain(|c| c.name != *name),
            }
        }
        for (i, base_v, wrote_v) in &own.money {
            if let (Some(v), Some(b)) = (live.st.players.get(*i), self.money.get_mut(*i)) {
                if v.money == *wrote_v {
                    *b = *base_v;
                }
            }
        }
        for (i, base_v, wrote_v) in &own.persona {
            if let (Some(v), Some(b)) = (live.st.players.get(*i), self.persona.get_mut(*i)) {
                if v.persona == *wrote_v {
                    *b = base_v.clone();
                }
            }
        }
        for (i, base_v, wrote_v) in &own.board.owners {
            if let (Some(v), Some(b)) = (live.st.owners.get(*i), self.board_owners.get_mut(*i)) {
                if *v == *wrote_v {
                    *b = *base_v;
                }
            }
        }
        for (i, base_v, wrote_v) in &own.board.houses {
            if let (Some(v), Some(b)) = (live.st.houses.get(*i), self.board_houses.get_mut(*i)) {
                if *v == *wrote_v {
                    *b = *base_v;
                }
            }
        }
        for (i, base_v, wrote_v) in &own.board.embers {
            if let (Some(v), Some(b)) = (live.st.embers.get(*i), self.board_embers.get_mut(*i)) {
                if *v == *wrote_v {
                    *b = *base_v;
                }
            }
        }
        for (i, base_v, wrote_v) in &own.board.mortgaged {
            if let (Some(v), Some(b)) =
                (live.st.mortgaged.get(*i), self.board_mortgaged.get_mut(*i))
            {
                if *v == *wrote_v {
                    *b = *base_v;
                }
            }
        }
        if let Some((base_v, wrote_v)) = &own.board.marks {
            if live.st.marks == *wrote_v {
                self.marks = base_v.clone();
            }
        }
        if let Some(d) = &own.events {
            if live.event_deck == d.wrote_deck {
                self.event_deck = d.base_deck.clone();
            }
            if live.st.event_top == d.wrote_top {
                self.event_top = d.base_top.clone();
            }
            if live.st.event_active == d.wrote_active {
                self.event_active = d.base_active.clone();
            }
            if live.event_discard == d.wrote_discard {
                self.event_discard = d.base_discard.clone();
            }
            if live.event_removed == d.wrote_removed {
                self.event_removed = d.base_removed.clone();
            }
        }
        for (i, d) in &own.piles {
            let (Some(h), Some(b)) = (live.hidden.get(*i), self.piles.get_mut(*i)) else {
                continue;
            };
            if let Some((base_v, wrote_v)) = &d.hand {
                if h.hand == *wrote_v {
                    b.0 = base_v.clone();
                }
            }
            if let Some((base_v, wrote_v)) = &d.deck {
                if h.deck == *wrote_v {
                    b.1 = base_v.clone();
                }
            }
            if let Some((base_v, wrote_v)) = &d.grave {
                if h.graveyard == *wrote_v {
                    b.2 = base_v.clone();
                }
            }
            if let Some((base_v, wrote_v)) = &d.next_steps {
                if h.next_steps == *wrote_v {
                    if let Some(bv) = self.next_steps.get_mut(*i) {
                        *bv = *base_v;
                    }
                }
            }
        }
        for d in &own.roll_picks {
            let Some(r) = live.open_rolls.iter().find(|r| r.id == d.id) else {
                continue;
            };
            let Some(b) = self.rolls.iter_mut().find(|r| r.0 == d.id) else {
                continue;
            };
            if r.chosen_index == d.chosen_index.1 {
                b.1 = d.chosen_index.0;
            }
            if r.unused_face == d.unused_face.1 {
                b.2 = d.unused_face.0;
            }
            if r.dirty == d.dirty.1 {
                b.3 = d.dirty.0;
            }
        }
        if let Some((base_v, wrote_v)) = &own.scheduled {
            if live.scheduled == *wrote_v {
                self.scheduled = base_v.clone();
            }
        }
    }
}

/// The identity of every row kind in a revert stack: `(kind, index, key, uid)`.
/// A surface is a single write however many rows name it, and reverting a
/// second row for the same surface puts the first one's base back, so the
/// three-way guard then adds the write again.
type SeenRows = std::collections::BTreeSet<(u8, usize, String, i32)>;

/// Keep only the topmost row per surface in `o`, against the rows an unwind
/// already reverted (`seen`). Whole-list rows (field ties, tile marks, the
/// turn-end list) drop entirely when an earlier row already carried them.
fn retain_unseen_overlay_rows(seen: &mut SeenRows, o: &mut GuestOverlay) {
    o.state
        .retain(|(i, k, _, _, _)| seen.insert((0, *i, k.clone(), 0)));
    o.field
        .added
        .retain(|(ow, _, f)| seen.insert((1, 0, format!("{ow:?}"), f.uid)));
    o.field
        .removed
        .retain(|(ow, _, f)| seen.insert((2, 0, format!("{ow:?}"), f.uid)));
    o.field
        .changed
        .retain(|(uid, _, _)| seen.insert((3, 0, String::new(), *uid)));
    // `o.tokens` is not filtered: its rows chain (see
    // `capture_guest_tokens_from`), so every one of them is a real step.
    o.money
        .retain(|(i, _, _)| seen.insert((5, *i, String::new(), 0)));
    o.persona
        .retain(|(i, _, _)| seen.insert((16, *i, String::new(), 0)));
    o.board
        .owners
        .retain(|(t, _, _)| seen.insert((6, *t, String::new(), 0)));
    o.board
        .houses
        .retain(|(t, _, _)| seen.insert((7, *t, String::new(), 0)));
    o.board
        .mortgaged
        .retain(|(t, _, _)| seen.insert((8, *t, String::new(), 0)));
    o.board
        .embers
        .retain(|(t, _, _)| seen.insert((9, *t, String::new(), 0)));
    if o.board.marks.is_some() && !seen.insert((10, 0, String::new(), 0)) {
        o.board.marks = None;
    }
    o.owners
        .retain(|(k, u)| seen.insert((11, *k as usize, String::new(), *u)));
    // `o.piles` is not filtered either: its rows chain against live the same
    // way the token rows do.
    o.roll_picks
        .retain(|d| seen.insert((13, 0, String::new(), d.id)));
    if o.scheduled.is_some() && !seen.insert((14, 0, String::new(), 0)) {
        o.scheduled = None;
    }
    if o.events.is_some() && !seen.insert((15, 0, String::new(), 0)) {
        o.events = None;
    }
}

/// Undo one crossed [`GuestOverlay`] on `w`, every row guarded three-way: a
/// surface something else (a host routine, a nested drive) moved since the
/// crossing keeps its value, and only the writer's own value is put back.
///
/// Token, pile and field rows carry `(live, wrote)` from their own boundary
/// rather than a pass base (see `capture_guest_tokens_from`), so a revert
/// undoes exactly one step and the reverse-order unwind composes with the
/// body's re-run instead of double-counting it.
///
/// Used by the pass top (on live) and by [`PlayCtx::pass_base_for`] (on a
/// clone, unwinding the overlay stack in reverse order), so the two see the
/// same world after the same rows.
///
/// Answers the field uids it put back, for the caller's `reinserted` set.
fn revert_overlay_on(w: &mut World, o: GuestOverlay) -> Vec<i32> {
    let GuestOverlay {
        state,
        field,
        tokens,
        piles,
        roll_picks,
        scheduled,
        money,
        persona,
        board,
        events,
        rule_bonus,
        frames,
        ..
    } = o;
    if let Some(d) = events {
        d.restore(w);
    }
    board.restore(w);
    if let Some((base, wrote)) = scheduled {
        ScheduledDelta { base, wrote }.restore(&mut w.scheduled);
    }
    for (name, base, wrote) in &rule_bonus {
        set_rule_bonus(w, name, *wrote, *base);
    }
    for (i, b, wrote) in money {
        if let Some(p) = w.st.players.get_mut(i) {
            if p.money == wrote {
                p.money = b;
            }
        }
    }
    for (i, b, wrote) in persona {
        if let Some(p) = w.st.players.get_mut(i) {
            if p.persona == wrote {
                p.persona = b;
            }
        }
    }
    for (i, d) in piles {
        if let Some(h) = w.hidden.get_mut(i) {
            d.restore(h);
        }
    }
    for d in frames {
        if let Some(f) = w.inspection_frames.iter_mut().find(|f| f.id == d.id) {
            if f.cards == d.wrote {
                f.cards = d.base;
            }
        }
    }
    for d in roll_picks {
        if let Some(r) = w.open_rolls.iter_mut().find(|r| r.id == d.id) {
            d.restore(r);
        }
    }
    for (i, k, base_v, wrote_v, crossed) in state {
        // A boundary increase the routine also moved never landed on live: the
        // routine's own value there is already the whole truth, and the delta
        // arithmetic below would subtract an increase live never carried.
        if !crossed {
            continue;
        }
        let now =
            w.st.players
                .get(i)
                .and_then(|p| p.state.get(&k).copied())
                .unwrap_or_default();
        let Some(p) = w.st.players.get_mut(i) else {
            continue;
        };
        // The routine left it (`now == wrote_v`), or the re-run already put the
        // base back (`now == base_v`, the body wrote the key again before this
        // revert): in both cases the world wants the base, not a delta.
        let back_at_base = match base_v {
            Some(b) => b.value == now.value,
            // An absent key reads as its `0` default.
            None => now.value == 0,
        };
        if now == wrote_v || back_at_base {
            // The routine left it: restore the base.
            match base_v {
                Some(v) => {
                    // Same floor as any other write: `state_set` honours
                    // `NEVER_NEGATIVE`. Install the pre-overlay item's
                    // declaration first.
                    if v.flags != 0 {
                        p.state_set_flags(&k, v.flags);
                    }
                    if let Some(exp) = v.expires {
                        p.state_set_expires(&k, Some(exp));
                    }
                    p.state_set(&k, v.value);
                }
                None => {
                    p.state.remove(&k);
                }
            }
        } else {
            // The routine changed it too: keep only its delta. The
            // reconstruction `base + (now - wrote)` can land negative --
            // route it through `state_set` so the writer-declared
            // `NEVER_NEGATIVE` floor applies.
            let mut v = now;
            v.value = (i64::from(base_v.map(|b| b.value).unwrap_or(0))
                + (i64::from(now.value) - i64::from(wrote_v.value)))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            // `now` may be a bare default (the routine removed the key).
            // Rebuild the declaration from the pre-overlay item so
            // `state_set` still applies `NEVER_NEGATIVE`.
            let flags = if v.flags != 0 {
                v.flags
            } else {
                base_v.map(|b| b.flags).unwrap_or(0)
            };
            let expires = v.expires.or_else(|| base_v.and_then(|b| b.expires));
            if flags != 0 {
                p.state_set_flags(&k, flags);
            }
            if let Some(exp) = expires {
                p.state_set_expires(&k, Some(exp));
            }
            p.state_set(&k, v.value);
        }
    }
    let reinserted = field.restore(w);
    for (i, name, base, wrote) in &tokens {
        write_token_row(w, *i, name, wrote.as_ref(), base.as_ref());
    }
    reinserted
}

/// Mutable field list of a [`PassBase`] (the board's or a player's).
fn base_field_mut<'b>(
    base: &'b mut PassBase,
    owner: Option<usize>,
) -> Option<&'b mut Vec<crate::state::FieldCard>> {
    match owner {
        Some(i) => base.field.get_mut(i),
        None => Some(&mut base.board_field),
    }
}

/// The per-instance attributes a guest body can write directly (`ctx::set_tile_at`,
/// `ctx::set_prop`, `ctx::counter`, `ctx::set_face_down`), captured as a
/// three-way merge against the pass base.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldWrite {
    pub tile: i32,
    pub face_down: bool,
    /// The row's attachment host (an attach/re-parent is a direct write to
    /// `on`, `ctx::attach_card`'s settle). Without it a crossing put the row
    /// back at `-1` and the next request's cycle guard saw a flat field.
    #[serde(default)]
    pub on: i32,
    /// The row's owner (`ctx::set_instance_owner` -- 「视为拥有」, a rehome to
    /// another seat). Without it a crossing dropped the transfer and left the
    /// row with its old holder.
    #[serde(default)]
    pub owner: i32,
    /// The row's 未生效 bit (`ctx::set_ineffective_at`). A leaving row's
    /// `discarded` raise reads it (`wasted`), so it has to cross like the
    /// rest of the row.
    #[serde(default = "field_write_default_true")]
    pub effective: bool,
    #[serde(default)]
    pub counters: std::collections::BTreeMap<String, i32>,
    #[serde(default)]
    pub props: std::collections::BTreeMap<String, i32>,
}

fn field_write_default_true() -> bool {
    true
}

impl FieldWrite {
    fn of(f: &crate::state::FieldCard) -> Self {
        FieldWrite {
            tile: f.tile,
            face_down: f.face_down,
            on: f.on,
            owner: f.owner,
            effective: f.effective,
            counters: f.counters.clone(),
            props: f.props.clone(),
        }
    }

    fn matches(&self, f: &crate::state::FieldCard) -> bool {
        self.tile == f.tile
            && self.face_down == f.face_down
            && self.on == f.on
            && self.owner == f.owner
            && self.effective == f.effective
            && self.counters == f.counters
            && self.props == f.props
    }

    fn write_into(&self, f: &mut crate::state::FieldCard) {
        f.tile = self.tile;
        f.face_down = self.face_down;
        f.on = self.on;
        f.owner = self.owner;
        f.effective = self.effective;
        f.counters = self.counters.clone();
        f.props = self.props.clone();
    }
}

/// Direct guest writes to the **field**: instances placed, taken off, or
/// rewritten in place. `None` in the owner slot is the board field, `Some(i)`
/// player `i`'s field.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldDelta {
    /// `(owner, index, instance)` -- positioned at the guest's index on apply.
    pub added: Vec<(Option<usize>, usize, crate::state::FieldCard)>,
    /// `(owner, index, instance)` -- the instance carries the pre-overlay copy
    /// so a restore can put it back.
    pub removed: Vec<(Option<usize>, usize, crate::state::FieldCard)>,
    /// `(uid, base, wrote)` -- the base is live's write at the boundary, like
    /// the token and pile rows.
    pub changed: Vec<(i32, FieldWrite, FieldWrite)>,
}

fn base_of<'a>(
    base: &'a PassBase,
    owner: Option<usize>,
    uid: i32,
) -> Option<&'a crate::state::FieldCard> {
    match owner {
        Some(i) => base.field.get(i)?.iter().find(|f| f.uid == uid),
        None => base.board_field.iter().find(|f| f.uid == uid),
    }
}

fn field_list(w: &World, owner: Option<usize>) -> &[crate::state::FieldCard] {
    match owner {
        Some(i) => w.st.players.get(i).map(|p| &p.field[..]).unwrap_or(&[]),
        None => &w.st.board_field,
    }
}

fn field_list_mut(
    w: &mut World,
    owner: Option<usize>,
) -> Option<&mut Vec<crate::state::FieldCard>> {
    match owner {
        Some(i) => Some(&mut w.st.players.get_mut(i)?.field),
        None => Some(&mut w.st.board_field),
    }
}

impl FieldDelta {
    /// Write the guest's field structure onto `w`. A rewrite only lands when
    /// the live instance still matches the pass base -- where the host routine
    /// wrote the same instance, its value wins (the state rule).
    pub fn apply(&self, w: &mut World) {
        for (owner, _i, uid) in self.removed.iter().map(|(o, i, f)| (o, i, f.uid)) {
            if let Some(l) = field_list_mut(w, *owner) {
                if let Some(j) = l.iter().position(|f| f.uid == uid) {
                    l.remove(j);
                }
            }
        }
        for (uid, base, wrote) in &self.changed {
            for owner in std::iter::once(None).chain((0..w.st.players.len()).map(Some)) {
                let Some(l) = field_list_mut(w, owner) else {
                    continue;
                };
                if let Some(f) = l.iter_mut().find(|f| f.uid == *uid) {
                    if base.matches(f) {
                        wrote.write_into(f);
                    }
                    break;
                }
            }
        }
        for (owner, i, inst) in &self.added {
            let Some(l) = field_list_mut(w, *owner) else {
                continue;
            };
            if l.iter().any(|f| f.uid == inst.uid) {
                continue;
            }
            let at = (*i).min(l.len());
            l.insert(at, inst.clone());
        }
        w.st.next_card_uid = w.st.next_card_uid.max(
            self.added
                .iter()
                .map(|(_, _, f)| f.uid + 1)
                .max()
                .unwrap_or(0),
        );
        w.sync_active_event_mirrors();
    }

    /// The restore twin: put the pre-overlay field back, keeping a host write
    /// that replaced the guest's. Answers the uids it put back for a re-run's
    /// reads: a replay that never takes one out again must not leave the row
    /// behind at the commit (`PlayCtx::reinserted`).
    pub fn restore(&self, w: &mut World) -> Vec<i32> {
        for (owner, _i, inst) in &self.added {
            if let Some(l) = field_list_mut(w, *owner) {
                if let Some(j) = l.iter().position(|f| f.uid == inst.uid) {
                    l.remove(j);
                }
            }
        }
        for (uid, base, wrote) in &self.changed {
            for owner in std::iter::once(None).chain((0..w.st.players.len()).map(Some)) {
                let Some(l) = field_list_mut(w, owner) else {
                    continue;
                };
                if let Some(f) = l.iter_mut().find(|f| f.uid == *uid) {
                    if wrote.matches(f) {
                        base.write_into(f);
                    }
                    break;
                }
            }
        }
        let mut reinserted = Vec::new();
        for (owner, i, inst) in &self.removed {
            let Some(l) = field_list_mut(w, *owner) else {
                continue;
            };
            if l.iter().any(|f| f.uid == inst.uid) {
                continue;
            }
            let at = (*i).min(l.len());
            l.insert(at, inst.clone());
            reinserted.push(inst.uid);
        }
        w.sync_active_event_mirrors();
        reinserted
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

fn token_matches(
    live: Option<&crate::state::Counter>,
    want: Option<&crate::state::Counter>,
) -> bool {
    match (live, want) {
        (None, None) => true,
        (Some(l), Some(b)) => l.value == b.value && l.instance == b.instance && l.max == b.max,
        _ => false,
    }
}

/// One held-counter row transition; a `None` side is an absent row. `from` is
/// the side the live row must still match, `to` what lands.
fn write_token_row(
    w: &mut World,
    i: usize,
    name: &str,
    from: Option<&crate::state::Counter>,
    to: Option<&crate::state::Counter>,
) {
    let Some(p) = w.st.players.get_mut(i) else {
        return;
    };
    if !token_matches(p.tokens.iter().find(|c| c.name == name), from) {
        return;
    }
    match to {
        Some(v) => match p.tokens.iter_mut().find(|c| c.name == name) {
            Some(c) => *c = v.clone(),
            None => p.tokens.push(v.clone()),
        },
        None => p.tokens.retain(|c| c.name != name),
    }
}

impl GuestOverlay {
    /// Apply only the recorded **state** rows: the copy rebuilt after a host
    /// request comes from live, which never carried the non-crossing
    /// increases, and the body reads them back.
    pub fn apply_state(&self, w: &mut World) {
        for (i, k, _base, wrote, _) in &self.state {
            let Some(p) = w.st.players.get_mut(*i) else {
                continue;
            };
            if p.state.get(k).map(|v| v.value) == Some(wrote.value) {
                continue;
            }
            if wrote.flags != 0 {
                p.state_set_flags(k, wrote.flags);
            }
            if let Some(exp) = wrote.expires {
                p.state_set_expires(k, Some(exp));
            }
            p.state_set(k, wrote.value);
        }
    }

    /// Re-apply the recorded decreases to `w` without recomputing them --
    /// the restore twin of [`PlayCtx::overlay_guest_state`], for the journal's
    /// re-application path. `w` is expected to be the request's pre-state
    /// (`HostJournalEntry::condition`).
    pub fn apply(&self, w: &mut World) {
        for (name, base, wrote) in &self.rule_bonus {
            set_rule_bonus(w, name, *base, *wrote);
        }
        for (i, name, base, wrote) in &self.tokens {
            write_token_row(w, *i, name, base.as_ref(), wrote.as_ref());
        }
        for (i, k, _base, wrote, _) in &self.state {
            let Some(p) = w.st.players.get_mut(*i) else {
                continue;
            };
            if wrote.flags != 0 {
                p.state_set_flags(k, wrote.flags);
            }
            if let Some(exp) = wrote.expires {
                p.state_set_expires(k, Some(exp));
            }
            p.state_set(k, wrote.value);
        }
        self.field.apply(w);
        self.board.apply(w);
        if let Some(d) = &self.events {
            d.apply(w);
        }
        for (i, base, wrote) in &self.money {
            if let Some(p) = w.st.players.get_mut(*i) {
                // A host move since the crossing is kept and the guest's own
                // delta composed onto it, as at capture time.
                p.money = if p.money == *base {
                    *wrote
                } else if p.money == *wrote {
                    p.money
                } else {
                    p.money + (*wrote - *base)
                };
            }
        }
        for (i, base, wrote) in &self.persona {
            if let Some(p) = w.st.players.get_mut(*i) {
                // Absolute: a face someone else moved to since the crossing is
                // theirs, not the guest's.
                if p.persona == *base {
                    p.persona = wrote.clone();
                }
            }
        }
        if let Some((base, wrote)) = &self.scheduled {
            ScheduledDelta {
                base: base.clone(),
                wrote: wrote.clone(),
            }
            .apply(&mut w.scheduled);
        }
        for (i, d) in &self.piles {
            if let Some(h) = w.hidden.get_mut(*i) {
                d.apply(h);
            }
        }
        for d in &self.roll_picks {
            if let Some(r) = w.open_rolls.iter_mut().find(|r| r.id == d.id) {
                d.apply(r);
            }
        }
        for d in &self.frames {
            if let Some(f) = w.inspection_frames.iter_mut().find(|f| f.id == d.id) {
                if f.cards == d.base {
                    f.cards = d.wrote.clone();
                }
            }
        }
        for (key, uid) in &self.owners {
            let owners = w.world_snapshot_owners.entry(*key).or_default();
            if !owners.contains(uid) {
                owners.push(*uid);
            }
        }
    }

    /// The restore twin of [`Self::apply`] (host routines may have written the
    /// same surfaces meanwhile; those writes stay). Delegates to the one
    /// revert implementation so the two twins cannot drift; `owners` stay,
    /// as they do on `apply`.
    pub fn restore(&self, w: &mut World) {
        revert_overlay_on(w, self.clone());
    }
}

impl<'a> PlayCtx<'a> {
    /// Press a skill button: run the rule's `On::Play` entry under its own id.
    ///
    /// A skill is a card rule -- it just lives on the player's field rather than
    /// in a hand -- so the card's play entry answers for it. Nothing moves: there
    /// is no hand card to spend and no destination to resolve, unlike playing a
    /// card. The gate is not re-checked here; `cant_act` has already asked the
    /// rule's `cant_play` and refused the press if it named a reason, which is
    /// the same shape as every other turn action.
    /// A copy of the replayable world (a rules host runs against one).
    /// The innermost settle's facts, or `None` outside a settle (G10 §6.1).
    /// Re-derived from the frame on every pass, like `settle_tile`.
    pub(crate) fn settle_facts(&self) -> Option<crate::engine::SettleFacts> {
        self.settle_stack
            .last()
            .map(|e| crate::engine::SettleFacts {
                cancelled: e.outcome == SettleOutcome::Cancelled && !e.body_replaced,
                body_replaced: e.body_replaced,
                paid: e.paid,
                paid_value: e.paid_value,
            })
    }

    pub fn world_copy(&self) -> World {
        #[cfg(feature = "bot-cost")]
        crate::engine::bot_cost::WORLD_CLONES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.w.deep_clone()
    }

    /// Immutable view of the live world -- **no clone**. Read-only queries
    /// (`out`, `hand_of`, the counteract pre-filter's window snapshot) go
    /// through this instead of [`Self::world_copy`].
    pub fn world(&self) -> &World {
        &self.w
    }

    /// Mutable view of the live world -- **no clone**. A policy context draws
    /// its randomness through this ([`World::rng`]); see [`Self::world`].
    pub fn world_mut(&mut self) -> &mut World {
        // The play memos key on the world stamp; drop them so the next probe
        // re-asks rather than trusting a verdict from before the write.
        self.play_memo.borrow_mut().clear();
        &mut self.w
    }

    /// A shareable handle to the live world -- a refcount bump, **no copy**
    /// (fix B). A pure check (`cant_play` probe, guard body, buy quote) runs
    /// against it; the first write inside the check's sandbox detaches a
    /// private copy (`Arc::make_mut`) and the live world is untouched. Prefer
    /// this over [`Self::world_copy`] whenever the consumer only reads, or
    /// writes into a throwaway sandbox.
    pub fn share_world(&self) -> crate::engine::world::SharedWorld {
        #[cfg(feature = "bot-cost")]
        crate::engine::bot_cost::WORLD_SHARES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        uid: i32,
    ) -> i32 {
        self.w.flush_walk();
        let e = self
            .w
            .card_activation(kind, owner, card, target, tile, negated, msg);
        e.parent = parent;
        e.uid = uid;
        e.id
    }

    /// Replace the message of an already-posted event (a re-run's correction).
    /// `false` when the id is gone (truncated tail) -- the caller then appends.
    pub fn relog_event(&mut self, id: i32, msg: Msg) -> bool {
        self.w.recent.replace_msg(id, msg)
    }

    /// Set `value` on already-posted events (a `"dice"` face, a `"charge"` spend).
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

    /// Set the `"card"` activation's flash flag (Ruling 1: owner-self-only
    /// activations keep the log entry but skip the card animation). No-op
    /// when the id is gone.
    pub fn set_event_flash(&mut self, id: i32, flash: bool) {
        let mut evs: Vec<crate::state::MatchEvent> = self.w.recent.iter().cloned().collect();
        let Some(e) = evs.iter_mut().find(|e| e.id == id) else {
            return;
        };
        e.flash = flash;
        let mut tail = crate::engine::world::EventTail::default();
        for e in evs {
            tail.push_back(e);
        }
        self.w.recent = tail;
    }

    /// Stamp the pile code on an event just posted (`play_post_value` leaves
    /// `to` unset). `false` when the id is no longer in the tail -- harmless,
    /// the reveal overlay still carries the pile.
    pub fn set_event_to(&mut self, id: i32, to: i32) -> bool {
        let mut evs: Vec<crate::state::MatchEvent> = self.w.recent.iter().cloned().collect();
        let Some(e) = evs.iter_mut().find(|e| e.id == id) else {
            return false;
        };
        e.to = to;
        let mut tail = crate::engine::world::EventTail::default();
        for e in evs {
            tail.push_back(e);
        }
        self.w.recent = tail;
        true
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
    /// Every `latch:` row of the live world (`World::latches`). A world
    /// restore is an undo of what the body derived; a latch is not derived, so
    /// it is taken out before the swap and laid back after.
    pub fn take_latches(&mut self) -> Vec<(usize, String, crate::state::StateVar)> {
        self.w.latches()
    }

    /// Lay [`Self::take_latches`] rows back over the restored world.
    pub fn put_latches(&mut self, rows: Vec<(usize, String, crate::state::StateVar)>) {
        self.w.put_latches(rows);
    }

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
        // The placement record is read when the play completes, on the live
        // world, while the body's `place_card` wrote it on the copy and the
        // play may pause between the two (G06 §2.15).
        t.placed_during_play = f.placed_during_play;
        // 规则书（3）: the waste bit rides the raise a `Leave` makes on the
        // live world (`discarded`'s `wasted`), so it has to cross with the
        // other turn policy rather than wait for the commit.
        t.play_ineffective = f.play_ineffective;
        t.no_money_loss = f.no_money_loss.clone();
        // The movement plan a card shaped just before pausing for a host
        // routine (`plan::set_pay_factor` / `set_rent_factor` / `set_can_build`
        // ) lives on the card's world copy; the routine
        // runs against the live world, so the knobs have to cross too --
        // otherwise 练习室里的风暴's (4-X)/4 and Repaint's 「支付减半」 are
        // dropped at the `SettleAt` / `Move` boundary.
        // `resolve` is re-read by `teleport_as_depth` after the pass window and
        // gates the settle stages. A 「不[触发结算]」 body clears it *before*
        // the `teleport_to` request (纯真振翅), so it must be on the live plan
        // at the request, not left stale from an earlier move.
        t.plan.settles = f.plan.settles;
        t.plan.pay_factor = f.plan.pay_factor;
        if crate::engine::stage_dbg() {
            eprintln!(
                "ADOPT rent_factor {} -> {} pay {} -> {}",
                t.plan.rent_factor, f.plan.rent_factor, t.plan.pay_factor, f.plan.pay_factor
            );
        }
        t.plan.rent_factor = f.plan.rent_factor;
        // The factor ledger crosses: the host routine runs against the live
        // world, and the re-run of this body must not compose its prefix's
        // writes a second time.
        t.factor_applied = f.factor_applied.clone();
        t.factor_gen = f.factor_gen;
        // The one-shot scopes are read by the *next* settle / move a routine
        // starts (`settle_tile_at`, `move`), which runs after the body returns.
        // A `NextSettle` / `NextMove` write made in the body lives on its copy
        // and has to cross like the plan knobs, or the factor is dropped at the
        // pause boundary.
        t.next_settle_pay_factor = f.next_settle_pay_factor;
        t.next_move_pay_factor = f.next_move_pay_factor;
        t.plan.can_build = f.plan.can_build;
        t.plan.no_buy = f.plan.no_buy;
        t.plan.tags = f.plan.tags.clone();
        // A card body may latch a decision across the pauses its host routines
        // cause -- e.g. 黑色生日 freezing 「每名你以外的资金不多于1000的玩家」 at
        // resolution so the replay's re-run takes the same branch across its
        // two pay entries.
        // That write lives on the card's world copy and is dropped when the run
        // pauses. Carry it over -- but *only* for keys a body explicitly
        // registers under the `latch.` prefix. (A latch is "already taken", so
        // the replay's re-write is a no-op and nothing double-counts.)
        for (to, from_p) in self.w.st.players.iter_mut().zip(from.st.players.iter()) {
            for (k, v) in &from_p.state {
                if !k.starts_with("latch:") {
                    continue;
                }
                // Only a *delta* (absent live key, or a different value):
                // a re-run re-writes the same latch and must stay a no-op.
                if to.state.get(k).map(|x| x.value) != Some(v.value) {
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
    /// (e.g. 壱雫空 zeroing a layer and then paying) has to be visible to
    /// `can_pay` -- without being *persisted*, because the body replays from
    /// the top and would re-apply the write (double-counting an additive one).
    /// [`Self::restore_guests_to`] drops it again, keeping only the routine's
    /// own effects; the replay then re-applies the guest writes.
    ///
    /// Only **decreases** cross (「清除」). A status the body itself just
    /// *applied* (心の雨's fallback the layer grant then `gain 1000`) must not gate
    /// its own follow-on money -- 「无法收付款」 blocking a card's own gain to a
    /// player it just gated is an open ruling, and the pre-overlay behaviour
    /// stands until it is decided. Money is not overlaid at all: a `gain_fixed`
    /// is re-applied on the replay and the pipeline's own moves are routine
    /// effects, not guest writes.
    /// Adopt the guest copy's move tags before handling a paused host request
    /// on the live world. A body's `plan::set_tag` lands on its own copy, while
    /// a nested settle's guards read the *live* plan (`card_settle_move` clones
    /// it), so without this a redirect tag written right before the settle is
    /// lost (`spec/abi/G04-move-plan.md` §4.3).
    ///
    /// Only the tags cross. Overlaying the whole frame would also move the
    /// `*_plan` outcome lines' baseline: those compare against the guest copy's
    /// own `was` (`plan::set_stop_at` etc.), and a live frame already carrying
    /// the replay's earlier write would suppress the `log.outcome.*` line.
    pub fn overlay_guest_plan_tags(&mut self, from: &World) {
        if let (Some(dst), Some(src)) = (
            self.w.turn.plan_stack.last_mut(),
            from.turn.plan_stack.last(),
        ) {
            dst.plan.tags = src.plan.tags.clone();
        } else {
            self.w.turn.plan.tags = from.turn.plan.tags.clone();
        }
    }

    /// The guest's own writes since the pass base, as an overlay. Computed
    /// against the **pass base**, not the current live world: a surface the
    /// host routine has written since the pass started is not the guest's, but
    /// the record still carries the guest's value so it compares equal to a
    /// live write of the same value and never clobbers one.
    pub fn capture_guest_overlay(&mut self, base: &PassBase, from: &World) -> GuestOverlay {
        GuestOverlay {
            state: self.capture_guest_state_from(&base.state, from, true).state,
            field: self.capture_guest_field_from(base, from),
            tokens: self.capture_guest_tokens_from(base, from),
            owners: self.capture_guest_snapshot_owners_from(base, from),
            piles: self.capture_guest_piles_from(base, from),
            roll_picks: self.capture_guest_roll_picks_from(base, from),
            rule_bonus: self.capture_guest_rule_bonus_from(from),
            scheduled: self.capture_guest_scheduled_from(base, from),
            events: self.capture_guest_events_from(base, from),
            money: self.capture_guest_money_from(base, from),
            persona: self.capture_guest_persona_from(base, from),
            board: self.capture_guest_board_from(base, from),
            frames: self.capture_guest_frames_from(from),
        }
    }

    pub fn overlay_guest_state(
        &mut self,
        base: &PassBase,
        from: &World,
        drive: u32,
    ) -> GuestOverlay {
        let o = self.capture_guest_overlay(base, from);
        self.guest_overlays.push((drive, o.clone()));
        o
    }

    /// Mint a drive id. A drive takes one at its start and passes it to every
    /// push and restore, so a pass restart tells its own crossings from a
    /// nested drive's. A nested drive runs on a context of its own, so the
    /// counter is process-wide; only equality within one overlay stack matters.
    pub fn next_drive_seq(&mut self) -> u32 {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Every player's state map, cloned -- the pass-start base of a guest-state
    /// diff ([`Self::capture_guest_state_from`]).
    pub fn state_maps(&self) -> Vec<std::collections::BTreeMap<String, crate::state::StateVar>> {
        self.w.st.players.iter().map(|p| p.state.clone()).collect()
    }

    /// The pass-start world surfaces a guest world is diffed against
    /// ([`Self::capture_guest_state_from`] / [`Self::capture_guest_field_from`]).
    pub fn pass_base(&self) -> PassBase {
        Self::pass_base_of(&self.w)
    }

    /// The diff base a drive's run re-bases onto after a consumed request:
    /// the world as it stands **minus the guest writes the drive itself has
    /// crossed above `floor`**. A pass top reverts exactly those, so a base
    /// that kept them would see the commit's re-application as a delta of
    /// zero and drop the write. Host effects (a routine's own, a nested
    /// drive's commit) are not on the overlay stack and stay in the base.
    pub fn pass_base_for(&self, floor: usize, drive: u32) -> PassBase {
        // The unwound world must be the one the pass top will leave: undo the
        // drive's own crossings in reverse on a clone, each guarded against
        // the clone as it goes (a write the re-run crossed after it is undone
        // first, so the earlier row is visible again). Guarding every row
        // against **live** instead would skip an earlier crossing whose
        // surface a later crossing still covers -- the base then keeps the
        // later value and the pass top, which does undo both, lands one step
        // below it.
        let mut w = self.w.deep_clone();
        let mut seen = SeenRows::new();
        for (d, o) in self.guest_overlays[floor.min(self.guest_overlays.len())..]
            .iter()
            .rev()
        {
            if *d != drive {
                continue;
            }
            let mut o = o.clone();
            retain_unseen_overlay_rows(&mut seen, &mut o);
            revert_overlay_on(&mut w, o);
        }
        Self::pass_base_of(&w)
    }

    /// The same snapshot taken of an arbitrary world (a request's pre-state
    /// `condition`, which is the base a consumed request's guest diff is
    /// measured against).
    pub fn pass_base_of(w: &World) -> PassBase {
        PassBase {
            state: w.st.players.iter().map(|p| p.state.clone()).collect(),
            field: w.st.players.iter().map(|p| p.field.clone()).collect(),
            board_field: w.st.board_field.clone(),
            tokens: w.st.players.iter().map(|p| p.tokens.clone()).collect(),
            owners: w
                .world_snapshot_owners
                .iter()
                .map(|(key, owners)| (*key, owners.clone()))
                .collect(),
            money: w.st.players.iter().map(|p| p.money).collect(),
            persona: w.st.players.iter().map(|p| p.persona.clone()).collect(),
            piles: w
                .hidden
                .iter()
                .map(|h| (h.hand.clone(), h.deck.clone(), h.graveyard.clone()))
                .collect(),
            next_steps: w.hidden.iter().map(|h| h.next_steps).collect(),
            rolls: w
                .open_rolls
                .iter()
                .map(|r| (r.id, r.chosen_index, r.unused_face, r.dirty))
                .collect(),
            scheduled: w.scheduled.clone(),
            event_deck: w.event_deck.clone(),
            event_top: w.st.event_top.clone(),
            event_discard: w.event_discard.clone(),
            event_removed: w.event_removed.clone(),
            event_active: w.st.event_active.clone(),
            board_owners: w.st.owners.clone(),
            board_houses: w.st.houses.clone(),
            board_mortgaged: w.st.mortgaged.clone(),
            board_embers: w.st.embers.clone(),
            marks: w.st.marks.clone(),
        }
    }

    /// Capture the private-pile writes the guest made against the pass base
    /// and carry them into `self.w`. A pile list the host routine is also
    /// writing is only crossed where the guest's copy differs from the base;
    /// the list is taken whole because a pile is an ordered list, not a
    /// keyed row.
    /// Capture the guest's board-row writes against the pass base and carry
    /// them into `self.w`.
    pub fn capture_guest_board_from(&mut self, base: &PassBase, from: &World) -> Box<BoardDelta> {
        let mut d = Box::<BoardDelta>::default();
        let n = base.board_owners.len().min(from.st.owners.len());
        for i in 0..n {
            if base.board_owners[i] != from.st.owners[i] {
                d.owners.push((i, base.board_owners[i], from.st.owners[i]));
            }
            if base.board_houses[i] != from.st.houses[i] {
                d.houses.push((i, base.board_houses[i], from.st.houses[i]));
            }
            if base.board_mortgaged[i] != from.st.mortgaged[i] {
                d.mortgaged
                    .push((i, base.board_mortgaged[i], from.st.mortgaged[i]));
            }
            if base.board_embers[i] != from.st.embers[i] {
                d.embers.push((i, base.board_embers[i], from.st.embers[i]));
            }
        }
        if base.marks != from.st.marks {
            d.marks = Some((base.marks.clone(), from.st.marks.clone()));
        }
        d.apply(&mut self.w);
        d
    }

    /// Capture the guest's raw money writes against the pass base and carry
    /// them into `self.w` (a pipeline move already landed live and differs
    /// from the base; the delta is then kept on top).
    pub fn capture_guest_money_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<(usize, i32, i32)> {
        let mut out = Vec::new();
        let n = base.money.len().min(from.st.players.len());
        for i in 0..n {
            let b = base.money[i];
            let wrote = from.st.players[i].money;
            if b == wrote {
                continue;
            }
            if let Some(p) = self.w.st.players.get_mut(i) {
                let live = p.money;
                p.money = if live == b {
                    wrote
                } else if live == wrote {
                    live
                } else {
                    live + (wrote - b)
                };
            }
            out.push((i, b, wrote));
        }
        out
    }

    /// Capture the guest's persona writes against the pass base and carry them
    /// into `self.w`. Nothing when the copy holds the face live already has.
    ///
    /// A face is an absolute name, so there is no delta to add: the guest's
    /// newest word replaces live's, and the row records the value it replaced
    /// as its base. The row is therefore measured against **live now**, not the
    /// pass base -- a body that writes a face, pauses, and writes another
    /// (`A -> B`, request, `A`) has a guest copy equal to the pass base at the
    /// second pause, and the pass-base comparison would skip the write while
    /// live still held `B`. No host routine ever writes a persona, so a live
    /// face differing from the pass base is always this drive's own earlier
    /// crossing.
    pub fn capture_guest_persona_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<(usize, String, String)> {
        let mut out = Vec::new();
        let n = base.persona.len().min(from.st.players.len());
        for i in 0..n {
            let wrote = &from.st.players[i].persona;
            let Some(p) = self.w.st.players.get_mut(i) else {
                continue;
            };
            // The unchanged case (most seats, most requests) clones nothing.
            if p.persona == *wrote {
                continue;
            }
            let b = std::mem::replace(&mut p.persona, wrote.clone());
            out.push((i, b, wrote.clone()));
        }
        out
    }

    /// Capture the guest's turn-end callback writes against the pass base and
    /// carry them into `self.w`. Nothing when the list is untouched.
    pub fn capture_guest_scheduled_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Option<(Vec<crate::engine::Scheduled>, Vec<crate::engine::Scheduled>)> {
        if base.scheduled == from.scheduled {
            return None;
        }
        let delta = ScheduledDelta {
            base: base.scheduled.clone(),
            wrote: from.scheduled.clone(),
        };
        let mut list = std::mem::take(&mut self.w.scheduled);
        delta.apply(&mut list);
        self.w.scheduled = list;
        Some((delta.base, delta.wrote))
    }

    /// Capture the event-stack writes the guest made against the pass base and
    /// carry them into `self.w`. Nothing when no list moved.
    pub fn capture_guest_events_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Option<EventDelta> {
        if base.event_deck == from.event_deck
            && base.event_top == from.st.event_top
            && base.event_discard == from.event_discard
            && base.event_removed == from.event_removed
            && base.event_active == from.st.event_active
        {
            return None;
        }
        let d = EventDelta {
            base_deck: base.event_deck.clone(),
            wrote_deck: from.event_deck.clone(),
            base_top: base.event_top.clone(),
            wrote_top: from.st.event_top.clone(),
            base_discard: base.event_discard.clone(),
            wrote_discard: from.event_discard.clone(),
            base_removed: base.event_removed.clone(),
            wrote_removed: from.event_removed.clone(),
            base_active: base.event_active.clone(),
            wrote_active: from.st.event_active.clone(),
        };
        d.apply(&mut self.w);
        Some(d)
    }

    /// Capture the rule-parameter bonus map against **live** (the chaining rule
    /// of the token and pile rows: an additive keyed write must undo exactly
    /// one step, or a re-run doubles it) and lay it onto `self.w`.
    pub fn capture_guest_rule_bonus_from(
        &mut self,
        from: &World,
    ) -> Vec<(String, Option<i32>, Option<i32>)> {
        let mut rows = Vec::new();
        for (k, v) in from.rule_param_bonus.iter() {
            let base = self.w.rule_param_bonus.get(k).copied();
            if base != Some(*v) {
                rows.push((k.clone(), base, Some(*v)));
            }
        }
        for (k, v) in self.w.rule_param_bonus.iter() {
            if !from.rule_param_bonus.contains_key(k) {
                rows.push((k.clone(), Some(*v), None));
            }
        }
        for (k, base, wrote) in &rows {
            set_rule_bonus(&mut self.w, k, *base, *wrote);
        }
        rows
    }

    pub fn capture_guest_piles_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<(usize, PileDelta)> {
        let mut out = Vec::new();
        let n = base.piles.len().min(from.hidden.len());
        for i in 0..n {
            let Some(live_row) = self.w.hidden.get(i) else {
                continue;
            };
            // The row is taken against live, not the pass base, for the same
            // reason as the token rows: a boundary's row has to chain onto the
            // previous boundary's `wrote`, or the reverse-order revert and the
            // commit's re-apply fight and the later write loses (the 共鸣 fuel
            // came back to the hand). A list the re-run already matched is no
            // row at all, and pile rows are therefore not deduped by
            // [`retain_unseen_overlay_rows`].
            let b_hand = live_row.hand.clone();
            let b_deck = live_row.deck.clone();
            let b_grave = live_row.graveyard.clone();
            let b_next = live_row.next_steps;
            let g = &from.hidden[i];
            let mut d = PileDelta::default();
            if g.hand != b_hand {
                d.hand = Some((b_hand.clone(), g.hand.clone()));
            }
            if g.deck != b_deck {
                d.deck = Some((b_deck.clone(), g.deck.clone()));
            }
            if g.graveyard != b_grave {
                d.grave = Some((b_grave.clone(), g.graveyard.clone()));
            }
            if g.next_steps != b_next {
                d.next_steps = Some((b_next, g.next_steps));
            }
            if d != PileDelta::default() {
                if let Some(live) = self.w.hidden.get_mut(i) {
                    d.apply(live);
                }
                out.push((i, d));
            }
        }
        out
    }

    /// Capture the inspection-frame card lists the guest edited (`ctx::take_card`
    /// / `ctx::add_to_dest` on `CardPile::Inspection`). The frame is matched by
    /// id on live; a frame the body closed since is gone and skipped.
    pub fn capture_guest_frames_from(&mut self, from: &World) -> Vec<FrameDelta> {
        let mut out = Vec::new();
        for g in &from.inspection_frames {
            let Some(live) = self.w.inspection_frames.iter_mut().find(|f| f.id == g.id) else {
                continue;
            };
            if live.cards == g.cards {
                continue;
            }
            let base = live.cards.clone();
            live.cards = g.cards.clone();
            out.push(FrameDelta {
                id: g.id,
                base,
                wrote: g.cards.clone(),
            });
        }
        out
    }

    /// Capture the roll pick columns the guest wrote against the pass base and
    /// carry them into `self.w` (`ctx::roll_pick`). A transaction the host
    /// opened after the pass start has no base row; its own columns are then
    /// the base. The settle reads the keep point, so the write has to cross
    /// before any host routine runs.
    pub fn capture_guest_roll_picks_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<RollPickDelta> {
        let mut out = Vec::new();
        for g in &from.open_rolls {
            let Some(live) = self.w.open_rolls.iter_mut().find(|r| r.id == g.id) else {
                continue;
            };
            let (b_chosen, b_unused, b_dirty) = base
                .rolls
                .iter()
                .find(|r| r.0 == g.id)
                .map(|r| (r.1, r.2, r.3))
                .unwrap_or((live.chosen_index, live.unused_face, live.dirty));
            if (b_chosen, b_unused, b_dirty) == (g.chosen_index, g.unused_face, g.dirty) {
                continue;
            }
            let d = RollPickDelta {
                id: g.id,
                chosen_index: (b_chosen, g.chosen_index),
                unused_face: (b_unused, g.unused_face),
                dirty: (b_dirty, g.dirty),
            };
            d.apply(live);
            out.push(d);
        }
        out
    }

    /// Capture the world-snapshot owners the guest added against the pass base
    /// and carry them into `self.w`. A card a body places adopts the body's
    /// keys (`adopt_snapshot_keys`) so the placed copy's own hooks can restore
    /// them; the row is host styling a nested drive reads, so the addition has
    /// to cross at the request boundary like the body's field writes. Only
    /// additions cross -- a drop removes the whole key, and the table itself
    /// rides the commit.
    pub fn capture_guest_snapshot_owners_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<(i32, i32)> {
        let mut rows = Vec::new();
        for (key, owners) in &from.world_snapshot_owners {
            let known = base
                .owners
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, o)| o.as_slice())
                .unwrap_or(&[]);
            for uid in owners {
                if !known.contains(uid) {
                    rows.push((*key, *uid));
                }
            }
        }
        for (key, uid) in &rows {
            let owners = self.w.world_snapshot_owners.entry(*key).or_default();
            if !owners.contains(uid) {
                owners.push(*uid);
            }
        }
        rows
    }

    /// Capture the guest's **held-counter writes** against the pass base and
    /// carry them into `self.w`. `ctx::add_held` (the marker kernel's write)
    /// runs on the guest copy; both directions cross -- a spent pot and a
    /// capped gain. A live row the routine moved meanwhile keeps the routine's.
    pub fn capture_guest_tokens_from(
        &mut self,
        base: &PassBase,
        from: &World,
    ) -> Vec<(
        usize,
        String,
        Option<crate::state::Counter>,
        Option<crate::state::Counter>,
    )> {
        let mut rows = Vec::new();
        let n = self.w.st.players.len().min(from.st.players.len());
        for i in 0..n {
            let guest = &from.st.players[i].tokens;
            if let Some(bs) = base.tokens.get(i) {
                for b in bs {
                    if !guest.iter().any(|c| c.name == b.name) {
                        rows.push((i, b.name.clone(), Some(b.clone()), None));
                    }
                }
            }
            for g in guest {
                // Live's current row, not the pass base: a boundary's row has
                // to chain onto the previous boundary's `wrote` or the
                // reverse-order revert and the commit's re-apply fight and
                // lose a write (a three-add loop landed at two). Token rows are
                // therefore not deduped by [`retain_unseen_overlay_rows`].
                let base_c = self
                    .w
                    .st
                    .players
                    .get(i)
                    .and_then(|p| p.tokens.iter().find(|c| c.name == g.name));
                match base_c {
                    Some(bc)
                        if bc.value != g.value || bc.instance != g.instance || bc.max != g.max =>
                    {
                        rows.push((i, g.name.clone(), Some(bc.clone()), Some(g.clone())))
                    }
                    None => rows.push((i, g.name.clone(), None, Some(g.clone()))),
                    _ => {}
                }
            }
        }
        for (i, name, base_c, wrote) in &rows {
            write_token_row(&mut self.w, *i, name, base_c.as_ref(), wrote.as_ref());
        }
        rows
    }

    /// Does an overlay row still on the stack carry this instance? Then the
    /// live copy is this drive's own earlier crossing, not a nested commit.
    fn own_overlay_has_field(&self, uid: i32) -> bool {
        self.guest_overlays
            .iter()
            .any(|(_, o)| o.field.added.iter().any(|(_, _, f)| f.uid == uid))
    }

    /// Capture the guest's **direct field writes** against the pass-start
    /// `base` into `self.w` and return them. Placement, removal and in-place
    /// rewrites (`set_tile_at`, `set_prop`, `counter`, face-down) are imports,
    /// so they run on the guest's detached copy; the commit boundary has to
    /// carry them across. A rewrite applies only where the live instance still
    /// matches `base`, so a host write to the same instance wins.
    pub fn capture_guest_field_from(&mut self, base: &PassBase, from: &World) -> Box<FieldDelta> {
        let mut d = Box::<FieldDelta>::default();
        let mut lists: Vec<(Option<usize>, usize)> = vec![(None, base.board_field.len())];
        for i in 0..base.field.len() {
            lists.push((Some(i), base.field[i].len()));
        }
        for (owner, _len) in &lists {
            let guest = field_list(from, *owner);
            let guest_uids: Vec<i32> = guest.iter().map(|f| f.uid).collect();
            for (i, b) in field_list(&self.w, *owner).iter().enumerate() {
                // Only instances the pass base held can be the guest's
                // removal; host-added ones are simply absent from the copy.
                if base_of(base, *owner, b.uid).is_some() && !guest_uids.contains(&b.uid) {
                    d.removed.push((*owner, i, b.clone()));
                }
            }
            for (i, g) in guest.iter().enumerate() {
                // The row is a step against **live**, not against the pass
                // base, like the token and pile rows. An instance live does
                // not carry is the guest's placement; one live already
                // carries (this body's own earlier crossing, a nested
                // commit) is a rewrite, and the revert puts back what live
                // held at this boundary -- a pass-base revert would hand the
                // re-run the pre-first-write world and double it (the
                // bang-dream crystal went to 2).
                let wrote = FieldWrite::of(g);
                match field_list(&self.w, *owner).iter().find(|f| f.uid == g.uid) {
                    None => d.added.push((*owner, i, g.clone())),
                    Some(l) => {
                        let base_w = FieldWrite::of(l);
                        if base_w != wrote {
                            d.changed.push((g.uid, base_w, wrote));
                        }
                    }
                }
            }
        }
        d.apply(&mut self.w);
        d
    }

    /// Capture the guest's state writes against the pass-start `base` into
    /// `self.w` and return them, without stacking an overlay (G10 §6.3.1: the
    /// journal keeps them; the live boundary stacks its own copy through
    /// [`Self::overlay_guest_state`]).
    ///
    /// Every touched key crosses, in both directions: a counter cleared
    /// *and* a latch raised (`exist.used.<uid>`) are ordinary guest writes, and
    /// the in-play write-through has to see both. Where the host routine moved
    /// the same key during the pass, only its delta on `base` is kept -- the
    /// body replays from the top, so an additive write must not double-count
    /// (the same rule [`Self::restore_guests_to`] applies).
    /// `boundary` narrows the crossing to **decreases** (a clear). A status the
    /// body just applied itself must not gate its own follow-on money (心の雨's
    /// fallback stuns, then gains): the increase stays on the run copy and the
    /// replay re-derives it. The commit pass (`boundary == false`) takes every
    /// row -- it is the diff that lands.
    pub fn capture_guest_state_from(
        &mut self,
        base: &[std::collections::BTreeMap<String, crate::state::StateVar>],
        from: &World,
        boundary: bool,
    ) -> GuestOverlay {
        let mut o = GuestOverlay::default();
        let n = self.w.st.players.len().min(from.st.players.len());
        for i in 0..n {
            // The guest map's keys only: a key the guest does not carry was not
            // written this iteration (the guest's copy came from `base`).
            let keys: Vec<String> = from.st.players[i].state.keys().cloned().collect();
            for k in keys {
                // A `latch:` key is a decision frozen by the body, not a
                // re-derivable write: the rewind must keep it (the replay takes
                // the same branch) or the branch flips when the world it read
                // has already moved. `adopt_turn_policy` crosses the write, so
                // the overlay row would only revert it again at the pass top.
                if k.starts_with("latch:") {
                    continue;
                }
                let wrote = from.st.players[i].state[&k];
                let base_v = base.get(i).and_then(|m| m.get(&k)).copied();
                if base_v == Some(wrote) {
                    continue;
                }
                // Decrease only at a pause boundary (see above); an absent base
                // reads as 0, so a key the guest created is never a decrease.
                // The increase still rides the row, so the copy rebuilt after
                // the request regains it (`GuestOverlay::apply_state`) and the
                // commit's diff carries it -- it just must not sit on live
                // while the request itself gates on it.
                let crosses = !boundary || wrote.value < base_v.map(|b| b.value).unwrap_or(0);
                let live = self.w.st.players[i].state.get(&k).copied();
                let mut wrote = wrote;
                // Already on the guest's value (the write crossed at a pause,
                // or the routine landed the same number): leave live alone.
                // Still at the base: take the guest's value. Otherwise the
                // routine moved it too -- keep only the guest's delta, on the
                // pass-start base.
                let now = live.map(|v| v.value);
                if live != base_v && now != Some(wrote.value) {
                    wrote.value =
                        now.unwrap_or(0) + (wrote.value - base_v.map(|b| b.value).unwrap_or(0));
                }
                // Route through `state_set` so the writer-declared
                // `NEVER_NEGATIVE` floor applies -- a raw `insert` would drop
                // it and the restore path can reconstruct a negative. The
                // copy carries the item's flags/expiry; install them first so
                // the enforcer sees the right declaration. Record the
                // post-floor value so restore compares against what actually
                // landed.
                {
                    let p = &mut self.w.st.players[i];
                    // Keep the live item's declaration if the guest copy is a
                    // bare default (flags == 0) -- the floor is sticky.
                    let flags = if wrote.flags != 0 {
                        wrote.flags
                    } else {
                        base_v.map(|b| b.flags).unwrap_or(0)
                    };
                    if flags != 0 {
                        p.state_set_flags(&k, flags);
                    }
                    if let Some(exp) = wrote.expires.or_else(|| base_v.and_then(|b| b.expires)) {
                        p.state_set_expires(&k, Some(exp));
                    }
                    if crosses {
                        wrote.value = p.state_set(&k, wrote.value);
                    }
                }
                o.state.push((i, k, base_v, wrote, crosses));
            }
        }
        o
    }

    /// Keep every overlay above `base` (a committing drive's writes are part of
    /// the world now; a later revert of an outer pass must not drop them).
    pub fn keep_guest_overlays_to(&mut self, base: usize) {
        self.guest_overlays.truncate(base);
    }

    /// Re-anchor the overlays an enclosing drive left below `base` after a
    /// nested commit replaced the live world. The rows that drive placed are
    /// part of the committed world now, so its later revert must not take them
    /// out again (`CardWorld::place_or` keeps a row it finds in place). Every
    /// other delta surface restores only while live still matches what the
    /// drive wrote, so a value the commit changed is kept already.
    pub fn rebase_guest_overlays(&mut self, base: usize) {
        for (_, o) in self.guest_overlays.iter_mut().take(base) {
            o.field.added.clear();
        }
    }

    /// How many guest overlays are currently on the stack. A drive records
    /// this at entry and calls [`Self::restore_guests_to`] with it, so nested
    /// drives never pop an outer one.
    pub fn guest_overlay_depth(&self) -> usize {
        self.guest_overlays.len()
    }

    /// Revert *every* overlay above `base`, nested drives included, back to
    /// `base`'s world. A restart re-runs the body from the top -- and the
    /// nested hooks its own host work fires come with it -- so anything
    /// crossed since the pass base is re-derived by the restart and must go
    /// now, or a nested effect (a status layer a `PassTile` hook granted)
    /// lands twice. Used at the learn/commit switch and at `commit_after`;
    /// [`Self::pass_base_for`] unwinds the same rows on a clone.
    pub fn restore_guests_all_to(&mut self, base: usize) {
        let mut seen = SeenRows::new();
        while self.guest_overlays.len() > base {
            let (_, mut o) = self.guest_overlays.pop().expect("len > base");
            retain_unseen_overlay_rows(&mut seen, &mut o);
            let reinserted = revert_overlay_on(&mut self.w, o);
            self.reinserted.extend(reinserted);
        }
    }

    /// Drain the uids the restores so far put back for a re-run's reads: the
    /// pass that starts now carries them, and drops the ones its replay does
    /// not take out again (see `commit_after`).
    pub fn take_reinserted(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.reinserted)
    }

    /// Drop every [`GuestOverlay`] this drive pushed (down to `base`), keeping
    /// only the host routine's own effects. The pass top after a prompt answer
    /// uses it: an outer drive's crossings stay valid there, so only `own`'s
    /// rows go.
    ///
    /// * a key the routine left alone goes back to its pre-overlay value (the
    ///   replay re-applies the guest write on top);
    /// * a key the routine also touched keeps the routine's **delta** on the
    ///   base value, so the replay's re-application composes rather than
    ///   double-counts.
    pub fn restore_guests_to(&mut self, base: usize, own: u32) {
        let mut seen = SeenRows::new();
        while self.guest_overlays.len() > base {
            let (drive, mut o) = self.guest_overlays.pop().expect("len > base");
            if o.state.iter().any(|(_, k, _, _, _)| k == "exile") {}
            if drive != own {
                // A nested drive crossed this and is in flight; the restart
                // above `base` re-runs this drive's body, not that one, so the
                // write would not be re-derived. Keep it in the world.
                continue;
            }
            // Several pauses of one run can record the same surface (the body
            // re-derives the write from the world it just changed). Undo it
            // once: the topmost row already carries the base the earlier ones
            // name, and reverting a second time makes the delta guard add the
            // write again (the transferred stay layer in s08 went 2 to 4, the
            // PAREO counter here went 2 to 3).
            retain_unseen_overlay_rows(&mut seen, &mut o);
            let reinserted = revert_overlay_on(&mut self.w, o);
            self.reinserted.extend(reinserted);
        }
    }

    /// Put the live world back to a suspended request's pre-state, plus the
    /// guest's own writes recorded at it.
    ///
    /// A re-run restarts at that request, so its prefix reads must see what
    /// the body saw when it issued the request; the answered prefix's host
    /// effects are re-applied at the request sites as the body reaches them.
    pub fn restore_run_point(&mut self, condition: &World, guest: &GuestOverlay) {
        // The latch rule of `restore_condition` / `Run::restore_from_world`:
        // the condition predates every latch the body froze after it, and the
        // undo must not unfreeze them.
        let latched = self.w.latches();
        let next_event = self.w.next_event;
        let recent = self.w.recent.clone();
        self.w = crate::engine::world::SharedWorld::from(condition.clone());
        guest.apply(&mut self.w);
        self.w.next_event = next_event;
        self.w.recent = recent;
        self.w.put_latches(latched);
    }

    /// Rewind the **live** world's piles to a drive's start (design G10
    /// §6.3.6). The replay's pile writes are host writes and land on live, so
    /// a pass top has to rewind live, not only the guest copy: the body's
    /// `ctx::deck_count` and kin read through it. Callers snapshot the pass
    /// base first -- the rewind must not erase the deltas the pass records.
    pub fn restore_pile_base(&mut self, base: &World) {
        // Piles only -- the RNG stream stays monotonic across a drive's passes
        // (see `PileCheckpoint::apply`: a rewound stream makes every pass draw
        // the same face).
        self.w.hidden = base.hidden.clone();
    }

    /// The shared game data (tile/card lookups).
    pub fn game_data(&self) -> &'a GameData {
        self.data
    }

    /// The rules in force (a nested run may call back into them).
    pub fn rules(&self) -> &'a dyn CardRules {
        self.rules
    }

    /// Forget the factor ledger -- a new body execution starts from zero, so
    /// the same card firing again this turn is not mistaken for the re-run of
    /// its last firing.
    pub fn reset_factor_ledger(&mut self) {
        self.w.turn.factor_ordinal = 0;
        self.w.turn.factor_gen = self.w.turn.factor_gen.wrapping_add(1);
        self.ledger_exec = self.w.turn.factor_gen;
    }

    /// The execution id factor writes of the running drive are keyed by.
    pub fn ledger_exec(&self) -> u32 {
        self.ledger_exec
    }

    /// How many settles are open right now (the [`PlayCtx::settle_stack`] depth).
    pub fn settle_open(&self) -> u32 {
        self.settle_stack.len() as u32
    }

    /// The innermost open settle's tile, `-1` when none is open.
    pub fn settle_tile(&self) -> i32 {
        self.settle_stack.last().map_or(-1, |s| s.tile)
    }

    /// The open settles, innermost last (the raise-time move stamping reads
    /// the top entry's `move_context`).
    pub fn settle_stack(&self) -> &[SettleEntry] {
        &self.settle_stack
    }

    /// The live plan's move tags (copied onto a stamped move).
    pub fn plan_tags(&self) -> Vec<(String, i32)> {
        self.w.plan().tags.clone()
    }

    /// The tile standing at board position `position` (the raise macro's
    /// `position =` arm converts through this).
    pub fn tile_at(&self, position: i32) -> i32 {
        self.w.tile_standing_at(position)
    }

    /// Declare `houseAdd` / `houseRemove` for tile `t` on a fresh `effect`
    /// window, then apply unless cancelled. Answers the post count (the
    /// pre-write count when cancelled). 设计 5.4.
    pub fn write_houses(
        &mut self,
        by: usize,
        t: usize,
        w: HouseWrite,
        by_card: Option<i32>,
    ) -> Flow<i32> {
        let cap = self.tile(t).rent.len().saturating_sub(1) as i32;
        let before = self.w.st.houses[t];
        let after = match w {
            HouseWrite::Set(n) => n.clamp(0, cap),
            HouseWrite::Add(d) => before.saturating_add(d).clamp(0, cap),
        };
        if after == before {
            return Ok(before);
        }
        let delta = after - before;
        let target = self.w.st.owners[t];
        let t = t as i32;
        // `value` is the size of the change, unsigned (135 is the removal
        // spelling; the kind carries the direction).
        let kind = if delta > 0 { "houseAdd" } else { "houseRemove" };
        let trig = raise!(
            self,
            "effect",
            by,
            tile = t,
            target = target,
            by_card = by_card,
            effects = vec![super::rules::Effect {
                kind: kind.into(),
                target,
                from: -1,
                tile: t,
                value: delta.abs(),
            }]
        )?;
        if trig.is_cancelled() {
            return Ok(before);
        }
        self.w.st.houses[t as usize] = after;
        Ok(after)
    }

    /// Declare `ownerSet` for tile `t`, then apply unless cancelled. Answers
    /// the new owner (the old one when cancelled). 设计 5.4.
    pub fn write_owner(
        &mut self,
        by: usize,
        t: usize,
        owner: i32,
        by_card: Option<i32>,
    ) -> Flow<i32> {
        let before = self.w.st.owners[t];
        if before == owner {
            return Ok(before);
        }
        let t = t as i32;
        let trig = raise!(
            self,
            "effect",
            by,
            tile = t,
            target = before,
            by_card = by_card,
            effects = vec![super::rules::Effect {
                kind: "ownerSet".into(),
                target: owner,
                from: -1,
                tile: t,
                value: owner,
            }]
        )?;
        if trig.is_cancelled() {
            return Ok(before);
        }
        // The one ownership write, so the acquisition ledger follows the
        // 「视为拥有」 / owner-set too (design G08 §7.7).
        self.w.set_tile_owner(t, owner);
        Ok(owner)
    }

    /// Owe one `houseAdded` / `houseRemoved` raise (`delta` signed) to the
    /// drive's commit; see [`Self::pending_house_log`].
    pub fn push_pending_house_log(&mut self, owner: i32, tile: i32, now: i32, delta: i32) {
        self.pending_house_log.push((owner, tile, now, delta));
    }

    /// Take the owed house raises.
    pub fn take_pending_house_log(&mut self) -> Vec<(i32, i32, i32, i32)> {
        std::mem::take(&mut self.pending_house_log)
    }

    /// The position tile `tile` stands at (the raise macro's `tile =` arm).
    pub fn position_of_tile(&self, tile: i32) -> i32 {
        self.w.position_of_tile(tile)
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
            guest_log: Default::default(),
            provider: None,
            delay: 0.0,
            money_depth: 0,
            reentrant_hooks: Vec::new(),
            settle_stack: Vec::new(),
            ledger_exec: 0,
            state_raise_depth: 0,
            in_teleport_tail: false,
            activation: Vec::new(),
            move_start: -1,
            move_depth: 0,
            guest_overlays: Vec::new(),
            pending_house_log: Vec::new(),
            reinserted: Vec::new(),
            play_memo: std::cell::RefCell::new(PlayMemo::default()),
            work_stack: Vec::new(),
            draining_work: false,
            suspend_base: 0,
            counteract_resume: None,
            counteract_chain: None,
            chain_keepalive: Vec::new(),
            last_drive_fate: None,
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
    /// membership, phase, `CannotPlay`, `normal`) run every time in
    /// [`Self::cant_play`] and are pure field reads. The exclusive-use check
    /// (`ExclusiveUse` gate) has its own half of this memo
    /// ([`Self::exclusive_play`]).
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
        let reason = self.rules.cant_play(self, player, card);
        self.play_memo
            .borrow_mut()
            .insert(player, card, reason.clone());
        reason
    }

    /// The `ExclusiveUse` gate with the per-world memo (same stamp rule as
    /// [`Self::rules_cant_play`]). `true` = a group-skill gate entry closed this
    /// exclusive play. The engine's generic own/base verdict is the caller's
    /// cheap pre-check; this only asks the ruleset.
    pub fn exclusive_gate_denied(&self, player: usize, card: &str, ask: &ExclusiveAsk<'_>) -> bool {
        let stamp = self.w.stamp();
        {
            let mut memo = self.play_memo.borrow_mut();
            if memo.exclusive_stamp != stamp {
                memo.clear_exclusive();
                memo.exclusive_stamp = stamp;
            }
            if let Some(denied) = memo.get_exclusive(player, card) {
                #[cfg(feature = "bot-cost")]
                crate::engine::bot_cost::CANT_PLAY_MEMO_HITS
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return denied;
            }
        }
        let denied = !self.w.exclusive_use(self.rules, player, ask);
        self.play_memo
            .borrow_mut()
            .insert_exclusive(player, card, denied);
        denied
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
        if std::env::var("DBG_OFFER").is_ok() {
            eprintln!(
                "ASK kind={} title={} players={:?} known={}",
                ask.view.kind,
                ask.view.title.key(),
                ask.view.players,
                self.answers.get(self.cursor).is_some()
            );
        }
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
                answered: a.clone(),
            });
        }
        if let Some(p) = self.provider.as_mut() {
            if let Some(a) = p.answer(&ask) {
                self.delay = 0.0;
                self.w.take_walk_delay();
                return Ok(Reply {
                    players: ask.view.players,
                    fallback: ask.view.fallback,
                    answered: a,
                });
            }
        }
        // Halt for player input: flush any pending walk segment first, so the
        // client has animated the approach to this point before the prompt
        // shows (a counteraction window mid-walk, a the start square reward, …).
        self.w.flush_walk();
        self.w.st.prompt = ask.view.clone();
        Err(Halt(HaltKind::Ask(Box::new(ask))))
    }

    /// Re-ask a prompt a restored fork halted on (`Routine::Ai`). An answer
    /// the caller already supplied comes back through the answer list; a seat
    /// still missing halts here again. The `Ask` is the one the fork was saved
    /// with, so its title, options and prefilled `ai` stay as the live match
    /// had them.
    pub(crate) fn resume_prompt(&mut self, ask: Ask) -> Flow<()> {
        self.ask(ask)?;
        Ok(())
    }

    /// Replace every [`AI_UNSET`] entry of `ask.ai` with the answer this seat's
    /// policy would give. Standard (and every human seat, including one taken
    /// over after a time-out) takes the prompt's fallback. Chaos picks uniformly
    /// among the non-default options -- never the "none" / "skip" / "do nothing"
    /// while an alternative exists -- and a tile prompt gets a random target
    /// rather than "none".
    ///
    /// A counteraction offer is the one exception -- and for both policies it is a
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
        if ask.view.kind == "multi" {
            // Bitmask of exactly `count` distinct options. Standard takes the
            // fallback (first `count`); chaos rolls a random valid mask.
            return if self.is_chaos(seat) {
                self.chaos_multi_pick(ask)
            } else {
                fallback
            };
        }
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
        } else if ask.view.title.key() == "ask.pass_reward.title" {
            // Standard: the card odds ramp with the seat's money (user
            // ruling 2026-10-28). Both the server bot and 托管 come through
            // here, so there is no second copy to keep in step.
            self.pass_reward_pick(seat, ask, fallback)
        } else {
            fallback
        }
    }

    /// Standard's [pass-reward] answer: the card on
    /// [`super::ai::pass_reward_card_chance`], else `fallback` (the money
    /// print). Both option wordings carry the card ("card" on a pass,
    /// "card_landed" on a stop on the start square). Only the intermediate
    /// band draws the world RNG, so the stream moves exactly where the
    /// outcome is undecided.
    fn pass_reward_pick(&mut self, seat: usize, ask: &Ask, fallback: i32) -> i32 {
        let money = self.w.st.players.get(seat).map_or(0, |p| p.money);
        let chance = super::ai::pass_reward_card_chance(money);
        if chance <= 0.0 {
            return fallback;
        }
        if chance < 1.0 && self.w.rng.f64() >= chance {
            return fallback;
        }
        ask.view
            .options
            .iter()
            .position(|o| {
                matches!(
                    o.key(),
                    "ask.pass_reward.card" | "ask.pass_reward.card_landed"
                )
            })
            .map_or(fallback, |i| i as i32)
    }

    /// Standard's counteraction answer: the first offered card whose
    /// [`crate::strategy::CounterParams`] propensity hits, else `fallback`
    /// (the skip). Each candidate draws from the world RNG (so it replays) --
    /// one draw per offered card until one hits, so with `k` offered cards
    /// the per-offer declare rate is `1 - (1 - p)^k` on a uniform per-card
    /// propensity `p` (600‰ default: 60 % for one card, 84 % for two). A
    /// zero propensity draws nothing and never hits, which is how a book
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

    /// A random valid multi-select mask: exactly `count` distinct options out
    /// of `0..options.len()`. Falls back to the prompt's fallback when the
    /// shape is degenerate (0 options, or `count` past what a 31-bit mask can
    /// name).
    fn chaos_multi_pick(&mut self, ask: &Ask) -> i32 {
        let n = ask.view.options.len().min(31) as i32;
        let want = ask.view.count.clamp(1, n.max(1));
        if n <= 0 || want > n {
            return ask.view.fallback;
        }
        // Draw `want` distinct indices without replacement.
        let mut pool: Vec<i32> = (0..n).collect();
        let mut mask = 0i32;
        for _ in 0..want {
            let i = self.w.rng.below(pool.len()) as usize;
            let x = pool.swap_remove(i);
            mask |= 1 << x;
        }
        mask
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
    /// `tile` where it ran (`-1` when not applicable). A negated activation
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
    ///
    /// Honours [`Extreme`]: a play under 「以理论最大值或最小值结算」 settles
    /// every die at its maximum / minimum instead of rolling it (same result as
    /// `World::roll_silent`). Which *plays* may be forced is the counteraction
    /// window's decision, not this function's -- AG 微小 limits itself to a
    /// numeric-interval play (「可以对YOLO生效但无法对无论是何种颜色的夕阳
    /// 生效」); that filter is still OPEN (BACKLOG ABI-04), so a branch-selecting
    /// die of a play the counter should not reach is forced today too.
    pub fn roll(&mut self, player_id: i32, count: i32, sides: i32, what: Option<Msg>) -> i32 {
        let extreme = self.w.turn.extreme;
        let faces: Vec<i32> = if extreme == Extreme::Plain {
            (0..count).map(|_| self.w.rng.d(sides)).collect()
        } else {
            let face = if extreme == Extreme::Max { sides } else { 1 };
            (0..count).map(|_| face).collect()
        };
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
            .opt(
                "what",
                what.map(|w| Msg::new("log.part.reason").msg("reason", w)),
            )
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

    /// `CanPay`: not out, and the status hosts' pay gate is open.
    pub(crate) fn can_pay(&self, i: usize) -> bool {
        let s = &self.w.st.players[i];
        !s.out() && self.w.gate_open(self.rules, super::Gate::Pay, i)
    }

    /// `Blocked(i)` -- why a player can't move money. The message is the
    /// ruleset's (its gate-refusal key for whichever capability is closed).
    pub(crate) fn blocked(&self, i: usize) -> crate::msg::Msg {
        if !self.w.gate_open(self.rules, super::Gate::OnBoard, i) {
            self.rules.gate_refusal_msg(super::Gate::OnBoard, i)
        } else {
            self.rules.gate_refusal_msg(super::Gate::Pay, i)
        }
    }

    /// Players starting at `from`, wrapping, that are in the game and on board
    /// (the ruleset's on-board gate).
    pub fn present_from(&self, from: usize) -> Vec<usize> {
        let n = self.w.player_count();
        (0..n)
            .map(|k| (from + k) % n)
            .filter(|&i| !self.out(i) && self.w.gate_open(self.rules, super::Gate::OnBoard, i))
            .collect()
    }
}
