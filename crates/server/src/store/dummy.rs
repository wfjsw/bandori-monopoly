//! In-memory [`CrossState`]: three maps, one process, no durability.
//!
//! This is the single-process default. A restart loses every session, room and
//! match -- which is exactly what the pre-trait behaviour was, kept honest
//! rather than implied by an absent abstraction.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{CrossState, RoomRecord, SessionRecord, StoreError};
use crate::state::Session;

/// Three independent maps. One mutex per bucket so a busy room does not stall
/// session lookups (the same reasoning as `state.rs`'s locking note).
pub struct Store {
    sessions: Mutex<HashMap<String, SessionRecord>>,
    rooms: Mutex<HashMap<String, RoomRecord>>,
    matches: Mutex<HashMap<String, String>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            rooms: Mutex::new(HashMap::new()),
            matches: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, StoreError> {
    m.lock().map_err(|_| StoreError::Backend("store lock poisoned".into()))
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
        Ok(lock(&self.sessions)?.values().cloned().map(Into::into).collect())
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
}