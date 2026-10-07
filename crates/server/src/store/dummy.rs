//! In-memory [`CrossState`]: a handful of maps, one process, no durability.
//!
//! This is the single-process default. A restart loses every session, room and
//! match -- which is exactly what the pre-trait behaviour was, kept honest
//! rather than implied by an absent abstraction.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{CrossState, RoomRecord, SessionRecord, StoreError, StoredRecord, RECORD_TTL_SECS};
use crate::state::Session;

/// How many finished records the in-memory store keeps. A real store expires
/// them by [`RECORD_TTL_SECS`] instead; this one has no clock, so it bounds the
/// map and drops the least recently used.
pub const RECORD_KEEP: usize = 64;

/// One mutex per bucket so a busy room does not stall session lookups (the
/// same reasoning as `state.rs`'s locking note).
pub struct Store {
    sessions: Mutex<HashMap<String, SessionRecord>>,
    rooms: Mutex<HashMap<String, RoomRecord>>,
    matches: Mutex<HashMap<String, String>>,
    /// `room -> (last-used stamp, record)`, evicted LRU past [`RECORD_KEEP`].
    records: Mutex<HashMap<String, (u64, StoredRecord)>>,
    /// `room -> record log lines`, for the match in progress.
    logs: Mutex<HashMap<String, Vec<String>>>,
    used: Mutex<u64>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            rooms: Mutex::new(HashMap::new()),
            matches: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
            used: Mutex::new(0),
        }
    }

    /// Monotonic "now" for the LRU order. Not wall time: this store has no
    /// expiry, only recency.
    fn tick(&self) -> u64 {
        let mut n = self.used.lock().unwrap_or_else(|e| e.into_inner());
        *n += 1;
        *n
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
        Ok(lock(&self.sessions)?.get(token).cloned().map(Into::into))
    }

    fn session_put(&self, s: &Session) -> Result<(), StoreError> {
        lock(&self.sessions)?.insert(s.token.clone(), SessionRecord::from(s));
        Ok(())
    }

    fn session_del(&self, token: &str) -> Result<(), StoreError> {
        lock(&self.sessions)?.remove(token);
        Ok(())
    }

    fn session_all(&self) -> Result<Vec<Session>, StoreError> {
        Ok(lock(&self.sessions)?
            .values()
            .cloned()
            .map(Into::into)
            .collect())
    }

    fn room_get(&self, id: &str) -> Result<Option<RoomRecord>, StoreError> {
        Ok(lock(&self.rooms)?.get(id).cloned())
    }

    fn room_put(&self, rec: &RoomRecord) -> Result<(), StoreError> {
        lock(&self.rooms)?.insert(rec.info.id.clone(), rec.clone());
        Ok(())
    }

    fn room_del(&self, id: &str) -> Result<(), StoreError> {
        lock(&self.rooms)?.remove(id);
        Ok(())
    }

    fn room_all(&self) -> Result<Vec<RoomRecord>, StoreError> {
        Ok(lock(&self.rooms)?.values().cloned().collect())
    }

    fn match_get(&self, room: &str) -> Result<Option<String>, StoreError> {
        Ok(lock(&self.matches)?.get(room).cloned())
    }

    fn match_put(&self, room: &str, blob: &str) -> Result<(), StoreError> {
        lock(&self.matches)?.insert(room.to_string(), blob.to_string());
        Ok(())
    }

    fn match_del(&self, room: &str) -> Result<(), StoreError> {
        lock(&self.matches)?.remove(room);
        Ok(())
    }

    fn record_log_append(&self, room: &str, line: &str) -> Result<(), StoreError> {
        lock(&self.logs)?
            .entry(room.to_string())
            .or_default()
            .push(line.to_string());
        Ok(())
    }

    fn record_log_get(&self, room: &str) -> Result<Vec<String>, StoreError> {
        Ok(lock(&self.logs)?.get(room).cloned().unwrap_or_default())
    }

    fn record_log_del(&self, room: &str) -> Result<(), StoreError> {
        lock(&self.logs)?.remove(room);
        Ok(())
    }

    fn record_put(&self, room: &str, rec: &StoredRecord) -> Result<(), StoreError> {
        let now = self.tick();
        let mut map = lock(&self.records)?;
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
        let mut map = lock(&self.records)?;
        if let Some((used, rec)) = map.get_mut(room) {
            *used = now;
            return Ok(Some(rec.clone()));
        }
        Ok(None)
    }
}