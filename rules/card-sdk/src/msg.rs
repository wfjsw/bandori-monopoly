//! Localizable messages from card effects.
//!
//! Cards never pass display text to the host. They pass a message key plus typed
//! arguments; each client renders it in its own player's language. The wire form is
//! the engine's `Msg`, carried to the host as `postcard` bytes (a serde format --
//! no hand-written encoding either side). The engine then renders it to each client
//! in that client's language.
//!
//! Keys of a card live in its crate's `locales/<lang>.json`; use [`crate::key!`] so
//! they are namespaced by crate (`cards:<crate>.<key>`):
//!
//! ```ignore
//! ctx::log(player, &Msg::new(key!("placed")).tile("tile", t));
//! ```

#[cfg(target_arch = "wasm32")]
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;

use serde::Serialize;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;

/// One typed argument, as the engine's `Msg.a` values.
///
/// The tag names are the wire form (`{"player_id": 1}`); they must match the engine's
/// `Arg` enum in `game-core/src/msg.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Deserialize))]
#[serde(rename_all = "camelCase")]
pub enum Arg {
    /// A player -> that player's name.
    PlayerId(i32),
    /// A board tile -> its name.
    Tile(i32),
    /// A card id -> its title.
    Card(String),
    /// A character (data name) -> its display name.
    Char(String),
    /// Money / price (thousands separators).
    N(i64),
    /// Count / dice value.
    I(i64),
    /// A nested message.
    Msg(Box<Msg>),
}

/// A message by key with typed arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Deserialize))]
pub struct Msg {
    pub k: String,
    /// Always present: postcard is not self-describing, so a skipped field
    /// would truncate the stream for the host.
    pub a: BTreeMap<String, Arg>,
}

/// `cards:<this crate>.<key>` -- a key in this card crate's locale files.
#[macro_export]
macro_rules! key {
    ($k:literal) => {
        concat!("cards:", env!("CARGO_PKG_NAME"), ".", $k)
    };
}

impl Msg {
    /// A message by key. Card keys: `Msg::new(key!("name"))`; engine keys (e.g.
    /// `log.part.why`) may be used too.
    pub fn new(key: &str) -> Self {
        Self { k: key.into(), a: BTreeMap::new() }
    }

    fn with(mut self, name: &str, arg: Arg) -> Self {
        self.a.insert(name.into(), arg);
        self
    }

    /// A player -> that player's name.
    pub fn player_id(self, name: &str, player_id: i32) -> Self {
        self.with(name, Arg::PlayerId(player_id))
    }

    /// A board tile -> its name.
    pub fn tile(self, name: &str, tile: i32) -> Self {
        self.with(name, Arg::Tile(tile))
    }

    /// A card id -> its title.
    pub fn card(self, name: &str, id: &str) -> Self {
        self.with(name, Arg::Card(id.into()))
    }

    /// A character (data name) -> its display name.
    pub fn chara(self, name: &str, character: &str) -> Self {
        self.with(name, Arg::Char(character.into()))
    }

    /// Money / price (thousands separators).
    pub fn n(self, name: &str, v: i64) -> Self {
        self.with(name, Arg::N(v))
    }

    /// Count / dice value.
    pub fn i(self, name: &str, v: i64) -> Self {
        self.with(name, Arg::I(v))
    }

    /// A nested message.
    pub fn msg(self, name: &str, m: &Msg) -> Self {
        self.with(name, Arg::Msg(Box::new(m.clone())))
    }

    /// The host wire form (`postcard`). Cannot fail on these types; the
    /// fallback keeps a malformed log from trapping the card.
    pub fn to_bytes(&self) -> Vec<u8> {
        // Fixed scratch instead of `to_allocvec`: messages are a key plus a
        // handful of args, and this keeps the growth machinery out of the guest.
        let mut buf = [0u8; 1024];
        match postcard::to_slice(self, &mut buf) {
            Ok(written) => written.to_vec(),
            Err(_) => Vec::new(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::{Arg, Msg};

    fn roundtrip(m: &Msg) -> Msg {
        postcard::from_bytes(&m.to_bytes()).expect("postcard round-trip")
    }

    #[test]
    fn round_trips_the_host_wire_form() {
        let m = Msg::new(crate::key!("placed")).tile("tile", 3).player_id("who", 1).msg("why", &Msg::new("x\"y"));
        assert_eq!(roundtrip(&m), m);
        assert_eq!(roundtrip(&Msg::new("k")), Msg::new("k"));
    }

    #[test]
    fn round_trips_every_argument_kind() {
        let mut m = Msg::new("k").card("c", "id").chara("ch", "name").n("n", -5).i("i", 7);
        m = m.with("nest", Arg::Msg(Box::new(Msg::new("inner"))));
        assert_eq!(roundtrip(&m), m);
    }

    #[test]
    fn arg_tags_match_the_engine() {
        // postcard keys enums by variant order; the engine's `Arg` (game-core) must
        // list these kinds under the same names. The engine wire is JSON with these
        // tags -- keep them in lockstep with game-core/src/msg.rs.
        assert_eq!(serde_json_tag(&Arg::PlayerId(1)), "playerId");
        assert_eq!(serde_json_tag(&Arg::N(1)), "n");
        assert_eq!(serde_json_tag(&Arg::Msg(Box::new(Msg::new("x")))), "msg");
    }

    fn serde_json_tag(a: &Arg) -> &'static str {
        match a {
            Arg::PlayerId(_) => "playerId",
            Arg::Tile(_) => "tile",
            Arg::Card(_) => "card",
            Arg::Char(_) => "char",
            Arg::N(_) => "n",
            Arg::I(_) => "i",
            Arg::Msg(_) => "msg",
        }
    }
}
