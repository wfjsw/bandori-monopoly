//! The [`Simulator`] trait and the [`MatchSim`] driver over the public
//! [`Match`] API (`docs/BOT.md` §3.4).
//!
//! A fork is a real [`Match`] restored from the determinizer's save JSON.
//! Decisions are applied through the public `act` commands; the engine's own
//! heuristic bot (`ai.rs` / `aiAnswer`) plays every seat during rollout. The
//! searching seat is never auto-played during tree descent: its `ai` flag is
//! dropped so the engine parks at its decisions, and restored for the rollout
//! so the heuristic drives it too.

use std::sync::Arc;
use std::time::Duration;

use crate::clock::Instant;

use game_core::data::GameData;
use game_core::engine::{wants_buy, CardRules, HeuristicProvider, Match};
use game_core::net::NetMessage;
use game_core::state::{MatchState, stage};

use crate::action::{self, Action, Surface};
use crate::determinize::{determinize_value, DeterminizerRng};
use crate::eval;
use crate::view::{AiAnswer, SeatView};

/// Why a simulation step failed.
#[derive(Debug, Clone)]
pub enum SimError {
    Act(String),
    Determinize(crate::determinize::DeterminizeError),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Act(e) => write!(f, "act refused: {e}"),
            SimError::Determinize(e) => write!(f, "determinize: {e}"),
        }
    }
}

impl std::error::Error for SimError {}

/// How far a rollout runs.
#[derive(Debug, Clone, Copy)]
pub struct Horizon {
    /// Stop once `state.round >= start_round + rounds` (or the game ends).
    pub rounds: u32,
}

/// What [`Simulator::advance`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The searching seat has a decision.
    Decision,
    /// The horizon was reached.
    Horizon,
    /// The game ended.
    Ended,
}

/// Target-independent simulation surface. The ISMCTS core only ever sees
/// this.
pub trait Simulator {
    type Fork;

    /// Sample a determinization of the root information set.
    fn determinize(&mut self, rng: &mut DeterminizerRng) -> Result<Self::Fork, SimError>;

    /// Abstracted legal actions for `seat` at the fork's current decision.
    /// Empty = heuristic-delegated (not a tree decision).
    fn legal_actions(&mut self, fork: &Self::Fork, seat: usize) -> Vec<Action>;

    /// Apply one abstracted action at the current decision.
    fn apply(&mut self, fork: &mut Self::Fork, seat: usize, action: &Action) -> Result<(), SimError>;

    /// Advance to the searching seat's next decision, the horizon, or the end.
    ///
    /// `intercept_seat` = true during tree descent: the seat's `ai` flag is
    /// off so the engine parks at its decisions. False during rollout: the
    /// seat is a bot and the heuristic plays it.
    fn advance(
        &mut self,
        fork: &mut Self::Fork,
        seat: usize,
        horizon: Horizon,
        intercept_seat: bool,
    ) -> Advance;

    /// Static evaluation for `seat`, normalised.
    fn evaluate(&self, fork: &Self::Fork, seat: usize) -> f64;

    /// The *simulation outcome* -- a separate statistic from [`Self::evaluate`]
    /// (Lanctot et al. CIG 2014): the terminal win / score when the rollout
    /// ended in a finished game, otherwise the cutoff value. Default: same as
    /// [`Self::evaluate`].
    fn simulation_value(&self, fork: &Self::Fork, seat: usize) -> f64 {
        self.evaluate(fork, seat)
    }

    /// Heuristic prior per action at the current decision, in `[0, 1]`
    /// (progressive bias, Chaslot et al. 2008). One call per node so the
    /// implementation can share the view work. Neutral `0.5` by default.
    fn action_priors(&self, fork: &Self::Fork, seat: usize, actions: &[Action]) -> Vec<f64> {
        let _ = (fork, seat);
        vec![0.5; actions.len()]
    }

    /// Information-set identity of the current decision.
    fn decision_key(&self, fork: &Self::Fork, seat: usize) -> u64;
}

/// [`Simulator`] over the engine's public [`Match`] API.
pub struct MatchSim {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    /// The root view -- the determinizer's only input.
    pub root: SeatView,
    /// The searching seat's player index.
    pub seat: usize,
    /// The searching seat's member id (`act` takes a member).
    pub member: i32,
    /// Safety valve on `tick` steps inside one `advance`.
    pub max_ticks: u32,
    /// Answer-provider mode (`docs/BOT.md` §3.2): during the rollout every
    /// prompt is answered inline by [`HeuristicProvider`] instead of halting
    /// the routine and replaying it, so a routine runs once rather than once
    /// per prompt. The tree descent keeps the halt/replay path for the prompts
    /// the tree branches on (they must surface as `Advance::Decision`); the
    /// old path is behind this flag for comparison. Default on.
    pub inline: bool,
}

impl MatchSim {
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        root: SeatView,
        seat: usize,
        member: i32,
    ) -> Self {
        Self {
            data,
            rules,
            root,
            seat,
            member,
            max_ticks: 4_000,
            inline: true,
        }
    }

    /// Compare against the pre-B2 halt/replay path.
    pub fn with_inline(mut self, inline: bool) -> Self {
        self.inline = inline;
        self
    }
}

impl Simulator for MatchSim {
    type Fork = Match;

    fn determinize(&mut self, rng: &mut DeterminizerRng) -> Result<Match, SimError> {
        // Materialise the fork in memory, then take the searching seat over:
        // its `ai` flag is dropped **before** `Match::restore_value`, so the
        // engine never auto-plays the seat (tree descent and rollout both
        // drive it explicitly). The flag lives in the public state, so this is
        // a documented edit of the sampled fork -- not of the view, and not of
        // the live match. Mid-prompt forks cannot be edited through
        // `world_mut` (it asserts no pending routine), which is why the flag
        // is set on the document rather than after restore. `restore_value`
        // skips the string round trip (`docs/BOT.md` B4).
        let (mut saved, _) = determinize_value(&self.root, self.data.as_ref(), rng)
            .map_err(SimError::Determinize)?;
        if let Some(players) = saved
            .get_mut("world")
            .and_then(|w| w.get_mut("st"))
            .and_then(|s| s.get_mut("players"))
            .and_then(|p| p.as_array_mut())
        {
            if let Some(p) = players.get_mut(self.seat) {
                p["ai"] = serde_json::Value::Bool(false);
            }
        }
        let m = Match::restore_value(self.data.clone(), self.rules.clone(), saved)
            .map_err(|e| SimError::Act(format!("{:?}", e)))?;
        Ok(m)
    }

    fn legal_actions(&mut self, fork: &Match, seat: usize) -> Vec<Action> {
        if seat != self.seat {
            return Vec::new();
        }
        let st = fork.state();
        let hand = fork.hand_of(self.member);
        let extra = fork.view_extra(self.member);
        let playable = extra
            .get("playable")
            .and_then(|v| serde_json::from_value::<Vec<bool>>(v.clone()).ok())
            .unwrap_or_default();
        let est_cost = extra
            .get("estCost")
            .and_then(|v| serde_json::from_value::<Vec<i32>>(v.clone()).ok())
            .unwrap_or_default();
        action::legal_actions_with_cost(&self.data, &st, &hand, &playable, &est_cost, seat)
    }

    fn apply(&mut self, fork: &mut Match, seat: usize, action: &Action) -> Result<(), SimError> {
        let st = fork.state();
        let msg = action::to_net_message(action, &st, seat);
        fork.act(self.member, &msg)
            .map_err(|e| SimError::Act(format!("{:?}", e)))
    }

    fn advance(
        &mut self,
        fork: &mut Match,
        seat: usize,
        horizon: Horizon,
        intercept_seat: bool,
    ) -> Advance {
        // Simulation mode (`docs/BOT.md` §3.2): during the rollout every
        // prompt is answered inline by the heuristic, so a routine runs once
        // instead of once per prompt. During tree descent the provider is off
        // -- a prompt the tree branches on must surface as `Advance::Decision`,
        // which is the halt/replay path. The old path is `inline = false`.
        let install = self.inline && !intercept_seat;
        if install {
            fork.set_provider(Some(Box::new(HeuristicProvider::new())));
        }
        let r = self.advance_inner(fork, seat, horizon, intercept_seat);
        if install {
            fork.set_provider(None);
        }
        r
    }

    fn evaluate(&self, fork: &Match, seat: usize) -> f64 {
        eval::relative_worth(&self.data, &fork.state(), seat)
    }

    /// Terminal win / score when the rollout finished the game, else the
    /// cutoff value (`docs/BOT-RESEARCH` #3: keep the two statistics apart).
    fn simulation_value(&self, fork: &Match, seat: usize) -> f64 {
        if fork.ended() {
            return eval::terminal_value(&fork.state(), seat);
        }
        eval::relative_worth(&self.data, &fork.state(), seat)
    }

    fn action_priors(&self, fork: &Match, seat: usize, actions: &[Action]) -> Vec<f64> {
        if seat != self.seat {
            return vec![0.5; actions.len()];
        }
        let st = fork.state();
        let hand = fork.hand_of(self.member);
        let extra = fork.view_extra(self.member);
        let ai_answer = extra
            .get("aiAnswer")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value::<AiAnswer>(v.clone()).ok());
        let est_cost = extra
            .get("estCost")
            .and_then(|v| serde_json::from_value::<Vec<i32>>(v.clone()).ok())
            .unwrap_or_default();
        action::action_priors(
            &self.data,
            &st,
            &hand,
            &est_cost,
            ai_answer.as_ref(),
            seat,
            actions,
        )
    }

    fn decision_key(&self, fork: &Match, seat: usize) -> u64 {
        let _ = seat;
        SeatView::from_match(fork, self.member).decision_key()
    }
}

impl MatchSim {
    /// [`Simulator::advance`] without the provider install -- see that method.
    fn advance_inner(
        &mut self,
        fork: &mut Match,
        seat: usize,
        horizon: Horizon,
        intercept_seat: bool,
    ) -> Advance {
        let start_round = fork.state().round.max(0) as u32;
        // After the heuristic answers `end`, the engine leaves `step == 结束`
        // and `turn == seat` until `NextTurn` runs -- the surface looks live
        // but the turn is over. Suppress it until the state moves on.
        let mut turn_ended = false;
        let mut ticks = 0u32;
        // Consecutive prompts where neither the heuristic nor the fallback
        // answer was accepted. A stale surface (an auction whose live minimum
        // moved under us, say) used to spin here up to `max_ticks` -- and each
        // tick lets the other seats' bots bid, making the surface staler
        // (the 58 ms/rollout regression in `docs/BOT.md` B4/B5).
        let mut refused_prompts = 0u32;
        loop {
            if fork.ended() {
                return Advance::Ended;
            }
            let st = fork.state();
            if st.round.max(0) as u32 >= start_round + horizon.rounds {
                return Advance::Horizon;
            }
            // The searching seat is never auto-played (`ai = false` in the
            // fork), so every decision of its is handled here: returned to
            // the tree in descent, answered by the heuristic in rollout.
            if st.prompt.id > 0 && st.prompt.waiting(seat as i32) {
                let searchable = matches!(
                    action::surface(&st, seat),
                    Some(Surface::Prompt(ref p)) if action::searchable_prompt(p)
                );
                if searchable && intercept_seat {
                    return Advance::Decision;
                }
                let msg = heuristic_message_with(&self.data, fork, self.member, &st);
                let ok = fork.act(self.member, &msg).is_ok();
                let ok = ok
                    || fork
                        .act(
                            self.member,
                            &NetMessage {
                                act: "answer".into(),
                                prompt: st.prompt.id,
                                value: st.prompt.fallback,
                                ..Default::default()
                            },
                        )
                        .is_ok();
                if ok {
                    refused_prompts = 0;
                    continue;
                }
                // Both answers refused. One more tick is fair (the clock has
                // to move); two refusals on the bounce mean the surface is
                // not answerable and the rollout ends here.
                refused_prompts += 1;
                if refused_prompts >= 2 {
                    return Advance::Horizon;
                }
            }
            if !turn_ended {
                if let Some(Surface::Turn) = action::surface(&st, seat) {
                    let acts = self.legal_actions(fork, seat);
                    if !acts.is_empty() && intercept_seat {
                        return Advance::Decision;
                    }
                    // Rollout (or heuristic-delegated surface): the engine's
                    // own turn policy -- buy / build / discard / end, or roll.
                    let msg = heuristic_message_with(&self.data, fork, self.member, &st);
                    let is_end = msg.act == "end";
                    if fork.act(self.member, &msg).is_err() {
                        // A refused roll (main move already spent) falls
                        // through to `end`.
                        let _ = fork.act(self.member, &NetMessage::act("end"));
                        turn_ended = true;
                    } else if is_end {
                        turn_ended = true;
                    }
                    continue;
                }
            }
            if turn_ended {
                // Clear the latch once the state has moved on.
                if st.turn != seat as i32
                    || (st.step != stage::END && st.step != stage::NONE)
                    || st.prompt.id > 0
                {
                    turn_ended = false;
                }
            }
            ticks += 1;
            if ticks > self.max_ticks {
                return Advance::Horizon;
            }
            fork.tick(0.25);
        }
    }
}

/// Map the engine's `aiAnswer` at a prompt onto an abstracted action. `seat`
/// is the answering player index (the auction rules need it).
pub fn ai_answer_to_action(st: &MatchState, a: &AiAnswer, seat: usize) -> Option<Action> {
    let p = &st.prompt;
    match p.kind.as_str() {
        "auction" => {
            // The engine refuses any positive bid from the current top
            // bidder (`err.already_top_bid`) and anything under `bid + 100`
            // (`err.bid_min`). A stale `aiAnswer.worth` that re-bids our own
            // bid or underbids the live minimum is a guaranteed refusal -- and
            // a refused answer lets the other seats' bots raise on the next
            // tick, so the state goes stale and the loop spins (B4 regression
            // triage 2026-10-07). Pass unless the heuristic's ceiling is a
            // *legal raise* from a seat that is not already on top.
            if p.bidder == seat as i32 {
                return Some(Action::Bid { amount: -1 });
            }
            if a.worth > p.bid && a.worth > 0 {
                let min = if p.bid <= 0 { 100 } else { p.bid + 100 };
                let amount = (a.worth.max(min) / 100 * 100).max(min);
                Some(Action::Bid { amount })
            } else {
                Some(Action::Bid { amount: -1 })
            }
        }
        "mortgage" => Some(Action::Mortgage {
            tiles: a.picked.iter().filter_map(|s| s.parse().ok()).collect(),
        }),
        "pick" => Some(Action::Pick {
            index: a.answer.max(0),
        }),
        "tile" => Some(Action::Pick {
            index: a.answer.max(0),
        }),
        "choice" => {
            if p.title.k.as_ref() == "ask.counteract.title" {
                Some(Action::Counteract {
                    card: if a.answer == p.fallback {
                        None
                    } else {
                        Some(p.card.clone())
                    },
                })
            } else {
                Some(Action::Offer {
                    index: a.answer.max(0),
                })
            }
        }
        _ => None,
    }
}

/// The heuristic command at the current decision, as a public `act` message.
/// Prompts use the engine's `aiAnswer`; the turn surface uses the `ai_step`
/// order (buy at 结束, else end; roll at 运营).
///
/// View-only: the bot service's fallback path works from the frame it was
/// sent, with no [`Match`] to re-ask (`docs/BOT.md` §1).
pub fn heuristic_message_view(data: &GameData, view: &SeatView) -> NetMessage {
    let st = &view.state;
    let player_id = view.player_id;
    if st.prompt.id > 0 && st.prompt.waiting(player_id) {
        if let Some(ans) = &view.ai_answer {
            if let Some(action) = ai_answer_to_action(st, ans, player_id.max(0) as usize) {
                return action::to_net_message(&action, st, player_id.max(0) as usize);
            }
        }
        return NetMessage {
            act: "answer".into(),
            prompt: st.prompt.id,
            value: st.prompt.fallback,
            ..Default::default()
        };
    }
    // Turn surface, ported from `ai_step` / autopilot.ts `turn` (standard).
    let me = player_id.max(0) as usize;
    // Over the hand limit: `end` is refused (`err.over_hand`) until the hand
    // is down to the limit -- `ai_step`'s discard branch.
    let limit = st.players.get(me).map(|p| p.hand_limit()).unwrap_or(5);
    let hand = &view.hand;
    if hand.len() as i32 > limit {
        if let Some(c) = hand.first() {
            return NetMessage {
                act: "discard".into(),
                card: c.clone(),
                ..Default::default()
            };
        }
    }
    if st.step == stage::OPS {
        // `end` only where the engine accepts it (`why_not_act`'s end branch).
        // The public `skip_move` re-derives from stay / exile and can disagree
        // with the gate (unstoppable / mid-turn [停留] / [除外]) -- trusting it
        // sent `end` into `err.roll_first` and stalled the seat (bot_cpu §6).
        if st.can_end_here && !st.can_roll_here {
            return NetMessage::act("end");
        }
        return NetMessage::act("roll");
    }
    if st.step == stage::END {
        let t = st.landed;
        if t >= 0 {
            let t = t as usize;
            let money = st.players.get(me).map(|p| p.money).unwrap_or(0);
            if action::buyable(data, st, me, t) && wants_buy(money, st.buy_price.max(0)) {
                return NetMessage {
                    act: "buy".into(),
                    value: t as i32,
                    ..Default::default()
                };
            }
            if action::can_build(data, st, me, t)
                && game_core::engine::wants_build(money, st.build_cost.max(0))
            {
                return NetMessage {
                    act: "build".into(),
                    value: t as i32,
                    ..Default::default()
                };
            }
        }
        // Over-hand is handled above; `ai_step`'s order is buy / build /
        // discard / end and the search branches on buy / build.
        return NetMessage::act("end");
    }
    NetMessage::act("end")
}

/// [`heuristic_message_view`] over a forked [`Match`]'s own accessors.
pub fn heuristic_message_with(
    data: &GameData,
    fork: &Match,
    member: i32,
    st: &MatchState,
) -> NetMessage {
    let mut view = SeatView::from_match(fork, member);
    // The caller often already holds a fresher `state()` than `from_match`
    // re-reads; prefer the one it passed.
    if st.prompt.id != view.state.prompt.id || st.seq != view.state.seq {
        view.state = st.clone();
    }
    heuristic_message_view(data, &view)
}

/// Drive a live (or forked) [`Match`] to the searching seat's next decision.
/// The seat is a human (`ai = false`) and the other seats are bots. Returns
/// false when the game ended first.
pub fn next_decision(m: &mut Match, member: i32, max_ticks: u32) -> bool {
    let player_id = m.state().player_of(member);
    for _ in 0..max_ticks {
        if m.ended() {
            return false;
        }
        let st = m.state();
        if st.prompt.id > 0 && st.prompt.waiting(player_id) {
            return true;
        }
        if st.phase == "play" && !st.busy {
            let my_turn = st.turn == player_id;
            if my_turn && matches!(st.step, s if s == stage::OPS || s == stage::END) {
                return true;
            }
        }
        m.tick(0.25);
    }
    false
}

/// Wall-clock budget for one decision.
pub fn budget_ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

/// A cheap deadline for the anytime search.
pub struct Deadline(Instant);

impl Deadline {
    pub fn after(d: Duration) -> Self {
        Self(Instant::now() + d)
    }
    pub fn passed(&self) -> bool {
        Instant::now() >= self.0
    }
}
