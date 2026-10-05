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
//! Activation is the same open shape as the character skills -- an always-on
//! `On::Skill`, not a placed card. See the module docs in `skill-characters`
//! before adding a file here.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::CardDef;

/// Every band skill. Empty until the `On::Skill` activation lands; see the
/// `skill-characters` module docs. Adding one is a file in this directory plus
/// an entry here.
pub const CARDS: &[CardDef] = &[];