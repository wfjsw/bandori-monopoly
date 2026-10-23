//! The one observed choke point: perform a player input on a live
//! [`Match`], settle, and check clauses over the resulting transition.
//!
//! Why a single choke point: 21 parallel units write clauses, and every one of
//! them must see the same `Step` shape. If a harness called `Match::act`
//! directly anywhere, that path would silently skip checking. So the game
//! harness (`rules/spec::Table`) routes **every** input through [`observe`] --
//! acts, prompt answers, and declines alike.
//!
//! Prompt-pending safety: this module only reads [`Match::world`], which is
//! always available. [`Match::world_mut`] asserts while a routine is pending
//! and is never called here.

use game_core::engine::Match;

use crate::{check, Clause, Step, StepAct, Violation};

/// What one observed input produced.
pub struct Observed {
    /// The engine's verdict on the input. `Err` means it was rejected and no
    /// transition happened -- the clauses still run, because a rejected input
    /// should be a pure no-op (frame-friendly).
    pub act_result: Result<(), String>,
    /// Clause failures over the transition. Empty when clean.
    pub violations: Vec<Violation>,
}

impl Observed {
    /// True when the input was accepted and no clause ran.
    pub fn clean(&self) -> bool {
        self.act_result.is_ok() && self.violations.is_empty()
    }

    /// Panic with the first violation (clause id + quote + detail + events).
    /// Call this from a harness that treats a discrepancy as a failing test.
    pub fn unwrap_violations(&self) {
        if let Some(v) = self.violations.first() {
            panic!("{v}");
        }
    }
}

/// Perform `act` on `m`, run `settle` (the caller's rest loop), then check
/// `clauses` over the whole transition.
///
/// **Frames are delimited by engine-level boundaries** -- the act and its
/// settle -- never by a presentation event (`"card"` activation, 「效果适用」
/// header position, `flash` flag, `results` lines). A prompting drive spans
/// two acts (the act that opens the prompt, the answer that continues it);
/// frame predicates must key on [`Step::act`] / [`Step::act_is_play`] /
/// [`Step::act_is_card`] (the engine input), not on scanning `step.events`
/// for a card activation. Presentation events may move (lazy headers, flash
/// elision) without changing the frame partition.
///

/// * `pre` is a clone of [`Match::world`] taken **before** the input. `World`
///   clones share the event tail by refcount, so this is cheap.
/// * The event mark is the newest event id before the input; the window is
///   everything logged after it. Read per-act: the tail is capped at
///   [`EVENT_KEEP`](game_core::engine::world::EVENT_KEEP).
/// * `settle` runs even when the input is rejected, so a prompt that the
///   input opened (or that was already open) still gets a chance to rest.
///   Typical value: the harness's `settle` loop. Pass `|_| {}` to observe
///   without settling (the caller settles itself and wants the window to
///   cover only the raw act -- prefer passing a settle closure).
///
/// Arranging writes (`world_mut` setters) are **not** observations: they are
/// setup, not transitions. Only inputs that go through this function are
/// checked.
pub fn observe(
    m: &mut Match,
    act: StepAct,
    settle: impl FnOnce(&mut Match),
    clauses: &[Clause],
) -> Observed {
    // Pre-image and event mark. `world()` is read-only and legal even with a
    // prompt pending or a routine halted mid-effect. The live prompt lives on
    // the host, not in `World`, so capture it alongside.
    let pre = m.world().clone();
    let mark = m.world().recent.back().map_or(0, |e| e.id);
    let pre_prompt_open = m.state().prompt.id != 0;

    // The input itself. A rejected input is still a step (a no-op one).
    let net = act.to_net();
    let act_result = m.act(act.who(), &net).map_err(|e| e.key().to_string());

    // Let the caller bring the match to rest (or answer the prompts the input
    // opened). Violations are checked over the *settled* world, so a clause
    // sees the transition as a player sees it: one command, then whatever it
    // caused.
    settle(m);

    let post = m.world();
    let events = m.events_since(mark);
    let data = m.data();
    let post_prompt_open = m.state().prompt.id != 0;
    let step = Step {
        pre: &pre,
        post,
        events: &events,
        act: &act,
        data: data.as_ref(),
        pre_prompt_open,
        post_prompt_open,
    };
    let violations = check(&step, clauses);
    Observed {
        act_result,
        violations,
    }
}

/// Observe without a post-act settle: the window covers only the raw input.
/// Useful when the caller wants to interleave its own settle policy per
/// prompt. Prefer [`observe`] with a settle closure.
pub fn observe_raw(m: &mut Match, act: StepAct, clauses: &[Clause]) -> Observed {
    observe(m, act, |_| {}, clauses)
}
