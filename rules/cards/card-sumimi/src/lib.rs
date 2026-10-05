//! Official cards of band `sumimi` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod childhood_cheer;
use childhood_cheer::CHILDHOOD_CHEER;
pub mod here_the_world;
use here_the_world::HERE_THE_WORLD;
pub mod idol_and_band;
use idol_and_band::IDOL_AND_BAND;
pub mod l11;
use l11::L11;
pub mod l12;
use l12::L12;
pub mod mana_champion;
use mana_champion::MANA_CHAMPION;
pub mod no_breakup;
use no_breakup::NO_BREAKUP;
pub mod now_sumimi;
use now_sumimi::NOW_SUMIMI;
pub mod sweet_escape;
use sweet_escape::SWEET_ESCAPE;
pub mod two_donuts;
use two_donuts::TWO_DONUTS;
pub mod two_in_one;
use two_in_one::TWO_IN_ONE;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    CHILDHOOD_CHEER,
    HERE_THE_WORLD,
    IDOL_AND_BAND,
    L11,
    L12,
    MANA_CHAMPION,
    NO_BREAKUP,
    NOW_SUMIMI,
    SWEET_ESCAPE,
    TWO_DONUTS,
    TWO_IN_ONE,
];