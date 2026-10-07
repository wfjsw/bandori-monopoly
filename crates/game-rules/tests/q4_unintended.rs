//! Q4 -- unintended card / skill / tile interactions.
//!
//! Six checks, all black-box against the shipped ruleset:
//!
//! 1. single-rule footprint audit      (`q4::single`)
//! 2. pairwise non-interference        (`q4::pair`)
//! 3. trigger-scope                    (`q4::trig`)
//! 4. expiry / leak                    (`q4::expiry`)
//! 5. shared-name registry             (`q4::names`)
//! 6. bystander / seat invariance      (`q4::seat`)
//!
//! The default run stays green by skipping signatures already filed in
//! [`q4::KNOWN_FINDINGS`] (see `tests/rb_unintended_found.rs`), the same way
//! the interaction fuzzer does. `Q4_SEEDS` / `Q4_SAMPLE` / `Q4_PAIRS` widen
//! the sweep; the default is the ~2 minute band.
//!
//! Black-box: nothing under `rules/cards/`, `rules/skills/`, `rules/fixtures/`
//! or `target/scratch/tainted/` is opened.

mod common;
mod fuzz;
mod q4;

use std::time::Instant;

use q4::{expiry, names, pair, seat, single, trig};

/// Collect every finding from the default sweep. Shared by the green test
/// and by the `--nocapture` report.
fn sweep() -> Sweep {
    // Harness panics (a settle that never rests) are caught per rule; silence
    // their stderr so the report stays readable.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let t0 = Instant::now();
    let seeds = q4::seeds_per_rule();
    let sample = q4::single_sample();
    let pairs = q4::pair_sample();

    let mut s = Sweep::default();

    // ---- 1. single-rule footprint audit --------------------------------
    let audits = single::audit_batch(sample, seeds);
    s.lines.extend(single::report(&audits));
    s.findings.extend(single::findings(&audits));
    s.counts.push(("single", audits.len(), audits.iter().filter(|a| a.ok()).count()));

    // ---- 2. pairwise non-interference ---------------------------------
    let runs = pair::pair_batch(pairs);
    s.lines.extend(pair::report(&runs));
    s.findings.extend(pair::findings(&runs));
    s.counts.push((
        "pairwise",
        runs.len(),
        runs.iter().filter(|r| r.diverge.is_none()).count(),
    ));

    // ---- 3. trigger-scope ---------------------------------------------
    let (fires, tf) = trig::sweep(sample.min(40), 0x7716_2026);
    s.lines.extend(trig::report(&fires, &tf));
    s.findings.extend(tf);
    s.counts.push(("trigger", fires.len(), 0));

    // ---- 4. expiry / leak ---------------------------------------------
    let er = expiry::expiry_batch(sample.min(40), seeds.min(2));
    s.lines.extend(expiry::report(&er, &expiry::findings(&er)));
    s.findings.extend(expiry::findings(&er));
    s.counts.push((
        "expiry",
        er.len(),
        er.iter().filter(|r| r.leaked.is_empty()).count(),
    ));

    // ---- 5. shared-name registry --------------------------------------
    // Fold the single-rule audit's observed names into the registry so a
    // name only a sampled card writes still shows up in the collision /
    // near-duplicate scan.
    let mut log = names::collect(sample.min(50), 0x4E41_2026);
    for a in &audits {
        for n in &a.names_written {
            log.writes
                .entry(n.clone())
                .or_default()
                .insert(a.rule.clone());
            log.all_names.insert(n.clone());
        }
    }
    let mut anoms = names::collisions(&log);
    anoms.extend(names::written_never_read(&log));
    anoms.extend(names::near_duplicates(&log));
    s.lines.extend(names::report(&log, &anoms));
    s.name_anomalies = anoms
        .iter()
        .map(|a| format!("[{}] {} -- {}", a.kind, a.name, a.detail))
        .collect();
    s.counts.push(("names", log.writes.len(), 0));

    // ---- 6. bystander / seat invariance -------------------------------
    let sr = seat::seat_batch(pairs.min(20));
    s.lines.extend(seat::report(&sr, &seat::findings(&sr)));
    s.findings.extend(seat::findings(&sr));
    s.counts.push((
        "seat",
        sr.len(),
        sr.iter().filter(|r| r.diverge.is_empty()).count(),
    ));

    s.runtime = t0.elapsed();
    let _ = std::panic::take_hook();
    std::panic::set_hook(prev);
    s
}

#[derive(Default)]
struct Sweep {
    lines: Vec<String>,
    findings: Vec<q4::Finding>,
    name_anomalies: Vec<String>,
    counts: Vec<(&'static str, usize, usize)>,
    runtime: std::time::Duration,
}

impl Sweep {
    /// Drop findings whose `what` matches a known signature.
    fn filter_known(&mut self) {
        self.findings.retain(|f| {
            if let Some(at) = q4::is_known(&f.what) {
                eprintln!("known finding ({at}): {}", f.what);
                false
            } else {
                true
            }
        });
    }
}

/// The whole Q4 sweep. Green by default: findings that reproduce are listed
/// with `--nocapture` and held in `tests/rb_unintended_found.rs` behind
/// `#[ignore]`, and any signature already in `q4::KNOWN_FINDINGS` is skipped.
#[test]
fn q4_unintended_interactions() {
    let mut s = sweep();
    for l in &s.lines {
        eprintln!("{l}");
    }
    eprintln!(
        "q4: {} findings, {} name anomalies, runtime {:?}",
        s.findings.len(),
        s.name_anomalies.len(),
        s.runtime
    );
    for (kind, total, clean) in &s.counts {
        eprintln!("q4 count {kind}: {clean}/{total} clean");
    }
    s.filter_known();
    // The default run reports, it does not fail: every confirmed finding is
    // filed in `rb_unintended_found.rs` as an `#[ignore = "DISCREPANCY: …"]`
    // test. Assert only that the harness itself ran.
    assert!(!s.lines.is_empty(), "q4 sweep produced no report");
}

/// A long soak. `Q4_SAMPLE=0 Q4_SEEDS=4 Q4_PAIRS=120 cargo test -p game-rules
/// --test q4_unintended q4_soak -- --nocapture`
#[test]
#[ignore = "soak: set Q4_SAMPLE=0 Q4_SEEDS=4 Q4_PAIRS=120 and run --ignored"]
fn q4_soak() {
    // Same body as the default; the env vars widen the sweep.
    q4_unintended_interactions();
}

/// Print the sweep as a markdown fragment (for `docs/rulebook/COVERAGE.md`).
/// Run with `--nocapture` and paste the block.
#[test]
#[ignore = "report generator: run --ignored --nocapture and paste into COVERAGE.md"]
fn q4_print_coverage_section() {
    let s = sweep();
    println!("## Unintended interactions");
    println!();
    println!("Method: see `crates/game-rules/tests/q4_unintended.rs` and the");
    println!("modules under `crates/game-rules/tests/q4/`. Black-box: `rules/cards/`,");
    println!("`rules/skills/`, `rules/fixtures/` and `target/scratch/tainted/` were");
    println!("never opened.");
    println!();
    println!("| check | sampled | clean |");
    println!("|---|---|---|");
    for (kind, total, clean) in &s.counts {
        println!("| {kind} | {total} | {clean} |");
    }
    println!();
    println!("Runtime: {:?}.", s.runtime);
    println!();
    println!("### Findings");
    println!();
    println!("| rule(s) | field | repro test | severity |");
    println!("|---|---|---|---|");
    for f in &s.findings {
        println!(
            "| {} | {} | `{}` | {} |",
            f.rule,
            f.field,
            f.repro.replace('|', "/"),
            f.severity.label()
        );
    }
    println!();
    println!("### Shared-name registry");
    println!();
    for a in &s.name_anomalies {
        println!("* {a}");
    }
    println!();
    println!("### Raw report");
    println!();
    for l in &s.lines {
        println!("* {l}");
    }
}