//! Official cards of band `ag` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod any_color_sunset;
use any_color_sunset::ANY_COLOR_SUNSET;
pub mod crimson_soul;
use crimson_soul::CRIMSON_SOUL;
pub mod declare_war;
use declare_war::DECLARE_WAR;
pub mod detour;
use detour::DETOUR;
pub mod himari_plus_one;
use himari_plus_one::HIMARI_PLUS_ONE;
pub mod moca_half;
use moca_half::MOCA_HALF;
pub mod one_of_us;
use one_of_us::ONE_OF_US;
pub mod proud_light;
use proud_light::PROUD_LIGHT;
pub mod ran_as_usual;
use ran_as_usual::RAN_AS_USUAL;
pub mod same_sky;
use same_sky::SAME_SKY;
pub mod shop_friends;
use shop_friends::SHOP_FRIENDS;
pub mod sunset;
use sunset::SUNSET;
pub mod tomoe_savior;
use tomoe_savior::TOMOE_SAVIOR;
pub mod tsugumi_can;
use tsugumi_can::TSUGUMI_CAN;
pub mod yolo;
use yolo::YOLO;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    ANY_COLOR_SUNSET,
    CRIMSON_SOUL,
    DECLARE_WAR,
    DETOUR,
    HIMARI_PLUS_ONE,
    MOCA_HALF,
    ONE_OF_US,
    PROUD_LIGHT,
    RAN_AS_USUAL,
    SAME_SKY,
    SHOP_FRIENDS,
    SUNSET,
    TOMOE_SAVIOR,
    TSUGUMI_CAN,
    YOLO,
];