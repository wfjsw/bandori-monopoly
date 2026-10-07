//! Interaction fuzzer: random play over the full card set, invariants checked
//! after every act, plus metamorphic paired runs.
//!
//! Iterations come from `FUZZ_ITERS` (default CI-cheap). Seed is fixed so the
//! run is reproducible; bump the env var for a long soak.
//!
//! Black-box: nothing under `rules/cards/`, `rules/skills/`, `rules/fixtures/`
//! or `target/scratch/tainted/` is opened.

mod common;
mod fuzz;

use std::sync::OnceLock;
use std::time::Instant;

use fuzz::drive::Trace;
use fuzz::meta;
use fuzz::{is_known, one_iter, Finding};

/// Default iterations -- the whole file should land in 1-2 minutes.
/// Bump with `FUZZ_ITERS=…` for a long soak (2000 iters ≈ 2 min on this tree).
const DEFAULT_ITERS: u32 = 900;

fn iters() -> u32 {
    static N: OnceLock<u32> = OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("FUZZ_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_ITERS)
    })
}

/// Seed for iteration `i`. Fixed base so CI is reproducible.
fn seed_for(i: u32) -> u64 {
    0xF022_F022_F022_F022u64
        .wrapping_add(i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

// =====================================================================
// the main fuzz loop
// =====================================================================

#[test]
fn fuzz_interactions_invariants() {
    let n = iters();
    let t0 = Instant::now();
    let mut findings: Vec<Finding> = vec![];
    let mut total = Trace::new();
    let turns_per = 4u32;

    for i in 0..n {
        let seed = seed_for(i);
        match one_iter(seed, turns_per) {
            Ok(tr) => {
                total.acts += tr.acts;
                total.prompts += tr.prompts;
                total.counteracts += tr.counteracts;
                total.traps += tr.traps;
                total.pairs_this_turn.extend(tr.pairs_this_turn);
                for (k, v) in tr.unaccounted_money {
                    *total.unaccounted_money.entry(k).or_default() += v;
                }
            }
            Err(f) => {
                eprintln!("--- FINDING (iter {i}, seed {:#x}) ---", f.seed);
                eprintln!("{}", f.what);
                eprintln!("repro: {}", f.repro);
                findings.push(f);
            }
        }
    }

    eprintln!(
        "fuzz: {n} iters x {turns_per} turns in {:.1?}; acts={} prompts={} counteracts={} pairs={}",
        t0.elapsed(),
        total.acts,
        total.prompts,
        total.counteracts,
        total.pairs_this_turn.len(),
    );
    {
        let sample: Vec<_> = total.pairs_this_turn.iter().take(12).collect();
        eprintln!("sample co-occurring pairs: {sample:?}");
    }
    if !total.unaccounted_money.is_empty() {
        let mut kinds: Vec<_> = total.unaccounted_money.iter().collect();
        kinds.sort();
        eprintln!("known-finding counts by kind:");
        for (k, v) in kinds {
            eprintln!("  {k}: {v}");
        }
    }

    // Unknown findings fail the run; known ones are already filed as ignored
    // tests in `rb_fuzz_found.rs`.
    let unknown: Vec<&Finding> = findings.iter().filter(|f| is_known(&f.what).is_none()).collect();
    assert!(
        unknown.is_empty(),
        "{} new findings:\n{}",
        unknown.len(),
        unknown
            .iter()
            .map(|f| format!("  * {} ({})", f.what, f.repro))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// =====================================================================
// determinism & save/restore
// =====================================================================

#[test]
fn fuzz_determinism() {
    let n = iters().min(6);
    for i in 0..n {
        let seed = seed_for(i) ^ 0xDE7;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            meta::check_determinism(seed, 3)
        }));
        let res = match r {
            Ok(x) => x,
            Err(p) => Err(p
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "determinism: driver panicked".into())),
        };
        match res {
            Ok(()) => {}
            Err(e) => {
                if is_known(&e).is_some() {
                    eprintln!("known finding skipped: {e}");
                    continue;
                }
                panic!("determinism (seed {seed:#x}): {e}");
            }
        }
    }
}

#[test]
fn fuzz_save_restore() {
    let n = iters().min(6);
    for i in 0..n {
        let seed = seed_for(i) ^ 0x5A7E;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            meta::check_save_restore(seed, 4)
        }));
        let res = match r {
            Ok(x) => x,
            Err(p) => Err(p
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "save/restore: driver panicked".into())),
        };
        match res {
            Ok(()) => {}
            Err(e) => {
                if is_known(&e).is_some() {
                    eprintln!("known finding skipped: {e}");
                    continue;
                }
                panic!("save/restore (seed {seed:#x}): {e}");
            }
        }
    }
}

// =====================================================================
// metamorphic checks
// =====================================================================

#[test]
fn fuzz_meta_negation() {
    let n = iters().min(4);
    for i in 0..n {
        let seed = seed_for(i) ^ 0x4E6;
        if let Err(e) = meta::check_negation(seed) {
            if is_known(&e).is_some() {
                continue;
            }
            panic!("negation (seed {seed:#x}): {e}");
        }
    }
}

#[test]
fn fuzz_meta_commutativity() {
    let n = iters().min(4);
    for i in 0..n {
        let seed = seed_for(i) ^ 0xC044;
        if let Err(e) = meta::check_commutativity(seed) {
            if is_known(&e).is_some() {
                continue;
            }
            panic!("commutativity (seed {seed:#x}): {e}");
        }
    }
}

#[test]
fn fuzz_meta_monotonicity() {
    let n = iters().min(4);
    for i in 0..n {
        let seed = seed_for(i) ^ 0x3040;
        if let Err(e) = meta::check_monotonicity(seed) {
            if is_known(&e).is_some() {
                continue;
            }
            panic!("monotonicity (seed {seed:#x}): {e}");
        }
    }
}

#[test]
fn fuzz_meta_seat_rotation() {
    let n = iters().min(3);
    for i in 0..n {
        let seed = seed_for(i) ^ 0x5EA7;
        if let Err(e) = meta::check_seat_rotation(seed, 3) {
            if is_known(&e).is_some() {
                continue;
            }
            panic!("seat rotation (seed {seed:#x}): {e}");
        }
    }
}

/// Nested money movements open their [反击] windows at every depth (the
/// sheet's 「[支付]时可以打出」 has no depth limit). The termination argument
/// is that a hook cannot re-trigger on its own movement; `MAX_MONEY_DEPTH = 32`
/// is a safety cap that traps loudly. See `docs/ENGINE.md`.
#[test]
fn fuzz_money_depth_probe() {
    match meta::check_money_depth(seed_for(0) ^ 0x3E97) {
        Ok(report) => eprintln!("{report}"),
        Err(e) => eprintln!("probe could not nest 3 deep: {e}"),
    }
}

// =====================================================================
// immunity metamorphism (its own test so a failure names the clause)
// =====================================================================

/// An effect on an immune / untargetable player leaves that player as if the
/// effect was never played. 夏日合宿 is 「直到你的下个回合开始时，只有你自己
/// 的效果可以指定你」 -- while it is up, a foreign [指定] must not touch the
/// owner.
#[test]
fn fuzz_meta_immunity() {
    let n = iters().min(4);
    for i in 0..n {
        let seed = seed_for(i) ^ 0x1777;
        if let Err(e) = check_immunity(seed) {
            if is_known(&e).is_some() {
                continue;
            }
            panic!("immunity (seed {seed:#x}): {e}");
        }
    }
}

fn check_immunity(seed: u64) -> Result<(), String> {
    let (mut t, _) = fuzz::gen::arrange(seed);
    let n = t.n;
    for i in 0..n {
        t.set_money(i, 10_000);
    }
    // P0 plays 夏日合宿 (immune to foreign designation until next turn start).
    t.give(0, &["Mor:夏日合宿"]);
    t.give(1, &["通用:登上武道馆"]);
    let shielded = t.play(0, "Mor:夏日合宿").is_ok();
    if !shielded {
        return Ok(());
    }
    let m0 = t.money(0);
    // P1 plays a multi-target pay that would include P0.
    if t.play(1, "通用:登上武道馆").is_err() {
        return Ok(());
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // P0 must not have paid (the shield swallowed the designation).
    if t.money(0) != m0 {
        return Err(format!(
            "immunity: shielded P0 lost money ({} -> {}) despite 夏日合宿",
            m0,
            t.money(0)
        ));
    }
    Ok(())
}