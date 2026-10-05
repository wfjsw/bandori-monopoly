//! A room: members, readiness, bots, and its match (`RoomHost.cs`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::msg::Msg;
use game_core::net::{self, NetMessage, RoomInfo, RoomMember};
use game_core::scoring::ScoreWeights;
use tokio::sync::broadcast;

use crate::error::{ApiError, ApiResult};

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
    pub game: Option<Match>,
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
    pub fn new(id: String, name: &str, ranked: bool, max_players: i32, password: &str, weights: ScoreWeights) -> Self {
        let max = if ranked { max_players.clamp(2, 6) } else { max_players.clamp(2, 10) };
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
            tx,
            presence: HashMap::new(),
            dissolved: None,
        }
    }

    pub fn notify(&self) {
        let _ = self.tx.send(Note::Changed);
    }

    pub fn member_of(&self, token: &str) -> Option<i32> {
        self.tokens.iter().find(|(_, t)| t.as_str() == token).map(|(&m, _)| m)
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
        self.presence.insert(id, Presence { streams: 0, since: Instant::now(), away: false });
        self.notify();
        Ok(id)
    }

    /// Leave the room. Mid-match this forfeits. Returns the token that left.
    pub fn leave(&mut self, member: i32) -> Option<String> {
        if let Some(g) = &mut self.game {
            if self.info.playing {
                let _ = g.act(member, &NetMessage::act("leave"));
            }
        }
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
        let m = self.info.members.iter_mut().find(|m| m.id == member).ok_or_else(|| ApiError::bad("err.room.not_member"))?;
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
        self.info.members.push(RoomMember { id, player, ready: true, bot: true, ..Default::default() });
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

    pub fn set_weights(&mut self, member: i32, w: ScoreWeights, rules_default: ScoreWeights) -> ApiResult<()> {
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
    pub fn start(&mut self, member: i32, force: bool, data: Arc<GameData>, rules: Arc<dyn CardRules>, seed: u64) -> ApiResult<()> {
        self.host_only(member)?;
        if self.info.playing {
            return Err(ApiError::bad("err.room.started"));
        }
        let n = self.info.members.len();
        let (lo, hi) = self.info.player_range();
        if n > hi {
            return Err(ApiError::bad(Msg::new("err.room.too_many").i("max", hi as i64)));
        }
        if n < 2 {
            return Err(ApiError::bad("err.room.too_few"));
        }
        if !force {
            if n < lo {
                return Err(ApiError::bad(Msg::new(if self.info.ranked { "err.room.short_ranked" } else { "err.room.short_casual" }).i("min", lo as i64)));
            }
            if self.info.members.iter().any(|m| !m.host && !m.bot && !m.ready) {
                return Err(ApiError::bad("err.room.not_ready"));
            }
        }
        let mode = self.info.mode();
        self.game = Some(Match::new(data, rules, &self.info.members, seed, mode, self.info.weights));
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

    pub fn act(&mut self, member: i32, msg: &NetMessage) -> ApiResult<()> {
        let g = self.game.as_mut().filter(|_| self.info.playing).ok_or_else(|| ApiError::bad("err.room.no_match"))?;
        g.act(member, msg).map_err(ApiError::bad)?;
        self.notify();
        Ok(())
    }

    // ---------------------------------------------------------------- presence

    pub fn stream_opened(&mut self, member: i32) {
        let Some(p) = self.presence.get_mut(&member) else { return };
        p.streams += 1;
        if p.away {
            p.away = false;
            if let Some(m) = self.info.members.iter_mut().find(|m| m.id == member) {
                m.away = false;
            }
            if let Some(g) = &mut self.game {
                g.member_back(member);
            }
            self.notify();
        }
    }

    pub fn stream_closed(&mut self, member: i32) {
        if let Some(p) = self.presence.get_mut(&member) {
            p.streams = p.streams.saturating_sub(1);
            if p.streams == 0 {
                p.since = Instant::now();
            }
        }
    }

    /// Advance the match and apply presence time-outs. Returns tokens of members
    /// removed from the room (their sessions must be cleared by the caller).
    pub fn tick(&mut self, dt: f32, timeout: std::time::Duration) -> Vec<String> {
        let mut changed = false;
        if let Some(g) = &mut self.game {
            if self.info.playing {
                g.tick(dt);
                changed |= g.take_changed();
                if g.ended() {
                    self.info.playing = false;
                    changed = true;
                }
            }
        }
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
                if let Some(g) = &mut self.game {
                    g.member_left(m, true);
                }
                changed = true;
            } else if let Some(t) = self.leave(m) {
                removed.push(t);
            }
        }
        if changed {
            self.notify();
        }
        removed
    }
}
