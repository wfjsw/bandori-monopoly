//! # rules-cond — G1 of docs/GUARDS.md
//!
//! A **serialized condition** per guarded entry, compiled once at load time
//! and evaluated natively against a window/candidate snapshot. The wasm guard
//! stays as the secondary (residual) filter; this crate is the layer that
//! replaces the per-card `can_counteract` probes before they instantiate.
//!
//! Layering (user ruling 2026-10-07): **category / condition / guard are
//! distinct; nothing is rejected twice.** That is an authoring discipline,
//! not a language limit: `kind` (alias `trigger.kind`) is an ordinary window
//! variable, so a multi-kind entry can still write per-kind clauses.
//!
//! This crate is **isolated**: no dependency on `game-core` / `card-sdk`, not
//! yet wired into the host. The host fills [`WindowCtx`] / [`CandidateCtx`]
//! in G2.
//!
//! ```
//! # #[cfg(feature = "compile")]
//! # fn main() {
//! use rules_cond::{compile, CandidateCtx, WindowCtx};
//!
//! let cond = compile("actor != owner && owner.money >= 500").unwrap();
//! let win = WindowCtx { actor: 1, ..WindowCtx::default() };
//! let cand = CandidateCtx { owner: 0, owner_money: 600, ..CandidateCtx::default() };
//! assert!(cond.eval(&win, &cand));
//! # }
//! # #[cfg(not(feature = "compile"))] fn main() {}
//! ```

mod compile;
mod ctx;
mod eval;
mod kinds;
/// The §4.2 vocabulary lint/rewrite: only the host (`compile`) walks it.
#[cfg(feature = "compile")]
mod schema;

pub use compile::Cond;
#[cfg(feature = "compile")]
pub use compile::compile;
pub use ctx::{CandidateCtx, ChainLink, MoveSnap, PlayerSnap, TileSnap, WindowCtx};
pub use eval::{EvalError, WindowScope};
pub use kinds::{constants as kind_constants, mv, trig, TELEPORT_TRIGGER};

/// Why a condition failed to compile. Fail-closed at build: an unparsable or
/// rejected condition is a `build-ruleset` error (G2 maps this to
/// `RuleError::BadPre`), never "treat as true" — the clauses it carries would
/// otherwise be gone from the guard too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CondError {
    /// The source does not parse.
    Parse(String),
    /// Identifier / dotted field is not in the §4.2 schema.
    UnknownVar(String),
    /// Function is not in the schema (or is float-producing).
    UnknownFn(String),
    /// Float literal (or uint — the schema is int-only).
    FloatLiteral(String),
    /// Float-producing call (`double`, …).
    FloatOp(String),
    /// The serialized form carries a format version this build does not read.
    /// Postcard is not self-describing: a fork bump that changes the AST
    /// shape must not silently decode garbage.
    WireVersion { found: u8, expected: u8 },
    /// The bytes are not a serialized condition.
    WireDecode(String),
}

impl std::fmt::Display for CondError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CondError::Parse(s) => write!(f, "parse error: {s}"),
            CondError::UnknownVar(s) => write!(f, "unknown variable: {s}"),
            CondError::UnknownFn(s) => write!(f, "unknown function: {s}"),
            CondError::FloatLiteral(s) => write!(f, "float literal (int-only): {s}"),
            CondError::FloatOp(s) => write!(f, "float operation (int-only): {s}"),
            CondError::WireVersion { found, expected } => write!(
                f,
                "serialized condition format version {found}, this build reads {expected}"
            ),
            CondError::WireDecode(s) => write!(f, "not a serialized condition: {s}"),
        }
    }
}

impl std::error::Error for CondError {}

// TODO(规则书) / G3 (docs/GUARDS.md §5.2): the `precheck` lint helper that
// flags kind-based rejection inside *guards* (not conditions) lives in a
// later phase. Conditions already reject kind here; the guard-side scan is
// intentionally out of scope for G1.