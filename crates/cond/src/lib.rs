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
pub mod eval;
pub mod ext;
mod kinds;
pub mod view;
mod view_impl;
pub mod vocab;
pub use view_impl::SnapshotView;
/// Build-time generator for the Kani guard IR table (feature `lower`).
#[cfg(feature = "lower")]
pub mod gen_ir;
/// AST → heap-free IR for Kani (host-only; feature `lower`).
#[cfg(feature = "lower")]
pub mod lower;
/// The §4.2 vocabulary lint/rewrite: only the host (`compile`) walks it.
#[cfg(feature = "compile")]
pub mod schema;

#[cfg(feature = "compile")]
pub use compile::compile;
#[cfg(feature = "compile")]
pub use compile::compile_with;
pub use compile::Cond;
pub use ext::{
    ext, install_ext, vocab, AlreadyInstalled, CondExt, LowerAs, Vocab, EMPTY_EXT,
};
pub use ctx::{
    fill_named_layers, CandidateCtx, ChainLink, MoveSnap, PlayerSnap, TileSnap, WindowCtx,
};
pub use eval::{EvalError, WindowScope};
pub use kinds::{ident_known, int_constants, mv, str_constants, trig};

/// Stable int id for a string identity (card id, content name, wire kind).
/// Not the data-table index -- a name-stable hash so a condition's string
/// literal (compared via `Eq` against a `state` / `kind` hash) survives a
/// data reshuffle. Collisions are astronomically unlikely at 64 bits and the
/// schema is int-only. FNV-1a 64.
pub fn id_of(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

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
            CondError::UnknownVar(s) => {
                write!(f, "unknown variable: {s}")?;
                if ext::ext().names.is_empty() {
                    write!(f, " (CondExt not installed)")?;
                }
                Ok(())
            }
            CondError::UnknownFn(s) => {
                write!(f, "unknown function: {s}")?;
                if ext::ext().names.is_empty() {
                    write!(f, " (CondExt not installed)")?;
                }
                Ok(())
            }
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

// G3 (docs/GUARDS.md §5.2 / §10.1): the `precheck` lint helper that flags
// kind-based rejection inside *guards* (not conditions) is
// `cargo run -p game-rules --example precheck`. Conditions already reject
// kind here.
