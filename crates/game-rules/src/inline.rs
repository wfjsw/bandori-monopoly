//! Inline answers for simulation mode (`docs/BOT.md` §3.2).
//!
//! The replay model pauses a card body at every `ask_*` and every
//! [`HostRequest`](crate::HostRequest), then re-runs it from the top with the
//! answer appended -- *k*+1 module runs for a body with *k* pauses, which is
//! the 82 % engine-shell cost B0 measured. In a simulation every decision
//! belongs to the bot, so the pauses can be answered *as they happen*: the
//! host import produces the answer (or applies the engine routine) and the
//! guest keeps going. One forward pass per body.
//!
//! The plumbing is a thread-local stack of [`InlineHost`]s, pushed by
//! [`crate::wasm_rules::RulesBridge`]'s drive loop around each guest call and
//! consulted by `hostfns` at every pause point. Both backends see it: the
//! sandbox's `Caller`/`HostState` and `rules-native`'s `NativeHost` share the
//! `hostfns` code, and the drive loop is backend-neutral.
//!
//! **Default path.** With no [`InlineHost`] pushed (the live match),
//! [`request_or_pause`] / [`prompt_or_pause`] fall through to the caller's
//! usual pause shape -- `host_request` / `asked` plus the sentinel or trap.
//! Byte-identical to the pre-B2 behaviour.

use std::cell::RefCell;

use crate::host::{HostCtx, HostErr, HostRequest, Prompt};

/// What a drive installs around one guest call so its asks and host requests
/// can be answered inline. Implemented by `wasm_rules`'s drive over the
/// engine's `Cx`; the thread-local stack is what `hostfns` reaches through the
/// wasm / native boundary.
///
/// The run's world is type-erased (`&mut dyn Any`, always a [`crate::Run`] --
/// `CardModules` pins it) so the trait stays object-safe while `hostfns` stays
/// generic over `HostCtx::World`.
pub trait InlineHost {
    /// Answer a player prompt. `run` is the guest's world so far (its writes
    /// included; downcast to [`crate::Run`]). `Some(v)` continues the guest
    /// with `v`; `None` pauses as today (the caller sets `asked` and the run
    /// is replayed).
    fn answer(&mut self, run: &mut dyn std::any::Any, p: Prompt) -> Option<i32>;

    /// Apply a host request mid-run. The implementer runs the engine routine
    /// against `run`'s world (swapped into its `Cx`), so guest writes and host
    /// effects accumulate in program order on one world and the body never
    /// replays. `Some(v)` is the guest's answer (`paid.moved()`, the gate's
    /// 0/1, ...); `None` pauses -- including when a nested prompt the provider
    /// declined halted the engine routine mid-request, in which case the
    /// implementer also parks the [`crate::Halt`] for the drive to propagate
    /// (the partially-applied request is discarded with the body).
    fn apply(&mut self, run: &mut dyn std::any::Any, req: HostRequest) -> Option<i32>;

    /// The engine-side [`game_core::engine::Halt`] a declined nested prompt
    /// raised, if any. The drive checks this right after the guest call and
    /// propagates it instead of folding the pause into the replay path.
    fn take_halt(&mut self) -> Option<game_core::engine::Halt>;
}

thread_local! {
    /// The inline-host stack. Nested drives push their own; a host import
    /// always talks to the innermost one (the run it is inside).
    static STACK: RefCell<Vec<*mut (dyn InlineHost + 'static)>> = const { RefCell::new(Vec::new()) };
}

/// RAII push of `h` for the duration of `f`. Pops on unwind too, so a nested
/// drive never leaves a stale host behind for its caller's next import.
pub fn with_inline_host<T>(h: &mut (dyn InlineHost + 'static), f: impl FnOnce() -> T) -> T {
    let ptr: *mut (dyn InlineHost + 'static) = h;
    STACK.with(|s| s.borrow_mut().push(ptr));
    // A guard so a panic (caught as a `RuleError::Trap` further out) still pops.
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            STACK.with(|s| {
                s.borrow_mut().pop();
            });
        }
    }
    let _pop = Pop;
    f()
}

/// The innermost host, if any (raw pointer valid while its `with_inline_host`
/// frame is live -- which covers every call that reaches here).
fn top() -> Option<*mut (dyn InlineHost + 'static)> {
    STACK.with(|s| s.borrow().last().copied())
}

/// Does a simulation host want inline answers at all? Only the engine side
/// knows (its [`crate::game_core::engine::AnswerProvider`]); the drive pushes a
/// host only when one is installed. This just reports whether the stack is
/// non-empty.
pub fn inline_active() -> bool {
    STACK.with(|s| !s.borrow().is_empty())
}

/// A marker host that refuses to answer -- pushed around pure queries
/// (`can_counteract` / `cant_play` / a hook guard) so a card that prompts from
/// one pauses and surfaces as `GuardPrompted` instead of borrowing the outer
/// drive's inline host (which would silently answer a question nobody asked).
struct Deny;

impl InlineHost for Deny {
    fn answer(&mut self, _run: &mut dyn std::any::Any, _p: Prompt) -> Option<i32> {
        None
    }
    fn apply(&mut self, _run: &mut dyn std::any::Any, _req: HostRequest) -> Option<i32> {
        None
    }
    fn take_halt(&mut self) -> Option<game_core::engine::Halt> {
        None
    }
}

/// Run `f` with inline answers refused (see [`Deny`]).
pub fn with_no_inline<T>(f: impl FnOnce() -> T) -> T {
    let mut d = Deny;
    with_inline_host(&mut d, f)
}

/// Take the guest's world out of `c` for the duration of `f`, then put it
/// back. The world inside is what the inline host swaps into the engine's
/// `Cx`; `HostState` cannot hold it while `f` runs (aliasing).
fn with_run<C: HostCtx, T>(
    c: &mut C,
    f: impl FnOnce(&mut dyn std::any::Any) -> T,
) -> Result<T, HostErr> {
    let mut run = {
        let st = c.st_mut();
        st.world
            .take()
            .ok_or_else(|| HostErr::trap("inline host with no world"))?
    };
    let v = f(&mut run);
    let st = c.st_mut();
    st.world = Some(run);
    Ok(v)
}

/// Answer `p` inline. `Ok(Some(v))` = the guest continues with `v` (recorded
/// on the run's answer log so a nested `play_card` cursor stays aligned);
/// `Ok(None)` = no inline host, or it declined -- the caller pauses as today.
pub fn prompt_or_inline<C: HostCtx>(c: &mut C, p: Prompt) -> Result<Option<i32>, HostErr> {
    let Some(h) = top() else { return Ok(None) };
    let v = with_run(c, |run| unsafe { &mut *h }.answer(run, p))?;
    record(c, v)
}

/// Apply `req` inline. Same result shape as [`prompt_or_inline`].
pub fn request_or_inline<C: HostCtx>(c: &mut C, req: HostRequest) -> Result<Option<i32>, HostErr> {
    let Some(h) = top() else { return Ok(None) };
    let v = with_run(c, |run| unsafe { &mut *h }.apply(run, req))?;
    record(c, v)
}

/// Push an inline answer onto the run's answer log and advance the cursor --
/// the same bookkeeping a replayed pause does, so nesting shares one log.
fn record<C: HostCtx>(c: &mut C, v: Option<i32>) -> Result<Option<i32>, HostErr> {
    if let Some(v) = v {
        let st = c.st_mut();
        st.answers.push(v);
        st.next_answer += 1;
    }
    Ok(v)
}

/// How a host function phrases its pause when it has no answer. The two shapes
/// fold into the same [`crate::Outcome::NeedHost`] / `NeedInput` at the
/// `finish` boundary; the difference is only whether the guest's entry wrapper
/// sees a normal return (the sentinel) or a trap. Kept per-site so the default
/// path is byte-identical.
#[derive(Debug, Clone, Copy)]
pub enum Pause {
    /// Return `card_sdk::abi::EXIT_NEED_INPUT`; the guest wrapper hands the
    /// card `Err(Prompt)`.
    Sentinel,
    /// Trap with `HostErr::NeedInput`; the guest stack tears down.
    Trap,
}

/// The shared pause point for a host request: consume a logged answer, else
/// answer inline (simulation mode), else park `req` on `host_request` and
/// return the caller's pause shape.
pub fn request_or_pause<C: HostCtx>(
    c: &mut C,
    req: HostRequest,
    pause: Pause,
) -> Result<i32, HostErr> {
    {
        let st = c.st_mut();
        if let Some(&x) = st.answers.get(st.next_answer) {
            st.next_answer += 1;
            return Ok(x);
        }
    }
    if let Some(v) = request_or_inline(c, req.clone())? {
        return Ok(v);
    }
    let st = c.st_mut();
    st.host_request = Some(req);
    match pause {
        Pause::Sentinel => Ok(card_sdk::abi::EXIT_NEED_INPUT),
        Pause::Trap => Err(HostErr::NeedInput),
    }
}

/// The shared pause point for a player prompt: consume a logged answer, else
/// answer inline, else park `p` on `asked` and return the sentinel (every
/// `ask_*` pauses that way -- `ctx::ask_*` turns it into `Err(Prompt)`).
pub fn prompt_or_pause<C: HostCtx>(c: &mut C, p: Prompt) -> Result<i32, HostErr> {
    {
        let st = c.st_mut();
        if let Some(&x) = st.answers.get(st.next_answer) {
            st.next_answer += 1;
            return Ok(x);
        }
    }
    if let Some(v) = prompt_or_inline(c, p.clone())? {
        return Ok(v);
    }
    let st = c.st_mut();
    st.asked = Some(p);
    Ok(card_sdk::abi::EXIT_NEED_INPUT)
}