//! Shared match state, data models, profile logic and wire types.
//!
//! Compiles for **both** native (server) and `wasm32-unknown-unknown` (browser).
//! No filesystem, no clock, no ambient randomness: time enters as `tick(dt)` or as
//! date strings, and randomness is seeded explicitly (`rng`).
//!
//! Every serialized type carries the JSON the data/wire files already use: the
//! same field names, and `#[serde(default)]` so missing fields take the typed
//! default.

pub mod clock;
pub mod data;
pub mod deck;
pub mod deck_book;
pub mod engine;
pub mod fair;
pub mod msg;
pub mod net;
pub mod profile;
pub mod progression;
pub mod record;
pub mod rng;
pub mod scoring;
pub mod state;
pub mod strategy;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Match flavour. Serialized as an integer on the wire / on disk
/// (`0` Solo, `1` Casual, `2` Ranked).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum MatchMode {
    #[default]
    Solo = 0,
    Casual = 1,
    Ranked = 2,
}

impl MatchMode {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Solo),
            1 => Some(Self::Casual),
            2 => Some(Self::Ranked),
            _ => None,
        }
    }

    /// Book seat bounds for this mode (`rulebook-doc.md` 开始游戏 / 娱乐 / 正规).
    ///
    /// * 娱乐 (Casual) -- 「游玩人数为3到10人」 (G-FLOW-12).
    /// * 正规 (Ranked) -- 「游玩人数须在5到6人」 (G-FLOW-14).
    /// * Solo is a practice table, not the book's 娱乐 mode: unbounded here
    ///   (the engine still caps at 10 seats).
    ///
    /// `None` = no book bound. `Some((lo, hi))` is inclusive.
    pub fn seat_range(self) -> Option<(usize, usize)> {
        match self {
            Self::Solo => None,
            Self::Casual => Some((3, 10)),
            Self::Ranked => Some((5, 6)),
        }
    }

    /// Does `n` fall inside this mode's book seat bound?
    pub fn seats_ok(self, n: usize) -> bool {
        self.seat_range().is_none_or(|(lo, hi)| (lo..=hi).contains(&n))
    }

    /// Reject a seat count the book forbids. Empty `Err` text is impossible:
    /// every refusal names the clause bound.
    pub fn check_seats(self, n: usize) -> Result<(), String> {
        if self.seats_ok(n) {
            return Ok(());
        }
        let (clause, lo, hi) = match self {
            Self::Casual => ("G-FLOW-12", 3, 10),
            Self::Ranked => ("G-FLOW-14", 5, 6),
            Self::Solo => return Ok(()),
        };
        Err(format!(
            "{clause}: {:?} mode seats {n} players, book says {lo}到{hi}人",
            self
        ))
    }
}

impl Serialize for MatchMode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(*self as i32)
    }
}

impl<'de> Deserialize<'de> for MatchMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i32::deserialize(d)?;
        Self::from_i32(v).ok_or_else(|| serde::de::Error::custom(format!("invalid MatchMode {v}")))
    }
}
