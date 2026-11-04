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

use crate::host::{HostCtx, HostError, HostRequest, Prompt};
use crate::world::CardWorld;

/// What a drive installs around one guest call so its asks and host requests
/// can be answered inline. Implemented by `wasm_rules`'s drive over the
/// engine's `PlayCtx`; the thread-local stack is what `hostfns` reaches through the
/// wasm / native boundary.
///
/// The run's world is type-erased (`&mut dyn Any`, always a [`crate::Run`] --
/// `CardModules` pins it) so the trait remains object-safe while `hostfns` remains
/// generic over `HostCtx::World`.
pub trait InlineHost {
    /// Answer a player prompt. `run` is the guest's world so far (its writes
    /// included; downcast to [`crate::Run`]). `Some(v)` continues the guest
    /// with `v`; `None` pauses as today (the caller sets `asked` and the run
    /// is replayed).
    fn answer(&mut self, run: &mut dyn std::any::Any, p: Prompt) -> Option<i32>;

    /// Apply a host request mid-run. The implementer runs the engine routine
    /// against `run`'s world (swapped into its `PlayCtx`), so guest writes and host
    /// effects accumulate in program order on one world and the body never
    /// replays. `Some(v)` is the guest's answer (`paid.moved()`, the gate's
    /// 0/1, ...); `None` pauses -- including when a nested prompt the provider
    /// declined halted the engine routine mid-request, in which case the
    /// implementer also parks the [`crate::Halt`] for the drive to propagate
    /// (the partially-applied request is discarded with the body).
    fn apply(
        &mut self,
        run: &mut dyn std::any::Any,
        log: &mut game_core::engine::GuestLog,
        start: game_core::engine::JournalStart,
        req: HostRequest,
    ) -> Option<i32>;

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
    fn apply(
        &mut self,
        _run: &mut dyn std::any::Any,
        _log: &mut game_core::engine::GuestLog,
        _start: game_core::engine::JournalStart,
        _req: HostRequest,
    ) -> Option<i32> {
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
/// `PlayCtx`; `HostState` cannot hold it while `f` runs (aliasing).
fn with_run<C: HostCtx, T>(
    c: &mut C,
    f: impl FnOnce(&mut dyn std::any::Any) -> T,
) -> Result<T, HostError> {
    let mut run = {
        let st = c.st_mut();
        st.world
            .take()
            .ok_or_else(|| HostError::trap("inline host with no world"))?
    };
    let v = f(&mut run);
    let st = c.st_mut();
    st.world = Some(run);
    Ok(v)
}

/// [`with_run`] plus the guest answer log: the apply owns both for its
/// duration (it completes the journal entry and writes the answer there).
fn with_run_log<C: HostCtx, T>(
    c: &mut C,
    f: impl FnOnce(&mut dyn std::any::Any, &mut game_core::engine::GuestLog) -> T,
) -> Result<T, HostError> {
    let mut run = {
        let st = c.st_mut();
        st.world
            .take()
            .ok_or_else(|| HostError::trap("inline host with no world"))?
    };
    let mut log = {
        let st = c.st_mut();
        std::mem::take(&mut st.log)
    };
    let v = f(&mut run, &mut log);
    let st = c.st_mut();
    st.world = Some(run);
    st.log = log;
    Ok(v)
}

/// Answer `p` inline. `Ok(Some(v))` = the guest continues with `v` (recorded
/// on the run's answer log so a nested `play_card` cursor remains aligned);
/// `Ok(None)` = no inline host, or it declined -- the caller pauses as today.
pub fn prompt_or_inline<C: HostCtx>(c: &mut C, p: Prompt) -> Result<Option<i32>, HostError> {
    let Some(h) = top() else { return Ok(None) };
    let v = with_run(c, |run| unsafe { &mut *h }.answer(run, p))?;
    record(c, v)
}

/// Apply `req` inline with the journal start the pause point decided. Same
/// result shape as [`prompt_or_inline`]; on an answer the apply itself has
/// already completed the entry and written the answer, so there is no
/// [`record`].
pub fn request_or_inline<C: HostCtx>(
    c: &mut C,
    req: HostRequest,
    start: game_core::engine::JournalStart,
) -> Result<Option<i32>, HostError> {
    let Some(h) = top() else { return Ok(None) };
    with_run_log(c, |run, log| unsafe { &mut *h }.apply(run, log, start, req))
}

/// Push an inline answer onto the run's answer log and advance the cursor --
/// the same bookkeeping a replayed pause does, so nesting shares one log.
fn record<C: HostCtx>(c: &mut C, v: Option<i32>) -> Result<Option<i32>, HostError> {
    if let Some(v) = v {
        let st = c.st_mut();
        st.log.answers.push(v);
        st.log.cursor += 1;
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
    /// Trap with `HostError::NeedInput`; the guest stack tears down.
    Trap,
}

/// Like [`request_or_pause`], but the routine runs on every live pass: a
/// logged answer is consumed for alignment and ignored (design G06 §6.4 -- a
/// join round must be rebuilt after any `Ask` re-runs the walk).
pub fn request_always<C: HostCtx>(
    c: &mut C,
    req: HostRequest,
    pause: Pause,
) -> Result<i32, HostError> {
    {
        let st = c.st_mut();
        if st.log.answers.get(st.log.cursor).is_some() {
            let answer = st.log.cursor;
            st.w().after_host(answer);
            st.log.cursor += 1;
        }
    }
    // A simulation has no join routine of its own; answer through the inline
    // host when there is one.
    if let Some(v) = request_or_inline(c, req.clone(), game_core::engine::JournalStart::Push)? {
        let st = c.st_mut();
        st.host_request = None;
        st.host_journal_start = None;
        return Ok(v);
    }
    let st = c.st_mut();
    st.host_request = Some(req);
    match pause {
        Pause::Sentinel => Ok(card_sdk::abi::EXIT_NEED_INPUT),
        Pause::Trap => Err(HostError::NeedInput),
    }
}

/// The shared pause point for a host request (design G10 §6.3.2): re-apply an
/// in-flight entry, consume a logged answer (replacing it when the body
/// recomputed the arguments), else answer inline (simulation mode), else park
/// `req` on `host_request` with the start the drive must apply it under.
pub fn request_or_pause<C: HostCtx>(
    c: &mut C,
    req: HostRequest,
    pause: Pause,
) -> Result<i32, HostError> {
    let mirrored = game_core::engine::JournalRequest::from(&req);
    let mut answer = None;
    {
        let st = c.st_mut();
        let at = st.log.cursor;
        if std::env::var("G10_DBG").is_ok() {
            eprintln!(
                "ROP at={at} req={mirrored:?} answers={:?} journal={:?} reapply={:?} last={:?}",
                st.log.answers,
                st.log.journal.iter().map(|e| e.slot).collect::<Vec<_>>(),
                st.log.reapply,
                st.log.last.keys().collect::<Vec<_>>()
            );
        }
        // 1. An in-flight entry is re-entered at its cursor. Several in-flight
        //    entries can share a cursor (an enclosing request's own answer is
        //    not written yet); the first entry still there is the outermost.
        let outstanding = st.log.reapply.get(&at).copied().unwrap_or(0);
        if outstanding > 0 {
            let mark = st.log.reapply.get_mut(&at).expect("mark present");
            *mark -= 1;
            if *mark == 0 {
                st.log.reapply.remove(&at);
            }
            if let Some(index) = st.log.journal.iter().position(|e| e.slot == at) {
                let entry = st.log.journal[index].clone();
                // The routine mutates live and has to start from the request's
                // pre-state.
                st.w().restore_from_world(&entry.condition);
                st.w().apply_state_overlay(&entry.guest);
                if entry.request != mirrored {
                    // The body recomputed this call's arguments from the
                    // post-request world: re-apply under the new ones, and drop
                    // the deeper entries the restore just undid.
                    st.log.journal[index].request = mirrored;
                    st.log.journal.truncate(index + 1);
                    st.log.reapply.retain(|&k, _| k <= at);
                }
                st.host_request = Some(req);
                st.host_journal_start = Some(game_core::engine::JournalStart::Resume(index));
                return pause_now(pause);
            }
            // A mark without an entry: fall through to the answer path.
        }
        // 2. A logged answer at this cursor.
        if let Some(x) = st.log.answers.get(at).copied() {
            match st.log.last.get(&at).cloned() {
                Some(entry) if entry.request == mirrored => {
                    st.w().after_host(at);
                    st.log.cursor += 1;
                    answer = Some(x);
                }
                Some(entry) => {
                    // The body recomputed its arguments from the post-request
                    // world. The cap bounds a pathological recompute: the
                    // fourth hit restores the completed condition and consumes
                    // the existing answer.
                    let capped = st.log.replace_count.get(&at).copied().unwrap_or(0) >= 4;
                    st.w().restore_from_world(&entry.condition);
                    st.w().apply_state_overlay(&entry.guest);
                    if capped {
                        st.w().after_host(at);
                        st.log.cursor += 1;
                        answer = Some(x);
                    } else {
                        *st.log.replace_count.entry(at).or_default() += 1;
                        st.host_request = Some(req);
                        st.host_journal_start = Some(game_core::engine::JournalStart::Replace(at));
                        return pause_now(pause);
                    }
                }
                None => {
                    // A dead nested answer (no completed row to compare with):
                    // consume it for alignment.
                    st.w().after_host(at);
                    st.log.cursor += 1;
                    answer = Some(x);
                }
            }
        }
    }
    let Some(x) = answer else {
        // 3. A brand-new request.
        if let Some(v) = request_or_inline(c, req.clone(), game_core::engine::JournalStart::Push)? {
            let st = c.st_mut();
            st.host_request = None;
            st.host_journal_start = None;
            return Ok(v);
        }
        let st = c.st_mut();
        st.host_request = Some(req);
        st.host_journal_start = Some(game_core::engine::JournalStart::Push);
        return pause_now(pause);
    };
    Ok(x)
}

fn pause_now(pause: Pause) -> Result<i32, HostError> {
    match pause {
        Pause::Sentinel => Ok(card_sdk::abi::EXIT_NEED_INPUT),
        Pause::Trap => Err(HostError::NeedInput),
    }
}

/// The shared pause point for a player prompt: consume a logged answer, else
/// answer inline, else park `p` on `asked` and return the sentinel (every
/// `ask_*` pauses that way -- `ctx::ask_*` turns it into `Err(Prompt)`).
pub fn prompt_or_pause<C: HostCtx>(c: &mut C, p: Prompt) -> Result<i32, HostError> {
    {
        let st = c.st_mut();
        if let Some(&x) = st.log.answers.get(st.log.cursor) {
            st.log.cursor += 1;
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
