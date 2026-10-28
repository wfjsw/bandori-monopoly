//! Server-wide state: the room table.
//!
//! Sessions, room records and match blobs live in the [`store`] -- two server
//! processes sharing one store serve the same rooms, and a restart keeps
//! them. What remains in memory is the room *handle* and its local broadcast
//! channel: a read-through cache the process needs anyway for its SSE pumps,
//! not a source of truth. Every mutation re-reads the record under the
//! cross-process room lock ([`Server::with_room`]); [`Server::room`] loads a
//! room on demand the first time this process touches it.
//!
//! Locking: the cross-process `room:{id}` lock serialises room mutations
//! across processes; the per-room mutex serialises threads inside one. Code
//! never holds either across a store or worker call unless the call *is* the
//! critical section (a match operation takes its own `match:{room}` lock).
//! Critical sections are short and never await.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::CardRules;

use crate::error::{ApiError, ApiResult};
use crate::pool::Pool;
use crate::room::Room;
use crate::store::{self, CrossState};

/// Seconds a seated player may be without an open stream before the AI takes over
/// (`NetProtocol.Timeout`).
pub const PRESENCE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub player: String,
    pub persona: String,
    pub cn_id: String,
    /// Room id and member id while in a room.
    pub room: Option<(String, i32)>,
}

/// One selectable **rule build**: a ruleset directory, its game tables, and
/// the worker pool that runs matches against them. A room binds one for its
/// life, so two builds never share engine identity (the fairness commitment
/// pins the build's ruleset hash).
pub struct Build {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Directory served at `/builds/{id}/data` (falls back to the server's).
    pub data_path: PathBuf,
    /// Directory served at `/builds/{id}/rules`.
    pub rules_path: PathBuf,
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    /// Match workers, started on this build's first room (a manifest may list
    /// more builds than a deployment ever plays).
    engine: std::sync::OnceLock<Arc<Pool>>,
    /// Worker count used if the pool has to start.
    workers: usize,
}

impl Build {
    pub fn new(
        id: String,
        name: String,
        description: String,
        data_path: PathBuf,
        rules_path: PathBuf,
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        workers: usize,
    ) -> Self {
        Self {
            id,
            name,
            description,
            data_path,
            rules_path,
            data,
            rules,
            engine: std::sync::OnceLock::new(),
            workers,
        }
    }

    /// This build's pool, started on first use.
    pub fn engine(&self) -> Arc<Pool> {
        self.engine
            .get_or_init(|| {
                Pool::start(
                    self.workers,
                    Pool::default_exe(),
                    self.data_path.clone(),
                    self.rules_path.clone(),
                )
            })
            .clone()
    }

    /// A build whose pool was already started (the default).
    pub fn with_engine(mut self, engine: Arc<Pool>) -> Self {
        let _ = self.engine.set(engine);
        self
    }
}

pub struct Server {
    /// The default build's tables/rules/pool. Kept as fields because the
    /// default is what a pre-build record resolves to; rooms use their own.
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    /// Match execution. Workers hold no match state, so this is shared freely.
    pub engine: Arc<Pool>,
    /// Cross-endpoint state (sessions, rooms, match blobs, locks, presence).
    /// Swappable: the in-memory [`crate::store::dummy`] store for one process
    /// (shareable between [`Server`]s), Redis to share or retain it across
    /// processes. See `store` for what is deliberately *not* behind it.
    pub store: Arc<dyn CrossState>,
    /// Local cache of room handles (the broadcast channel lives here). The
    /// store's room records are the source of truth; see [`Server::room`].
    pub rooms: Mutex<HashMap<String, Arc<Mutex<Room>>>>,
    /// This process's identity: lock owner and presence stream prefix. Two
    /// processes never share one.
    pub instance: String,
    /// Overridable for tests.
    pub presence_timeout: Duration,
    /// Game clock speed (1.0 in production; tests run faster).
    pub time_scale: f32,
    /// The advanced-bot decision service (`docs/BOT.md` B5). `None` = no
    /// `--bot-service`: advanced bots then play as standard and the match
    /// never asks this process for anything. All processes of one deployment
    /// should agree -- the rewrite at match start is per-process.
    pub bots: Option<Arc<crate::botsvc::BotService>>,
    /// Seats with a decision request in flight, and when to next probe each
    /// seat (idle throttle). Keyed `(room, member)`. Claims live in the store
    /// so two processes never probe the same seat; this map is the local
    /// half (ponder slot, probe pacing).
    pub bot_drive: Mutex<crate::botsvc::Drive>,
    /// Test knobs: force the search budget / outer deadline / ponder budget
    /// instead of the production defaults. `None` = compute from the prompt
    /// clock ([`crate::botsvc::budget_ms`]),
    /// [`crate::botsvc::ask_timeout`], and [`crate::botsvc::PONDER_BUDGET_MS`].
    pub bot_budget_ms: Option<u64>,
    pub bot_ask_timeout: Option<Duration>,
    pub bot_ponder_budget_ms: Option<u64>,
    /// Interval between empty-comment keep-alives on SSE streams
    /// (`--sse-keepalive` / `BM_SSE_KEEPALIVE`, seconds).
    pub sse_keepalive: Duration,
    /// Selectable rule builds, by id. A room names one; the default build is
    /// also `data`/`rules`/`engine` above.
    pub builds: HashMap<String, Arc<Build>>,
    pub default_build: String,
}

impl Server {
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        engine: Arc<Pool>,
        store: Arc<dyn CrossState>,
    ) -> Arc<Self> {
        // Prime the engine-stamp cache: the commit-reveal slot binds it and
        // rolls under a room lock, where a worker round-trip must not happen
        // (`docs/FAIRNESS.md` §1.1).
        if let Err(e) = engine.info() {
            eprintln!("engine stamp unavailable at startup: {e}");
        }
        let default = Arc::new(
            Build::new(
                "default".to_string(),
                "默认规则".to_string(),
                String::new(),
                PathBuf::new(),
                PathBuf::new(),
                data.clone(),
                rules.clone(),
                0,
            )
            .with_engine(engine.clone()),
        );
        Arc::new(Self {
            data,
            rules,
            engine,
            builds: HashMap::from([("default".to_string(), default)]),
            default_build: "default".to_string(),
            store,
            rooms: Mutex::new(HashMap::new()),
            instance: random_hex(8),
            presence_timeout: PRESENCE_TIMEOUT,
            time_scale: 1.0,
            bots: None,
            bot_drive: Mutex::new(crate::botsvc::Drive::default()),
            bot_budget_ms: None,
            bot_ask_timeout: None,
            bot_ponder_budget_ms: None,
            sse_keepalive: Duration::from_secs(30),
        })
    }

    /// A build by id; `""` means the default. `None` = no such build.
    pub fn build(&self, id: &str) -> Option<Arc<Build>> {
        if id.is_empty() {
            return self
                .builds
                .get(&self.default_build)
                .cloned()
                .or_else(|| self.builds.values().next().cloned());
        }
        self.builds.get(id).cloned()
    }

    /// The build a room resolves to: its own, or the default when the record
    /// predates builds (or names a build this process does not have).
    pub fn build_or_default(&self, id: &str) -> Option<Arc<Build>> {
        self.build(id).or_else(|| self.build(&self.default_build))
    }

    /// A signed-in client, by opaque local token. The game server mints its
    /// own tokens; the profile server's token is never forwarded here -- the
    /// player name is the only thing the two servers share.
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

    /// Warm the room cache from the store. Not the source of truth --
    /// [`Server::room`] loads whatever this missed -- but a restart that calls
    /// this once brings every live room back before the first request.
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
            let build = self
                .build_or_default(&rec.info.build)
                .expect("default build");
            let mut r = Room::from_record(
                rec,
                &build.id,
                build.data.clone(),
                build.engine(),
                self.store.clone(),
            );
            r.bot_search = self.bots.is_some();
            r.presence_timeout = self.presence_timeout;
            r.restore_game();
            eprintln!(
                "restored room {id} (match: {})",
                if r.game.is_some() { "live" } else { "none" }
            );
            rooms.insert(id, Arc::new(Mutex::new(r)));
        }
    }

    /// A room handle: the cached one if this process already has it, otherwise
    /// built on demand from the store's record. `None` = no such room (never
    /// existed, or dissolved).
    pub fn room(&self, id: &str) -> Option<Arc<Mutex<Room>>> {
        if let Some(r) = self.rooms.lock().unwrap().get(id) {
            return Some(r.clone());
        }
        let rec = self.store.room_get(id).ok().flatten()?;
        let build = self.build_or_default(&rec.info.build)?;
        let mut r = Room::from_record(
            rec,
            &build.id,
            build.data.clone(),
            build.engine(),
            self.store.clone(),
        );
        r.bot_search = self.bots.is_some();
        r.presence_timeout = self.presence_timeout;
        r.restore_game();
        let r = Arc::new(Mutex::new(r));
        self.rooms.lock().unwrap().insert(id.to_string(), r.clone());
        Some(r)
    }

    /// Drop a room handle from the local cache (it dissolved).
    pub fn forget_room(&self, id: &str) {
        self.rooms.lock().unwrap().remove(id);
    }

    /// Run `f` on a room under the **cross-process room lock**, with the
    /// record re-read from the store first -- so two servers cannot both
    /// consume a `next_member` / fair slot or clobber each other's roster.
    ///
    /// The local mutex is taken inside the cross-process lock (one order,
    /// everywhere). `f`'s error is passed through; a lock failure is an error
    /// and never runs `f` unlocked.
    pub fn with_room<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Room) -> ApiResult<T>,
    ) -> ApiResult<T> {
        let room = self
            .room(id)
            .ok_or_else(|| ApiError::not_found("err.room.gone"))?;
        let key = format!("room:{id}");
        let owner = format!("{}:{}", self.instance, random_hex(4));
        let out = store::try_with_lock(
            self.store.as_ref(),
            &key,
            &owner,
            store::LOCK_TTL_MS,
            store::LOCK_DEADLINE,
            || {
                let mut r = room.lock().unwrap();
                if let Err(e) = r.reload() {
                    return Err(ApiError::not_found(format!("err.room.gone: {e}").as_str()));
                }
                r.presence_timeout = self.presence_timeout;
                f(&mut r)
            },
        )
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?;
        out
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

/// Short room code, unambiguous personas only.
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
