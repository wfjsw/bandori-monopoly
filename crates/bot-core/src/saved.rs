//! The engine's save format, built from the outside (`Match::restore`'s JSON).
//!
//! `Match::save` writes a private `Saved` struct (`engine/mod.rs`). Its field
//! names and serde shapes are the public wire format (`SAVE_VERSION` is public
//! and `record::EngineStamp` names it), so a fork can be materialised by
//! writing the same JSON and calling the public [`game_core::engine::Match::restore`]
//! -- no engine internals touched. The private pieces (`Pending` / `Live` /
//! `Routine`) are mirrored here; the rest are the engine's own public serde
//! types.
//!
//! ## Required engine APIs (docs/BOT.md B4 list)
//!
//! The view does **not** carry everything `Saved` holds. What is missing and
//! how it is stubbed here:
//!
//! * `Pending.snapshot` -- the world the halted routine will re-run from.
//!   The view shows the *partial* world at the halt, not the pre-routine
//!   snapshot. Stub: the partial world. Continuation of a mid-prompt fork is
//!   approximate until `Match::view_routine_base` (below) lands.
//! * `Pending.routine` -- which routine halted. Not in the view. Stub:
//!   `Routine::Ai(turn)` so a completed prompt continues through the
//!   heuristic step machine instead of the true routine's remainder.
//! * `Ask.ai` / `Ask.ai_picked` / `Ask.worth` -- per-seat heuristic answers
//!   and auction ceilings, computed at ask time. Only the *own* seat's entry
//!   is public (`view_extra`). Stub: own seat from `aiAnswer`, others =
//!   prompt fallback; auction `worth` = the quoted price capped at
//!   `money - 1000`.
//! * `World.turn` (`TurnCtx`) extras, `scheduled`, `targeted`, `gains`,
//!   `ring_bonus`, `leftovers`, `next_turn_pending`, `ask_seq`, `signals` --
//!   none of these are in [`game_core::state::MatchState`]. Stub: engine
//!   defaults.
//!
//! Proposed small engine API (exact signature), once the purchasing migration
//! is done -- a view-side extractor, so the bot service (which only ever
//! receives a view) can rebuild a faithful `Pending`:
//!
//! ```ignore
//! impl Match {
//!     /// Public projection of the halted routine's restart base, plus the
//!     /// World fields `MatchState` does not carry. Hidden zones are sizes
//!     /// only; the determinizer fills samples. `None` when nothing is pending.
//!     pub fn view_routine_base(&self, member: i32) -> Option<serde_json::Value>;
//! }
//! ```
//!
//! returning
//! `{ "routine": <Routine>, "snapshotState": <MatchState>,
//!    "snapshotHiddenSizes": [[hand, draw], ...], "answers": [<Answered>],
//!    "turn": {...}, "scheduled": [...], "nextEvent": i32, "askSeq": i32,
//!    "nextTurnPending": bool, "ringBonus": i32,
//!    "askAi": [...], "askPicked": [...], "askWorth": [...] }`.
//!
//! With that, everything below marked *stub* becomes exact.

use game_core::data::GameData;
use game_core::engine::{Answered, Ask, World, SAVE_VERSION};
use game_core::net::NetMessage;
use game_core::rng::Rng;
use game_core::state::{MatchPrompt, MatchVote};
use game_core::MatchMode;
use serde::{Deserialize, Serialize};

use crate::view::AiAnswer;

/// Mirror of the engine's private `Routine` enum (default externally-tagged
/// serde, same names as `engine/mod.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutineMirror {
    Opening,
    NextTurn,
    Ai(usize),
    Act(usize, Box<NetMessage>),
    Leftovers(Vec<usize>),
}

/// Mirror of the engine's private `Live`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMirror {
    pub ask: Ask,
    pub answers: Vec<i32>,
    pub picked: Vec<String>,
    pub ai_at: Vec<Option<f32>>,
    pub time_left: f32,
    pub bid: i32,
    pub bidder: i32,
}

/// Mirror of the engine's private `Pending`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMirror {
    pub routine: RoutineMirror,
    pub snapshot: World,
    pub answers: Vec<Answered>,
    pub live: LiveMirror,
}

/// Mirror of the engine's private `Saved` -- what `Match::restore` reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMirror {
    pub version: u32,
    pub mode: MatchMode,
    pub world: World,
    pub pending: Option<PendingMirror>,
    pub live_rng: Rng,
    pub wait: f32,
    pub bank: Vec<f32>,
    pub shield: f32,
    pub timed_out: bool,
    pub vote: MatchVote,
    pub vote_seq: i32,
    pub vote_cooldown: f32,
    /// `Deferred` is private and never reconstructed: the fork starts clean.
    pub deferred: Vec<serde_json::Value>,
    pub seq: i32,
}

impl SavedMirror {
    /// The `Match::restore` JSON for a determinized world.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("save mirror serializes")
    }

    /// The same document as a value, for [`game_core::engine::Match::restore_value`]
    /// -- skips the string round trip the fork materialisation used to pay
    /// (~10 ms per iteration, `docs/BOT.md` B4).
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("save mirror serializes")
    }
}

/// What the stubbed [`PendingMirror`] reconstruction approximated, so callers
/// (and the consistency tests) can see the gaps rather than trust them.
#[derive(Debug, Clone, Default)]
pub struct PendingGaps {
    /// The routine restart base is the partial world, not the true snapshot.
    pub snapshot_is_partial: bool,
    /// The halted routine was replaced by `Routine::Ai(turn)`.
    pub routine_is_stub: bool,
    /// Other seats' `ask.ai` / `ask.worth` were filled from fallbacks/price.
    pub ask_ai_is_stub: bool,
}

/// Rebuild a `Pending` for a mid-prompt fork from the view's prompt.
///
/// `prompt` is the view's `MatchState.prompt` (already carrying `answers` /
/// `bid` / `bidder` as the client sees them). `turn` is the current turn seat
/// (the stub's `Routine::Ai` target). `ai_self` is the searching seat's public
/// `aiAnswer`, when the prompt carries one.
pub fn stub_pending(
    data: &GameData,
    world: &World,
    prompt: &MatchPrompt,
    turn: usize,
    ai_self: Option<(i32, &AiAnswer)>,
) -> (PendingMirror, PendingGaps) {
    let mut ai = vec![prompt.fallback; prompt.players.len()];
    let mut ai_picked = vec![Vec::<String>::new(); prompt.players.len()];
    let mut worth = vec![0; prompt.players.len()];
    if let Some((pid, ans)) = ai_self {
        if let Some(k) = prompt.player_index(pid) {
            ai[k] = ans.answer;
            ai_picked[k] = ans.picked.clone();
            worth[k] = ans.worth;
        }
    }
    // Auction ceilings for the other seats are hidden. Approximate with the
    // quoted tile price: a standard bot's ceiling is around the price (the
    // real one randomises 0.6-1.3x, capped at money - 1000).
    if prompt.kind == "auction" {
        let base = if prompt.tile >= 0 {
            game_core::engine::purchase::quote_native(data, &world.st, prompt.tile as usize).max(0)
        } else {
            0
        };
        for (k, &pid) in prompt.players.iter().enumerate() {
            if worth[k] == 0 {
                let money = world
                    .st
                    .players
                    .get(pid as usize)
                    .map(|p| p.money)
                    .unwrap_or(0);
                worth[k] = base.min(money.saturating_sub(1000)).max(0);
            }
        }
    }
    let ask_json = serde_json::json!({
        "view": prompt,
        "ai": ai,
        "ai_picked": ai_picked,
        "worth": worth,
    });
    let ask: Ask = serde_json::from_value(ask_json).expect("Ask mirrors its serde shape");

    let live = LiveMirror {
        ask,
        answers: prompt.answers.clone(),
        picked: Vec::new(),
        ai_at: vec![None; prompt.players.len()],
        time_left: prompt.time_left.max(0.0),
        bid: prompt.bid,
        bidder: prompt.bidder,
    };
    let pending = PendingMirror {
        // Stub: continue through the heuristic step machine from the partial
        // world. See the module docs -- `Match::view_routine_base` makes this
        // exact.
        routine: RoutineMirror::Ai(turn),
        snapshot: world.clone(),
        answers: Vec::new(),
        live,
    };
    (
        pending,
        PendingGaps {
            snapshot_is_partial: true,
            routine_is_stub: true,
            ask_ai_is_stub: true,
        },
    )
}

/// Assemble the full [`SavedMirror`] for a determinized world.
pub fn saved(world: World, pending: Option<PendingMirror>, live_rng: Rng, seq: i32) -> SavedMirror {
    let mode = MatchMode::from_i32(world.st.mode).unwrap_or(MatchMode::Solo);
    let n = world.st.players.len();
    SavedMirror {
        version: SAVE_VERSION,
        mode,
        bank: vec![0.0; n],
        vote_seq: world.st.vote.id,
        vote: world.st.vote.clone(),
        world,
        pending,
        live_rng,
        wait: 0.0,
        shield: 0.0,
        timed_out: false,
        vote_cooldown: 0.0,
        deferred: Vec::new(),
        seq,
    }
}