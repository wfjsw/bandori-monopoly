//! Per-unit reference specs (FROM clause docs only).
//!
//! Each unit owns only its own file; this `mod.rs` is written once.
//! Authors never edit this file after the foundation lands.

pub mod ag;
pub mod crychic;
pub mod events;
pub mod general;
pub mod hhw;
pub mod markers;
pub mod mor;
pub mod mujica;
pub mod mygo;
pub mod pp;
pub mod ppp;
pub mod r;
pub mod ras;
pub mod skills_ag;
pub mod skills_bands;
pub mod skills_characters;
pub mod skills_crychic;
pub mod skills_general;
pub mod skills_hhw;
pub mod skills_mor;
pub mod skills_mujica;
pub mod skills_mygo;
pub mod skills_pp;
pub mod skills_ppp;
pub mod skills_r;
pub mod skills_ras;
pub mod skills_sumimi;
pub mod sumimi;
pub mod tiles;

pub mod pilot;
pub use pilot::*;
