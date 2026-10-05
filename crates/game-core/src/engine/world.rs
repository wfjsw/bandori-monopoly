//! The replayable part of a match.
//!
//! Everything a routine reads or writes lives here, including the RNG and the
//! event/prompt counters, so re-running a routine from a snapshot reproduces the same
//! dice, the same event ids and the same prompt ids. Clocks, the live prompt, auction
//! bidding and the end-match vote belong to the host ([`super::Match`]) instead.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::msg::Msg;
use crate::rng::Rng;
use crate::state::{MatchEvent, MatchState};

/// Money each player starts with (`BeginPlay`).
pub const START_MONEY: i32 = 10_000;
/// Opening hand size (`StartHandOf`).
pub const START_HAND: usize = 2;
/// Base hand limit (`HandLimitOf`).
pub const HAND_LIMIT: usize = 5;
/// CiRCLE reward in money (`CircleReward`).
pub const CIRCLE_MONEY: i32 = 2000;
/// Events kept for `events_since` (`EventKeep`).
pub const EVENT_KEEP: usize = 400;
/// Events carried in every `MatchState` snapshot.
pub const STATE_EVENTS: usize = 80;

/// Per-player private card zones (C# `Hidden`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hidden {
    pub hand: Vec<String>,
    /// Top of the pile is the **end** of the vec.
    pub draw: Vec<String>,
    pub discard: Vec<String>,
    /// The player's next main move walks exactly this many steps instead of the
    /// roll (C# `NextStepsFx.Steps`); consumed by that move.
    #[serde(default)]
    pub next_steps: Option<i32>,
}

/// Per-turn bookkeeping (the parts of C# `TurnCtx` the shell uses).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnCtx {
    /// `C# TurnCtx.Plan` -- the movement being planned / walked. Runtime-only:
    /// it carries roll tables and closures, so it never reaches the wire (the
    /// broadcast summary is `MatchState::plan`).
    #[serde(skip)]
    pub plan: crate::engine::move_ctx::MoveCtx,
    pub player_id: usize,
    pub extra: bool,
    pub main_moved: bool,
    pub played: Vec<String>,
    /// Players whose money cannot drop for the rest of this turn (C#
    /// `TurnCtx.NoMoneyLoss`). Auctions are not payments, so they are exempt.
    #[serde(default)]
    pub no_money_loss: Vec<usize>,
    /// This turn's main-move roll is fixed (C# `TurnCtx.Plan.FixedRoll`).
    #[serde(default)]
    pub fixed_roll: Option<i32>,
    /// Steps this turn's main move walked (C# `TurnCtx.LastMain`); 0 = none yet.
    #[serde(default)]
    pub main_steps: i32,
    /// C# `_abnormalTurn` -- abnormal effects that got through to each player
    /// this turn (indexed by player; a new turn starts them all at 0).
    #[serde(default)]
    pub abnormal: Vec<i32>,
}

/// A card that asked to be called at a turn end (C# `TurnCtx.AfterEnd` /
/// `AtEnd`, and the "end of your next turn" effects). The host runs the card's
/// `react` with kind `turnEnd` when `target`'s turn ends, then drops it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scheduled {
    pub card: String,
    /// The player the card acts for (passed to its `react`).
    pub owner: i32,
    /// Whose turn end fires it.
    pub target: usize,
    /// Let one `target` turn end pass first -- set when "the end of your next
    /// turn" is scheduled during that player's own current turn.
    pub skip: bool,
    /// C# `AtEnd`: before the status wear-off (`TurnEndBefore`), rather than
    /// `AfterEnd` after it (`TurnEndAfter`).
    #[serde(default)]
    pub early: bool,
}

/// Things a routine asks the host to do once it commits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    /// A turn began for this player: refill its time bank and start the clock.
    StartTimer(usize),
    /// A new turn record was created (clears the host's time-out flag).
    TurnBegan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub st: MatchState,
    pub hidden: Vec<Hidden>,
    pub rng: Rng,
    pub next_event: i32,
    pub recent: VecDeque<MatchEvent>,
    pub turn: TurnCtx,
    pub extra_turns: Vec<usize>,
    pub out_count: i32,
    pub event_deck: Vec<String>,
    pub event_discard: Vec<String>,
    pub event_removed: Vec<String>,
    /// The current turn is over; the host starts the next one.
    pub next_turn_pending: bool,
    /// Deeds of players who left, waiting to be auctioned.
    pub leftovers: VecDeque<Vec<usize>>,
    /// `H._ringBonus` -- added to the RiNG rent multiplier by cards.
    pub ring_bonus: i32,
    pub ask_seq: i32,
    pub signals: Vec<Signal>,
    /// Turn-end callbacks waiting for their turn end.
    #[serde(default)]
    pub scheduled: Vec<Scheduled>,
    /// C# `H._targeted` -- per player, times other players' cards targeted it since
    /// its own turn last started.
    #[serde(default)]
    pub targeted: Vec<i32>,
}

impl World {
    pub fn new(st: MatchState, players: usize, seed: u64) -> Self {
        Self {
            st,
            hidden: vec![Hidden::default(); players],
            rng: Rng::new(seed),
            next_event: 1,
            recent: VecDeque::new(),
            turn: TurnCtx::default(),
            extra_turns: vec![],
            out_count: 0,
            event_deck: vec![],
            event_discard: vec![],
            event_removed: vec![],
            next_turn_pending: false,
            leftovers: VecDeque::new(),
            ring_bonus: 0,
            ask_seq: 0,
            signals: vec![],
            scheduled: vec![],
            targeted: vec![],
        }
    }

    /// Append an event (`MatchHost.Log`). Returns it for further fields.
    pub fn log(&mut self, kind: &str, player_id: i32, msg: Msg) -> &mut MatchEvent {
        let e = MatchEvent { id: self.next_event, r#type: kind.into(), player_id, msg, ..MatchEvent::default() };
        self.next_event += 1;
        self.recent.push_back(e);
        if self.recent.len() > EVENT_KEEP + 50 {
            let drop = self.recent.len() - EVENT_KEEP;
            self.recent.drain(..drop);
        }
        self.recent.back_mut().expect("just pushed")
    }

    pub fn player_count(&self) -> usize {
        self.st.players.len()
    }

    pub fn out(&self, i: usize) -> bool {
        self.st.players.get(i).is_none_or(|s| s.out())
    }

    /// Public state: hidden-zone sizes and the event tail filled in (`SyncAll`).
    pub fn public_state(&self) -> MatchState {
        let mut st = self.st.clone();
        for (player_id, h) in st.players.iter_mut().zip(&self.hidden) {
            player_id.hand = h.hand.len() as i32;
            player_id.draw = h.draw.len() as i32;
            player_id.discard = h.discard.clone();
        }
        let n = self.recent.len();
        st.events = self.recent.iter().skip(n.saturating_sub(STATE_EVENTS)).cloned().collect();
        st.event_deck = self.event_deck.len() as i32;
        st.event_discard = self.event_discard.clone();
        st.event_removed = self.event_removed.clone();
        st
    }
}
