//! Explicit work stack for nested host routines (STACK-01).
//!
//! Nested `[触发结算]` used to recurse
//! `card_settle_at → settle_at → raise → counteract → drive → apply_host_request → card_settle_at`
//! with no depth bound. The Rust stack carried every level, so a long bot game
//! (or a server `spawn_blocking` worker, ~2 MiB) overflowed.
//!
//! Structure now:
//!
//! * [`Cx::card_settle_at`] / [`Cx::settle_at`] enqueue a [`SettleFrame`] and
//!   enter [`Cx::drain_work`] -- the **only** pump.
//! * A drive that needs a nested settle while the pump is already running
//!   **suspends**: it pushes a resume work item and the settle job, then
//!   returns [`Halt::suspended`](super::Halt). The pump runs the settle first,
//!   then the resume -- the same order as the old synchronous nest, one Rust
//!   frame deep.
//! * Callers with work after a raise that can suspend insert a continuation at
//!   the current [`Cx::suspend_base`] so it runs after the nested work.
//!
//! Prompt halts (`Halt::Ask`) still unwind to [`Match::execute`](super::Match)
//! and re-run the routine from its snapshot -- unchanged. A nested settle that
//! prompts therefore still restarts the routine with a longer answer log; the
//! work stack is transient and rebuilt by that re-run.

use super::move_ctx::MoveCtx as Move;
use super::Cx;
use super::Flow;

/// One unit of deferred host work. Created by the engine or by card rules
/// (the wasm drive's resume frames) and run by [`Cx::drain_work`].
pub trait WorkItem {
    fn run(self: Box<Self>, cx: &mut Cx) -> Flow<()>;
}

/// A `[触发结算]` of one tile for one player, plus how far it has got.
#[derive(Debug, Clone)]
pub struct SettleFrame {
    pub player: usize,
    pub at: usize,
    pub m: Move,
    pub stage: SettleStage,
    /// Owner of `at` when the settle opened (read once, as today).
    pub owner: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleStage {
    /// Raise `settle` (declaration + [反击] window).
    RaiseSettle,
    /// After `settle` returned: cancel check, then body or terminal.
    AfterSettle,
    /// Raise `settleBody` (field replace).
    RaiseSettleBody,
    /// Run `rules.settle_tile` unless the body cancelled.
    SettleTile,
    /// Raise `settleAfter`.
    RaiseSettleAfter,
    /// Raise `tileResolved` (terminal, also on cancel).
    RaiseTileResolved,
    /// Done.
    Done,
}

impl SettleFrame {
    pub fn new(player: usize, at: usize, m: Move) -> Self {
        Self {
            player,
            at,
            m,
            stage: SettleStage::RaiseSettle,
            owner: -1,
        }
    }
}

/// Enqueue a settle and run it to completion (or until it suspends, in which
/// case [`ContinueSettleWork`] carries the rest).
pub struct SettleWork {
    pub frame: SettleFrame,
}

impl WorkItem for SettleWork {
    fn run(self: Box<Self>, cx: &mut Cx) -> Flow<()> {
        let mut frame = self.frame;
        cx.step_settle(&mut frame)
    }
}

/// Continue a settle that suspended mid-stage (a nested settle was requested).
pub struct ContinueSettleWork {
    pub frame: SettleFrame,
}

impl WorkItem for ContinueSettleWork {
    fn run(self: Box<Self>, cx: &mut Cx) -> Flow<()> {
        let mut frame = self.frame;
        cx.step_settle(&mut frame)
    }
}

/// Apply a card's post-drive fate (discard / hand / ...) once a resumed drive
/// returns its dest. STACK-01: the fate used to sit on the Rust stack after
/// `drive` returned; a suspension must run it from the work stack instead.
pub struct AfterDriveFateWork {
    pub f: Box<dyn FnOnce(&mut Cx) -> Flow<()>>,
}

impl WorkItem for AfterDriveFateWork {
    fn run(self: Box<Self>, cx: &mut Cx) -> Flow<()> {
        (self.f)(cx)
    }
}

/// Resume a card-rules drive / counteract that suspended for a nested settle.
/// The closure owns the wasm-side state (call, answers, remaining jobs) and
/// re-enters the bridge; it must not hold a borrow of `Cx`.
pub struct ResumeFnWork {
    pub f: Box<dyn FnOnce(&mut Cx) -> Flow<()>>,
}

impl WorkItem for ResumeFnWork {
    fn run(self: Box<Self>, cx: &mut Cx) -> Flow<()> {
        (self.f)(cx)
    }
}

impl<'a> Cx<'a> {
    /// Is the work-stack pump running? A nested `card_settle_at` checks this
    /// and suspends instead of recursing.
    pub fn is_draining_work(&self) -> bool {
        self.draining_work
    }

    /// Push work onto the stack (LIFO: last pushed runs first).
    pub fn push_work(&mut self, w: impl WorkItem + 'static) {
        self.work_stack.push(Box::new(w));
    }

    /// Insert a continuation at the current suspension-group base so it runs
    /// **after** the nested settle and every resume already queued.
    pub fn insert_work(&mut self, w: impl WorkItem + 'static) {
        let at = self.suspend_base.min(self.work_stack.len());
        self.work_stack.insert(at, Box::new(w));
    }

    /// Record the start of a new suspension group (before pushing resume +
    /// settle). Callers that propagate [`super::Halt::suspended`] insert their
    /// continuations at this base.
    pub fn begin_suspend_group(&mut self) {
        self.suspend_base = self.work_stack.len();
    }

    /// Park a counteract continuation (remaining drive jobs + half-updated
    /// trigger) so a re-entered `CardRules::counteract` adopts it.
    pub fn set_counteract_resume(&mut self, state: Box<dyn std::any::Any + Send>) {
        self.counteract_resume = Some(state);
    }

    /// Take the parked counteract continuation, if any.
    pub fn take_counteract_resume(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        self.counteract_resume.take()
    }

    /// Bump the nested-settle depth and report whether the rulebook cap
    /// ([`MAX_SETTLE_DEPTH`](super::MAX_SETTLE_DEPTH)) has been hit.
    pub fn settle_depth_guard(&mut self) -> bool {
        if self.settle_depth >= super::MAX_SETTLE_DEPTH {
            return true;
        }
        self.settle_depth += 1;
        false
    }

    /// Store / take the dest a resumed drive returned (card fate).
    pub fn set_drive_dest(&mut self, dest: i32) {
        self.last_drive_dest = Some(dest);
    }
    pub fn take_drive_dest(&mut self) -> Option<i32> {
        self.last_drive_dest.take()
    }

    /// Pump the work stack until it is empty. The only iterative driver of
    /// nested settles (STACK-01). Re-entrant calls return immediately -- the
    /// outermost pump owns the loop -- **unless** the caller suspended, in
    /// which case it must not reach here.
    pub fn drain_work(&mut self) -> Flow<()> {
        if self.draining_work {
            return Ok(());
        }
        self.draining_work = true;
        self.suspend_base = 0;
        let mut result = Ok(());
        while let Some(w) = self.work_stack.pop() {
            self.suspend_base = 0;
            match w.run(self) {
                Ok(()) | Err(super::Halt(super::HaltKind::Suspended)) => {
                    // Suspended work items re-pushed their own continuation
                    // (or a fresh resume). Keep pumping.
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        self.draining_work = false;
        self.suspend_base = 0;
        result
    }
}