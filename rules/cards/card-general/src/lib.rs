//! Official cards of band `general` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod budokan;
use budokan::BUDOKAN;
pub mod clear_cp;
use clear_cp::CLEAR_CP;
pub mod effort;
use effort::EFFORT;
pub mod encore;
use encore::ENCORE;
pub mod fever;
use fever::FEVER;
pub mod gacha10;
use gacha10::GACHA10;
pub mod great;
use great::GREAT;
pub mod marina_work;
use marina_work::MARINA_WORK;
pub mod net_error;
use net_error::NET_ERROR;
pub mod parking_space;
use parking_space::PARKING_SPACE;
pub mod perfect;
use perfect::PERFECT;
pub mod rain;
use rain::RAIN;
pub mod thanks_party;
use thanks_party::THANKS_PARTY;
pub mod tsugu_ycm;
use tsugu_ycm::TSUGU_YCM;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    BUDOKAN,
    CLEAR_CP,
    EFFORT,
    ENCORE,
    FEVER,
    GACHA10,
    GREAT,
    MARINA_WORK,
    NET_ERROR,
    PARKING_SPACE,
    PERFECT,
    RAIN,
    THANKS_PARTY,
    TSUGU_YCM,
];