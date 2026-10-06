//! Official cards of band `mujica` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod black_birthday;
use black_birthday::BLACK_BIRTHDAY;
pub mod cant_look_away;
use cant_look_away::CANT_LOOK_AWAY;
pub mod crystal_swap;
use crystal_swap::CRYSTAL_SWAP;
pub mod dice_cast;
use dice_cast::DICE_CAST;
pub mod doll_garden;
use doll_garden::DOLL_GARDEN;
pub mod heart_rain;
use heart_rain::HEART_RAIN;
pub mod j11;
use j11::J11;
pub mod mortis_instinct;
use mortis_instinct::MORTIS_INSTINCT;
pub mod nyamu_card;
use nyamu_card::NYAMU_CARD;
pub mod saki_move;
use saki_move::SAKI_MOVE;
pub mod sakiko_cut;
use sakiko_cut::SAKIKO_CUT;
pub mod sparkler;
use sparkler::SPARKLER;
pub mod uika_fearless;
use uika_fearless::UIKA_FEARLESS;
pub mod umiri_card;
use umiri_card::UMIRI_CARD;
pub mod welcome_mujica;
use welcome_mujica::WELCOME_MUJICA;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    BLACK_BIRTHDAY,
    CANT_LOOK_AWAY,
    CRYSTAL_SWAP,
    DICE_CAST,
    DOLL_GARDEN,
    HEART_RAIN,
    J11,
    MORTIS_INSTINCT,
    NYAMU_CARD,
    SAKI_MOVE,
    SAKIKO_CUT,
    SPARKLER,
    UIKA_FEARLESS,
    UMIRI_CARD,
    WELCOME_MUJICA,
];
