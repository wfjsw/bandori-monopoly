//! Machine-checked formal verification of BanG Dream Monopoly card clauses
//! (`docs/rulebook/spec/KANI.md`, `docs/rulebook/spec/KANI-FOUNDATION.md`).
//!
//! Layers:
//! * [`world`] / [`board`] -- general symbolic model of the host world + tiles.
//! * [`intern`] -- u16 interned names (no `String`/`memcmp` under `cfg(kani)`).
//! * [`host`] -- `ruleset_*` backend over that model (installed via `kani_host`).
//! * [`invoke`] -- registry lookup of shipped `CardDef` / skill / event entries.
//! * [`spec`] -- reference specifications written FROM the clause docs only.
//! * [`harness`] / [`shim`] -- clause-proof kit and temporary guard assumes.
//! * [`generated`] / [`ir_fill`] -- single-source guard IRs + World → IrCtx.
//! * [`proofs`] -- `#[kani::proof]` harnesses asserting card == spec (iff).

#![allow(dead_code)]

pub mod board;
#[cfg(kani)]
pub mod kani_stubs;
pub mod harness;
pub mod host;
pub mod intern;
pub mod invoke;
pub mod shim;
pub mod spec;
pub mod world;
pub mod generated;
pub mod ir_fill;

#[cfg(kani)]
pub mod proofs;

/// Absolute path anchor for docs (used by the KANI.md write-up).
pub const CLAUSE_DOC_ROOT: &str = "docs/rulebook/spec/";

/// Re-export the guard shim marker so use sites stay greppable from one place.
pub use shim::GUARD_ASSUMED_MARKER;