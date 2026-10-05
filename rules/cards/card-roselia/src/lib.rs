//! Official cards of band `roselia` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod ako_dark;
use ako_dark::AKO_DARK;
pub mod before_live;
use before_live::BEFORE_LIVE;
pub mod blue_rose;
use blue_rose::BLUE_ROSE;
pub mod council_check;
use council_check::COUNCIL_CHECK;
pub mod cookie_time;
use cookie_time::COOKIE_TIME;
pub mod fire_bird;
use fire_bird::FIRE_BIRD;
pub mod lisa_bond;
use lisa_bond::LISA_BOND;
pub mod louder;
use louder::LOUDER;
pub mod nfo;
use nfo::NFO;
pub mod own_stage;
use own_stage::OWN_STAGE;
pub mod press;
use press::PRESS;
pub mod resolve;
use resolve::RESOLVE;
pub mod ringing_bloom;
use ringing_bloom::RINGING_BLOOM;
pub mod sayo_play;
use sayo_play::SAYO_PLAY;
pub mod sprechchor;
use sprechchor::SPRECHCHOR;
pub mod to_the_peak;
use to_the_peak::TO_THE_PEAK;
pub mod trajectory;
use trajectory::TRAJECTORY;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    AKO_DARK,
    BEFORE_LIVE,
    BLUE_ROSE,
    COUNCIL_CHECK,
    COOKIE_TIME,
    FIRE_BIRD,
    LISA_BOND,
    LOUDER,
    NFO,
    OWN_STAGE,
    PRESS,
    RESOLVE,
    RINGING_BLOOM,
    SAYO_PLAY,
    SPRECHCHOR,
    TO_THE_PEAK,
    TRAJECTORY,
];