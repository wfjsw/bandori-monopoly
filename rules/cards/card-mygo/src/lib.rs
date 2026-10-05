//! Official cards of band `mygo` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod anon_tokyo;
use anon_tokyo::ANON_TOKYO;
pub mod endless_journey;
use endless_journey::ENDLESS_JOURNEY;
pub mod even_lost;
use even_lost::EVEN_LOST;
pub mod haneoka;
use haneoka::HANEOKA;
pub mod hitoshizuku;
use hitoshizuku::HITOSHIZUKU;
pub mod meet_again;
use meet_again::MEET_AGAIN;
pub mod miracle;
use miracle::MIRACLE;
pub mod no_road;
use no_road::NO_ROAD;
pub mod ordinary;
use ordinary::ORDINARY;
pub mod rana_funny;
use rana_funny::RANA_FUNNY;
pub mod rinne_rain;
use rinne_rain::RINNE_RAIN;
pub mod soyo_colors;
use soyo_colors::SOYO_COLORS;
pub mod taki_serious;
use taki_serious::TAKI_SERIOUS;
pub mod that_day_rain;
use that_day_rain::THAT_DAY_RAIN;
pub mod tomori_no_longer;
use tomori_no_longer::TOMORI_NO_LONGER;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    ANON_TOKYO,
    ENDLESS_JOURNEY,
    EVEN_LOST,
    HANEOKA,
    HITOSHIZUKU,
    MEET_AGAIN,
    MIRACLE,
    NO_ROAD,
    ORDINARY,
    RANA_FUNNY,
    RINNE_RAIN,
    SOYO_COLORS,
    TAKI_SERIOUS,
    THAT_DAY_RAIN,
    TOMORI_NO_LONGER,
];