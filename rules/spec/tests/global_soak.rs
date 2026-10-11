//! One soak test per global area: fuzz arrange + drive with that area's
//! clauses checked on every observed input.
//!
//! Units 1-8 **only add clauses** to `rules/spec/src/global/<area>.rs`. This
//! file picks the slices up automatically via [`rules_spec::global::area`]
//! and never needs editing.
//!
//! A discrepancy is a plain failing test: no `#[ignore]`, no allowlist. The
//! failure names the clause id, the quote, the detail, and the seed.

use rules_spec::global;

/// Default soak length. Override with `FUZZ_ITERS` (the plan's recipe uses
/// `FUZZ_ITERS=300` for a real soak).
const DEFAULT_ITERS: u32 = 60;

fn soak_area(name: &str) {
    let clauses = global::area(name);
    assert!(
        !clauses.is_empty(),
        "area {name:?} exported no clauses -- units 1-8 must fill CLAUSES"
    );
    if let Err(e) = rules_spec::fuzz::soak(clauses, DEFAULT_ITERS) {
        panic!("area {name:?}: {e}");
    }
}

#[test]
fn money() {
    soak_area("money");
}

#[test]
fn cards() {
    soak_area("cards");
}

#[test]
fn status() {
    soak_area("status");
}

#[test]
fn movement() {
    soak_area("movement");
}

#[test]
fn settle() {
    soak_area("settle");
}

#[test]
fn flow() {
    soak_area("flow");
}

#[test]
fn bankruptcy() {
    soak_area("bankruptcy");
}

#[test]
fn counteract() {
    soak_area("counteract");
}