//! Official cards of band `crychic` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[macro_use]
extern crate alloc;

pub mod debut_success;
use debut_success::DEBUT_SUCCESS;
pub mod elegant_shout;
use elegant_shout::ELEGANT_SHOUT;
pub mod fate_together;
use fate_together::FATE_TOGETHER;
pub mod forever;
use forever::FOREVER;
pub mod haruhikage;
use haruhikage::HARUHIKAGE;
pub mod karaoke;
use karaoke::KARAOKE;
pub mod mutsumi_never;
use mutsumi_never::MUTSUMI_NEVER;
pub mod my_own_problem;
use my_own_problem::MY_OWN_PROBLEM;
pub mod sakiko_lead;
use sakiko_lead::SAKIKO_LEAD;
pub mod soyo_back;
use soyo_back::SOYO_BACK;
pub mod taki_even_if;
use taki_even_if::TAKI_EVEN_IF;
pub mod tomori_inner_shout;
use tomori_inner_shout::TOMORI_INNER_SHOUT;
pub mod vocal_too_hard;
use vocal_too_hard::VOCAL_TOO_HARD;
pub mod want_human;
use want_human::WANT_HUMAN;
pub mod want_to_grab;
use want_to_grab::WANT_TO_GRAB;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    DEBUT_SUCCESS,
    ELEGANT_SHOUT,
    FATE_TOGETHER,
    FOREVER,
    HARUHIKAGE,
    KARAOKE,
    MUTSUMI_NEVER,
    MY_OWN_PROBLEM,
    SAKIKO_LEAD,
    SOYO_BACK,
    TAKI_EVEN_IF,
    TOMORI_INNER_SHOUT,
    VOCAL_TOO_HARD,
    WANT_HUMAN,
    WANT_TO_GRAB,
];