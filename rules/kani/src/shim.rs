//! Temporary shims for pieces of the engine that are **not** run under Kani.
//!
//! ## `guard_assumed!`
//!
//! Host-side guard/condition strings (CEL, `docs/GUARDS.md`) are compiled by
//! the engine and evaluated natively. Kani does **not** evaluate them. Until
//! the single-source condition evaluator (branch `cond-single`) lands, a
//! harness states the condition as an `assume` and proves the residual body
//! against the clause's own Condition.
//!
//! Every use site is greppable: `guard_assumed!`. Delete this module once
//! `cond-single` replaces the guard `assume` with the real evaluator; then
//! each site becomes a plain `kani::assume` of the condition's truth, or a
//! call into the evaluator.
//!
//! ```ignore
//! // Clause condition: `value > 0` (host pre) ∧ `poorest` (card residual).
//! guard_assumed!(value > 0, "C-PP-aya-haruka-02 host pre");
//! ```
//!
//! **Do not** use this to hide a clause condition that the proof is supposed
//! to establish. Only for the host-side pre that the engine evaluates outside
//! the card body.

/// Assert (under Kani) or document (native) that a host-side guard holds.
///
/// Expands to `kani::assume(cond)` in proofs. Outside `cfg(kani)` it is a
/// no-op that still evaluates `cond` for side-effect-free syntax, so the same
/// harness file typechecks in `cargo check`.
#[macro_export]
macro_rules! guard_assumed {
    ($cond:expr, $why:expr) => {{
        #[cfg(kani)]
        {
            kani::assume($cond);
            let _ = $why;
        }
        #[cfg(not(kani))]
        {
            let _ = ($cond, $why);
        }
    }};
    ($cond:expr) => {
        $crate::guard_assumed!($cond, "guard_assumed")
    };
}

/// Greppable marker for every temporary guard assume (see module docs).
pub const GUARD_ASSUMED_MARKER: &str = "guard_assumed!";

#[allow(dead_code)]
fn _touch() {
    let _ = GUARD_ASSUMED_MARKER;
}