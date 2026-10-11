//! Per-unit `#[kani::proof]` harnesses.
//!
//! Each unit owns only its own file; this `mod.rs` is written once.
//! `money_shave` / `teleport` / `prompt` are the pilot proofs and stay.
//!
//! Every harness:
//! 1. builds a symbolic pre-state (`kani::any` + `assume`ed ranges);
//! 2. evaluates the **single-source** guard IR (no `assume` of the host
//!    condition) and only runs the body when the guard admits;
//! 3. asserts post == spec(pre) and the effect log is exactly the clause's
//!    effects (no less, no more);
//! 4. includes a MUTANT whose disagreement is shown reachable.

pub mod pp;
pub mod ppp;
pub mod r;
pub mod ag;
pub mod hhw;
pub mod mor;
pub mod ras;
pub mod mygo;
pub mod mujica;
pub mod crychic;
pub mod sumimi;
pub mod general;
pub mod events;
pub mod skills_pp;
pub mod skills_ppp;
pub mod skills_r;
pub mod skills_ag;
pub mod skills_hhw;
pub mod skills_mor;
pub mod skills_ras;
pub mod skills_mygo;
pub mod skills_mujica;
pub mod skills_crychic;
pub mod skills_sumimi;
pub mod skills_general;
pub mod tiles;
pub mod tile_marks;
pub mod skills_bands;
pub mod skills_characters;
pub mod cond_ops;
pub mod money_shave;
pub mod teleport;
pub mod prompt;
pub mod wrong_guard;