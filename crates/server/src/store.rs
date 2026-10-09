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
use game_core::record::{EngineStamp, Init, Input};
use serde::{Deserialize, Serialize};

use crate::state::Session;

/// How long a finished match's record stays downloadable. Past that it is
/// stale: the engines it names may have moved on, and the hands it reveals are
/// nobody's business any more.
pub const RECORD_TTL_SECS: u64 = 24 * 60 * 60;

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

    /// The room's **record log** for the match in progress: one JSON line per
    /// [`RecordHead`] / [`Input`] / [`Cp`] (`docs/REPLAY.md` §4). It lives
    /// beside the match blob rather than inside it -- the blob is the engine's
    ///, and the log is the driver's. Cleared when the match is sealed into a
    /// [`StoredRecord`].
    fn record_log_append(&self, room: &str, line: &str) -> Result<(), StoreError>;
    fn record_log_get(&self, room: &str) -> Result<Vec<String>, StoreError>;
    fn record_log_del(&self, room: &str) -> Result<(), StoreError>;

    /// The last **finished** match's record for a room, kept for
    /// [`RECORD_TTL_SECS`] (a store with native expiry may drop it sooner).
    fn record_put(&self, room: &str, rec: &StoredRecord) -> Result<(), StoreError>;
    fn record_get(&self, room: &str) -> Result<Option<StoredRecord>, StoreError>;
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

// ---------------------------------------------------------------- record log

/// One line of a room's record log. Externally tagged, so a line is either a
/// [`RecordHead`] (written once, at `start`), an [`Input`] of the match, or the
/// [`Cp`] checkpoint that follows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogEntry {
    Head(RecordHead),
    Input(Input),
    Cp(Cp),
}

/// The first log line: how the match was started and who is writing it. Lives
/// in the log rather than in memory so a restart can still seal the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordHead {
    /// `Init::Seed` -- members (bot mentality included), seed and weights.
    pub init: Init,
    /// The engine that is running the match (`pool.info()`).
    pub stamp: EngineStamp,
    /// The tick quantum this match actually uses (`0.05 * time_scale`).
    pub step: f32,
    /// `yyyy-MM-dd HH:mm` on the server's clock, for display only.
    pub created: String,
    /// [`game_core::MatchMode`] as its wire integer.
    pub mode: i32,
    /// Commit-reveal material (`docs/FAIRNESS.md`): the openings, written
    /// here when the match is created and copied into the sealed record's
    /// header when it ends. This log line is server-side storage, not a
    /// client-facing payload -- it is the only place the seed/salt live
    /// between match creation and the reveal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair: Option<game_core::fair::Fairness>,
}

/// A turn boundary. `at` counts [`Input`]s (the open tick run included), so the
/// checkpoint always sits between two of them -- the same shape
/// [`game_core::record::Checkpoint`] has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cp {
    pub at: u32,
    #[serde(with = "game_core::record::u64_str")]
    pub tick: u64,
    pub round: i32,
    pub turn: i32,
    /// `hash_save` of the `Match::save()` at the boundary.
    pub hash: String,
}

/// A finished match's record, as `record:{room}:last` holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredRecord {
    /// The sealed `.bdrec` as bytes: a **zstd** frame of the record JSON (what
    /// [`crate::room`] writes at seal). A store written before compression may
    /// hold plain record JSON here; the endpoint serves either as-is and names
    /// the type by sniffing.
    pub record: Vec<u8>,
    /// Member ids that sat down for this match. `GET /api/rooms/{id}/record`
    /// only serves one of these, resolved through the room's tokens.
    pub members: Vec<i32>,
}

impl LogEntry {
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("log entry serializes")
    }

    pub fn parse_line(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}
