//! Server-wide state: the room table.
//!
//! Sessions and match blobs are **not** held here -- they live in the [`store`],
//! so a non-Dummy store means no durable state in this process at all. What
//! stays in memory is the room *handle*: its lock, its broadcast channel, and
//! the per-connection presence counts. Locking: each room has its own mutex and
//! code never holds one lock across a store or worker call. Critical sections
//! are short and never await.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::CardRules;

use crate::pool::Pool;
use crate::room::Room;
use crate::store::CrossState;

/// Seconds a seated player may be without an open stream before the AI takes over
/// (`NetProtocol.Timeout`).
pub const PRESENCE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub player: String,
    pub character: String,
    pub cn_id: String,
    /// Room id and member id while in a room.
    pub room: Option<(String, i32)>,
}

pub struct Server {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    /// Match execution. Workers hold no match state, so this is shared freely.
    pub engine: Arc<Pool>,
    /// Cross-endpoint state (sessions, rooms, match blobs). Swappable: the
    /// in-memory [`crate::store::dummy`] store for one process, Redis to share
    /// or retain it. See `store` for what is deliberately *not* behind it.
    pub store: Arc<dyn CrossState>,
    pub rooms: Mutex<HashMap<String, Arc<Mutex<Room>>>>,
    /// Overridable for tests.
    pub presence_timeout: Duration,
    /// Game clock speed (1.0 in production; tests run faster).
    pub time_scale: f32,
    /// The advanced-bot decision service (`docs/BOT.md` B5). `None` = no
    /// `--bot-service`: advanced bots then play as standard and the match
    /// never asks this process for anything.
    pub bots: Option<Arc<crate::botsvc::BotService>>,
    /// Seats with a decision request in flight, and when to next probe each
    /// seat (idle throttle). Keyed `(room, member)`.
    pub bot_drive: Mutex<crate::botsvc::Drive>,
    /// Test knobs: force the search budget / outer deadline / ponder budget
    /// instead of the production defaults. `None` = compute from the prompt
    /// clock ([`crate::botsvc::budget_ms`]),
    /// [`crate::botsvc::ask_timeout`], and [`crate::botsvc::PONDER_BUDGET_MS`].
    pub bot_budget_ms: Option<u64>,
    pub bot_ask_timeout: Option<Duration>,
    pub bot_ponder_budget_ms: Option<u64>,
}

impl Server {
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        engine: Arc<Pool>,
        store: Arc<dyn CrossState>,
    ) -> Arc<Self> {
        Arc::new(Self {
            data,
            rules,
            engine,
            store,
            rooms: Mutex::new(HashMap::new()),
            presence_timeout: PRESENCE_TIMEOUT,
            time_scale: 1.0,
            bots: None,
            bot_drive: Mutex::new(crate::botsvc::Drive::default()),
            bot_budget_ms: None,
            bot_ask_timeout: None,
            bot_ponder_budget_ms: None,
        })
    }

    /// A signed-in client. Goes to the store, so it survives across server
    /// processes when the store does.
    pub fn session(&self, token: &str) -> Option<Session> {
        self.store.session_get(token).ok().flatten()
    }

    /// Record (or clear) a session's room. The store is the only copy.
    pub fn set_room(&self, token: &str, room: Option<(String, i32)>) {
        let Some(mut s) = self.session(token) else {
            return;
        };
        s.room = room;
        let _ = self.store.session_put(&s);
    }

    /// Drop a session entirely (logout / sweep).
    pub fn drop_session(&self, token: &str) {
        let _ = self.store.session_del(token);
    }

    /// Rebuild every room the store knows about. This is what makes a restart
    /// silent: the roster comes back from [`CrossState`], each room's match
    /// blob is already there, and the clients' SSE reconnects into the same
    /// game. Call once at startup, before the ticker starts.
    pub fn restore_rooms(&self) {
        let recs = match self.store.room_all() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("room restore skipped: {e}");
                return;
            }
        };
        if recs.is_empty() {
            return;
        }
        let mut rooms = self.rooms.lock().unwrap();
        for rec in recs {
            let id = rec.info.id.clone();
            let mut r = Room::from_record(rec, self.engine.clone(), self.store.clone());
            r.bot_search = self.bots.is_some();
            r.restore_game();
            eprintln!(
                "restored room {id} (match: {})",
                if r.game.is_some() { "live" } else { "none" }
            );
            rooms.insert(id, Arc::new(Mutex::new(r)));
        }
    }

    pub fn room(&self, id: &str) -> Option<Arc<Mutex<Room>>> {
        self.rooms.lock().unwrap().get(id).cloned()
    }
}

/// 32 random bytes -- the server's secret commit-reveal seed / salt
/// (`docs/FAIRNESS.md`).
pub fn random_32() -> [u8; 32] {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("OS randomness");
    buf
}

/// `n` random bytes as lower-case hex.
pub fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).expect("OS randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn random_u64() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("OS randomness");
    u64::from_le_bytes(buf)
}

/// Short room code, unambiguous characters only.
pub fn room_code() -> String {
    const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
    let mut buf = [0u8; 6];
    getrandom::fill(&mut buf).expect("OS randomness");
    buf.iter()
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

/// `yyyy-MM-dd HH:mm` UTC, for a record's `created` (display only). Formatted
/// by hand -- the server has no calendar dependency and this is the one place
/// that needs one.
pub fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let tod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        tod / 3600,
        (tod % 3600) / 60
    )
}

/// `yyyymmdd-hhmm` UTC, for the download filename.
pub fn now_file_stamp() -> String {
    let s = now_stamp();
    format!("{}-{}", s[..10].replace('-', ""), s[11..].replace(':', ""))
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 -> `(y, m, d)`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
