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

#[cfg(feature = "band-ag")]
pub mod ag;
pub mod cond_ops;
#[cfg(feature = "band-crychic")]
pub mod crychic;
#[cfg(feature = "events")]
pub mod events;
#[cfg(feature = "band-general")]
pub mod general;
#[cfg(feature = "band-hhw")]
pub mod hhw;
pub mod hostops_smoke;
#[cfg(feature = "tiles")]
pub mod markers;
#[cfg(feature = "band-pp")]
pub mod money_shave;
#[cfg(feature = "band-morfonica")]
pub mod mor;
#[cfg(feature = "band-mujica")]
pub mod mujica;
#[cfg(feature = "band-mygo")]
pub mod mygo;
#[cfg(feature = "band-pp")]
pub mod pp;
#[cfg(feature = "band-ppp")]
pub mod ppp;
#[cfg(feature = "band-pp")]
pub mod prompt;
#[cfg(feature = "band-roselia")]
pub mod r;
#[cfg(feature = "band-ras")]
pub mod ras;
#[cfg(all(feature = "band-ag", feature = "skills-personas"))]
pub mod skills_ag;
#[cfg(feature = "skills-groups")]
pub mod skills_bands;
#[cfg(feature = "skills-personas")]
pub mod skills_characters;
#[cfg(all(feature = "band-crychic", feature = "skills-groups"))]
pub mod skills_crychic;
#[cfg(feature = "band-general")]
pub mod skills_general;
#[cfg(all(feature = "band-hhw", feature = "skills-personas"))]
pub mod skills_hhw;
#[cfg(all(feature = "band-morfonica", feature = "skills-groups"))]
pub mod skills_mor;
#[cfg(all(feature = "band-mujica", feature = "skills-groups"))]
pub mod skills_mujica;
#[cfg(all(feature = "band-mygo", feature = "skills-groups"))]
pub mod skills_mygo;
#[cfg(all(feature = "band-pp", feature = "skills-personas"))]
pub mod skills_pp;
#[cfg(all(feature = "band-ppp", feature = "skills-personas"))]
pub mod skills_ppp;
#[cfg(all(feature = "band-roselia", feature = "skills-personas"))]
pub mod skills_r;
#[cfg(all(feature = "band-ras", feature = "skills-personas"))]
pub mod skills_ras;
#[cfg(all(feature = "band-sumimi", feature = "skills-groups"))]
pub mod skills_sumimi;
#[cfg(all(feature = "band-sumimi", feature = "skills-personas"))]
pub mod sumimi;
#[cfg(feature = "band-ppp")]
pub mod teleport;
#[cfg(feature = "tiles")]
pub mod tiles;
pub mod wrong_guard;
