//! # spec-kit -- generic formal-verification kit
//!
//! One checkable **clause** per rulebook sentence, evaluated over one observed
//! engine transition. The kit is **generic**: it names only engine types
//! ([`World`], [`MatchEvent`], [`StepAct`], [`GameData`]) and never speaks a
//! game's vocabulary (no group / persona / tile / token / card names). The
//! game-specific harness and clause tables live in `rules/spec`.
//!
//! ## The unit of observation
//!
//! A [`Step`] is one player input and everything it caused:
//!
//! * `pre` -- the world just before the input (cloned via `Match::world()`,
//!   which is read-only and available even while a prompt is pending;
//!   `world_mut` asserts and must not be used here).
//! * `post` -- the world after the input **and** the caller's settle loop.
//! * `events` -- the event-tail window the input logged (read per-act: the
//!   tail is capped at [`EVENT_KEEP`](game_core::engine::world::EVENT_KEEP)
//!   entries).
//! * `act` -- the input itself ([`StepAct`]).
//! * `data` -- the static game data the match loaded.
//!
//! A [`Clause`] is `{id, source, quote, check}`. `check` is a plain function
//! over one [`Step`]; `Ok(())` means the transition honours the quote, `Err`
//! describes how it did not. [`check`] runs a clause list and collects
//! [`Violation`]s. A discrepancy is a **plain failing test** -- never
//! `#[ignore]`d, never allowlisted.
//!
//! ## Where clauses get run
//!
//! * [`observe::observe`] -- the one choke point that performs a player input
//!   on a [`Match`](game_core::engine::Match), settles, and checks. The
//!   game harness (`rules/spec::Table`) routes **every** input through it.
//! * [`snap::frame`] -- a frame condition: everything outside the allowed
//!   path prefixes is unchanged.
//! * [`prop::forall`] -- a seeded property, for per-card contracts.
//!
//! ## Adding a clause
//!
//! 1. Write the doc entry (`docs/rulebook/spec/...`) with ID / Source / Quote /
//!    Trigger / Condition / Effect / Frame / Rulings & OPEN / Executable.
//!    The doc ID and the code ID are the same string.
//! 2. Add the `Clause` to the area's `CLAUSES` slice in `rules/spec/src/global/`
//!    (or to a card contract in `rules/tests/tests/spec_card_*.rs`).
//! 3. The `check` closure/fn must follow from the quote alone. Never encode
//!    observed engine behaviour.

pub mod observe;
pub mod prop;
pub mod snap;

use std::fmt;

use game_core::data::GameData;
use game_core::engine::World;
use game_core::net::NetMessage;
use game_core::state::MatchEvent;

pub use observe::{observe, Observed};
pub use prop::{forall, Rng};
pub use snap::{diff, frame, snapshot, Delta, Snap};

/// One checkable rulebook clause. The doc entry and this struct share `id`.
#[derive(Clone, Copy)]
pub struct Clause {
    /// Stable identifier, e.g. `"G-MONEY-01"`. Doc and code agree.
    pub id: &'static str,
    /// Where the text comes from, e.g. `"rulebook-doc.md:60"`.
    pub source: &'static str,
    /// Verbatim quote of the source. Shown in every violation.
    pub quote: &'static str,
    /// The executable check over one observed transition.
    pub check: fn(&Step<'_>) -> Result<(), String>,
}

impl Clause {
    /// Build a clause. Keeps the field order readable at the definition site.
    pub const fn new(
        id: &'static str,
        source: &'static str,
        quote: &'static str,
        check: fn(&Step<'_>) -> Result<(), String>,
    ) -> Self {
        Self {
            id,
            source,
            quote,
            check,
        }
    }
}

impl fmt::Debug for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clause({} from {})", self.id, self.source)
    }
}

/// One observed transition: pre-world, post-world, the events it logged,
/// the act that caused it, and the static game data.
pub struct Step<'a> {
    /// The world just before the input.
    pub pre: &'a World,
    /// The world after the input and the settle loop.
    pub post: &'a World,
    /// Events logged by this input (and its settle). Per-act window: the
    /// engine's event tail is capped, so never walk further back than this.
    pub events: &'a [MatchEvent],
    /// The player input that produced this step.
    pub act: &'a StepAct,
    /// Static data (tiles, cards, personas, events, match rules).
    pub data: &'a GameData,
    /// A prompt was waiting **before** the input. The live prompt lives on the
    /// host (`Match`), not in [`World`], so it is captured alongside the step.
    pub pre_prompt_open: bool,
    /// A prompt is waiting **after** the settle loop. A clause about "at
    /// rest" must skip when this is true -- the step stopped mid-decision.
    pub post_prompt_open: bool,
}

impl Step<'_> {
    /// Seat index of the player who sent [`self.act`](StepAct).
    ///
    /// `StepAct::who` is the engine **member** id, not a player index. The
    /// game harness seats player `k` as member `k + 1`, but always look the
    /// member up rather than assuming -- a reordered roster would silently
    /// check the wrong seat.
    pub fn player_of_act(&self) -> usize {
        let member = self.act.who();
        self.pre
            .st
            .players
            .iter()
            .position(|p| p.member == member)
            .unwrap_or_else(|| panic!("no player for member {member}"))
    }

    /// Message keys of this step's events (see [`event_key`]), for violation
    /// reports and assertion messages.
    pub fn event_keys(&self) -> Vec<String> {
        self.events.iter().map(event_key).collect()
    }

    /// Did this step's **engine act** play `card`? Reads the act's
    /// [`NetMessage`] (`act == "play"` + `card`), not the event stream --
    /// frame predicates must key on engine boundaries (the act), never on a
    /// presentation event (a `"card"` activation / 「效果适用」 header), so a
    /// lazy header cannot move the marker into a different act's window.
    pub fn act_is_play(&self, card: &str) -> bool {
        match self.act {
            StepAct::Act { msg, .. } => msg.act == "play" && msg.card == card,
            _ => false,
        }
    }

    /// Did this step's engine act answer a prompt (not a decline)?
    pub fn act_is_answer(&self) -> bool {
        matches!(self.act, StepAct::Answer { .. })
    }

    /// Did this step's engine act name `card` (play / counteract / any act
    /// whose [`NetMessage::card`] is `card`)? Engine-level; never reads the
    /// event stream.
    pub fn act_is_card(&self, card: &str) -> bool {
        match self.act {
            StepAct::Act { msg, .. } => msg.card == card,
            StepAct::Answer { cards, .. } => cards.iter().any(|c| c == card),
            _ => false,
        }
    }

    /// Money of player `i` before the input.
    pub fn pre_money(&self, i: usize) -> i32 {
        self.pre.st.players.get(i).map(|p| p.money).unwrap_or(0)
    }

    /// Money of player `i` after the input.
    pub fn post_money(&self, i: usize) -> i32 {
        self.post.st.players.get(i).map(|p| p.money).unwrap_or(0)
    }

    /// Hand size of player `i` before the input (private zones, not the
    /// public `MatchPlayer::hand` mirror).
    pub fn pre_hand_len(&self, i: usize) -> usize {
        self.pre.hidden.get(i).map(|h| h.hand.len()).unwrap_or(0)
    }

    /// Hand size of player `i` after the input.
    pub fn post_hand_len(&self, i: usize) -> usize {
        self.post.hidden.get(i).map(|h| h.hand.len()).unwrap_or(0)
    }

    /// Events of this step whose engine `type` is one of `types`.
    pub fn events_of<'s>(&'s self, types: &[&str]) -> Vec<&'s MatchEvent> {
        self.events
            .iter()
            .filter(|e| types.contains(&e.r#type.as_str()))
            .collect()
    }
}

/// The player input that produced a [`Step`].
///
/// The game harness maps every `Match::act` call onto one of these three
/// shapes. A decline is a distinct act path so a clause can tell "the player
/// skipped" from "the player chose an option".
#[derive(Clone, Debug)]
pub enum StepAct {
    /// A player command: play / skill / roll / buy / build / mortgage /
    /// redeem / discard / end / ... -- anything that is not a prompt answer.
    Act {
        /// Engine member id (`Table` seats: player `k` is member `k + 1`).
        who: i32,
        /// The raw input message.
        msg: NetMessage,
    },
    /// A prompt answer that is **not** a decline: an option index, a tile
    /// pick, a card pick, a bid, ...
    Answer {
        /// Engine member id of the answering player.
        who: i32,
        /// The prompt id being answered.
        prompt: i32,
        /// The chosen value (option index / tile index / bid / ...).
        value: i32,
        /// The chosen items for `pick` / `mortgage` prompts.
        cards: Vec<String>,
    },
    /// The prompt's fallback / "none" / skip answer. Used by `Table::decline`
    /// and by a driver that deliberately skips.
    Decline {
        /// Engine member id of the answering player.
        who: i32,
        /// The prompt id being answered.
        prompt: i32,
        /// The fallback value that was sent (the prompt's `fallback`, or
        /// `items.len()` for a `tile` prompt meaning "none").
        value: i32,
    },
}

impl StepAct {
    /// The engine member id that sent this input.
    pub fn who(&self) -> i32 {
        match self {
            StepAct::Act { who, .. } => *who,
            StepAct::Answer { who, .. } => *who,
            StepAct::Decline { who, .. } => *who,
        }
    }

    /// The `NetMessage` this act sends to [`Match::act`](game_core::engine::Match::act).
    pub fn to_net(&self) -> NetMessage {
        match self {
            StepAct::Act { msg, .. } => msg.clone(),
            StepAct::Answer {
                prompt,
                value,
                cards,
                ..
            } => NetMessage {
                prompt: *prompt,
                value: *value,
                cards: cards.clone(),
                ..NetMessage::act("answer")
            },
            StepAct::Decline { prompt, value, .. } => NetMessage {
                prompt: *prompt,
                value: *value,
                ..NetMessage::act("answer")
            },
        }
    }

    /// Short label for messages: `Act(play)`, `Answer`, `Decline`, ...
    pub fn label(&self) -> String {
        match self {
            StepAct::Act { msg, .. } => format!("Act({})", msg.act),
            StepAct::Answer { value, cards, .. } => {
                if cards.is_empty() {
                    format!("Answer({value})")
                } else {
                    format!("Answer({value}, {} cards)", cards.len())
                }
            }
            StepAct::Decline { value, .. } => format!("Decline({value})"),
        }
    }
}

/// A clause failure: the id, the quote, how the step broke it, and the
/// event keys of the window (so a panic message can name what happened).
#[derive(Clone, Debug)]
pub struct Violation {
    pub id: &'static str,
    pub quote: &'static str,
    pub detail: String,
    pub event_keys: Vec<String>,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "clause {} violated: {}\n  quote: {}\n  events: [{}]",
            self.id,
            self.detail,
            self.quote,
            self.event_keys.join(", ")
        )
    }
}

/// Run every clause over one step. An empty vec means the transition is clean.
pub fn check(step: &Step<'_>, clauses: &[Clause]) -> Vec<Violation> {
    let mut out = Vec::new();
    for c in clauses {
        if let Err(detail) = (c.check)(step) {
            out.push(Violation {
                id: c.id,
                quote: c.quote,
                detail,
                event_keys: step.event_keys(),
            });
        }
    }
    out
}

/// The key a log event speaks as: its own message key, or the `what` nested
/// inside an attribution line (`log.card_effect`).
///
/// Generic over the engine's log shape -- no game vocabulary. Used so a
/// violation's `event_keys` reads like the game log, not like raw message
/// structs.
pub fn event_key(e: &MatchEvent) -> String {
    use game_core::msg::Arg;
    let m = if e.msg.key() == "log.card_effect" {
        match e.msg.a.get("what") {
            Some(Arg::Msg(inner)) => inner.as_ref(),
            _ => &e.msg,
        }
    } else {
        &e.msg
    };
    let mut k = m.key().to_string();
    // A bare card activation with no line of its own speaks as "" -- name it
    // after the trigger kind so the window is still readable.
    if k.is_empty() && e.r#type == "card" {
        k = format!("card:{}", e.kind);
    }
    k
}

/// Integer argument of a message (`.i` / `.n`), for amount-like fields.
pub fn int_arg(msg: &game_core::msg::Msg, name: &str) -> Option<i64> {
    use game_core::msg::Arg;
    match msg.a.get(name) {
        Some(Arg::I(v)) | Some(Arg::N(v)) => Some(*v),
        _ => None,
    }
}

/// Player-id argument of a message (`.player_id`-style slots).
pub fn player_arg(msg: &game_core::msg::Msg, name: &str) -> Option<i32> {
    use game_core::msg::Arg;
    match msg.a.get(name) {
        Some(Arg::PlayerId(v)) => Some(*v),
        _ => None,
    }
}
