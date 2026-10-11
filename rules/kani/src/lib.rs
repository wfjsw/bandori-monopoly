//! Machine-checked pilot: prove unmodified card bodies do EXACTLY what the
//! 规则书 clause docs specify (`docs/rulebook/spec/KANI.md`).
//!
//! Layers:
//! * [`world`] -- small symbolic model of the host world (N<=4) + effect log.
//! * [`host`] / [`host_stubs`] -- `bandori_*` shims over that model.
//! * [`invoke`] -- run an unmodified `CardDef` entry against the model.
//! * [`spec`] -- reference specifications written FROM the clause docs only.
//! * [`proofs`] -- `#[kani::proof]` harnesses asserting card == spec (iff).

#![allow(dead_code)]

pub mod world;
pub mod host;
pub mod invoke;
pub mod spec;

#[cfg(kani)]
pub mod proofs;

/// Absolute path anchor for docs (used by the KANI.md write-up).
pub const CLAUSE_DOC_ROOT: &str = "docs/rulebook/spec/";
