//! Shared match state, data models, profile logic and wire types.
//!
//! Compiles for **both** native (server) and `wasm32-unknown-unknown` (browser).
//! No filesystem, no clock, no ambient randomness: time enters as `tick(dt)` or as
//! date strings, and randomness is seeded explicitly (`rng`).
//!
//! Every serialized type mirrors a C# `[Serializable]` class from the decompiled
//! game, with the same JSON field names and the same defaults for missing fields.

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

/// `MatchMode.cs`. Serialized as an integer, like Unity's `JsonUtility` does.
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
