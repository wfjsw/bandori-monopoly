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

use game_rules::{Ruleset, Trigger, TriggerKind};

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
    let fired = r.run_hook(&w, call, &[], None).unwrap();
    assert!(
        fired.is_none(),
        "condition `false` must keep the hook from firing"
    );
    assert!(guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1);
}

// ------------------------------------------------ v49: Gate / AtEnd / RollPlan / Settle --
// docs/GUARDS.md §4.2c: these kinds carry the same `pre` + residual-guard
// pair as `On::Hook`, and the host evaluates category → condition → guard →
// body. A rejecting entry runs no body and so emits no `card` flash --
// `on_body` is exactly that flash (host.rs `run` / `run_hook`).

/// `On::Gate` whose condition rejects: the gate raise reaches no body, so the
/// card never answers (and never flashes).
#[test]
fn pre_gate_does_not_answer() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preGate").expect("fixture present");
    let mut w = effect_window(1, 1);
    w.trigger.kind = TriggerKind::ImmuneAll;
    let call = Call::Hook {
        card,
        kind: TriggerKind::ImmuneAll,
        player_id: 0,
    };
    let mut flashed = false;
    let mut on_body = |_w: &mut TestWorld, _e: i32| flashed = true;
    let fired = r.run_hook(&w, call, &[], Some(&mut on_body)).unwrap();
    assert!(
        fired.is_none(),
        "condition `false` must keep the gate from answering"
    );
    assert!(!flashed, "a rejecting gate must not flash");
    assert!(
        guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1,
        "the condition was evaluated and rejected"
    );
}

/// `On::RollPlan` whose condition rejects: the move being planned reaches no
/// routine (and no flash).
#[test]
fn pre_rollplan_does_not_shape_the_move() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preRollPlan").expect("fixture present");
    let mut w = effect_window(0, 0);
    w.trigger.kind = TriggerKind::RollPlan;
    let call = Call::RollPlan {
        card,
        player_id: 0,
    };
    let mut flashed = false;
    let mut on_body = |_w: &mut TestWorld, _e: i32| flashed = true;
    let out = r.run(&w, call, &[], Some(&mut on_body)).unwrap();
    assert!(
        matches!(out, Outcome::Done(_)),
        "a rejecting routine must not pause"
    );
    assert!(!flashed, "a rejecting RollPlan must not flash");
    assert!(
        guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1,
        "the condition was evaluated and rejected"
    );
}

/// `On::AtEnd` whose condition rejects: the scheduled turn-end callback runs
/// no body (and no flash).
#[test]
fn pre_at_end_does_not_run() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preAtEnd").expect("fixture present");
    let mut w = effect_window(0, 0);
    w.trigger.kind = TriggerKind::TurnEndAfter;
    let call = Call::AtEnd {
        card,
        player_id: 0,
    };
    let mut flashed = false;
    let mut on_body = |_w: &mut TestWorld, _e: i32| flashed = true;
    let out = r.run(&w, call, &[], Some(&mut on_body)).unwrap();
    assert!(
        matches!(out, Outcome::Done(_)),
        "a rejecting AtEnd must not pause"
    );
    assert!(!flashed, "a rejecting AtEnd must not flash");
    assert!(
        guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1,
        "the condition was evaluated and rejected"
    );
}

/// `On::Settle` whose condition rejects: the settle body does not govern the
/// settle (no body, no flash).
#[test]
fn pre_settle_does_not_govern() {
    let _guard = lock_guard_cost();
    guard_cost::reset();
    let r = fixtures();
    let card = r.card("TEST:preSettle").expect("fixture present");
    let mut w = effect_window(0, 0);
    w.trigger.kind = TriggerKind::Settle;
    let call = Call::Settle {
        card,
        player_id: 0,
    };
    let mut flashed = false;
    let mut on_body = |_w: &mut TestWorld, _e: i32| flashed = true;
    let out = r.run(&w, call, &[], Some(&mut on_body)).unwrap();
    assert!(
        matches!(out, Outcome::Done(_)),
        "a rejecting settle body must not pause"
    );
    assert!(!flashed, "a rejecting settle body must not flash");
    assert!(
        guard_cost::SKIPPED_BY_CONDITION.load(Relaxed) >= 1,
        "the condition was evaluated and rejected"
    );
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

// ------------------------------------------- shipped precompiled blob (G4) --

/// The `conds-*.bin` `tools/build-ruleset.mjs` publishes is exactly this host
/// compile of every shipped `pre`, and a ruleset built the browser way (from
/// those blobs via `RulesetBuilder::precompiled`) answers every window the
/// host-compiled one does. This is the invariant that makes a solo match in
/// the browser agree with the server: the glue has no CEL parser
/// (docs/GUARDS.md §8.3) and evaluates only what this blob carries.
#[test]
fn shipped_precompiled_blobs_match_host_compile() {
    let dir = dist("cards");
    let text = std::fs::read_to_string(dir.join("index.json"))
        .unwrap_or_else(|e| panic!("{}: {e} -- run tools/build-ruleset.mjs", dir.join("index.json").display()));
    let index: serde_json::Value = serde_json::from_str(&text).unwrap();
    let conds_meta = index
        .get("conds")
        .and_then(|c| c.get("file"))
        .and_then(|f| f.as_str())
        .expect("a shipped set with guard conditions must publish a conds blob");
    let blob_path = dir.join(conds_meta);
    let bytes = std::fs::read(&blob_path).unwrap_or_else(|e| panic!("{}: {e}", blob_path.display()));
    let shipped =
        game_rules::PrecompiledConds::from_bytes(&bytes).expect("shipped blob must decode");

    // Host compile (what the server / rules-worker do): modules only.
    let host = load(&["cards"]);
    // The browser path: same modules + the shipped blobs.
    let mut b = Ruleset::builder();
    for m in modules("cards") {
        b.add(&m).expect("module should load");
    }
    for e in &shipped.entries {
        b.precompiled(&e.card, e.entry, e.blob.clone());
    }
    let runtime = b.build().expect("precompiled path must build");

    // 1. Blob for blob, the shipped file is this build's compile.
    let fresh = host.precompiled_conds();
    assert_eq!(
        fresh.entries.len(),
        shipped.entries.len(),
        "every shipped entry is a host compile and vice versa"
    );
    for (f, s) in fresh.entries.iter().zip(&shipped.entries) {
        assert_eq!(f.card, s.card, "entries are in the same canonical order");
        assert_eq!(f.entry, s.entry);
        assert_eq!(
            f.blob, s.blob,
            "shipped blob for {}/{} is not this build's compile -- rebuild with tools/build-ruleset.mjs",
            f.card, f.entry
        );
    }

    // 2. Same entries carry a condition in both builds, and every probe
    //    agrees -- the precompiled path *is* the host compile, behaviourally.
    for (ci, c) in host.cards().iter().enumerate() {
        for ei in 0..c.on.len() {
            let h = host.pre_at(ci as i32, ei as i32);
            let r = runtime.pre_at(ci as i32, ei as i32);
            assert_eq!(h.is_some(), r.is_some(), "{}/{}: condition presence", c.id, ei);
            let (Some(h), Some(r)) = (h, r) else { continue };
            for (actor, owner) in [(0, 0), (1, 0), (0, 1), (2, 2)] {
                for value in [0, 5000] {
                    let mut w = effect_window(actor, value);
                    w.money = vec![1000, 2000, 3000, 4000];
                    let win = cond_pre::fill_window(&w);
                    let cand = cond_pre::fill_candidate(&w, owner, &c.id, false);
                    assert_eq!(
                        h.eval(&win, &cand),
                        r.eval(&win, &cand),
                        "{}/{e} disagrees for actor={actor} owner={owner} value={value}",
                        c.id,
                        e = ei
                    );
                }
            }
        }
    }

    // 3. The set identity covers the compiled conditions, not just the module
    //    bytes (docs/GUARDS.md §8.2): the documented recipe -- sorted module
    //    hashes + a `conds` marker + one blob hash per entry -- reproduces
    //    `Ruleset::sha256`, and the plain module-only recipe does not.
    use sha2::{Digest, Sha256};
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mut parts: Vec<String> = index["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["sha256"].as_str().unwrap().to_string())
        .collect();
    parts.sort_unstable();
    let module_only = hex(&Sha256::digest(parts.join("\n").as_bytes()));
    assert_ne!(
        host.sha256(),
        module_only,
        "the ruleset sha must change when the conditions change"
    );
    parts.push("conds".to_string());
    parts.extend(fresh.identity_lines());
    assert_eq!(
        host.sha256(),
        hex(&Sha256::digest(parts.join("\n").as_bytes())),
        "sha256 == hash(sorted module hashes + conds identity lines)"
    );
    assert_eq!(
        runtime.sha256(),
        host.sha256(),
        "the browser path and the host compile stamp the same identity"
    );
}

/// Fail-closed, browser side: a card with a `pre` and no precompiled entry is
/// a build error -- never "treat as true". And a blob that disagrees with the
/// source is refused rather than silently preferred.
#[test]
fn precompiled_missing_or_mismatched_is_a_build_error() {
    let card = "AG:回家的路上绕个道";
    let entry = 1;
    let host = load(&["cards"]);
    let want = host
        .pre_at(
            host.card(card).expect("shipped card present"),
            entry,
        )
        .expect("reported entry declares a condition");
    let blob = want.blob.clone();

    // Native compile has the source, so a missing blob is not an error here --
    // it is the wasm32 (browser) build that cannot compile it (that arm is
    // `compile_pre`'s `None` case; the node gate
    // `webui/src/game/ruleset.test.ts` drives it through the real glue). What
    // IS visible natively is that a *wrong* blob is refused:
    let mut wrong = Ruleset::builder();
    for m in modules("cards") {
        wrong.add(&m).unwrap();
    }
    let mut tampered = blob.clone();
    tampered.push(0);
    wrong.precompiled(card, entry, tampered);
    let err = wrong.build().err().expect("mismatched blob must not build");
    let msg = err.to_string();
    assert!(
        msg.contains("does not match") || msg.contains("bad condition"),
        "loud, not ignored: {msg}"
    );

    // A blob for an entry that declares no condition is a stale blob: refused.
    let mut stale = Ruleset::builder();
    for m in modules("cards") {
        stale.add(&m).unwrap();
    }
    stale.precompiled("AG:回家的路上绕个道", 0, blob);
    let err = stale.build().err().expect("stale blob entry must not build");
    assert!(
        err.to_string().contains("no condition"),
        "loud, not ignored: {err}"
    );

    // And a blob with the wrong envelope version fails loudly (no migration).
    let mut envelope = game_rules::PrecompiledConds {
        version: game_rules::PRECOMPILED_CONDS_VERSION + 1,
        entries: vec![],
    };
    envelope.entries.push(game_rules::PrecompiledCond {
        card: card.to_string(),
        entry,
        blob: want.blob.clone(),
    });
    assert!(
        game_rules::PrecompiledConds::from_bytes(&envelope.to_bytes()).is_err(),
        "a foreign envelope version must not decode"
    );
}
