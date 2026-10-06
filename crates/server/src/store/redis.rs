//! Redis-backed [`CrossState`]: shared and durable.
//!
//! Several server processes can serve the same clients, and a restart keeps
//! them. Keys are namespaced under `bm:` -- `bm:s:<token>`, `bm:r:<id>`,
//! `bm:m:<room>` -- so one Redis can back several deployments as long as the
//! prefix differs.
//!
//! One connection behind a mutex: correct and simple, and the right shape for
//! a sync trait. A pool (or `ConnectionManager`) is the production upgrade if
//! this ever shows up on a profile; the interface does not change either way.

use std::sync::Mutex;

use redis::Commands;

use super::{CrossState, RoomRecord, SessionRecord, StoreError};
use crate::state::Session;

const NS_SESSION: &str = "bm:s:";
const NS_ROOM: &str = "bm:r:";
const NS_MATCH: &str = "bm:m:";

pub struct Store {
    conn: Mutex<redis::Connection>,
}

impl Store {
    /// Connect to `url` (e.g. `redis://127.0.0.1:6379`).
    pub fn connect(url: &str) -> Result<Self, StoreError> {
        let client = redis::Client::open(url).map_err(|e| StoreError::Backend(e.to_string()))?;
        let conn = client
            .get_connection()
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn call<T>(
        &self,
        f: impl FnOnce(&mut redis::Connection) -> redis::RedisResult<T>,
    ) -> Result<T, StoreError> {
        let mut c = self
            .conn
            .lock()
            .map_err(|_| StoreError::Backend("redis lock poisoned".into()))?;
        f(&mut c).map_err(|e| StoreError::Backend(e.to_string()))
    }

    /// Every key in one namespace, via `SCAN` (not `KEYS` -- that blocks the
    /// server for the whole keyspace).
    fn keys(&self, ns: &str) -> Result<Vec<String>, StoreError> {
        let pattern = format!("{ns}*");
        // `scan_match` borrows the connection, so collect inside the closure --
        // the guard is still held there and drops at the end of it.
        self.call(|c| {
            let mut out = Vec::new();
            for k in c.scan_match::<_, String>(&pattern)? {
                out.push(k?);
            }
            Ok(out)
        })
    }
}

fn json<T: serde::Serialize>(v: &T) -> Result<String, StoreError> {
    serde_json::to_string(v).map_err(|e| StoreError::Corrupt(e.to_string()))
}

fn from_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    serde_json::from_str(s).map_err(|e| StoreError::Corrupt(e.to_string()))
}

impl CrossState for Store {
    fn session_get(&self, token: &str) -> Result<Option<Session>, StoreError> {
        let v: Option<String> = self.call(|c| c.get(format!("{NS_SESSION}{token}")))?;
        match v {
            None => Ok(None),
            Some(s) => Ok(Some(from_json::<SessionRecord>(&s)?.into())),
        }
    }

    fn session_put(&self, s: &Session) -> Result<(), StoreError> {
        let v = json(&SessionRecord::from(s))?;
        self.call(|c| c.set::<_, _, ()>(format!("{}{}", NS_SESSION, s.token), v))
            .map(|_| ())
    }

    fn session_del(&self, token: &str) -> Result<(), StoreError> {
        self.call(|c| c.del::<_, ()>(format!("{NS_SESSION}{token}")))
            .map(|_| ())
    }

    fn session_all(&self) -> Result<Vec<Session>, StoreError> {
        let mut out = Vec::new();
        for k in self.keys(NS_SESSION)? {
            let v: Option<String> = self.call(|c| c.get(&k))?;
            if let Some(v) = v {
                out.push(from_json::<SessionRecord>(&v)?.into());
            }
        }
        Ok(out)
    }

    fn room_get(&self, id: &str) -> Result<Option<RoomRecord>, StoreError> {
        let v: Option<String> = self.call(|c| c.get(format!("{NS_ROOM}{id}")))?;
        match v {
            None => Ok(None),
            Some(s) => Ok(Some(from_json(&s)?)),
        }
    }

    fn room_put(&self, rec: &RoomRecord) -> Result<(), StoreError> {
        let v = json(rec)?;
        self.call(|c| c.set::<_, _, ()>(format!("{}{}", NS_ROOM, rec.info.id), v))
            .map(|_| ())
    }

    fn room_del(&self, id: &str) -> Result<(), StoreError> {
        self.call(|c| c.del::<_, ()>(format!("{NS_ROOM}{id}")))
            .map(|_| ())
    }

    fn room_all(&self) -> Result<Vec<RoomRecord>, StoreError> {
        let mut out = Vec::new();
        for k in self.keys(NS_ROOM)? {
            let v: Option<String> = self.call(|c| c.get(&k))?;
            if let Some(v) = v {
                out.push(from_json::<RoomRecord>(&v)?);
            }
        }
        Ok(out)
    }

    fn match_get(&self, room: &str) -> Result<Option<String>, StoreError> {
        self.call(|c| c.get(format!("{NS_MATCH}{room}")))
    }

    fn match_put(&self, room: &str, blob: &str) -> Result<(), StoreError> {
        self.call(|c| c.set::<_, _, ()>(format!("{NS_MATCH}{room}"), blob))
            .map(|_| ())
    }

    fn match_del(&self, room: &str) -> Result<(), StoreError> {
        self.call(|c| c.del::<_, ()>(format!("{NS_MATCH}{room}")))
            .map(|_| ())
    }
}
