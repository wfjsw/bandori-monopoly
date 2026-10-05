//! Official cards of band `morfonica` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod centrifugal;
use centrifugal::CENTRIFUGAL;
pub mod courage_wings;
use courage_wings::COURAGE_WINGS;
pub mod hold_hands_again;
use hold_hands_again::HOLD_HANDS_AGAIN;
pub mod mashiro_pay;
use mashiro_pay::MASHIRO_PAY;
pub mod nanami_effort;
use nanami_effort::NANAMI_EFFORT;
pub mod noble_blue;
use noble_blue::NOBLE_BLUE;
pub mod pure_wings;
use pure_wings::PURE_WINGS;
pub mod rui_devil;
use rui_devil::RUI_DEVIL;
pub mod secret_rainbow;
use secret_rainbow::SECRET_RAINBOW;
pub mod starry_night;
use starry_night::STARRY_NIGHT;
pub mod summer_camp;
use summer_camp::SUMMER_CAMP;
pub mod toko_swap;
use toko_swap::TOKO_SWAP;
pub mod tritone;
use tritone::TRITONE;
pub mod tsukushi_garden;
use tsukushi_garden::TSUKUSHI_GARDEN;
pub mod your_light;
use your_light::YOUR_LIGHT;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    CENTRIFUGAL,
    COURAGE_WINGS,
    HOLD_HANDS_AGAIN,
    MASHIRO_PAY,
    NANAMI_EFFORT,
    NOBLE_BLUE,
    PURE_WINGS,
    RUI_DEVIL,
    SECRET_RAINBOW,
    STARRY_NIGHT,
    SUMMER_CAMP,
    TOKO_SWAP,
    TRITONE,
    TSUKUSHI_GARDEN,
    YOUR_LIGHT,
];