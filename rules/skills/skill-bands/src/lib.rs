//! Band skills as card rules -- one file per skill, one crate for the lot.
//!
//! Separated from `rules/cards/` for the same reason as
//! [`skill-characters`](../skill_characters): these are abilities a band comes
//! with, not cards you draw. Same rule language ([`CardDef`]), separate crate.
//!
//! The rule text lives in `data/bands.json` (`skill` is the name, `text` /
//! `simple` the wording). A band skill that hands out [奇迹水晶] is a consumer
//! of the keyed state (`state::add` / `ctx::add_band_crystals`); one that
//! mandates a cap writes the bound and every card reads it back. See
//! `game_core::state::StateVar` for the "engine holds values, enforces nothing"
//! rule.
//!
//! Hooks: the same two the character skills use, no third kind. A half the
//! player presses is [`On::Play`] with its gate; a half a field event calls is
//! [`On::Hook`] (the skill is a field card, C# `Fx`). See the module docs in
//! `skill-characters` for the shape and for what is still missing (id
//! resolution, not hooks).

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::CardDef;

pub mod ave_mujica;
use ave_mujica::AVE_MUJICA;
pub mod afterglow;
use afterglow::AFTERGLOW;
pub mod roselia;
use roselia::ROSELIA;
pub mod morfonica;
use morfonica::MORFONICA;
pub mod ras;
use ras::RAS;
pub mod mygo;
use mygo::MYGO;
pub mod sumimi;
use sumimi::SUMIMI;
pub mod pastel;
use pastel::PASTEL;
pub mod hhw;
use hhw::HHW;
pub mod circle_staff;
use circle_staff::CIRCLE_STAFF;
pub mod poppin;
use poppin::POPPIN;
pub mod crychic;
use crychic::CRYCHIC;

// The 规则书 text for these is a Google Sheet (see `skill-rulebook-sheets` in
// the project memory, or fetch `export?format=csv&gid=1398197110` on the
// `1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY` document), column `技能`.
// `data/bands.json` (`text`) is a faithful copy of it -- 12/12 rows match modulo
// the sheet's literal backslash-n escapes -- so the JSON is safe to code against.
//
// All 12 band skills are written. What is left on each is its own
// `TODO(规则书)` / `TODO(ABI)` note.
//
pub const CARDS: &[CardDef] = &[
    AVE_MUJICA,
    AFTERGLOW,
    ROSELIA,
    MORFONICA,
    RAS,
    MYGO,
    SUMIMI,
    PASTEL,
    HHW,
    CIRCLE_STAFF,
    POPPIN,
    CRYCHIC,
];