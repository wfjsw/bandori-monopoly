//! A room: members, readiness, bots, and its match (`RoomHost.cs`).
//!
//! The match itself lives in a `rules-worker` process, not here: `game` is the
//! `Match::save` blob and every operation is a round-trip through the pool. The
//! room therefore holds no engine state -- only the blob the caller must keep.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use game_core::msg::Msg;
use game_core::net::{self, NetMessage, RoomInfo, RoomMember};
use game_core::scoring::ScoreWeights;
use serde_json::Value;
use tokio::sync::broadcast;

use std::sync::Mutex;

use crate::error::{ApiError, ApiResult};
use crate::pool::Pool;

/// A running match: its blob, plus the order its operations apply in.
///
/// The **room** lock is never held across a worker round-trip -- callers clone
/// this handle out of the room and drop that lock first (see `state.rs`:
/// critical sections are short). `gate` is what keeps two operations on one
/// match from racing on the blob; it is held for exactly one round-trip, which
/// the pool bounds with its deadline. `state` is held only to copy a blob in
/// and out.
pub struct MatchHandle {
    gate: Mutex<()>,
    state: Mutex<String>,
    engine: Arc<Pool>,
}

impl MatchHandle {
    pub fn new(engine: Arc<Pool>, blob: String) -> Self {
        Self {
            gate: Mutex::new(()),
            state: Mutex::new(blob),
            engine,
        }
    }

    /// The blob, for callers that only need to read it (no round-trip).
    pub fn snapshot(&self) -> String {
        self.state.lock().unwrap().clone()
    }

    /// One operation. `f` gets the current blob; `Some` in its first return
    /// slot replaces the blob. The gate is held for the whole call.
    fn run<T>(
        &self,
        f: impl FnOnce(&str, &Pool) -> Result<(Option<String>, T), String>,
    ) -> Result<T, String> {
        let _order = self.gate.lock().unwrap();
        let before = self.state.lock().unwrap().clone();
        let (after, out) = f(&before, &self.engine)?;
        if let Some(a) = after {
            *self.state.lock().unwrap() = a;
        }
        Ok(out)
    }

    /// `(error message if the command was rejected)`. The blob moves on either
    /// way: a rejected command may still have changed the match.
    pub fn act(&self, member: i32, cmd: &NetMessage) -> Result<Option<Msg>, String> {
        self.run(|s, e| {
            let (s2, err) = e.act(s, member, cmd)?;
            Ok((Some(s2), err))
        })
    }

    /// Advance the clock. Returns whether the match changed.
    pub fn tick(&self, dt: f32) -> Result<bool, String> {
        self.run(|s, e| {
            let s2 = e.tick(s, dt)?;
            let changed = e.changed(&s2)?;
            Ok((Some(s2), changed))
        })
    }

    pub fn ended(&self) -> Result<bool, String> {
        self.run(|s, e| Ok((None, e.ended(s)?)))
    }

    pub fn quick_start(&self) -> Result<(), String> {
        self.run(|s, e| Ok((Some(e.quick_start(s)?), ())))
    }

    pub fn finish(&self) -> Result<(), String> {
        self.run(|s, e| Ok((Some(e.finish(s)?), ())))
    }

    pub fn member_left(&self, member: i32, can_return: bool) -> Result<(), String> {
        self.run(|s, e| Ok((Some(e.member_left(s, member, can_return)?), ())))
    }

    pub fn member_back(&self, member: i32) -> Result<(), String> {
        self.run(|s, e| Ok((Some(e.member_back(s, member)?), ())))
    }

    pub fn view(&self, member: i32) -> Result<Value, String> {
        self.run(|s, e| Ok((None, e.view(s, member)?)))
    }

    pub fn events(&self, since: i32) -> Result<Vec<Value>, String> {
        self.run(|s, e| Ok((None, e.events(s, since)?)))
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
    pub info: RoomInfo,
    password: String,
    /// Human member id -> session token.
    pub tokens: HashMap<i32, String>,
    next_member: i32,
    /// The running match. The engine is in a worker process; this holds only
    /// the `Match::save` blob and the order its operations apply in.
    pub game: Option<Arc<MatchHandle>>,
    pub engine: Arc<Pool>,
    pub tx: broadcast::Sender<Note>,
    presence: HashMap<i32, Presence>,
    pub dissolved: Option<Msg>,
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
            },
            password: password.to_string(),
            tokens: HashMap::new(),
            next_member: 1,
            game: None,
            engine,
            tx,
            presence: HashMap::new(),
            dissolved: None,
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
        });
        self.tokens.insert(id, who.token.into());
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
        }
        self.notify();
        token
    }

    pub fn dissolve(&mut self, reason: Msg) {
        if self.dissolved.is_none() {
            self.dissolved = Some(reason.clone());
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
        self.notify();
        Ok(())
    }

    /// `names`: the data's bot name list (`match_rules.json`).
    pub fn add_bot(&mut self, member: i32, names: &[String]) -> ApiResult<()> {
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
            ..Default::default()
        });
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
    pub fn start(&mut self, member: i32, force: bool, seed: u64) -> ApiResult<()> {
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
        let blob = self
            .engine
            .new_match(&self.info.members, seed, mode as i32, &self.info.weights)
            .map_err(|e| ApiError::bad(e.as_str()))?;
        self.game = Some(Arc::new(MatchHandle::new(self.engine.clone(), blob)));
        self.info.playing = true;
        for m in &mut self.info.members {
            m.away = false;
            if !m.host && !m.bot {
                m.ready = false;
            }
        }
        self.notify();
        Ok(())
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
