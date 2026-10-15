//! In-memory [`CrossState`]: a handful of maps, one process, no durability.
//!
//! This is the single-process default, and it is **shareable**: the handle is
//! `Clone` over an [`Arc`], so two [`crate::Server`]s can hold the same store
//! and behave exactly as they would against one Redis (the multi-instance
//! tests do). A restart still loses every session, room and match -- which is
//! exactly what the pre-trait behaviour was, kept honest rather than implied
//! by an absent abstraction.
//!
//! TTLs (locks, presence) expire on a monotonic clock, matching what Redis
//! does with `PX`: a lock a dead holder left behind frees itself, and a
//! presence key a dead SSE pump stopped refreshing vanishes.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use super::{
    CrossState, PubMsg, RecMeta, RoomRecord, SessionRecord, StoreError, StoredRecord,
    RECORD_TTL_SECS,
};
use crate::state::Session;

/// How many finished records the in-memory store keeps. A real store expires
/// them by [`RECORD_TTL_SECS`] instead; this one bounds the map and drops the
/// least recently used.
pub const RECORD_KEEP: usize = 64;

/// One TTL'd string: a held lock or a live presence key.
struct Ttl {
    value: String,
    expires: Instant,
}

impl Ttl {
    fn live(&self, now: Instant) -> bool {
        now < self.expires
    }
}

/// The maps. Behind one [`Arc`] so the outer [`Store`] handle is cheap to
/// clone and every clone sees the same state.
#[derive(Default)]
struct Inner {
    sessions: Mutex<HashMap<String, SessionRecord>>,
    rooms: Mutex<HashMap<String, RoomRecord>>,
    matches: Mutex<HashMap<String, String>>,
    /// `room -> (last-used stamp, record)`, evicted LRU past [`RECORD_KEEP`].
    records: Mutex<HashMap<String, (u64, StoredRecord)>>,
    /// `room -> record log lines`, for the match in progress.
    logs: Mutex<HashMap<String, Vec<String>>>,
    /// `room -> record-log bookkeeping` ([`RecMeta`]), under the match lock.
    rec_meta: Mutex<HashMap<String, RecMeta>>,
    /// `key -> (owner, expiry)`: the cross-process locks.
    locks: Mutex<HashMap<String, Ttl>>,
    /// `room|member|stream -> expiry`: the presence keys.
    presence: Mutex<HashMap<String, Ttl>>,
    /// Live pub/sub subscribers. Disconnected receivers are dropped on the
    /// next publish.
    subs: Mutex<HashMap<String, Vec<mpsc::Sender<PubMsg>>>>,
    used: Mutex<u64>,
}

/// In-memory store. `Clone` shares the same state (see [`Inner`]).
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::default()),
        }
    }

    /// Monotonic "now" relative to store creation.
    fn now(&self) -> Instant {
        Instant::now()
    }

    /// Monotonic "now" for the LRU order. Not wall time: this store has no
    /// wall clock, only recency.
    fn tick(&self) -> u64 {
        let mut n = self.inner.used.lock().unwrap_or_else(|e| e.into_inner());
        *n += 1;
        *n
    }

    fn pres_key(room: &str, member: i32, stream: &str) -> String {
        format!("{room}|{member}|{stream}")
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, StoreError> {
    m.lock()
        .map_err(|_| StoreError::Backend("store lock poisoned".into()))
}

impl CrossState for Store {
    fn session_get(&self, token: &str) -> Result<Option<Session>, StoreError> {
        Ok(lock(&self.inner.sessions)?.get(token).cloned().map(Into::into))
    }

    fn session_put(&self, s: &Session) -> Result<(), StoreError> {
        lock(&self.inner.sessions)?.insert(s.token.clone(), SessionRecord::from(s));
        Ok(())
    }

    fn session_del(&self, token: &str) -> Result<(), StoreError> {
        lock(&self.inner.sessions)?.remove(token);
        Ok(())
    }

    fn session_all(&self) -> Result<Vec<Session>, StoreError> {
        Ok(lock(&self.inner.sessions)?
            .values()
            .cloned()
            .map(Into::into)
            .collect())
    }

    fn room_get(&self, id: &str) -> Result<Option<RoomRecord>, StoreError> {
        Ok(lock(&self.inner.rooms)?.get(id).cloned())
    }

    fn room_put(&self, rec: &RoomRecord) -> Result<(), StoreError> {
        lock(&self.inner.rooms)?.insert(rec.info.id.clone(), rec.clone());
        Ok(())
    }

    fn room_del(&self, id: &str) -> Result<(), StoreError> {
        lock(&self.inner.rooms)?.remove(id);
        Ok(())
    }

    fn room_all(&self) -> Result<Vec<RoomRecord>, StoreError> {
        Ok(lock(&self.inner.rooms)?.values().cloned().collect())
    }

    fn match_get(&self, room: &str) -> Result<Option<String>, StoreError> {
        Ok(lock(&self.inner.matches)?.get(room).cloned())
    }

    fn match_put(&self, room: &str, blob: &str) -> Result<(), StoreError> {
        lock(&self.inner.matches)?.insert(room.to_string(), blob.to_string());
        Ok(())
    }

    fn match_del(&self, room: &str) -> Result<(), StoreError> {
        lock(&self.inner.matches)?.remove(room);
        Ok(())
    }

    fn record_log_append(&self, room: &str, line: &str) -> Result<(), StoreError> {
        lock(&self.inner.logs)?
            .entry(room.to_string())
            .or_default()
            .push(line.to_string());
        Ok(())
    }

    fn record_log_get(&self, room: &str) -> Result<Vec<String>, StoreError> {
        Ok(lock(&self.inner.logs)?.get(room).cloned().unwrap_or_default())
    }

    fn record_log_del(&self, room: &str) -> Result<(), StoreError> {
        lock(&self.inner.logs)?.remove(room);
        Ok(())
    }

    fn record_put(&self, room: &str, rec: &StoredRecord) -> Result<(), StoreError> {
        let now = self.tick();
        let mut map = lock(&self.inner.records)?;
        map.insert(room.to_string(), (now, rec.clone()));
        // LRU past the cap. `RECORD_TTL_SECS` is what the Redis store uses;
        // here the bound is the map size instead.
        let _ = RECORD_TTL_SECS;
        while map.len() > RECORD_KEEP {
            let Some(oldest) = map
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            map.remove(&oldest);
        }
        Ok(())
    }

    fn record_get(&self, room: &str) -> Result<Option<StoredRecord>, StoreError> {
        let now = self.tick();
        let mut map = lock(&self.inner.records)?;
        if let Some((used, rec)) = map.get_mut(room) {
            *used = now;
            return Ok(Some(rec.clone()));
        }
        Ok(None)
    }

    fn lock_acquire(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError> {
        let now = self.now();
        let mut map = lock(&self.inner.locks)?;
        // An expired lock is free -- whoever gets here first takes it.
        if let Some(t) = map.get(key) {
            if t.live(now) {
                return Ok(false);
            }
        }
        map.insert(
            key.to_string(),
            Ttl {
                value: owner.to_string(),
                expires: now + Duration::from_millis(ttl_ms),
            },
        );
        Ok(true)
    }

    fn lock_renew(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError> {
        let now = self.now();
        let mut map = lock(&self.inner.locks)?;
        match map.get_mut(key) {
            Some(t) if t.live(now) && t.value == owner => {
                t.expires = now + Duration::from_millis(ttl_ms);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn lock_release(&self, key: &str, owner: &str) -> Result<(), StoreError> {
        let now = self.now();
        let mut map = lock(&self.inner.locks)?;
        if let Some(t) = map.get(key) {
            // Compare-and-delete: a stale release must not free someone
            // else's lock.
            if t.live(now) && t.value == owner {
                map.remove(key);
            } else if !t.live(now) {
                map.remove(key);
            }
        }
        Ok(())
    }

    fn publish(&self, channel: &str, msg: &str) -> Result<(), StoreError> {
        let mut subs = lock(&self.inner.subs)?;
        if let Some(list) = subs.get_mut(channel) {
            list.retain(|tx| {
                tx.send(PubMsg {
                    channel: channel.to_string(),
                    payload: msg.to_string(),
                })
                .is_ok()
            });
        }
        Ok(())
    }

    fn subscribe(&self, channel: &str) -> Result<mpsc::Receiver<PubMsg>, StoreError> {
        let (tx, rx) = mpsc::channel();
        lock(&self.inner.subs)?
            .entry(channel.to_string())
            .or_default()
            .push(tx);
        Ok(rx)
    }

    fn presence_set(
        &self,
        room: &str,
        member: i32,
        stream: &str,
        ttl_ms: u64,
    ) -> Result<(), StoreError> {
        let key = Self::pres_key(room, member, stream);
        lock(&self.inner.presence)?.insert(
            key,
            Ttl {
                value: String::new(),
                expires: self.now() + Duration::from_millis(ttl_ms),
            },
        );
        Ok(())
    }

    fn presence_del(&self, room: &str, member: i32, stream: &str) -> Result<(), StoreError> {
        lock(&self.inner.presence)?.remove(&Self::pres_key(room, member, stream));
        Ok(())
    }

    fn presence_clear(&self, room: &str, member: i32) -> Result<(), StoreError> {
        let prefix = format!("{room}|{member}|");
        lock(&self.inner.presence)?.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }

    fn presence_count(&self, room: &str, member: i32) -> Result<u32, StoreError> {
        let now = self.now();
        let prefix = format!("{room}|{member}|");
        Ok(lock(&self.inner.presence)?
            .iter()
            .filter(|(k, t)| k.starts_with(&prefix) && t.live(now))
            .count() as u32)
    }

    fn rec_meta_get(&self, room: &str) -> Result<Option<RecMeta>, StoreError> {
        Ok(lock(&self.inner.rec_meta)?.get(room).cloned())
    }

    fn rec_meta_put(&self, room: &str, meta: &RecMeta) -> Result<(), StoreError> {
        lock(&self.inner.rec_meta)?.insert(room.to_string(), meta.clone());
        Ok(())
    }

    fn rec_meta_del(&self, room: &str) -> Result<(), StoreError> {
        lock(&self.inner.rec_meta)?.remove(room);
        Ok(())
    }
}