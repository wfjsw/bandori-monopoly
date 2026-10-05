//! Official cards of band `ppp` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod arisa_wait;
use arisa_wait::ARISA_WAIT;
pub mod auction_pulled;
use auction_pulled::AUCTION_PULLED;
pub mod bang_dream;
use bang_dream::BANG_DREAM;
pub mod caught;
use caught::CAUGHT;
pub mod kasumi_love_all;
use kasumi_love_all::KASUMI_LOVE_ALL;
pub mod maze_warehouse;
use maze_warehouse::MAZE_WAREHOUSE;
pub mod pipopa;
use pipopa::PIPOPA;
pub mod popipa;
use popipa::POPIPA;
pub mod popipapapipopa;
use popipapapipopa::POPIPAPAPIPOPA;
pub mod random_star;
use random_star::RANDOM_STAR;
pub mod returns;
use returns::RETURNS;
pub mod rimi_choco;
use rimi_choco::RIMI_CHOCO;
pub mod saaya_sky;
use saaya_sky::SAAYA_SKY;
pub mod signpost;
use signpost::SIGNPOST;
pub mod star_beat;
use star_beat::STAR_BEAT;
pub mod tae_sound;
use tae_sound::TAE_SOUND;
pub mod to_you_far_away;
use to_you_far_away::TO_YOU_FAR_AWAY;
pub mod tomorrows_door;
use tomorrows_door::TOMORROWS_DOOR;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    ARISA_WAIT,
    AUCTION_PULLED,
    BANG_DREAM,
    CAUGHT,
    KASUMI_LOVE_ALL,
    MAZE_WAREHOUSE,
    PIPOPA,
    POPIPA,
    POPIPAPAPIPOPA,
    RANDOM_STAR,
    RETURNS,
    RIMI_CHOCO,
    SAAYA_SKY,
    SIGNPOST,
    STAR_BEAT,
    TAE_SOUND,
    TO_YOU_FAR_AWAY,
    TOMORROWS_DOOR,
];