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

#[cfg(feature = "band-pp")]
pub mod pp;
#[cfg(feature = "band-ppp")]
pub mod ppp;
#[cfg(feature = "band-roselia")]
pub mod r;
#[cfg(feature = "band-ag")]
pub mod ag;
#[cfg(feature = "band-hhw")]
pub mod hhw;
#[cfg(feature = "band-morfonica")]
pub mod mor;
#[cfg(feature = "band-ras")]
pub mod ras;
#[cfg(feature = "band-mygo")]
pub mod mygo;
#[cfg(feature = "band-mujica")]
pub mod mujica;
#[cfg(feature = "band-crychic")]
pub mod crychic;
#[cfg(all(feature = "band-sumimi", feature = "skills-characters"))]
pub mod sumimi;
#[cfg(feature = "band-general")]
pub mod general;
#[cfg(feature = "events")]
pub mod events;
#[cfg(all(feature = "band-pp", feature = "skills-characters"))]
pub mod skills_pp;
#[cfg(all(feature = "band-ppp", feature = "skills-characters"))]
pub mod skills_ppp;
#[cfg(all(feature = "band-roselia", feature = "skills-characters"))]
pub mod skills_r;
#[cfg(all(feature = "band-ag", feature = "skills-characters"))]
pub mod skills_ag;
#[cfg(all(feature = "band-hhw", feature = "skills-characters"))]
pub mod skills_hhw;
#[cfg(all(feature = "band-morfonica", feature = "skills-bands"))]
pub mod skills_mor;
#[cfg(all(feature = "band-ras", feature = "skills-characters"))]
pub mod skills_ras;
#[cfg(all(feature = "band-mygo", feature = "skills-bands"))]
pub mod skills_mygo;
#[cfg(all(feature = "band-mujica", feature = "skills-bands"))]
pub mod skills_mujica;
#[cfg(all(feature = "band-crychic", feature = "skills-bands"))]
pub mod skills_crychic;
#[cfg(all(feature = "band-sumimi", feature = "skills-bands"))]
pub mod skills_sumimi;
#[cfg(feature = "band-general")]
pub mod skills_general;
#[cfg(feature = "tiles")]
pub mod tiles;
#[cfg(feature = "tiles")]
pub mod tile_marks;
#[cfg(feature = "skills-bands")]
pub mod skills_bands;
#[cfg(feature = "skills-characters")]
pub mod skills_characters;
pub mod cond_ops;
#[cfg(feature = "band-pp")]
pub mod money_shave;
#[cfg(feature = "band-ppp")]
pub mod teleport;
#[cfg(feature = "band-pp")]
pub mod prompt;
pub mod wrong_guard;
pub mod hostops_smoke;