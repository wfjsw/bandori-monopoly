//! `bot-core` -- target-independent bot brain (`docs/BOT.md` B4).
//!
//! Two parts:
//!
//! 1. **Hidden-state determinizer** ([`determinize`]): from *one seat's view*
//!    (the exact frame the client gets -- [`SeatView`]) plus a search-side RNG,
//!    sample every hidden zone and materialise a playable fork through the
//!    engine's save format and the public [`Match::restore`]. The bot never
//!    reads a `World`, a seed, or another seat's hand / deck order /
//!    `aiAnswer`.
//! 2. **ISMCTS core** ([`Ismcts`]): single-observer information-set MCTS,
//!    UCB over abstracted actions ([`Action`]), a fresh determinization per
//!    iteration, heuristic rollouts to a round horizon, static net-worth
//!    evaluation, anytime (wall-clock budget + iteration cap). Generic over
//!    the [`Simulator`] trait; [`MatchSim`] is the driver over the public
//!    [`Match`] API.
//!
//! Advisory only: the sandboxed engine stays authoritative (§1). Search uses
//! its own RNG ([`DeterminizerRng`], seeded per decision) and only ever works
//! on forks.

pub mod action;
pub mod determinize;
pub mod eval;
pub mod ismcts;
pub mod saved;
pub mod sim;
pub mod view;

pub use action::{action_priors, Action, Surface};
pub use determinize::{determinize, determinize_json, DeterminizeError, DeterminizerRng, SampleReport};
pub use eval::{net_worth, relative_worth, terminal_value};
pub use ismcts::{
    merge_stats, seed_for_thread, ActionStats, Ismcts, SearchConfig, SearchOutcome, Timings,
};
pub use sim::{
    ai_answer_to_action, heuristic_message_view, heuristic_message_with, next_decision, Advance,
    Deadline, Horizon, MatchSim, SimError, Simulator,
};
pub use view::{AiAnswer, SeatView};