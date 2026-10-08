//! docs/GUARDS.md G0/G2: guard **condition** per guarded entry.
//!
//! Own test binary on purpose: the `guard_cost` counters these tests assert
//! on are process-global, and `ruleset.rs`'s other tests bump them through
//! every `can_counteract` / `run_hook` / `cant_play`. A separate binary keeps
//! the counters to this file's threads only; the `Mutex` below serialises the
//! tests among themselves and is poison-tolerant so one failing assertion
//! cannot cascade into the rest.

#[path = "testworld.rs"]
mod testworld;
use testworld::*;

use game_rules::{CardWorld, Ruleset, Trigger, TriggerKind};

// ------------------------------------------------ docs/GUARDS.md G0/G2 --
// Guard **condition** per guarded entry. The condition is layer 1 (the
// category filter is layer 0, the wasm guard is layer 2); nothing is rejected
// twice. With no `pre` on any shipped card the behaviour above is untouched;
// the `TEST:pre*` fixtures below exercise the new layer.

use game_rules::cond_pre::{self, guard_cost, CompiledPre};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Mutex;

/// The `guard_cost` counters are process-global; serialise the tests that
/// assert on them so parallel test threads cannot interleave `reset()`s.
static GUARD_COST_LOCK: Mutex<()> = Mutex::new(());

/// Poison-tolerant: one panicking test must not cascade into the others.
fn lock_guard_cost() -> std::sync::MutexGuard<'static, ()> {
    GUARD_COST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The fixture ruleset: shipped cards + `TEST:*` probes.
fn fixtures() -> Ruleset {
    load(&["cards", "fixtures"])
}

/// An `Effect` chain window owned by `actor`, of `value`.
fn effect_window(actor: i32, value: i32) -> TestWorld {
    let mut w = TestWorld::new(7);
    w.trigger = Trigger {
        kind: TriggerKind::Effect,
        player_id: actor,
        value,
        ..Trigger::default()
    };
    w
}

/// A rejecting condition skips the guard entirely: `can_counteract` is false
/// and the "skipped by condition" counter moves -- the wasm guard never runs.
#[test]
fn pre_reject_skips_guard() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preReject").expect("fixture present");
    // actor == owner (0) holds, but `value >= 999999` does not.
    let w = effect_window(0, 1);
    assert!(!r.can_counteract(&w, card, 0).unwrap());
    assert_eq!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed), 1);
    assert_eq!(guard_cost::GUARD_ASKED.load(Relaxed), 0);
    // And another seat's trigger rejects too (actor != owner).
    let w = effect_window(1, 1);
    assert!(!r.can_counteract(&w, card, 0).unwrap());
    assert_eq!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed), 2);
}

/// An accepting condition runs the residual guard (`pre::MINE` =
/// `actor == owner`).
#[test]
fn pre_accept_runs_guard() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preAccept").expect("fixture present");
    let w = effect_window(0, 1);
    assert!(r.can_counteract(&w, card, 0).unwrap());
    assert_eq!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed), 0);
    assert_eq!(guard_cost::GUARD_ASKED.load(Relaxed), 1);
    // Another seat's trigger: the condition rejects.
    let w = effect_window(1, 1);
    assert!(!r.can_counteract(&w, card, 0).unwrap());
    assert_eq!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed), 1);
}

/// A rejecting condition on a play gate is a block (`err.play_pre`) and the
/// gate is skipped.
#[test]
fn pre_play_blocks_without_gate() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:prePlay").expect("fixture present");
    let w = TestWorld::new(7); // money 10_000 < 999_999
    let why = r.cant_play(&w, card, 0).unwrap();
    let why = why.expect("condition rejects -> blocked");
    assert_eq!(&*why.k, "err.play_pre");
    assert_eq!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed), 1);
}

/// A rejecting condition on a field hook means the hook does not fire.
#[test]
fn pre_hook_does_not_fire() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preHook").expect("fixture present");
    let mut w = effect_window(0, 1);
    w.trigger.kind = TriggerKind::PayAdd;
    let call = Call::Hook {
        card,
        kind: TriggerKind::PayAdd,
        player_id: 0,
    };
    let fired = r.run_hook(&w, call, &[]).unwrap();
    assert!(
        fired.is_none(),
        "condition `false` must keep the hook from firing"
    );
    assert!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1);
}

/// Compile errors fail closed (docs/GUARDS.md §5.3): a parse / unknown-var /
/// float condition is never "treat as true".
#[test]
fn pre_compile_errors_fail_closed() {
    for bad in [
        "actor ==",                 // parse
        "unknown_var == 1",         // unknown variable
        "no_such_fn(owner) == 1",   // unknown function
        "owner.money >= 1.5",       // float literal (int-only)
        "double(owner.money) >= 1", // float op
    ] {
        assert!(CompiledPre::compile(bad).is_err(), "should fail closed: {bad}");
    }
    assert!(CompiledPre::compile("actor == owner").is_ok());
}

/// The browser runtime-only path (`Cond::from_bytes` + eval) answers exactly
/// what the host compile path does -- same lean blob, same verdict.
#[test]
fn pre_runtime_only_matches_host() {
    let host = CompiledPre::compile("actor != owner && owner.money >= 500").unwrap();
    let browser = CompiledPre::from_bytes(&host.blob).unwrap();
    let world = effect_window(1, 0);
    let win = cond_pre::fill_window(&world);
    for owner in 0..4i32 {
        let cand = cond_pre::fill_candidate(&world, owner, "TEST:preAccept", false);
        assert_eq!(
            host.eval(&win, &cand),
            browser.eval(&win, &cand),
            "host and runtime-only disagree for owner={owner}"
        );
    }
}
