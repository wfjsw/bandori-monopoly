//! Per-viewer extras computed from **one seat's view** -- the client-side twin
//! of [`game_core::engine::Match::view_extra`] (`docs/BOT.md` §1, the seat-view
//! engine).
//!
//! The live match's `view_extra` reads its own [`Match`] (public state + the
//! viewer's hidden zones + the precomputed [`Ask`] fill). This entry takes the
//! frame the client already holds ([`SeatView`] / `MatchView`), samples every
//! hidden zone once ([`determinize`]), and runs the **same** engine gates on
//! the resulting fork: [`Match::view_extra_typed`] for `playable` / `estCost` /
//! `skills` (`cant_play` / `why_not_act` / `card_prop`), and
//! [`Match::compute_ai_answer`] for `aiAnswer` (the `ai_*` choice functions).
//!
//! Information boundary: the only input is the viewer's own frame. `seed` is
//! the determinizer's sampling seed (never the match's); hidden zones the
//! frame does not pin are sampled from it.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::CardRules;
use game_core::state::SkillAction;

use crate::determinize::{determinize, DeterminizeError, DeterminizerRng};
use crate::view::{AiAnswer, SeatView};

/// The per-viewer extras the client's hand UI / skill stack / 托管 autopilot
/// read on top of the shared `MatchState`. Same field names as
/// [`game_core::engine::Match::view_extra`] / the client's `MatchView`.
#[derive(Debug, Clone, Default)]
pub struct ViewExtras {
    /// Parallel to [`SeatView::hand`]: would `cant_play` allow each card now?
    pub playable: Vec<bool>,
    /// Parallel to `playable`: the card's bot-only estimated execution cost
    /// (`prop::EST_COST`). Reserve check only, never legality.
    pub est_cost: Vec<i32>,
    /// The viewer's pressable skills, gated through `why_not_act`.
    pub skills: Vec<SkillAction>,
    /// The standard bot's answer for the viewer's own live prompt, if any.
    pub ai_answer: Option<AiAnswer>,
    /// The policy's whole next command (`Match::bot_action`) -- the one
    /// implementation the client 托管 driver sends, instead of re-deciding in
    /// TypeScript. `None` when the seat has nothing to decide.
    pub auto_action: Option<game_core::net::NetMessage>,
}

impl ViewExtras {
    /// The `MatchView` / `view_extra` JSON shape (camelCase field names).
    pub fn to_json(&self) -> serde_json::Value {
        let ai = self.ai_answer.as_ref().map(|a| {
            serde_json::json!({
                "answer": a.answer,
                "picked": a.picked,
                "worth": a.worth,
            })
        });
        serde_json::json!({
            "playable": self.playable,
            "estCost": self.est_cost,
            "skills": self.skills,
            "aiAnswer": ai,
            "autoAction": self.auto_action,
        })
    }
}

/// Compute the viewer's extras from their own seat view.
///
/// Builds the determinized fork once and runs the same engine gates the
/// server's [`Match::view_extra`] runs. `seed` seeds the determinizer's
/// sampling stream (hidden zones the frame does not pin); it is **not** the
/// match RNG.
pub fn view_extras(
    view: &SeatView,
    data: &Arc<GameData>,
    rules: &Arc<dyn CardRules>,
    seed: u64,
) -> Result<ViewExtras, DeterminizeError> {
    let seat = view.player_id.max(0) as usize;
    if seat >= view.state.players.len() {
        return Err(DeterminizeError::NoSeat);
    }
    let member = view.you;
    let mut rng = DeterminizerRng::new(seed);
    let (mut m, _report) = determinize(view, data, rules, &mut rng)?;
    // Same gates as the server path (`Match::view_extra_parts`): `cant_play`
    // per hand card, `card_prop(EST_COST)`, `why_not_act` per skill.
    let extra = m.view_extra_typed(member);
    // The client 托管 driver's whole command, from the same policy the server
    // runs -- no second policy on the page. It is **computed** on the fork
    // (the `ai_*` choice functions), not read from the stubbed `Ask` fill --
    // the online frame no longer carries the live match's precomputed answer,
    // and the fork's stub can only echo it. `aiAnswer` is derived from it (one
    // computation, so the two can never disagree on a shared RNG draw).
    let auto_action = extra.auto_action;
    let ai_answer = auto_action.as_ref().and_then(|cmd| {
        if cmd.act != "answer" {
            return None;
        }
        Some(AiAnswer {
            answer: cmd.value,
            picked: cmd.cards.clone(),
            // An auction's ceiling rides in `value`; other prompts' is 0.
            worth: if view.state.prompt.kind == "auction" {
                cmd.value
            } else {
                0
            },
        })
    });
    Ok(ViewExtras {
        playable: extra.playable,
        est_cost: extra.est_cost,
        skills: extra.skills,
        auto_action,
        ai_answer: ai_answer.or_else(|| {
            extra.ai_answer.map(|a| AiAnswer {
                answer: a.answer,
                picked: a.picked,
                worth: a.worth,
            })
        }),
    })
}
