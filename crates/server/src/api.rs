//! HTTP handlers. Client -> server is always POST (or GET for reads); server ->
//! client is the SSE stream in `sse.rs`.

use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use game_core::net::{self, NetMessage, RoomInfo};
use game_core::scoring::ScoreWeights;
use game_core::state::MatchState;
use serde::{Deserialize, Serialize};

use crate::auth::{Auth, COOKIE};
use crate::error::{ApiError, ApiResult};
use crate::room::{NewMember, Room};
use crate::state::{room_code, Server, Session};
use crate::store;

type S = State<Arc<Server>>;

// ------------------------------------------------------------------ session

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionReq {
    pub player: String,
    pub persona: String,
    pub cn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub token: String,
    pub player: String,
    pub room: Option<String>,
    pub member: i32,
}

fn view(s: &Session) -> SessionView {
    SessionView {
        token: s.token.clone(),
        player: s.player.clone(),
        room: s.room.as_ref().map(|r| r.0.clone()),
        member: s.room.as_ref().map_or(0, |r| r.1),
    }
}

/// `POST /api/session` -- a new player session. Also sets the cookie the browser's
/// `EventSource` will send.
///
/// The name is the only identity: the game server never sees the profile
/// server or its ticket. A client re-opens a session here with the same name
/// on every game server it connects to.
pub async fn create_session(
    State(s): S,
    Json(req): Json<SessionReq>,
) -> ApiResult<impl IntoResponse> {
    let player = net::clean_name(&req.player);
    if player.is_empty() {
        return Err(ApiError::bad("err.name_required"));
    }
    let token = crate::state::random_hex(24);
    let session = Session {
        token,
        player,
        persona: req.persona,
        cn_id: req.cn_id,
        room: None,
    };
    let body = view(&session);
    let cookie = format!("{COOKIE}={}; HttpOnly; SameSite=Lax; Path=/", session.token);
    let _ = s.store.session_put(&session);
    Ok(([(header::SET_COOKIE, cookie)], Json(body)))
}

/// `GET /api/session`
pub async fn get_session(Auth(sess): Auth) -> Json<SessionView> {
    Json(view(&sess))
}

pub async fn health(State(s): S) -> Json<serde_json::Value> {
    // The lobby's room count is the store's, not this process's cache.
    let rooms = s.store.room_all().map(|r| r.len()).unwrap_or(0);
    Json(serde_json::json!({
        "game": net::GAME,
        "version": net::VERSION,
        "rooms": rooms,
        "builds": s.builds.len(),
    }))
}

/// One advertised rule build.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub ruleset_sha256: Option<String>,
    pub cards: usize,
    pub default: bool,
}

/// `GET /api/builds` -- the rule builds this server offers. The client picks
/// one when opening a room (or a solo match) and loads its modules from
/// `/builds/{id}/rules`.
pub async fn list_builds(State(s): S) -> Json<Vec<BuildView>> {
    let mut list: Vec<BuildView> = s
        .builds
        .values()
        .map(|b| BuildView {
            id: b.id.clone(),
            name: b.name.clone(),
            description: b.description.clone(),
            ruleset_sha256: b.rules.ruleset_sha256().map(str::to_string),
            cards: b.data.cards.len(),
            default: b.id == s.default_build,
        })
        .collect();
    list.sort_by(|a, b| b.default.cmp(&a.default).then(a.name.cmp(&b.name)));
    Json(list)
}

// ------------------------------------------------------------------ rooms

/// `GET /api/rooms` -- the lobby list (passwords never leave the server).
/// Straight from the store: every process sees every room, including the ones
/// this one has never touched.
pub async fn list_rooms(State(s): S) -> Json<Vec<RoomInfo>> {
    let mut list: Vec<RoomInfo> = s
        .store
        .room_all()
        .unwrap_or_default()
        .into_iter()
        .map(|rec| rec.info)
        .collect();
    list.sort_by(|a, b| a.playing.cmp(&b.playing).then(a.name.cmp(&b.name)));
    Json(list)
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateRoom {
    pub name: String,
    pub ranked: bool,
    pub max_players: i32,
    pub password: String,
    pub weights: Option<ScoreWeights>,
    /// Match clock / timer settings. Absent = defaults (the pre-settings
    /// constants). Sanitized on the way in (`ClockSettings::sanitized`).
    pub clock: Option<game_core::clock::ClockSettings>,
    /// Rule build to open the room under (`GET /api/builds`). Absent = the
    /// server's default.
    pub build: String,
}

impl Default for CreateRoom {
    fn default() -> Self {
        Self {
            name: String::new(),
            ranked: false,
            max_players: 6,
            password: String::new(),
            weights: None,
            clock: None,
            build: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Joined {
    pub room: RoomInfo,
    pub you: i32,
}

fn rules_weights(_s: &Server) -> ScoreWeights {
    ScoreWeights::default()
}

/// Leave whatever room the session is in.
fn leave_current(s: &Server, sess: &Session) {
    let Some((rid, member)) = &sess.room else {
        return;
    };
    // Mid-match, leaving a **seat** forfeits: run that on the handle first,
    // with the room lock dropped (the match has its own lock), then take the
    // room lock for the bookkeeping. A spectator has no seat to forfeit.
    let (m, spectating) = s
        .room(rid)
        .map(|room| {
            let r = room.lock().unwrap();
            (r.match_handle(), r.is_spectator(*member))
        })
        .unwrap_or((None, false));
    if let Some(m) = m {
        if !spectating {
            let _ = m.act(*member, &game_core::net::NetMessage::act("leave"));
        }
    }
    let dissolved = s
        .with_room(rid, |r| {
            r.leave(*member);
            Ok(r.dissolved.is_some())
        })
        .unwrap_or(false);
    if dissolved {
        s.forget_room(rid);
    }
    s.set_room(&sess.token, None);
}

/// `POST /api/rooms` -- create a room and join it as host.
///
/// The room's commit-reveal slot is drawn here (`docs/FAIRNESS.md`): the
/// commitment in the reply was sealed before this (or any) member could
/// contribute a nonce.
pub async fn create_room(
    State(s): S,
    Auth(sess): Auth,
    Json(req): Json<CreateRoom>,
) -> ApiResult<Json<Joined>> {
    leave_current(&s, &sess);
    // Reserve the code in the store so two processes cannot both create the
    // same short id (`SET NX` on `roomcode:{id}`).
    let id = loop {
        let id = room_code();
        if s.store
            .room_get(&id)
            .map_err(|e| ApiError::bad(e.to_string().as_str()))?
            .is_some()
        {
            continue;
        }
        let taken = !s
            .store
            .lock_acquire(&format!("roomcode:{id}"), &s.instance, store::LOCK_TTL_MS)
            .unwrap_or(false);
        if !taken {
            break id;
        }
    };
    // An empty name is shown by clients as "<host>'s room" in their own language.
    let name = req.name.trim().to_string();
    let defaults = rules_weights(&s);
    let weights = req.weights.unwrap_or(defaults).sanitized(defaults);
    let clock = req.clock.unwrap_or_default().sanitized();
    let build = s
        .build(&req.build)
        .ok_or_else(|| ApiError::bad("err.build.unknown"))?;
    let engine = build.engine();
    let build_id = build.id.clone();
    let data = build.data.clone();
    let store = s.store.clone();
    let bot_search = s.bots.is_some();
    let presence_timeout = s.presence_timeout;
    let (token, player, persona, cn_id) = (
        sess.token.clone(),
        sess.player.clone(),
        sess.persona.clone(),
        sess.cn_id.clone(),
    );
    // The slot roll reads the (cached) engine stamp; run it off the async
    // runtime like every other engine touch.
    let rid = id.clone();
    let (room, you) = tokio::task::spawn_blocking(move || {
        let mut room = Room::new(
            rid,
            &name,
            req.ranked,
            req.max_players,
            &req.password,
            weights,
            clock,
            &build_id,
            data,
            engine,
            store,
        );
        // Advanced bots need the bot-service; without one they play as standard
        // (`docs/BOT.md` B5, `Room::start`).
        room.bot_search = bot_search;
        room.presence_timeout = presence_timeout;
        room.roll_fair().map_err(|e| ApiError::bad(e.as_str()))?;
        let who = NewMember {
            token: &token,
            player: &player,
            persona: &persona,
            cn_id: &cn_id,
        };
        let you = room.join(who, &req.password, "", false)?;
        Ok::<_, ApiError>((room, you))
    })
    .await
    .map_err(|e| ApiError::bad(format!("create task: {e}").as_str()))??;
    let info = room.info.clone();
    s.rooms
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(Mutex::new(room)));
    s.set_room(&sess.token, Some((id, you)));
    Ok(Json(Joined { room: info, you }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct JoinReq {
    pub password: String,
    pub version: String,
    /// Join as a watcher: no seat, allowed while the room is playing or full.
    /// The password still applies. See [`game_core::net::RoomSpectator`].
    pub spectate: bool,
}

/// `POST /api/rooms/{id}/join` -- join (or rejoin) a room, as a seat or (with
/// `spectate`) as a watcher.
pub async fn join_room(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<JoinReq>,
) -> ApiResult<Json<Joined>> {
    if sess.room.as_ref().is_some_and(|r| r.0 != id) {
        leave_current(&s, &sess);
    }
    let who = NewMember {
        token: &sess.token,
        player: &sess.player,
        persona: &sess.persona,
        cn_id: &sess.cn_id,
    };
    // Under the room lock with a fresh record: `next_member` is allocated from
    // the store's copy, so two servers cannot hand out the same id.
    let (you, info) = s.with_room(&id, |r| {
        if r.dissolved.is_some() {
            return Err(ApiError::not_found("err.room.gone"));
        }
        let you = r.join(who, &req.password, &req.version, req.spectate)?;
        Ok((you, r.info.clone()))
    })?;
    s.set_room(&sess.token, Some((id.clone(), you)));
    Ok(Json(Joined { room: info, you }))
}

/// The session's room and member id, checked against `id`. Membership is
/// verified against the **store's** roster (via [`Server::with_room`]), so a
/// stale local cache cannot admit a non-member.
pub fn member_room(s: &Server, sess: &Session, id: &str) -> ApiResult<(Arc<Mutex<Room>>, i32)> {
    let me = member_in(sess, id)?;
    let token = sess.token.clone();
    let room = s
        .room(id)
        .ok_or_else(|| ApiError::not_found("err.room.gone"))?;
    s.with_room(id, |r| {
        if r.member_of(&token) != Some(me) {
            return Err(ApiError::bad("err.room.not_member"));
        }
        Ok(())
    })?;
    Ok((room, me))
}

/// The session's member id in `id`, with no roster check (the caller does that
/// under the room lock).
pub fn member_in(sess: &Session, id: &str) -> ApiResult<i32> {
    let (rid, member) = sess
        .room
        .clone()
        .ok_or_else(|| ApiError::bad("err.room.not_member"))?;
    if rid != id {
        return Err(ApiError::bad("err.room.not_member"));
    }
    Ok(member)
}

/// Run `f` as a **seat** of the room: membership checked under the room lock,
/// then the mutation runs against a freshly loaded record. Spectators are not
/// seats and are refused — this guards `act` / `ready` / `start` / the host
/// ops. Use [`as_viewer`] for read-only access.
pub fn as_member<T>(
    s: &Server,
    sess: &Session,
    id: &str,
    f: impl FnOnce(&mut Room, i32) -> ApiResult<T>,
) -> ApiResult<T> {
    let me = member_in(sess, id)?;
    let token = sess.token.clone();
    s.with_room(id, move |r| {
        if r.member_of(&token) != Some(me) {
            return Err(ApiError::bad("err.room.not_member"));
        }
        if r.is_spectator(me) {
            return Err(ApiError::bad("err.not_a_player"));
        }
        f(r, me)
    })
}

/// Run `f` as anyone in the room — a seat **or** a spectator. For read-only
/// endpoints (`GET …/state`, the SSE stream) where a watcher is a legitimate
/// caller but must never reach the engine.
pub fn as_viewer<T>(
    s: &Server,
    sess: &Session,
    id: &str,
    f: impl FnOnce(&Room, i32) -> ApiResult<T>,
) -> ApiResult<T> {
    let me = member_in(sess, id)?;
    let token = sess.token.clone();
    s.with_room(id, move |r| {
        if r.member_of(&token) != Some(me) {
            return Err(ApiError::bad("err.room.not_member"));
        }
        f(&*r, me)
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReadyReq {
    pub on: bool,
}

pub async fn ready(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<ReadyReq>,
) -> ApiResult<Json<RoomInfo>> {
    let info = as_member(&s, &sess, &id, |r, me| {
        r.set_ready(me, req.on)?;
        Ok(r.info.clone())
    })?;
    Ok(Json(info))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BotReq {
    /// `"add"` or `"remove"`.
    pub op: String,
    pub member: i32,
    /// Bot mentality for `op: "add"`: `"standard"` (default) or `"chaos"`.
    pub mentality: String,
}

pub async fn bots(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<BotReq>,
) -> ApiResult<Json<RoomInfo>> {
    let info = as_member(&s, &sess, &id, |r, me| {
        match req.op.as_str() {
            "add" => {
                let m = game_core::state::BotMentality::parse(&req.mentality)
                    .ok_or_else(|| ApiError::bad("err.bad_mentality"))?;
                let names = r.data.match_rules.bot_names.clone();
                r.add_bot(me, &names, m)?
            }
            "remove" => r.remove_bot(me, req.member)?,
            _ => return Err(ApiError::bad("err.bad_bot_op")),
        }
        Ok(r.info.clone())
    })?;
    Ok(Json(info))
}

pub async fn weights(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(w): Json<ScoreWeights>,
) -> ApiResult<Json<RoomInfo>> {
    let defaults = rules_weights(&s);
    let info = as_member(&s, &sess, &id, |r, me| {
        r.set_weights(me, w, defaults)?;
        Ok(r.info.clone())
    })?;
    Ok(Json(info))
}

/// `POST /api/rooms/{id}/clock` -- host-only edit of the match clock / timer
/// settings (the `weights` sibling). Body is `ClockSettings` (camelCase).
pub async fn clock(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(c): Json<game_core::clock::ClockSettings>,
) -> ApiResult<Json<RoomInfo>> {
    let info = as_member(&s, &sess, &id, |r, me| {
        r.set_clock(me, c)?;
        Ok(r.info.clone())
    })?;
    Ok(Json(info))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StartReq {
    /// Start even if not everyone is ready or the room is short of players.
    pub force: bool,
}

pub async fn start(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<StartReq>,
) -> ApiResult<Json<RoomInfo>> {
    // The tick quantum this match will run on, sealed into its record header
    // so a replay reproduces the same `tick(dt)` f32s (docs/REPLAY.md §1).
    let step = crate::TICK_QUANTUM * s.time_scale;
    let info = tokio::task::spawn_blocking(move || {
        // Under the room lock with a fresh record: the fair slot is consumed
        // here, so two servers cannot both roll it away.
        as_member(&s, &sess, &id, |r, me| {
            // Creates the match immediately: the commitment was fixed at room
            // creation and the player nonces are already in (`docs/FAIRNESS.md`).
            // Nothing waits.
            r.start(me, req.force, step)?;
            Ok(r.info.clone())
        })
    })
    .await
    .map_err(|e| ApiError::bad(format!("start task: {e}").as_str()))??;
    Ok(Json(info))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NonceReq {
    /// 32 random bytes as lower-case hex (`crypto.getRandomValues` on the
    /// client). `docs/FAIRNESS.md`.
    pub nonce: String,
}

/// `POST /api/rooms/{id}/nonce` -- one human's entropy contribution, posted as
/// the player enters the room (and again whenever the room shows a new
/// commitment after a match). Accepted until a match consumes the slot; a
/// missing nonce is simply absent and nothing ever blocks on one.
pub async fn nonce(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<NonceReq>,
) -> ApiResult<Json<RoomInfo>> {
    let info = tokio::task::spawn_blocking(move || {
        as_member(&s, &sess, &id, |r, me| {
            r.submit_nonce(me, &req.nonce)?;
            Ok(r.info.clone())
        })
    })
    .await
    .map_err(|e| ApiError::bad(format!("nonce task: {e}").as_str()))??;
    Ok(Json(info))
}

pub async fn leave(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    member_room(&s, &sess, &id)?;
    leave_current(&s, &sess);
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ------------------------------------------------------------------ match

/// What one member may see: the shared state plus their own hand. The worker
/// produces this shape; it is only re-typed here.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchView {
    pub state: MatchState,
    pub hand: Vec<String>,
    pub hand_notes: Vec<game_core::msg::Msg>,
    /// The member's remaining draw pile, sorted by card id (see
    /// [`game_core::engine::Match::draw_of`]).
    pub draw: Vec<String>,
    pub you: i32,
    pub player_id: i32,
    /// Per-tile board captions from this member's seat
    /// ([`game_core::engine::Match::tile_quotes`]). Optional so a frame from
    /// an older worker still deserializes.
    ///
    /// NB: the worker's `aiAnswer` / `playable` / `estCost` / `skills` extras
    /// are **not** re-typed here and are dropped on the online path. A human
    /// client derives those itself from the seat view (the seat-view engine,
    /// `docs/BOT.md` §1, `bot-glue`'s `extras`) -- what it already legitimately
    /// knows. Whether `aiAnswer` may reach a human seat online is a fairness
    /// question (`docs/FAIRNESS.md`); the client-side computation answers it:
    /// the seat's own prompt is its own business, and the server never ships
    /// another seat's entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_quotes: Option<Vec<Option<game_core::state::TileQuote>>>,
    /// Absolute turn-clock deadlines on the monotonic server clock (ms).
    /// Absent while the clock is held (`state.clockPaused`), on a machine
    /// seat, in solo, or outside the play phase -- the client then shows the
    /// clock held at full (or no clock at all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<crate::clock::ClockWire>,
    /// Server monotonic ms when this frame was built. Piggybacked so a client
    /// can refine its offset without a dedicated ping (see `crate::clock`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_time_ms: Option<f64>,
}

/// Ask the worker for one member's view. Blocking; see `pool`.
pub fn match_view(m: &crate::room::MatchHandle, member: i32) -> ApiResult<MatchView> {
    // Human clients compute the per-viewer extras themselves (the seat-view
    // engine, `docs/BOT.md` §1) -- do not pay the worker for them here.
    let v = m
        .view(member, false)
        .map_err(|e| ApiError::bad(e.as_str()))?;
    let mut view: MatchView =
        serde_json::from_value(v).map_err(|e| ApiError::bad(format!("view: {e}").as_str()))?;
    let now = crate::clock::server_time_ms();
    view.clock = crate::clock::clock_for_state(&view.state, now);
    view.server_time_ms = Some(now);
    Ok(view)
}

/// `POST /api/clock` -- one NTP-style sample. `t1` is the client's
/// `performance.now()` at send; `t2` is stamped at receive and `t3` just
/// before the response leaves. See `crate::clock` and `webui/src/game/clockSync.ts`.
pub async fn clock_ping(
    Json(ping): Json<crate::clock::ClockPing>,
) -> Json<crate::clock::ClockPong> {
    // T2 as late as possible: the first thing after the body is parsed.
    let t2 = crate::clock::server_time_ms();
    let t3 = crate::clock::server_time_ms();
    Json(crate::clock::ClockPong {
        t1: ping.t1,
        t2,
        t3,
    })
}

#[derive(Debug, Serialize)]
pub struct RoomState {
    pub room: RoomInfo,
    pub you: i32,
    #[serde(rename = "match")]
    pub game: Option<MatchView>,
}

/// `GET /api/rooms/{id}/state` -- full snapshot (reconnect, or clients that poll).
pub async fn room_state(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<RoomState>> {
    // Fresh roster under the room lock; the match view is a worker call and
    // runs after the lock is dropped (the handle is stateless). Spectators may
    // read: their frame is the public view (`playerId` -1, no hand).
    let (info, me, m) = as_viewer(&s, &sess, &id, |r, me| {
        Ok((r.info.clone(), me, r.match_handle()))
    })?;
    let game = match m {
        None => None,
        Some(m) => Some(
            tokio::task::spawn_blocking(move || match_view(&m, me))
                .await
                .map_err(|e| ApiError::bad(format!("view task: {e}").as_str()))?,
        ),
    };
    let game = game.transpose()?;
    Ok(Json(RoomState {
        room: info,
        you: me,
        game,
    }))
}

/// `POST /api/rooms/{id}/anim-done` -- the acting seat's animation queue has
/// drained for state version `seq`. Releases the turn-clock hold (10s cap
/// applies if this never arrives). Recorded as `Input::AnimDone`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimDoneReq {
    pub seq: i32,
}

pub async fn anim_done(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<AnimDoneReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let (m, me) = as_member(&s, &sess, &id, |r, me| {
        let m = r
            .match_handle()
            .ok_or_else(|| ApiError::bad("err.room.no_match"))?;
        Ok((m, me))
    })?;
    let released = tokio::task::spawn_blocking(move || m.anim_done(me, req.seq))
        .await
        .map_err(|e| ApiError::bad(format!("anim_done task: {e}").as_str()))?
        .map_err(|e| ApiError::bad(e.as_str()))?;
    if let Some(room) = s.room(&id) {
        room.lock().unwrap().notify();
    }
    Ok(Json(
        serde_json::json!({ "ok": true, "released": released }),
    ))
}

/// `POST /api/rooms/{id}/act` -- a match command (`NetMessage` with `act` set).
pub async fn act(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(msg): Json<NetMessage>,
) -> ApiResult<Json<serde_json::Value>> {
    // Console cheats (`engine/debug.rs`) are compiled out of release builds,
    // so a release server refuses `debug` at the door with `err.unknown_act`:
    // the same answer its engine would give, no cheat key leaks into the
    // binary, and a forged message reaches neither the engine nor the record
    // log (which keeps even refused inputs). A debug-built server forwards
    // them -- cheats run online in debug builds and the engine marks the match
    // (`MatchState.debugOpen`) so the record shows the cheat use.
    #[cfg(not(debug_assertions))]
    {
        if msg.act == "debug" {
            return Err(ApiError::bad("err.unknown_act"));
        }
    }
    // Check the match out under the room lock, drop it, then run the command
    // under the match lock.
    let (m, me) = as_member(&s, &sess, &id, |r, me| {
        let m = r
            .match_handle()
            .ok_or_else(|| ApiError::bad("err.room.no_match"))?;
        Ok((m, me))
    })?;
    let err = tokio::task::spawn_blocking(move || m.act(me, &msg))
        .await
        .map_err(|e| ApiError::bad(format!("act task: {e}").as_str()))?
        .map_err(|e| ApiError::bad(e.as_str()))?;
    if let Some(e) = err {
        return Err(ApiError::bad(e));
    }
    if let Some(room) = s.room(&id) {
        room.lock().unwrap().notify();
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ------------------------------------------------------------------ record

/// `GET /api/rooms/{id}/record` -- the last finished match's `.bdrec`.
///
/// The full record reveals every hand and the deck order, so it is only served
/// **after** the match ends, and only to the people who sat down for it
/// (`docs/REPLAY.md` §1). `409 err.record.live` while the room is playing,
/// `404 err.record.none` when there is nothing sealed yet, `403
/// err.record.forbidden` to anyone the room's tokens do not put in the match.
///
/// The body is the sealed bytes **as stored**: a zstd-framed `.bdrec` today,
/// served as `application/zstd`. Deliberately **not** `Content-Encoding: zstd`
/// -- a browser would transparently decode that, and inconsistently; the
/// client decompresses in wasm (`from_record_bytes`). A record stored before
/// compression is plain JSON and is served as-is as `application/json`.
/// `Content-Disposition` names the file the way a download should.
pub async fn record(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // `playing` is the store's flag (any instance may have started the match).
    let playing = s
        .store
        .room_get(&id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        .is_some_and(|rec| rec.info.playing);
    if playing {
        return Err(ApiError::new(StatusCode::CONFLICT, "err.record.live"));
    }
    let rec = s
        .store
        .record_get(&id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        .ok_or_else(|| ApiError::not_found("err.record.none"))?;
    // "whoever the room's tokens put in this match" -- a member id, checked
    // against the seats the record was sealed for. The room may already be
    // dissolved; the session's own memory of the seat still counts.
    let me = match &sess.room {
        Some((rid, member)) if *rid == id => Some(*member),
        _ => s
            .room(&id)
            .and_then(|r| r.lock().unwrap().member_of(&sess.token)),
    };
    if !me.is_some_and(|m| rec.members.contains(&m)) {
        return Err(ApiError::forbidden("err.record.forbidden"));
    }
    let ctype = if game_core::record::is_zstd(&rec.record) {
        "application/zstd"
    } else {
        "application/json"
    };
    let name = format!("bdrec-{id}-{}.bdrec", crate::state::now_file_stamp());
    Ok((
        [
            (header::CONTENT_TYPE, ctype.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        rec.record,
    ))
}
