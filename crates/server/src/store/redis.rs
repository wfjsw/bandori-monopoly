//! Redis-backed [`CrossState`]: shared and durable.
//!
//! Several server processes can serve the same clients, and a restart keeps
//! them. Keys are namespaced under `bm:` -- `bm:s:<token>`, `bm:r:<id>`,
//! `bm:m:<room>` -- so one Redis can back several deployments as long as the
//! prefix differs.
//!
//! One connection behind a mutex for the ordinary calls: correct and simple,
//! and the right shape for a sync trait. A pool (or `ConnectionManager`) is
//! the production upgrade if this ever shows up on a profile; the interface
//! does not change either way. Pub/sub needs its own connection (a subscribed
//! connection is useless for commands), so [`Store::subscribe`] opens one and
//! runs a small thread over it that reconnects on failure.
//!
//! Locks are `SET NX PX` plus two compare-and-act Lua scripts (renew /
//! release), so a stale holder can never free or extend somebody else's lock.
//! Presence keys are ordinary TTL keys; a stream a dead process stopped
//! refreshing vanishes on its own.

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use redis::Commands;

use super::{
    CrossState, PubMsg, RecMeta, RoomRecord, SessionRecord, StoreError, StoredRecord,
    RECORD_TTL_SECS,
};
use crate::state::Session;

const NS_SESSION: &str = "bm:s:";
const NS_ROOM: &str = "bm:r:";
const NS_MATCH: &str = "bm:m:";
/// `record:{room}:last` -- the last finished match's record.
const NS_RECORD: &str = "bm:rec:";
/// The match's input log, one JSON line per entry.
const NS_RECLOG: &str = "bm:reclog:";
/// The record-log bookkeeping ([`RecMeta`]), under the match lock.
const NS_RECMETA: &str = "bm:recmeta:";
/// Cross-process locks (`room:{id}`, `match:{room}`, `tick:{room}`, ...).
const NS_LOCK: &str = "bm:lock:";
/// Per-stream presence keys: `bm:pres:{room}:{member}:{stream}`.
const NS_PRES: &str = "bm:pres:";

pub struct Store {
    client: redis::Client,
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
            client,
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

/// `GET key` == `owner`? Used as the guard of the two compare-and-act scripts.
const SCRIPT_RENEW: &str = "if redis.call('get', KEYS[1]) == ARGV[1] then \
     return redis.call('pexpire', KEYS[1], ARGV[2]) else return 0 end";
const SCRIPT_RELEASE: &str = "if redis.call('get', KEYS[1]) == ARGV[1] then \
     return redis.call('del', KEYS[1]) else return 0 end";

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

    fn record_log_append(&self, room: &str, line: &str) -> Result<(), StoreError> {
        let key = format!("{NS_RECLOG}{room}");
        self.call(|c| {
            c.rpush::<_, _, ()>(&key, line)?;
            // A match that is never sealed (the box died mid-play) must not
            // leak a list forever.
            c.expire::<_, ()>(&key, RECORD_TTL_SECS as i64)
        })
        .map(|_| ())
    }

    fn record_log_get(&self, room: &str) -> Result<Vec<String>, StoreError> {
        self.call(|c| c.lrange(format!("{NS_RECLOG}{room}"), 0, -1))
    }

    fn record_log_del(&self, room: &str) -> Result<(), StoreError> {
        self.call(|c| c.del::<_, ()>(format!("{NS_RECLOG}{room}")))
            .map(|_| ())
    }

    fn record_put(&self, room: &str, rec: &StoredRecord) -> Result<(), StoreError> {
        let key = format!("{NS_RECORD}{room}");
        let mkey = format!("{NS_RECORD}{room}:members");
        let members = json(&rec.members)?;
        // `SET key value EX <ttl>` -- spelled out so the TTL is visible at the
        // call site rather than buried in a helper. The record itself is the
        // sealed `.bdrec` bytes (a zstd frame), so the value is written as
        // bytes: a Redis bulk string is binary-safe, and JSON-encoding it
        // would be both wasteful and a UTF-8 hazard. The member list rides a
        // companion key under the same TTL.
        self.call(|c| {
            redis::cmd("SET")
                .arg(&key)
                .arg(&rec.record)
                .arg("EX")
                .arg(RECORD_TTL_SECS)
                .query::<()>(c)?;
            redis::cmd("SET")
                .arg(&mkey)
                .arg(&members)
                .arg("EX")
                .arg(RECORD_TTL_SECS)
                .query::<()>(c)
        })
        .map(|_| ())
    }

    fn record_get(&self, room: &str) -> Result<Option<StoredRecord>, StoreError> {
        let key = format!("{NS_RECORD}{room}");
        let mkey = format!("{NS_RECORD}{room}:members");
        let raw: Option<Vec<u8>> = self.call(|c| c.get(&key))?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        if let Some(members) = self.call(|c| c.get::<_, Option<String>>(mkey))? {
            let members = from_json(&members)?;
            return Ok(Some(StoredRecord {
                record: raw,
                members,
            }));
        }
        // A value written before compression: one JSON envelope with the
        // record as a string. Read it through so that record still downloads
        // (as plain JSON -- the endpoint sniffs and names the type).
        #[derive(serde::Deserialize)]
        struct Legacy {
            record: String,
            members: Vec<i32>,
        }
        if let Ok(env) = serde_json::from_slice::<Legacy>(&raw) {
            return Ok(Some(StoredRecord {
                record: env.record.into_bytes(),
                members: env.members,
            }));
        }
        // The bytes, with nobody cleared to download them.
        Ok(Some(StoredRecord {
            record: raw,
            members: vec![],
        }))
    }

    fn lock_acquire(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError> {
        // `SET key owner NX PX ttl` -- the whole acquire is one round-trip and
        // atomic on the server.
        let got: Option<String> = self.call(|c| {
            redis::cmd("SET")
                .arg(format!("{NS_LOCK}{key}"))
                .arg(owner)
                .arg("NX")
                .arg("PX")
                .arg(ttl_ms)
                .query(c)
        })?;
        Ok(got.as_deref() == Some("OK"))
    }

    fn lock_renew(&self, key: &str, owner: &str, ttl_ms: u64) -> Result<bool, StoreError> {
        // `EVAL` rather than `Script` so the crate needs no extra feature; the
        // script body is the compare-and-extend.
        let n: i32 = self.call(|c| {
            redis::cmd("EVAL")
                .arg(SCRIPT_RENEW)
                .arg(1)
                .arg(format!("{NS_LOCK}{key}"))
                .arg(owner)
                .arg(ttl_ms)
                .query(c)
        })?;
        Ok(n == 1)
    }

    fn lock_release(&self, key: &str, owner: &str) -> Result<(), StoreError> {
        self.call(|c| {
            redis::cmd("EVAL")
                .arg(SCRIPT_RELEASE)
                .arg(1)
                .arg(format!("{NS_LOCK}{key}"))
                .arg(owner)
                .query::<i32>(c)
        })?;
        Ok(())
    }

    fn marker_set(&self, key: &str, ttl_ms: u64) -> Result<(), StoreError> {
        self.call(|c| {
            redis::cmd("SET")
                .arg(format!("{NS_LOCK}{key}"))
                .arg("1")
                .arg("PX")
                .arg(ttl_ms)
                .query::<()>(c)
        })
        .map(|_| ())
    }

    fn marker_live(&self, key: &str) -> Result<bool, StoreError> {
        self.call(|c| c.exists::<_, bool>(format!("{NS_LOCK}{key}")))
    }

    fn publish(&self, channel: &str, msg: &str) -> Result<(), StoreError> {
        self.call(|c| redis::cmd("PUBLISH").arg(channel).arg(msg).query::<()>(c))
            .map(|_| ())
    }

    fn subscribe(&self, channel: &str) -> Result<mpsc::Receiver<PubMsg>, StoreError> {
        let (tx, rx) = mpsc::channel();
        // Reconnect off the original client (a subscribed connection is no
        // good for commands, so this is a second one).
        let client = self.client.clone();
        let channel = channel.to_string();
        thread::Builder::new()
            .name("store-subscribe".into())
            .spawn(move || {
                let mut backoff = Duration::from_millis(100);
                loop {
                    match client.get_connection() {
                        Ok(mut conn) => {
                            backoff = Duration::from_millis(100);
                            match pump_sub(&mut conn, &channel, &tx) {
                                // Receiver gone: this subscriber is finished.
                                Ok(true) => return,
                                // Connection broken: reconnect below.
                                Ok(false) | Err(_) => {}
                            }
                        }
                        Err(_) => {
                            // Keep the process alive and retry; the consumers
                            // re-poll the store, so this is latency only.
                            thread::sleep(backoff);
                            backoff = (backoff * 2).min(Duration::from_secs(5));
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            })
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(rx)
    }

    fn presence_set(
        &self,
        room: &str,
        member: i32,
        stream: &str,
        ttl_ms: u64,
    ) -> Result<(), StoreError> {
        let key = format!("{NS_PRES}{room}:{member}:{stream}");
        self.call(|c| {
            redis::cmd("SET")
                .arg(&key)
                .arg("1")
                .arg("PX")
                .arg(ttl_ms)
                .query::<()>(c)
        })
        .map(|_| ())
    }

    fn presence_del(&self, room: &str, member: i32, stream: &str) -> Result<(), StoreError> {
        self.call(|c| {
            c.del::<_, ()>(format!("{NS_PRES}{room}:{member}:{stream}"))
        })
        .map(|_| ())
    }

    fn presence_clear(&self, room: &str, member: i32) -> Result<(), StoreError> {
        // Bounded: a member has a handful of streams, not a keyspace walk.
        // Collect first -- `scan_match` borrows the connection.
        let keys = self.keys(&format!("{NS_PRES}{room}:{member}:"))?;
        self.call(|c| {
            for k in keys {
                c.del::<_, ()>(k)?;
            }
            Ok(())
        })
    }

    fn presence_count(&self, room: &str, member: i32) -> Result<u32, StoreError> {
        let keys = self.keys(&format!("{NS_PRES}{room}:{member}:"))?;
        self.call(|c| {
            let mut n = 0u32;
            for k in keys {
                // The key may have expired between SCAN and GET; `exists` is
                // the honest count of what is alive now.
                if c.exists::<_, bool>(&k)? {
                    n += 1;
                }
            }
            Ok(n)
        })
    }

    fn rec_meta_get(&self, room: &str) -> Result<Option<RecMeta>, StoreError> {
        let v: Option<String> = self.call(|c| c.get(format!("{NS_RECMETA}{room}")))?;
        match v {
            None => Ok(None),
            Some(s) => Ok(Some(from_json(&s)?)),
        }
    }

    fn rec_meta_put(&self, room: &str, meta: &RecMeta) -> Result<(), StoreError> {
        let v = json(meta)?;
        self.call(|c| {
            c.set::<_, _, ()>(format!("{NS_RECMETA}{room}"), v)
        })
        .map(|_| ())
    }

    fn rec_meta_del(&self, room: &str) -> Result<(), StoreError> {
        self.call(|c| c.del::<_, ()>(format!("{NS_RECMETA}{room}")))
            .map(|_| ())
    }
}

/// Forward messages from one subscribed connection until it breaks or the
/// receiver goes away. `Ok(true)` = the receiver is gone (the subscriber is
/// finished); `Ok(false)` / `Err` = the connection broke (the caller
/// reconnects).
fn pump_sub(
    conn: &mut redis::Connection,
    channel: &str,
    tx: &mpsc::Sender<PubMsg>,
) -> Result<bool, StoreError> {
    let mut ps = conn.as_pubsub();
    // A bounded read lets the loop notice a torn connection promptly; a
    // timeout is not one, so it just loops again.
    let _ = ps.set_read_timeout(Some(Duration::from_secs(2)));
    ps.subscribe(channel)
        .map_err(|e| StoreError::Backend(e.to_string()))?;
    loop {
        match ps.get_message() {
            Ok(msg) => {
                let payload = msg.get_payload::<String>().unwrap_or_default();
                if tx
                    .send(PubMsg {
                        channel: msg.get_channel_name().to_string(),
                        payload,
                    })
                    .is_err()
                {
                    return Ok(true);
                }
            }
            Err(e) => {
                // Timeouts are normal with a read deadline; anything else is a
                // dead connection and worth a reconnect.
                let text = e.to_string();
                if text.contains("timed out") || text.contains("timeout") {
                    continue;
                }
                return Err(StoreError::Backend(text));
            }
        }
    }
}