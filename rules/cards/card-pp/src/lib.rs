//! Official cards of band `pp` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod aya_longing;
use aya_longing::AYA_LONGING;
pub mod chisato_mask;
use chisato_mask::CHISATO_MASK;
pub mod dream_ahead;
use dream_ahead::DREAM_AHEAD;
pub mod eve_bushido;
use eve_bushido::EVE_BUSHIDO;
pub mod first_live_accident;
use first_live_accident::FIRST_LIVE_ACCIDENT;
pub mod hina_sound;
use hina_sound::HINA_SOUND;
pub mod infinite_possibility;
use infinite_possibility::INFINITE_POSSIBILITY;
pub mod jennifer;
use jennifer::JENNIFER;
pub mod no_expectation;
use no_expectation::NO_EXPECTATION;
pub mod overlapping_voices;
use overlapping_voices::OVERLAPPING_VOICES;
pub mod ranger;
use ranger::RANGER;
pub mod resonance;
use resonance::RESONANCE;
pub mod same_dream;
use same_dream::SAME_DREAM;
pub mod see_you_tomorrow;
use see_you_tomorrow::SEE_YOU_TOMORROW;
pub mod shine_again;
use shine_again::SHINE_AGAIN;
pub mod strong_flower;
use strong_flower::STRONG_FLOWER;
pub mod title_idol;
use title_idol::TITLE_IDOL;
pub mod together_here;
use together_here::TOGETHER_HERE;
pub mod trainee_guide;
use trainee_guide::TRAINEE_GUIDE;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    AYA_LONGING,
    CHISATO_MASK,
    DREAM_AHEAD,
    EVE_BUSHIDO,
    FIRST_LIVE_ACCIDENT,
    HINA_SOUND,
    INFINITE_POSSIBILITY,
    JENNIFER,
    NO_EXPECTATION,
    OVERLAPPING_VOICES,
    RANGER,
    RESONANCE,
    SAME_DREAM,
    SEE_YOU_TOMORROW,
    SHINE_AGAIN,
    STRONG_FLOWER,
    TITLE_IDOL,
    TOGETHER_HERE,
    TRAINEE_GUIDE,
];