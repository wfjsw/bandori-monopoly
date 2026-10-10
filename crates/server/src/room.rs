//! A room: members, readiness, bots, and its match (`RoomHost.cs`).
//!
//! The match itself lives in a `rules-worker` process, not here: `game` is the
//! `Match::save` blob and every operation is a round-trip through the pool. The
//! room therefore holds no engine state -- only the blob the caller must keep.
//!
//! Beside the blob lives the **record log** (`docs/REPLAY.md` §4): every input
//! the match took, with a checkpoint at each turn boundary. It is appended
//! after a successful store put and sealed into a [`StoredRecord`] when the
//! match ends, so a restart mid-match loses at most the buffered tick run (and
//! flags the record `gaps`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use game_core::msg::Msg;
use game_core::net::{self, NetMessage, RoomInfo, RoomMember};
use game_core::fair::{self, CanonSettings, FairPublic, Fairness, NonceEntry};
use game_core::record::{
    body_check, hash_save, Init, Input, MatchSetup, Origin, RecordBody, RecordFile, RecordHeader,
    SeatInfo, MAGIC,
};
use game_core::rng::Seed256;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use serde_json::Value;
use tokio::sync::broadcast;

use std::sync::Mutex;

use crate::error::{ApiError, ApiResult};
use crate::pool::{Out, Pool};
use crate::store::{CrossState, LogEntry, RecordHead, RoomFair, RoomRecord, StoredRecord};

/// A room's commit-reveal slot (`docs/FAIRNESS.md`): the openings of the
/// **next** match this room will deal, drawn at room creation and redrawn
/// after every match -- always published before that match's nonces can
/// arrive, so the server is already committed when a client posts one.
///
/// There is no window and no wait: a nonce is collected as its member enters
/// the room, the ones that do not sit down for the match are dropped at the
/// start, and the match is created immediately.
///
/// In-memory beside the store's [`RoomFair`] copy (a restart picks the same
/// slot back up). The openings also reach the server-side record log head
/// when the match is created and the sealed record when it ends; they never
/// enter a client-facing payload.
#[derive(Debug, Clone)]
struct FairState {
    /// Server secret 256-bit seed. Hex in the sealed record; never in a view.
    seed: [u8; 32],
    /// Server secret 256-bit salt. Same.
    salt: [u8; 32],
    /// The commitment shown in the room (hex).
    commit: String,
    /// Human nonces collected for the next match (member -> 32 bytes). Last
    /// write wins; locked when the match starts.
    nonces: std::collections::BTreeMap<i32, [u8; 32]>,
    /// A match has consumed this slot. The commit still shows for the running
    /// match (people copy it from there); no nonce is accepted, and the next
    /// match waits for a fresh slot rolled when this one ends.
    consumed: bool,
}

impl FairState {
    /// Draw a fresh slot over the engine identity `stamp` names.
    fn fresh(stamp: &game_core::record::EngineStamp) -> Self {
        let seed = crate::state::random_32();
        let salt = crate::state::random_32();
        Self {
            seed,
            salt,
            commit: fair::commit_hex(&seed, &salt, &stamp.bundle, &stamp.ruleset_sha256),
            nonces: std::collections::BTreeMap::new(),
            consumed: false,
        }
    }

    fn to_record(&self) -> RoomFair {
        RoomFair {
            seed: fair::hex32(&self.seed),
            salt: fair::hex32(&self.salt),
            commit: self.commit.clone(),
            nonces: self
                .nonces
                .iter()
                .map(|(&m, n)| (m, fair::hex32(n)))
                .collect(),
            consumed: self.consumed,
        }
    }

    fn from_record(r: &RoomFair) -> Option<Self> {
        let seed = fair::unhex32(&r.seed).ok()?;
        let salt = fair::unhex32(&r.salt).ok()?;
        let mut nonces = std::collections::BTreeMap::new();
        for (m, n) in &r.nonces {
            nonces.insert(*m, fair::unhex32(n).ok()?);
        }
        Some(Self {
            seed,
            salt,
            commit: r.commit.clone(),
            nonces,
            consumed: r.consumed,
        })
    }
}

/// Bookkeeping for one match's record log. Lives beside the blob rather than
/// inside it: the blob is the engine's, this is the driver's.
#[derive(Debug, Default)]
struct RecState {
    /// The open tick run, buffered in memory (one `Input::Ticks`).
    buf: Option<(u8, u32)>,
    /// Inputs the log holds, counting the buffered run as one.
    inputs: u32,
    /// Tick calls recorded -- the sum of every `Ticks.n`.
    ticks: u64,
    /// `at` of the last checkpoint, so a run closes at a turn boundary.
    last_cp_at: u32,
    /// Buffered ticks were lost (a restart mid-match). The record is then
    /// unverifiable: see `RecordHeader::gaps`.
    gaps: bool,
    /// Members seated when the match started -- the record's participants.
    members: Vec<i32>,
    /// Sealed; stop appending.
    done: bool,
}

/// A running match: its blob, plus the order its operations apply in.
///
/// The **room** lock is never held across a worker round-trip -- callers clone
/// this handle out of the room and drop that lock first (see `state.rs`:
/// critical sections are short). `gate` is what keeps two operations on one
/// match from racing on the blob; it is held for exactly one round-trip, which
/// the pool bounds with its deadline.
///
/// **The blob is not held here** -- it lives in the [`CrossState`], so a
/// non-Dummy store means no match state in this process: any worker serves any
/// request, and a restart keeps every live match.
pub struct MatchHandle {
    id: String,
    store: Arc<dyn CrossState>,
    gate: Mutex<()>,
    engine: Arc<Pool>,
    rec: Mutex<RecState>,
}

impl MatchHandle {
    /// Start of a match: `head` becomes the log's first line, and `members` are
    /// the seats that may download the record once it is sealed.
    pub fn start(
        id: String,
        store: Arc<dyn CrossState>,
        engine: Arc<Pool>,
        blob: String,
        head: RecordHead,
        members: Vec<i32>,
    ) -> Result<Self, String> {
        store.match_put(&id, &blob).map_err(|e| e.to_string())?;
        // A stale log from a match that never sealed must not mix with this one.
        let _ = store.record_log_del(&id);
        let _ = store.record_log_append(&id, &LogEntry::Head(head).to_line());
        Ok(Self {
            id,
            store,
            gate: Mutex::new(()),
            engine,
            rec: Mutex::new(RecState {
                members,
                ..RecState::default()
            }),
        })
    }

    /// A match already in progress, picked up after a restart. The in-memory
    /// tick buffer went with the process, so anything still unflushed is gone:
    /// that is what `gaps` marks.
    pub fn restore(
        id: String,
        store: Arc<dyn CrossState>,
        engine: Arc<Pool>,
        blob: String,
    ) -> Result<Self, String> {
        store.match_put(&id, &blob).map_err(|e| e.to_string())?;
        let lines = store.record_log_get(&id).map_err(|e| e.to_string())?;
        let mut st = RecState::default();
        for line in &lines {
            match LogEntry::parse_line(line) {
                Some(LogEntry::Head(h)) => {
                    st.members = match &h.init {
                        Init::Seed(s) => s.members.iter().map(|m| m.id).collect(),
                        Init::Snapshot { .. } => vec![],
                    };
                }
                Some(LogEntry::Input(i)) => {
                    st.inputs += 1;
                    if let Input::Ticks { n, .. } = i {
                        st.ticks += n as u64;
                    }
                }
                Some(LogEntry::Cp(c)) => st.last_cp_at = c.at,
                None => {}
            }
        }
        st.gaps = !lines.is_empty();
        Ok(Self {
            id,
            store,
            gate: Mutex::new(()),
            engine,
            rec: Mutex::new(st),
        })
    }

    /// The blob, straight from the store.
    pub fn snapshot(&self) -> Result<String, String> {
        self.store
            .match_get(&self.id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "no match".to_string())
    }

    /// One **recorded** operation. `f` gets the current blob and returns the
    /// new one; the gate is held for the whole call. Only a successful
    /// round-trip **and** a successful store put reaches the log -- a failure
    /// leaves no trace, because there is no matching engine state to replay
    /// it against.
    fn run<T>(
        &self,
        log: impl FnOnce(&Out<T>) -> Input,
        f: impl FnOnce(&str, &Pool) -> Result<Out<T>, String>,
    ) -> Result<T, String> {
        let _order = self.gate.lock().unwrap();
        let before = self.snapshot()?;
        let out = f(&before, &self.engine)?;
        self.store
            .match_put(&self.id, &out.state)
            .map_err(|e| e.to_string())?;
        self.note(log(&out), &out);
        let ended = out.ended;
        drop(_order);
        if ended {
            self.finalize();
        }
        Ok(out.value)
    }

    /// One read-only operation: nothing is appended to the log.
    fn peek<T>(&self, f: impl FnOnce(&str, &Pool) -> Result<T, String>) -> Result<T, String> {
        let _order = self.gate.lock().unwrap();
        let before = self.snapshot()?;
        f(&before, &self.engine)
    }

    /// `(error message if the command was rejected)`. The blob moves on either
    /// way: a rejected command may still have changed the match -- and it is
    /// recorded either way, with `ok: false`.
    pub fn act(&self, member: i32, cmd: &NetMessage) -> Result<Option<Msg>, String> {
        let for_log = cmd.clone();
        self.run(
            move |out: &Out<Option<Msg>>| Input::Act {
                m: member,
                msg: for_log.clone(),
                ok: out.value.is_none(),
            },
            |s, e| e.act(s, member, cmd),
        )
    }

    /// Advance the clock by `dt` (`k` quanta of `header.step`). Returns whether
    /// the match changed; `k` is what the record's `Ticks` run-length counts.
    pub fn tick(&self, dt: f32, k: u8) -> Result<bool, String> {
        self.run(|_| Input::Ticks { k, n: 1 }, |s, e| e.tick(s, dt))
    }

    pub fn ended(&self) -> Result<bool, String> {
        self.peek(|s, e| e.ended(s))
    }

    pub fn quick_start(&self) -> Result<(), String> {
        self.run(|_| Input::QuickStart, |s, e| e.quick_start(s))
    }

    pub fn finish(&self) -> Result<(), String> {
        self.run(|_| Input::Finish, |s, e| e.finish(s))
    }

    pub fn member_left(&self, member: i32, can_return: bool) -> Result<(), String> {
        self.run(
            |_| Input::Left { m: member, can_return },
            |s, e| e.member_left(s, member, can_return),
        )
    }

    pub fn member_back(&self, member: i32) -> Result<(), String> {
        self.run(|_| Input::Back { m: member }, |s, e| e.member_back(s, member))
    }

    /// One member's view frame. `need_extras` = the caller wants the
    /// per-viewer `view_extra` fields (the bot service does; a human client
    /// computes them itself from the seat view).
    pub fn view(&self, member: i32, need_extras: bool) -> Result<Value, String> {
        self.peek(|s, e| e.view(s, member, need_extras))
    }

    pub fn events(&self, since: i32) -> Result<Vec<Value>, String> {
        self.peek(|s, e| e.events(s, since))
    }

    // ---------------------------------------------------------------- record log

    /// Append `input` (and the checkpoint its op crossed) to the log. Mirrors
    /// `RecordedMatch`: consecutive ticks with the same `k` run-length merge
    /// into one `Ticks` input, and a turn boundary closes the run so the
    /// checkpoint sits between two inputs.
    fn note<T>(&self, input: Input, out: &Out<T>) {
        let mut rec = self.rec.lock().unwrap();
        if rec.done {
            return;
        }
        match &input {
            Input::Ticks { k, .. } => {
                rec.ticks += 1;
                let closed = rec.last_cp_at == rec.inputs;
                match rec.buf {
                    Some((k2, n)) if k2 == *k && !closed => rec.buf = Some((k2, n + 1)),
                    _ => {
                        Self::flush_buf(&self.store, &self.id, &mut rec);
                        rec.buf = Some((*k, 1));
                        rec.inputs += 1;
                    }
                }
                // Bound how much a crash can lose.
                if rec.ticks % 100 == 0 {
                    Self::flush_buf(&self.store, &self.id, &mut rec);
                }
            }
            other => {
                // The tick run comes first, so the log stays ordered.
                Self::flush_buf(&self.store, &self.id, &mut rec);
                let line = LogEntry::Input(other.clone()).to_line();
                if self.store.record_log_append(&self.id, &line).is_err() {
                    rec.gaps = true;
                    return;
                }
                rec.inputs += 1;
            }
        }
        if let Some(cp) = &out.cp {
            Self::flush_buf(&self.store, &self.id, &mut rec);
            let entry = crate::store::Cp {
                at: rec.inputs,
                tick: rec.ticks,
                round: cp.round,
                turn: cp.turn,
                hash: cp.hash.clone(),
            };
            let line = LogEntry::Cp(entry).to_line();
            if self.store.record_log_append(&self.id, &line).is_err() {
                rec.gaps = true;
            } else {
                rec.last_cp_at = rec.inputs;
            }
        }
    }

    fn flush_buf(store: &Arc<dyn CrossState>, id: &str, rec: &mut RecState) {
        if let Some((k, n)) = rec.buf.take() {
            let line = LogEntry::Input(Input::Ticks { k, n }).to_line();
            if store.record_log_append(id, &line).is_err() {
                rec.gaps = true;
            }
        }
    }

    /// Seal the record: seats from the final view, `final_hash` from the blob
    /// the server kept. Idempotent -- the first call wins. The log is cleared
    /// afterwards so the next match in this room starts clean.
    pub fn finalize(&self) {
        let (lines, members, ticks, gaps) = {
            let mut rec = self.rec.lock().unwrap();
            if rec.done {
                return;
            }
            rec.done = true;
            Self::flush_buf(&self.store, &self.id, &mut rec);
            let ticks = rec.ticks;
            let gaps = rec.gaps;
            let members = rec.members.clone();
            let lines = self.store.record_log_get(&self.id).unwrap_or_default();
            (lines, members, ticks, gaps)
        };
        if let Err(e) = self.seal(&lines, &members, ticks, gaps) {
            eprintln!("room {}: record seal failed: {e}", self.id);
        }
        let _ = self.store.record_log_del(&self.id);
    }

    fn seal(
        &self,
        lines: &[String],
        members: &[i32],
        ticks: u64,
        gaps: bool,
    ) -> Result<(), String> {
        let mut head: Option<RecordHead> = None;
        let mut inputs = Vec::new();
        let mut checkpoints = Vec::new();
        for line in lines {
            match LogEntry::parse_line(line) {
                Some(LogEntry::Head(h)) => head = Some(h),
                Some(LogEntry::Input(i)) => inputs.push(i),
                Some(LogEntry::Cp(c)) => checkpoints.push(game_core::record::Checkpoint {
                    at: c.at,
                    tick: c.tick,
                    round: c.round,
                    turn: c.turn,
                    hash: c.hash,
                }),
                None => {}
            }
        }
        let Some(head) = head else {
            return Err("record log has no head".into());
        };
        let blob = self.snapshot()?;
        // Any seated view carries the whole public `MatchState`; the seats'
        // ranks and scores are what the results screen showed.
        let member = members.first().copied().unwrap_or(0);
        let view = self.engine.view(&blob, member, false)?;
        let st: game_core::state::MatchState = serde_json::from_value(
            view.get("state").cloned().unwrap_or(Value::Null),
        )
        .map_err(|e| format!("final view: {e}"))?;
        let seats = st
            .players
            .iter()
            .map(|p| SeatInfo {
                member: p.member,
                player: p.player.clone(),
                bot: p.bot,
                mentality: p.mentality,
                character: p.character.clone(),
                rank: p.rank,
                score: p.score,
            })
            .collect();
        let body = RecordBody {
            init: head.init.clone(),
            inputs,
            checkpoints,
            final_hash: hash_save(&blob),
            events: None,
        };
        let header = RecordHeader {
            engine: head.stamp.clone(),
            mode: MatchMode::from_i32(head.mode).unwrap_or_default(),
            step: head.step,
            origin: Origin::Online {
                room: self.id.clone(),
            },
            created: head.created.clone(),
            seats,
            partial: matches!(head.init, Init::Snapshot { .. }),
            ended: true,
            reason: st.end_reason.clone(),
            rounds: st.round,
            total_ticks: ticks,
            gaps,
            fair: head.fair.clone(),
        };
        let check = body_check(&body);
        let file = RecordFile {
            magic: MAGIC.to_string(),
            header,
            body,
            check,
        };
        // Sealed compressed: `record:{room}:last` holds the zstd-framed
        // `.bdrec` the endpoint serves as-is (`docs/SERVER.md` "Match records").
        let rec = StoredRecord {
            record: game_core::record::encode_record_zst(&file),
            members: members.to_vec(),
        };
        self.store
            .record_put(&self.id, &rec)
            .map_err(|e| e.to_string())
    }
}

/// What stream tasks are told.
#[derive(Debug, Clone)]
pub enum Note {
    Changed,
    Dissolved(Msg),
}

#[derive(Debug, Clone)]
struct Presence {
    streams: u32,
    /// When the last stream closed (or the member joined).
    since: Instant,
    /// The AI has taken over the player.
    away: bool,
}

pub struct Room {
    // DEV PHASE: `info` / `password` / `tokens` / `next_member` are held here
    // rather than in the [`CrossState`]. The match blob and the sessions are
    // already on the store; the roster is the remaining process-local durable
    // data. When a real database lands (see `store`), these move behind the
    // trait as a fourth bucket and `Room` keeps only the genuinely
    // per-connection parts -- `presence` and `tx`.
    pub info: RoomInfo,
    password: String,
    /// Human member id -> session token.
    pub tokens: HashMap<i32, String>,
    next_member: i32,
    /// The running match. The engine is in a worker process; this holds only
    /// the `Match::save` blob and the order its operations apply in.
    pub game: Option<Arc<MatchHandle>>,
    pub engine: Arc<Pool>,
    pub store: Arc<dyn CrossState>,
    pub tx: broadcast::Sender<Note>,
    presence: HashMap<i32, Presence>,
    pub dissolved: Option<Msg>,
    /// Does this server have a `bot-service` to drive Advanced bots
    /// (`docs/BOT.md` B5)? When false, [`Room::start`] rewrites those seats to
    /// Standard before the match is built, so they play as ordinary engine
    /// bots instead of waiting on answers nobody will send.
    pub bot_search: bool,
    /// The commit-reveal slot (`docs/FAIRNESS.md`): drawn at room creation and
    /// redrawn after every match. `None` only until the first [`Room::roll_fair`]
    /// (or when a stored room carries none and the re-roll has not run yet).
    fair: Option<FairState>,
}

pub struct NewMember<'a> {
    pub token: &'a str,
    pub player: &'a str,
    pub character: &'a str,
    pub cn_id: &'a str,
}

impl Room {
    pub fn new(
        id: String,
        name: &str,
        ranked: bool,
        max_players: i32,
        password: &str,
        weights: ScoreWeights,
        engine: Arc<Pool>,
        store: Arc<dyn CrossState>,
    ) -> Self {
        let max = if ranked {
            max_players.clamp(2, 6)
        } else {
            max_players.clamp(2, 10)
        };
        let (tx, _) = broadcast::channel(64);
        Self {
            info: RoomInfo {
                id,
                name: name.trim().chars().take(24).collect(),
                ranked,
                max_players: max,
                locked: !password.is_empty(),
                playing: false,
                theme: String::new(),
                weights,
                members: vec![],
                fair: None,
            },
            password: password.to_string(),
            tokens: HashMap::new(),
            next_member: 1,
            game: None,
            engine,
            store,
            tx,
            presence: HashMap::new(),
            dissolved: None,
            bot_search: false,
            fair: None,
        }
    }

    /// The durable half of this room, for the store.
    pub fn record(&self) -> RoomRecord {
        RoomRecord {
            info: self.info.clone(),
            password: self.password.clone(),
            next_member: self.next_member,
            tokens: self.tokens.iter().map(|(&k, v)| (k, v.clone())).collect(),
            fair: self.fair.as_ref().map(|f| f.to_record()),
        }
    }

    /// Write the durable half back. Called after anything that changes it, so a
    /// restart can rebuild the room (see [`Room::from_record`]).
    fn persist(&self) {
        if let Err(e) = self.store.room_put(&self.record()) {
            eprintln!("room {} persist failed: {e}", self.info.id);
        }
    }

    /// Rebuild a room the store already knows about -- what a restart does for
    /// every [`RoomRecord`]. The match blob is picked up separately by
    /// [`MatchHandle`]; presence starts at zero streams, so everyone is briefly
    /// `away` until their SSE reconnects (well inside the presence timeout).
    ///
    /// The commit-reveal slot rides the record (`docs/FAIRNESS.md`): same
    /// commitment, same already-collected nonces, so a restart is invisible to
    /// the fairness scheme. A record from before the slot was persisted falls
    /// back to a fresh one on [`Room::restore_game`].
    pub fn from_record(rec: RoomRecord, engine: Arc<Pool>, store: Arc<dyn CrossState>) -> Self {
        let tokens: HashMap<i32, String> = rec.tokens.into_iter().collect();
        let presence = tokens
            .keys()
            .map(|&m| {
                (
                    m,
                    Presence {
                        streams: 0,
                        since: Instant::now(),
                        away: false,
                    },
                )
            })
            .collect();
        let (tx, _) = broadcast::channel(64);
        let fair = rec.fair.as_ref().and_then(FairState::from_record);
        let mut info = rec.info;
        if let Some(f) = &fair {
            info.fair = Some(FairPublic {
                commit: f.commit.clone(),
            });
        }
        Self {
            info,
            password: rec.password,
            tokens,
            next_member: rec.next_member,
            game: None,
            engine,
            store,
            tx,
            presence,
            dissolved: None,
            bot_search: false,
            fair,
        }
    }

    /// The running match as the store holds it, for restore. Also brings the
    /// commit-reveal slot up: a room record written before the slot was
    /// persisted (or a corrupt one) gets a fresh commitment here, before
    /// anybody can post a nonce against it.
    pub fn restore_game(&mut self) {
        if self.fair.is_none() {
            if let Err(e) = self.roll_fair() {
                eprintln!("room {}: fair slot roll failed: {e}", self.info.id);
            }
        }
        let blob = match self.store.match_get(&self.info.id) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("room {} match restore failed: {e}", self.info.id);
                return;
            }
        };
        if let Some(blob) = blob {
            match MatchHandle::restore(
                self.info.id.clone(),
                self.store.clone(),
                self.engine.clone(),
                blob,
            ) {
                Ok(m) => self.game = Some(Arc::new(m)),
                Err(e) => eprintln!("room {} match restore failed: {e}", self.info.id),
            }
        }
    }

    pub fn notify(&self) {
        let _ = self.tx.send(Note::Changed);
    }

    pub fn member_of(&self, token: &str) -> Option<i32> {
        self.tokens
            .iter()
            .find(|(_, t)| t.as_str() == token)
            .map(|(&m, _)| m)
    }

    pub fn is_host(&self, member: i32) -> bool {
        self.info.members.iter().any(|m| m.id == member && m.host)
    }

    fn humans(&self) -> usize {
        self.info.members.iter().filter(|m| !m.bot).count()
    }

    /// `hello` handling: version, full, playing, password. Rejoining is always allowed.
    pub fn join(&mut self, who: NewMember, password: &str, version: &str) -> ApiResult<i32> {
        if let Some(m) = self.member_of(who.token) {
            return Ok(m);
        }
        if !version.is_empty() && version != net::VERSION {
            return Err(ApiError::reject(net::reject::VERSION));
        }
        if self.info.playing {
            return Err(ApiError::reject(net::reject::PLAYING));
        }
        if self.info.is_full() {
            return Err(ApiError::reject(net::reject::FULL));
        }
        if self.info.locked && password != self.password {
            return Err(ApiError::reject(net::reject::PASSWORD));
        }
        let id = self.next_member;
        self.next_member += 1;
        let host = self.humans() == 0;
        self.info.members.push(RoomMember {
            id,
            player: net::clean_name(who.player),
            character: who.character.into(),
            cn_id: who.cn_id.into(),
            ready: false,
            host,
            bot: false,
            away: false,
            mentality: Default::default(),
        });
        self.tokens.insert(id, who.token.into());
        self.persist();
        self.presence.insert(
            id,
            Presence {
                streams: 0,
                since: Instant::now(),
                away: false,
            },
        );
        self.notify();
        Ok(id)
    }

    /// Leave the room. Mid-match this forfeits -- the caller runs the leave
    /// command on the handle first (see [`Room::match_handle`]), then calls
    /// this. Returns the token that left.
    pub fn leave(&mut self, member: i32) -> Option<String> {
        let token = self.tokens.remove(&member);
        self.presence.remove(&member);
        // A leaver's nonce goes with them (`docs/FAIRNESS.md`): only the
        // members who actually sit down for the match contribute.
        if let Some(f) = self.fair.as_mut() {
            f.nonces.remove(&member);
        }
        let was_host = self.is_host(member);
        self.info.members.retain(|m| m.id != member);
        if was_host {
            // The original dissolved the room (the host *was* the server). A dedicated
            // server can keep it alive: hosting passes to the next human.
            if let Some(m) = self.info.members.iter_mut().find(|m| !m.bot) {
                m.host = true;
                m.ready = false;
            }
        }
        if self.humans() == 0 {
            self.dissolve(Msg::new("room.dissolved_empty"));
        } else {
            self.persist();
        }
        self.notify();
        token
    }

    pub fn dissolve(&mut self, reason: Msg) {
        if self.dissolved.is_none() {
            self.dissolved = Some(reason.clone());
            // A dissolved room leaves no trace: the room record, its match blob
            // and any in-progress record log go, so a restart does not
            // resurrect it. The **sealed** record (`record:{id}:last`) stays
            // until its TTL -- a participant may still want last match's replay.
            let _ = self.store.room_del(&self.info.id);
            let _ = self.store.match_del(&self.info.id);
            let _ = self.store.record_log_del(&self.info.id);
            let _ = self.tx.send(Note::Dissolved(reason));
        }
    }

    pub fn set_ready(&mut self, member: i32, on: bool) -> ApiResult<()> {
        if self.info.playing {
            return Err(ApiError::bad("err.room.playing"));
        }
        let m = self
            .info
            .members
            .iter_mut()
            .find(|m| m.id == member)
            .ok_or_else(|| ApiError::bad("err.room.not_member"))?;
        m.ready = on;
        self.persist();
        self.notify();
        Ok(())
    }

    /// `names`: the data's bot name list (`match_rules.json`). `mentality` is
    /// the bot's decision policy; it rides the room record and is applied when
    /// the match starts (`Match::new` copies it onto the seat).
    pub fn add_bot(
        &mut self,
        member: i32,
        names: &[String],
        mentality: game_core::state::BotMentality,
    ) -> ApiResult<()> {
        self.host_only(member)?;
        if self.info.playing {
            return Err(ApiError::bad("err.room.bot_add_playing"));
        }
        if self.info.is_full() {
            return Err(ApiError::reject(net::reject::FULL));
        }
        let id = self.next_member;
        self.next_member += 1;
        let player = net::bot_name(names, self.info.members.iter().map(|m| m.player.as_str()));
        self.info.members.push(RoomMember {
            id,
            player,
            ready: true,
            bot: true,
            mentality,
            ..Default::default()
        });
        self.persist();
        self.notify();
        Ok(())
    }

    pub fn remove_bot(&mut self, member: i32, bot: i32) -> ApiResult<()> {
        self.host_only(member)?;
        if self.info.playing {
            return Err(ApiError::bad("err.room.bot_remove_playing"));
        }
        let before = self.info.members.len();
        self.info.members.retain(|m| !(m.id == bot && m.bot));
        if self.info.members.len() == before {
            return Err(ApiError::bad("err.room.no_such_bot"));
        }
        self.persist();
        self.notify();
        Ok(())
    }

    pub fn set_weights(
        &mut self,
        member: i32,
        w: ScoreWeights,
        rules_default: ScoreWeights,
    ) -> ApiResult<()> {
        self.host_only(member)?;
        if self.info.playing {
            return Err(ApiError::bad("err.room.playing"));
        }
        self.info.weights = w.sanitized(rules_default);
        self.persist();
        self.notify();
        Ok(())
    }

    fn host_only(&self, member: i32) -> ApiResult<()> {
        if self.is_host(member) {
            Ok(())
        } else {
            Err(ApiError::forbidden("err.room.host_only"))
        }
    }

    /// Start a match. Without `force`, everyone must be ready and the player count
    /// must fit the mode (Casual 3-10, Ranked 5-6).
    ///
    /// `step` is the tick quantum this match will run on (`0.05 * time_scale`,
    /// see `docs/REPLAY.md` §1): the record seals it into its header so a
    /// replay reproduces the same `tick(dt)` f32s.
    ///
    /// This creates the match, immediately (`docs/FAIRNESS.md`). The
    /// commitment was published at **room creation** -- before any nonce could
    /// exist -- and the player nonces were collected as their members entered
    /// the room. The nonces of exactly the members who sit down are mixed in;
    /// anything else (a leaver, a kicked member) is dropped. A member without
    /// a nonce is simply absent. Nothing waits on anybody.
    pub fn start(&mut self, member: i32, force: bool, step: f32) -> ApiResult<()> {
        self.host_only(member)?;
        if self.info.playing {
            return Err(ApiError::bad("err.room.started"));
        }
        let n = self.info.members.len();
        let (lo, hi) = self.info.player_range();
        if n > hi {
            return Err(ApiError::bad(
                Msg::new("err.room.too_many").i("max", hi as i64),
            ));
        }
        if n < 2 {
            return Err(ApiError::bad("err.room.too_few"));
        }
        if !force {
            if n < lo {
                return Err(ApiError::bad(
                    Msg::new(if self.info.ranked {
                        "err.room.short_ranked"
                    } else {
                        "err.room.short_casual"
                    })
                    .i("min", lo as i64),
                ));
            }
            if self
                .info
                .members
                .iter()
                .any(|m| !m.host && !m.bot && !m.ready)
            {
                return Err(ApiError::bad("err.room.not_ready"));
            }
        }
        let mode = self.info.mode();
        // Advanced bots are answered by the server's `bot-service`
        // (`docs/BOT.md` B5). With no service attached they play as standard
        // engine bots instead -- the rewrite covers both the match and its
        // record, so a replay starts the same table.
        let mut members = self.info.members.clone();
        if !self.bot_search {
            for m in &mut members {
                if m.bot && m.mentality == game_core::state::BotMentality::Advanced {
                    m.mentality = game_core::state::BotMentality::Standard;
                }
            }
        }
        // Commit-reveal (`docs/FAIRNESS.md`): the slot was drawn at room
        // creation and is already published. Take the nonces of exactly the
        // members in this match (ascending member id -- the derivation sorts,
        // and the record stores them sorted too) and drop the rest.
        let f = self
            .fair
            .as_ref()
            .filter(|f| !f.consumed)
            .ok_or_else(|| ApiError::bad("err.fair.closed"))?;
        let (seed, salt, commit) = (f.seed, f.salt, f.commit.clone());
        let seated: std::collections::BTreeSet<i32> = members.iter().map(|m| m.id).collect();
        let nonces: Vec<(i32, [u8; 32])> = f
            .nonces
            .iter()
            .filter(|(m, _)| seated.contains(m))
            .map(|(&m, &n)| (m, n))
            .collect();
        let derived = fair::derive_match_seed(&seed, &nonces);
        let settings = CanonSettings::new(mode, step, &self.info.weights, &members).canon();
        let blob = self
            .engine
            .new_match(&members, derived, mode as i32, &self.info.weights)
            .map_err(|e| ApiError::bad(e.as_str()))?;
        // The record starts from the same inputs `new_match` got, plus the
        // engine's own identity (`docs/REPLAY.md` §4) and the full
        // commit-reveal material (`docs/FAIRNESS.md`). The openings are in
        // this server-side log line only; the sealed record copies them out
        // after the match.
        let stamp = self.engine.info().map_err(|e| ApiError::bad(e.as_str()))?;
        let entries: Vec<NonceEntry> = nonces
            .iter()
            .map(|(m, n)| NonceEntry {
                member: *m,
                nonce: fair::hex32(n),
            })
            .collect();
        let head = RecordHead {
            init: Init::Seed(MatchSetup {
                members,
                seed: 0,
                seed256: Some(Seed256(derived)),
                weights: self.info.weights.clone(),
            }),
            stamp,
            step,
            created: crate::state::now_stamp(),
            mode: mode as i32,
            fair: Some(Fairness {
                v: fair::FAIR_VERSION,
                commit,
                seed: fair::hex32(&seed),
                salt: fair::hex32(&salt),
                nonces: entries,
                settings,
            }),
        };
        let participants = self.info.members.iter().map(|m| m.id).collect();
        let m = MatchHandle::start(
            self.info.id.clone(),
            self.store.clone(),
            self.engine.clone(),
            blob,
            head,
            participants,
        )
        .map_err(|e| ApiError::bad(e.as_str()))?;
        self.game = Some(Arc::new(m));
        // The slot is consumed: its commit stays up for the running match
        // (people copy it from there), and the next one is rolled when this
        // match ends.
        if let Some(f) = self.fair.as_mut() {
            f.consumed = true;
        }
        self.info.playing = true;
        self.persist();
        for m in &mut self.info.members {
            m.away = false;
            if !m.host && !m.bot {
                m.ready = false;
            }
        }
        self.notify();
        Ok(())
    }

    /// One human's entropy contribution (`docs/FAIRNESS.md`): 32 bytes as
    /// hex, collected as the member enters the room and re-accepted (last
    /// write wins) until a match consumes the slot. A member that never posts
    /// is simply absent from the derived seed; one that leaves before the
    /// start has its nonce dropped with it. Nobody sees anyone else's nonce
    /// until the sealed record -- and the server's seed was committed at room
    /// creation, before any nonce existed, so it cannot adapt either.
    pub fn submit_nonce(&mut self, member: i32, nonce_hex: &str) -> ApiResult<()> {
        let Some(f) = self.fair.as_mut() else {
            return Err(ApiError::bad("err.fair.closed"));
        };
        if f.consumed {
            return Err(ApiError::bad("err.fair.closed"));
        }
        if self.info.members.iter().any(|m| m.id == member && m.bot) {
            return Err(ApiError::bad("err.fair.not_human"));
        }
        let nonce = fair::unhex32(nonce_hex).map_err(|e| {
            ApiError::bad(Msg::new("err.fair.nonce").text("detail", e))
        })?;
        f.nonces.insert(member, nonce);
        self.persist();
        Ok(())
    }

    /// Test-only: the openings of this room's commit-reveal state (hex seed,
    /// hex salt), so a leak scan can assert they never appear in a
    /// client-facing payload. Server-side; never in a RoomInfo / view / SSE frame.
    #[doc(hidden)]
    pub fn debug_fair_openings(&self) -> Option<(String, String)> {
        self.fair
            .as_ref()
            .map(|f| (fair::hex32(&f.seed), fair::hex32(&f.salt)))
    }

    /// Draw a fresh commit-reveal slot for the next match and publish its
    /// commitment (`docs/FAIRNESS.md`). Called at room creation and again
    /// whenever a match ends -- always before that match's nonces can arrive,
    /// so the server is committed first. Clients re-submit automatically when
    /// they see the new commit; nobody waits.
    pub fn roll_fair(&mut self) -> Result<(), String> {
        let stamp = self.engine.info()?;
        let f = FairState::fresh(&stamp);
        self.info.fair = Some(FairPublic {
            commit: f.commit.clone(),
        });
        self.fair = Some(f);
        self.persist();
        self.notify();
        Ok(())
    }

    /// True when the last match consumed the slot and the next one has not
    /// been rolled yet -- what the room ticker uses to roll it the moment the
    /// match ends (and to recover if a roll failed).
    pub fn fair_needs_roll(&self) -> bool {
        self.fair.as_ref().is_some_and(|f| f.consumed) && !self.info.playing
    }

    /// The running match, if play is live. Clone it out and **drop the room
    /// lock** before calling into it -- the round-trip must not hold the lock.
    pub fn match_handle(&self) -> Option<Arc<MatchHandle>> {
        self.game.clone().filter(|_| self.info.playing)
    }

    // ---------------------------------------------------------------- presence

    /// Note a stream opening. Returns true when the member had been marked
    /// away and is back -- the caller then runs `member_back` on the handle
    /// (with the room lock dropped).
    pub fn stream_opened(&mut self, member: i32) -> bool {
        let Some(p) = self.presence.get_mut(&member) else {
            return false;
        };
        p.streams += 1;
        let mut back = false;
        if p.away {
            p.away = false;
            if let Some(m) = self.info.members.iter_mut().find(|m| m.id == member) {
                m.away = false;
            }
            back = true;
            self.notify();
        }
        back
    }

    pub fn stream_closed(&mut self, member: i32) {
        if let Some(p) = self.presence.get_mut(&member) {
            p.streams = p.streams.saturating_sub(1);
            if p.streams == 0 {
                p.since = Instant::now();
            }
        }
    }

    /// Apply presence time-outs. Returns the members dropped mid-match (the
    /// caller runs `member_left` on the handle and clears them) and, outside a
    /// match, the tokens removed from the room.
    pub fn tick_presence(&mut self, timeout: std::time::Duration) -> (Vec<i32>, Vec<String>) {
        let mut dropped = vec![];
        let mut removed = vec![];
        let stale: Vec<i32> = self
            .presence
            .iter()
            .filter(|(_, p)| p.streams == 0 && !p.away && p.since.elapsed() >= timeout)
            .map(|(&m, _)| m)
            .collect();
        for m in stale {
            if self.info.playing {
                if let Some(p) = self.presence.get_mut(&m) {
                    p.away = true;
                }
                if let Some(x) = self.info.members.iter_mut().find(|x| x.id == m) {
                    x.away = true;
                }
                dropped.push(m);
            } else if let Some(t) = self.leave(m) {
                removed.push(t);
            }
        }
        (dropped, removed)
    }
}
