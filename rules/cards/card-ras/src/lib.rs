//! Official cards of band `ras` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod be_strongest;
use be_strongest::BE_STRONGEST;
pub mod change_world;
use change_world::CHANGE_WORLD;
pub mod chuchu_music;
use chuchu_music::CHUCHU_MUSIC;
pub mod crush_drum;
use crush_drum::CRUSH_DRUM;
pub mod exist;
use exist::EXIST;
pub mod guerrilla;
use guerrilla::GUERRILLA;
pub mod hey_kids;
use hey_kids::HEY_KIDS;
pub mod layer_keep;
use layer_keep::LAYER_KEEP;
pub mod lock_dream;
use lock_dream::LOCK_DREAM;
pub mod pareo_far;
use pareo_far::PAREO_FAR;
pub mod please_choose;
use please_choose::PLEASE_CHOOSE;
pub mod repaint;
use repaint::REPAINT;
pub mod riot;
use riot::RIOT;
pub mod studio_storm;
use studio_storm::STUDIO_STORM;
pub mod unstoppable;
use unstoppable::UNSTOPPABLE;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    BE_STRONGEST,
    CHANGE_WORLD,
    CHUCHU_MUSIC,
    CRUSH_DRUM,
    EXIST,
    GUERRILLA,
    HEY_KIDS,
    LAYER_KEEP,
    LOCK_DREAM,
    PAREO_FAR,
    PLEASE_CHOOSE,
    REPAINT,
    RIOT,
    STUDIO_STORM,
    UNSTOPPABLE,
];