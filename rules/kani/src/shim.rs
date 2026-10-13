//! Guard evaluation shims for Kani harnesses.
//!
//! ## `guard!` / `guard_id!` (the real path)
//!
//! Host-side guard/condition strings (CEL, `docs/GUARDS.md`) have a single
//! source of truth: the `&'static str` on the `On::*` entry. Under Kani we
//! evaluate the **lowered IR** of that string against an [`IrCtx`] filled
//! from the symbolic world — never `assume` the condition.
//!
//! ```ignore
//! let id = guard_id!("actor == owner && value > 0"); // const, compile-checked
//! let admitted = guard!(id, &ctx);
//! ```
//!
//! * [`guard_id!`] resolves at **const time** (unknown string = compile
//!   error via `cond_irs::const_id`).
//! * [`guard!`] indexes `generated::BY_ID[id]` — never a 139-way string
//!   compare (that is a CBMC unwinding / OOM cliff).
//!
//! ## `guard_assumed!` (temporary, greppable)
//!
//! **Only** for guards that genuinely do not lower. Every use site is
//! greppable: `guard_assumed!`. There should be none or few after
//! `cond-single` landed; list any remainder in `KANI-FOUNDATION.md`.
//!
//! **Do not** use it to hide a clause condition that the proof is supposed
//! to establish.

/// `guard_id!` (in `generated/mod.rs`) resolves the guard string to its id in a
/// `const` via `cond_irs::const_id` over the build-generated `SRCS` table
/// (see `rules_cond::gen_ir`, run from build.rs).
///
/// ```ignore
/// let id = guard_id!("actor == owner && value > 0"); // compile-checked
/// ```

/// Evaluate a single-source guard by const id against `ctx`.
///
/// Returns `bool` — the host root rule (any evaluation error = false).
///
/// ```ignore
/// let admitted = guard!(guard_id!("actor == owner && value > 0"), &ctx);
/// let admitted = guard!(id, &ctx);
/// ```
#[macro_export]
macro_rules! guard {
    ($id:expr, $ctx:expr) => {
        $crate::generated::cond_irs::eval_id($id, $ctx)
    };
}

/// Assert (under Kani) or document (native) that a host-side guard holds.
///
/// **Deprecated for lowerable guards** — use [`guard!`] instead. Kept only
/// for guards that `gen_cond_ir` cannot lower. Greppable:
/// `guard_assumed!`.
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