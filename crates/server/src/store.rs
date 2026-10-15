//! Cross-endpoint state: whatever must survive from one request to the next.
//!
//! The server is the owner of mutable state and the rule workers are stateless
//! (see [`crate::pool`]); this trait is the boundary that state crosses. Two
//! stores implement it:
//!
//! * [`dummy`] -- in-process maps. One **shared** store object (it is
//!   `Clone` over an `Arc`, and two [`crate::Server`]s can hold the same one),
//!   no durability across processes: a restart loses every session, room and
//!   match. **This is the development default, and that is a deliberate choice,
//!   not a gap** -- during development we are not running a database, and the
//!   in-memory store is the honest version of that.
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
//! Everything a server process needs between two requests lives here, so two
//! processes sharing one store can serve the same match and either can restart
//! mid-game. That includes the primitives that *coordinate* processes rather
//! than describe the game: the short-lived locks ([`CrossState::lock_acquire`]
//! and friends, used through [`with_lock`]), the change notes
//! ([`CrossState::publish`] / [`CrossState::subscribe`]), the per-stream
//! presence keys, and the record-log bookkeeping ([`RecMeta`]). Per-connection
//! caches (the SSE cursor, the local broadcast fan-out, bot ponder trees) stay
//! in the process -- they are performance only, and a missed note is recovered
//! by re-polling the store.
//!
//! The trait is **sync**. Both stores are used from blocking closures
//! (`spawn_blocking`) as well as from async handlers, so a sync surface serves
//! both; a Redis call is a round-trip, so async callers should not make it on a
//! runtime thread (the same rule as the worker pool).

use std::fmt;
use std::time::Duration;

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
    /// [`RecordHead`] / [`Input`] / [`Checkpoint`] (`docs/REPLAY.md` §4). It lives
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

    // ------------------------------------------------------------- locks
    //
    // Short-lived exclusive claims across processes: the per-room mutation
    // lock, the per-match operation lock, the per-room tick lease, the bot
    // probe claims. `owner` is a per-process token so a holder only renews or
    // releases its own lock. Semantics: `SET key owner NX PX ttl` -- acquire
    // fails while someone else holds it; a TTL'd lock a dead process left
    // behind frees itself.

    /// Take `key` for `owner` for `ttl_ms`. `false` when someone else holds it.
    fn lock_acquire(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError>;
    /// Extend a lock this `owner` still holds. `false` when it does not (it
    /// expired and may now belong to somebody else).
    fn lock_renew(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError>;
    /// Drop a lock this `owner` holds. A lock held by anybody else is left
    /// alone (compare-and-delete) -- a stale release must not free it.
    fn lock_release(&self, key: &str, owner: &str) -> Result<(), StoreError>;

    // ------------------------------------------------------------ pub/sub
    //
    // Room change notes and dissolves. Delivery is best-effort: every consumer
    // also re-polls the store, so a dropped message is latency, never
    // correctness.

    /// Send `msg` to everything subscribed to `channel`.
    fn publish(&self, channel: &str, msg: &str) -> Result<(), StoreError>;
    /// Subscribe to `channel`. The receiver gets every message published from
    /// now on; a backend that reconnects may drop messages in between (the
    /// consumers re-poll).
    fn subscribe(&self, channel: &str) -> Result<std::sync::mpsc::Receiver<PubMsg>, StoreError>;

    // ----------------------------------------------------------- presence
    //
    // One TTL key per live SSE stream (`presence:{room}:{member}:{stream}`),
    // plus an `idle` key that covers "joined but no stream yet" and the grace
    // after the last stream closed. A member is present while
    // [`Self::presence_count`] is non-zero; the keys' TTLs are what make a
    // dead process's streams vanish on their own.

    /// Note (or refresh) one live stream of `member` in `room`.
    fn presence_set(&self, room: &str, member: i32, stream: &str, ttl_ms: u64)
        -> Result<(), StoreError>;
    /// That one stream closed.
    fn presence_del(&self, room: &str, member: i32, stream: &str) -> Result<(), StoreError>;
    /// Every stream of `member` is gone (the member left the room).
    fn presence_clear(&self, room: &str, member: i32) -> Result<(), StoreError>;
    /// How many of `member`'s keys are still alive. `0` = gone for at least
    /// one full TTL.
    fn presence_count(&self, room: &str, member: i32) -> Result<u32, StoreError>;

    // ------------------------------------------------ record-log bookkeeping
    //
    // [`RecMeta`], the driver's counters beside the record log. Always read
    // and written under the match lock (see [`with_lock`]), so one JSON blob
    // is enough -- no CAS.

    fn rec_meta_get(&self, room: &str) -> Result<Option<RecMeta>, StoreError>;
    fn rec_meta_put(&self, room: &str, meta: &RecMeta) -> Result<(), StoreError>;
    fn rec_meta_del(&self, room: &str) -> Result<(), StoreError>;
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
    /// The room's commit-reveal slot (`docs/FAIRNESS.md`). Server-side storage,
    /// exactly like the record log head: the openings live here from room
    /// creation until the reveal, and a restart picks the same slot back up so
    /// the commitment the lobby showed does not change under the clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair: Option<RoomFair>,
}

/// The persisted half of a room's commit-reveal slot. Hex strings, because the
/// store's encoding is JSON; the in-memory [`crate::room`] form keeps the raw
/// 32 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomFair {
    /// Server secret 256-bit seed (hex).
    pub seed: String,
    /// Server secret 256-bit salt (hex).
    pub salt: String,
    /// The commitment (hex) shown in the room -- drawn at room creation,
    /// before any nonce can exist.
    pub commit: String,
    /// Nonces collected for the next match, ascending member id.
    pub nonces: Vec<(i32, String)>,
    /// A match has consumed this slot: the commit still shows for the running
    /// match, but no nonce is accepted and the next match waits for a fresh
    /// slot (rolled when the match ends).
    pub consumed: bool,
}

/// The record a store persists for a session. Mirrors [`Session`]; split out so
/// the wire/persisted shape is explicit and can version independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub token: String,
    pub player: String,
    pub persona: String,
    pub cn_id: String,
    pub room: Option<(String, i32)>,
}

impl From<&Session> for SessionRecord {
    fn from(s: &Session) -> Self {
        Self {
            token: s.token.clone(),
            player: s.player.clone(),
            persona: s.persona.clone(),
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
            persona: r.persona,
            cn_id: r.cn_id,
            room: r.room,
        }
    }
}

/// One published note: the channel it came in on and its payload.
#[derive(Debug, Clone)]
pub struct PubMsg {
    pub channel: String,
    pub payload: String,
}

/// Bookkeeping for one match's record log -- the counters [`crate::room`] used
/// to keep in memory. Persisted beside the log so another process (or a
/// restarted one) picks up exactly where the last writer left off: the open
/// tick run included, so a change of hands loses nothing.
///
/// Read and written only under the match lock.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RecMeta {
    /// The open tick run (`Input::Ticks`), not yet flushed to the log. Run-
    /// length merging of consecutive ticks is what keeps a long match's log
    /// small; the run lives here (not in a process) until it closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buf: Option<(u8, u32)>,
    /// Inputs the log holds, counting the buffered run as one.
    pub inputs: u32,
    /// Tick calls recorded -- the sum of every `Ticks.n`.
    pub ticks: u64,
    /// `at` of the last checkpoint, so a run closes at a turn boundary.
    pub last_ckpt_at: u32,
    /// Something was lost: a store write failed, or a previous holder died
    /// mid-operation (see [`RecMeta::dirty`]). The sealed record is then
    /// unverifiable (`RecordHeader::gaps`).
    pub gaps: bool,
    /// Members seated when the match started -- the record's participants.
    #[serde(default)]
    pub members: Vec<i32>,
    /// Sealed; stop appending.
    pub done: bool,
    /// A recorded operation was in flight. Set under the match lock *before*
    /// the worker call and cleared after its log lines are written; a leftover
    /// on load means that operation's input may be missing from the log and
    /// [`RecMeta::gaps`] must be raised. Never set outside the lock.
    pub dirty: bool,
}

/// Default TTL for a cross-process lock: long enough for one match / room
/// operation (a worker round-trip plus a store write), short enough that a
/// dead holder's lock frees itself quickly. Single operations do not renew.
pub const LOCK_TTL_MS: u64 = 10_000;
/// How long [`with_lock`] retries before giving up. Below [`LOCK_TTL_MS`] so
/// a waiter cannot out-wait the lock's own expiry and stampede.
pub const LOCK_DEADLINE: Duration = Duration::from_secs(8);

/// The pub/sub channel for room change notes and dissolves. Payload is a small
/// JSON object (`{room, kind, reason?}`); delivery is best-effort.
pub const NOTE_CHANNEL: &str = "bm:note";

/// A held cross-process lock. Releases (compare-and-delete) on drop, so a
/// panic inside the critical section cannot leak the lock past its owner.
pub struct LockGuard<'a> {
    store: &'a dyn CrossState,
    key: String,
    owner: String,
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        let _ = self.store.lock_release(&self.key, &self.owner);
    }
}

/// One attempt at a cross-process lock. `None` = somebody else has it.
pub fn try_lock<'a>(
    store: &'a dyn CrossState,
    key: &str,
    owner: &str,
    ttl_ms: u64,
) -> Result<Option<LockGuard<'a>>, StoreError> {
    if store.lock_acquire(key, owner, ttl_ms)? {
        Ok(Some(LockGuard {
            store,
            key: key.to_string(),
            owner: owner.to_string(),
        }))
    } else {
        Ok(None)
    }
}

/// Run `f` while holding the cross-process lock `key`.
///
/// Retries with short sleeps up to `deadline`. The outer `Err` is the lock
/// itself (never a fallthrough to unlocked execution); the inner `Result` is
/// whatever `f` returned. Match / room operations are short, so the
/// [`LOCK_TTL_MS`] default needs no renewal.
pub fn try_with_lock<T, E>(
    store: &dyn CrossState,
    key: &str,
    owner: &str,
    ttl_ms: u64,
    deadline: Duration,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<Result<T, E>, StoreError> {
    let start = std::time::Instant::now();
    loop {
        match try_lock(store, key, owner, ttl_ms) {
            Ok(Some(_guard)) => {
                let out = f();
                // `_guard` drops (and releases) after `f` returns or panics.
                return Ok(out);
            }
            Ok(None) => {
                if start.elapsed() >= deadline {
                    return Err(StoreError::Backend(format!(
                        "lock {key} not acquired within {deadline:?}"
                    )));
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => return Err(e),
        }
    }
}

/// [`try_with_lock`] for callers whose error type is [`String`]: the lock
/// failure is flattened into the same `Err` channel.
pub fn with_lock<T>(
    store: &dyn CrossState,
    key: &str,
    owner: &str,
    ttl_ms: u64,
    deadline: Duration,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    match try_with_lock(store, key, owner, ttl_ms, deadline, f) {
        Ok(inner) => inner,
        Err(e) => Err(format!("lock {key}: {e}")),
    }
}

pub mod dummy;
pub mod redis;

// ---------------------------------------------------------------- record log

/// One line of a room's record log. Externally tagged, so a line is either a
/// [`RecordHead`] (written once, at `start`), an [`Input`] of the match, or the
/// [`Checkpoint`] checkpoint that follows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogEntry {
    Head(RecordHead),
    Input(Input),
    Checkpoint(Checkpoint),
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
pub struct Checkpoint {
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
