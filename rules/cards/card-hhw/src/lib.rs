//! Official cards of band `hhw` (C# `Card*` classes in MatchHost.cs).
//!
//! One crate per band, one module per card; the card id is a data key from
//! `data/cards.json`. Message keys are namespaced by the crate name and
//! translated in `locales/`.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[macro_use]
extern crate alloc;

pub mod balloon_show;
use balloon_show::BALLOON_SHOW;
pub mod backstage_tour;
use backstage_tour::BACKSTAGE_TOUR;
pub mod believe_you;
use believe_you::BELIEVE_YOU;
pub mod black_suits;
use black_suits::BLACK_SUITS;
pub mod charity_show;
use charity_show::CHARITY_SHOW;
pub mod dream_return;
use dream_return::DREAM_RETURN;
pub mod hagumi_marks;
use hagumi_marks::HAGUMI_MARKS;
pub mod happy_lucky;
use happy_lucky::HAPPY_LUCKY;
pub mod kanon_march;
use kanon_march::KANON_MARCH;
pub mod kaoru_thief;
use kaoru_thief::KAORU_THIEF;
pub mod kokoro_circle;
use kokoro_circle::KOKORO_CIRCLE;
pub mod misaki_card;
use misaki_card::MISAKI_CARD;
pub mod smile_parade;
use smile_parade::SMILE_PARADE;
pub mod smile_patrol;
use smile_patrol::SMILE_PATROL;
pub mod sports_talent;
use sports_talent::SPORTS_TALENT;

/// Every card of this band, in registration order. The shipped module
/// (`cards/card-all`) concatenates these tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    BALLOON_SHOW,
    BACKSTAGE_TOUR,
    BELIEVE_YOU,
    BLACK_SUITS,
    CHARITY_SHOW,
    DREAM_RETURN,
    HAGUMI_MARKS,
    HAPPY_LUCKY,
    KANON_MARCH,
    KAORU_THIEF,
    KOKORO_CIRCLE,
    MISAKI_CARD,
    SMILE_PARADE,
    SMILE_PATROL,
    SPORTS_TALENT,
];