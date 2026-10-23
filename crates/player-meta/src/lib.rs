//! Player meta layer around the match: player profile, levels/EXP/charge/coins/stars,
//! ranked placement, match history, Live2D picks, saved decks.
//!
//! The game engine lives in `game-core`. This crate depends on it (never the
//! reverse) and holds only the persistent player state that sits outside a match.

pub mod decks;
pub mod profile;
pub mod progression;
