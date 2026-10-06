//! Cross-endpoint state: whatever must survive from one request to the next.
//!
//! The server is the owner of mutable state and the rule workers are stateless
//! (see [`crate::pool`]); this trait is the boundary that state crosses. Two
//! stores implement it:
//!
//! * [`dummy`] -- in-process maps. One server, no durability: a restart loses
//!   every session, room and match. **This is the development default, and that
//!   is a deliberate choice, not a gap** -- during development we are not
//!   running a database, and the in-memory store is the honest version of that.
//! * [`redis`] -- shared and durable. Several server processes can serve the
//!   same clients, and a restart keeps them. This is the "no state in this
//!   process" option.
//!
//! A proper database is the production answer for the room roster and the
//! session table (queryable, transactional, survives the box). It would be a
//! **third `CrossState` impl**, added behind this same trait -- which is the
//! whole reason the seam exists. Nothing above is specific to the two stores
//! here, so that is additive: the trait, the `SessionRecord`/`RoomInfo`
//! shapes, and the call sites do not change.
//!
//! Deliberately **not** here: per-connection bookkeeping (SSE stream counts,
//! the room's broadcast channel) and the ordering gate on a match. Those are
//! properties of a live process, not of the state it is holding -- moving them
//! behind this trait would buy nothing and cost a network hop per frame.
//!
//! The trait is **sync**. Both stores are used from blocking closures
//! (`spawn_blocking`) as well as from async handlers, so a sync surface serves
//! both; a Redis call is a round-trip, so async callers should not make it on a
//! runtime thread (the same rule as the worker pool).

use std::fmt;

use game_core::net::RoomInfo;
use serde::{Deserialize, Serialize};

use crate::state::Session;

/// What a store failed to do. Carries enough to log; never a stack.
#[derive(Debug, Clone)]
pub enum StoreError {
    /// The backend is not reachable or the operation failed underneath.
    Backend(String),
    /// A value in the store could not be decoded -- corruption, or a version
    /// change the code did not migrate.
    Corrupt(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Backend(e) => write!(f, "store backend: {e}"),
            StoreError::Corrupt(e) => write!(f, "store corrupt: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The buckets of state a match server keeps between endpoints.
///
/// Keys are the server's own: a session token, a room id. Values are the plain
/// serde records, so a store that persists them must round-trip them exactly.
pub trait CrossState: Send + Sync {
    /// A signed-in client. `None` for an unknown or expired token.
    fn session_get(&self, token: &str) -> Result<Option<Session>, StoreError>;
    /// Create or update a session (this is also how a room join is recorded).
    fn session_put(&self, s: &Session) -> Result<(), StoreError>;
    fn session_del(&self, token: &str) -> Result<(), StoreError>;
    /// Every live session -- what a sweep walks. A store with native expiry may
    /// legitimately return fewer than were written.
    fn session_all(&self) -> Result<Vec<Session>, StoreError>;

    /// A room's public record (the lobby lists these).
    fn room_get(&self, id: &str) -> Result<Option<RoomRecord>, StoreError>;
    fn room_put(&self, rec: &RoomRecord) -> Result<(), StoreError>;
    fn room_del(&self, id: &str) -> Result<(), StoreError>;
    fn room_all(&self) -> Result<Vec<RoomRecord>, StoreError>;

    /// The running match for a room: the `Match::save` blob, if play is live.
    /// This is the engine state; the worker sees it only as a request payload.
    fn match_get(&self, room: &str) -> Result<Option<String>, StoreError>;
    fn match_put(&self, room: &str, blob: &str) -> Result<(), StoreError>;
    fn match_del(&self, room: &str) -> Result<(), StoreError>;
}

/// Everything needed to rebuild a [`crate::room::Room`] after a restart.
///
/// [`RoomInfo`] is the lobby's *public* view: name, mode, members. A restart
/// also needs the private half -- the join password, the next member id, and
/// the member -> session-token map -- otherwise a client can hold a valid
/// session naming a room the server no longer knows how to rejoin. With this
/// record on the store, a restart is transparent: the room comes back, the
/// match blob is already there, and the client's SSE reconnects into the same
/// game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomRecord {
    /// The lobby-visible half.
    pub info: RoomInfo,
    /// The join password ("" = unlocked).
    pub password: String,
    /// The next member id to hand out.
    pub next_member: i32,
    /// Member id -> session token. `Vec` so the encoding is stable.
    pub tokens: Vec<(i32, String)>,
}

/// The record a store persists for a session. Mirrors [`Session`]; split out so
/// the wire/persisted shape is explicit and can version independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub token: String,
    pub player: String,
    pub character: String,
    pub cn_id: String,
    pub room: Option<(String, i32)>,
}

impl From<&Session> for SessionRecord {
    fn from(s: &Session) -> Self {
        Self {
            token: s.token.clone(),
            player: s.player.clone(),
            character: s.character.clone(),
            cn_id: s.cn_id.clone(),
            room: s.room.clone(),
        }
    }
}

impl From<SessionRecord> for Session {
    fn from(r: SessionRecord) -> Self {
        Session {
            token: r.token,
            player: r.player,
            character: r.character,
            cn_id: r.cn_id,
            room: r.room,
        }
    }
}

pub mod dummy;
pub mod redis;