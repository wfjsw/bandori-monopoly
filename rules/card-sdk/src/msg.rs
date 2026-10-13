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
//!
//! # `cfg(kani)` facade
//!
//! Under the Kani verifier the symbolic host records narration as a **counter**
//! only (`Op::log` → `push_narration`); the book's effect frame is about game
//! effects, not display text. Building / dropping a production `Msg` walks
//! `String` + `BTreeMap<String, Arg>` + `Vec<u8>` through postcard — deep
//! recursive heap that does not unwind under CBMC (the money_shave profile
//! showed `Vec::extend_with` / `drop_glue` on that path).
//!
//! So under `cfg(kani)` a `Msg` is **only** an interned key plus a small
//! fixed-capacity inline arg array (`Copy`, no heap). Builder methods still
//! exist so card bodies compile unmodified, but they never construct `Arg`
//! (and therefore never allocate). `ctx::log` / `mj` hand the host two ints —
//! the interned-key handle and the arg count — with no `Vec<u8>`.
//!
//! Production wasm / native paths are the `cfg(not(kani))` twins below and are
//! byte-for-byte master's logic.

#[cfg(all(target_arch = "wasm32", not(kani)))]
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
#[cfg(all(target_arch = "wasm32", kani))]
use alloc::vec::Vec;
#[cfg(all(not(target_arch = "wasm32"), not(kani)))]
use std::collections::BTreeMap;
#[cfg(all(not(target_arch = "wasm32"), kani))]
use std::vec::Vec;

#[cfg(not(kani))]
use serde::Serialize;
// Decoding is the *host's* job -- card guests only ever encode a Msg to hand to
// the host, so the derive is kept off their cdylibs. Gate on the `guest`
// feature rather than on `target_arch`: the solo engine is game-rules built for
// wasm32 and it decodes guest messages, so "wasm32" was never the real
// condition (it left `postcard::from_bytes::<Msg>` unbuildable there).
// On a native `guest` build (`rules-native`) the same binary is both the
// card and the host, so it decodes Msg too -- only the wasm32 cdylib keeps
// the derive off for size.
#[cfg(not(kani))]
#[cfg(not(all(feature = "guest", target_arch = "wasm32")))]
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Production wire form (wasm / native). Unchanged from master.
// ---------------------------------------------------------------------------

/// One typed argument, as the engine's `Msg.a` values.
///
/// The tag names are the wire form (`{"player_id": 1}`); they must match the engine's
/// `Arg` enum in `game-core/src/msg.rs`.
#[cfg(not(kani))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(not(all(feature = "guest", target_arch = "wasm32")), derive(Deserialize))]
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

/// A message by key with typed arguments (production).
#[cfg(not(kani))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(not(all(feature = "guest", target_arch = "wasm32")), derive(Deserialize))]
pub struct Msg {
    pub k: String,
    /// Always present on the wire: postcard is not self-describing.
    pub a: BTreeMap<String, Arg>,
}

// ---------------------------------------------------------------------------
// Kani facade: interned key + fixed inline args. `Copy`, no heap.
// ---------------------------------------------------------------------------

/// How many typed args a Kani `Msg` can carry inline. Builder methods past this
/// cap drop the excess (loud only if a proof ever starts reading args; the
/// symbolic host counts log lines and ignores the payload). 4 covers every
/// `ctx::log` / `ctx::effect` call shape in the card corpus.
#[cfg(kani)]
pub const MAX_INLINE_ARGS: usize = 4;

/// Kani-side message: interned key + fixed-capacity inline args.
///
/// `Copy` makes drop/clone O(1) loop iterations (none). The host model does
/// not read `k`/`a` (`Op::log` only bumps a narration counter), so the values
/// are carried for counterexample readability and future arg-aware models.
#[cfg(kani)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msg {
    /// Interned key id (`crate::intern::NameId.0`). 0 is a valid interned name.
    k: u16,
    /// Inline args, little-endian integers. String args are recorded as 0.
    a: [i64; MAX_INLINE_ARGS],
    /// How many of `a` are live.
    n: u8,
}

/// `cards:<this crate>.<key>` -- a key in this card crate's locale files.
#[macro_export]
macro_rules! key {
    ($k:literal) => {
        concat!("cards:", env!("CARGO_PKG_NAME"), ".", $k)
    };
}

#[cfg(not(kani))]
impl Msg {
    /// A message by key. Card keys: `Msg::new(key!("name"))`; engine keys (e.g.
    /// `log.part.why`) may be used too.
    pub fn new(key: &str) -> Self {
        Self {
            k: key.into(),
            a: BTreeMap::new(),
        }
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

#[cfg(kani)]
impl Msg {
    /// A message by key. The key is interned (zero-loop under Kani) so `mj`
    /// can hand the host a `NameId` handle; no `String` is allocated.
    pub fn new(key: &str) -> Self {
        let id = crate::intern::intern(key);
        kani::assert(id.is_valid(), "msg key intern table exhausted or key too long");
        Self {
            k: id.0,
            a: [0; MAX_INLINE_ARGS],
            n: 0,
        }
    }

    /// Push one inline arg (values unused by the symbolic host).
    #[inline(always)]
    fn push(mut self, v: i64) -> Self {
        let i = self.n as usize;
        if i < MAX_INLINE_ARGS {
            self.a[i] = v;
            self.n = self.n.saturating_add(1);
        }
        self
    }

    /// A player -> that player's name. (Value recorded; name dropped — O(1).)
    pub fn player_id(self, _name: &str, player_id: i32) -> Self {
        self.push(player_id as i64)
    }

    /// A board tile -> its name.
    pub fn tile(self, _name: &str, tile: i32) -> Self {
        self.push(tile as i64)
    }

    /// A card id -> its title. The id string is **not** interned (dynamic ids
    /// must not grow the verification intern table); the arg slot is 0.
    pub fn card(self, _name: &str, _id: &str) -> Self {
        self.push(0)
    }

    /// A character (data name) -> its display name. String arg, not interned.
    pub fn chara(self, _name: &str, _character: &str) -> Self {
        self.push(0)
    }

    /// Money / price (thousands separators).
    pub fn n(self, _name: &str, v: i64) -> Self {
        self.push(v)
    }

    /// Count / dice value.
    pub fn i(self, _name: &str, v: i64) -> Self {
        self.push(v)
    }

    /// A nested message. Nested payload is dropped (the host counts one line).
    pub fn msg(self, _name: &str, _m: &Msg) -> Self {
        self.push(0)
    }

    /// ABI pair for `ctx::log` / `mj`: `(intern-tagged key handle, argc)`.
    /// No `Vec<u8>`, no arena intern, O(1).
    #[inline(always)]
    pub fn abi_pair(&self) -> (i32, i32) {
        (
            crate::intern::encode_handle(crate::intern::NameId(self.k)),
            self.n as i32,
        )
    }

    /// Empty bytes. Kept so call sites typed `-> Vec<u8>` still compile; the
    /// Kani `mj` path never calls this (it uses [`Self::abi_pair`]).
    pub fn to_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(kani))]
    use super::{Arg, Msg};

    #[cfg(not(kani))]
    fn roundtrip(m: &Msg) -> Msg {
        postcard::from_bytes(&m.to_bytes()).expect("postcard round-trip")
    }

    #[test]
    #[cfg(not(kani))]
    fn round_trips_the_host_wire_form() {
        let m = Msg::new(crate::key!("placed"))
            .tile("tile", 3)
            .player_id("who", 1)
            .msg("why", &Msg::new("x\"y"));
        assert_eq!(roundtrip(&m), m);
        assert_eq!(roundtrip(&Msg::new("k")), Msg::new("k"));
    }

    #[test]
    #[cfg(not(kani))]
    fn round_trips_every_argument_kind() {
        let mut m = Msg::new("k")
            .card("c", "id")
            .chara("ch", "name")
            .n("n", -5)
            .i("i", 7);
        m = m.with("nest", Arg::Msg(Box::new(Msg::new("inner"))));
        assert_eq!(roundtrip(&m), m);
    }

    #[test]
    #[cfg(not(kani))]
    fn arg_tags_match_the_engine() {
        // postcard keys enums by variant order; the engine's `Arg` (game-core) must
        // list these kinds under the same names. The engine wire is JSON with these
        // tags -- keep them in lockstep with game-core/src/msg.rs.
        assert_eq!(serde_json_tag(&Arg::PlayerId(1)), "playerId");
        assert_eq!(serde_json_tag(&Arg::N(1)), "n");
        assert_eq!(serde_json_tag(&Arg::Msg(Box::new(Msg::new("x")))), "msg");
    }

    #[cfg(not(kani))]
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

    /// Kani facade is `Copy` and never allocates — the money_shave cliff.
    #[test]
    #[cfg(not(kani))]
    fn production_msg_still_carries_typed_args() {
        let m = Msg::new("k").i("n", 7).player_id("who", 2);
        assert_eq!(m.a.len(), 2);
        assert_eq!(m.a.get("n"), Some(&Arg::I(7)));
    }
}