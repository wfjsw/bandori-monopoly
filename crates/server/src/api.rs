//! HTTP handlers. Client -> server is always POST (or GET for reads); server ->
//! client is the SSE stream in `sse.rs`.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
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
pub struct SessionRequest {
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
    Json(req): Json<SessionRequest>,
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
pub struct JoinRequest {
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
    Json(req): Json<JoinRequest>,
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
pub struct ReadyRequest {
    pub on: bool,
}

pub async fn ready(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<ReadyRequest>,
) -> ApiResult<Json<RoomInfo>> {
    let info = as_member(&s, &sess, &id, |r, me| {
        r.set_ready(me, req.on)?;
        Ok(r.info.clone())
    })?;
    Ok(Json(info))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BotRequest {
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
    Json(req): Json<BotRequest>,
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
pub struct StartRequest {
    /// Start even if not everyone is ready or the room is short of players.
    pub force: bool,
}

pub async fn start(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<StartRequest>,
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
pub struct NonceRequest {
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
    Json(req): Json<NonceRequest>,
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
    /// Per-tile board captions from **this member's seat**
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
    /// The member's own open deck inspection, while one is running
    /// ([`game_core::engine::Match::inspection_of`]). Only the looker sees it;
    /// a public frame's ids also ride the owner's `state.players[].reveal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection: Option<game_core::state::InspectionView>,
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
pub fn match_view(
    m: &crate::room::MatchHandle,
    member: i32,
    need_quotes: bool,
) -> ApiResult<MatchView> {
    // Human clients compute the per-viewer extras themselves (the seat-view
    // engine, `docs/BOT.md` §1) -- do not pay the worker for them here.
    let v = m
        .view(member, false, need_quotes)
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
    // read: their frame is the public view (`playerId` -1, no hand). The
    // sealed match stays in the snapshot (`frame_handle`) so a refresh on the
    // results screen finds it again after the room's reset.
    let (info, me, m) = as_viewer(&s, &sess, &id, |r, me| {
        Ok((r.info.clone(), me, r.frame_handle()))
    })?;
    let game = match m {
        None => None,
        Some(m) => Some(
            tokio::task::spawn_blocking(move || match_view(&m, me, true))
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EventsQuery {
    pub since: i32,
}

/// `GET /api/rooms/{id}/events?since=N` -- the match's retained events after
/// `since`, oldest first. This is the log's mount-time history seed; the
/// engine keeps only a tail, so `since=0` is "as far back as it still has".
/// Seats and spectators alike may read (the event log is public).
pub async fn events(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Query(q): Query<EventsQuery>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let m = as_viewer(&s, &sess, &id, |r, _me| Ok(r.match_handle()))?;
    let Some(m) = m else {
        return Ok(Json(Vec::new()));
    };
    let since = q.since;
    let evs = tokio::task::spawn_blocking(move || m.events(since))
        .await
        .map_err(|e| ApiError::bad(format!("events task: {e}").as_str()))?
        .map_err(|e| ApiError::bad(e.as_str()))?;
    Ok(Json(evs))
}

/// `POST /api/rooms/{id}/anim-done` -- the seat's animation queue has drained
/// for state version `seq`. Clears that seat from the turn-clock hold, which
/// releases once every pending human seat has acked (10s cap applies if one
/// never arrives). A releasing ack is recorded as `Input::AnimDone`; an ack
/// that releases nothing (no hold, stale seq, seat not held -- e.g. a
/// spectator's, or the echo of the release frame) answers `released: false`
/// and writes neither the blob nor the log.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationDoneRequest {
    pub seq: i32,
}

pub async fn anim_done(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<AnimationDoneRequest>,
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

/// `POST /api/rooms/{id}/order-ready` -- this seat's client has the rolled
/// turn order on screen. The order phase ends once every seat with a client
/// has acked (a 4s cap covers a seat that never does). Recorded as
/// `Input::OrderReady` so a replay reproduces the shortened phase.
pub async fn order_ready(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let (m, me) = as_member(&s, &sess, &id, |r, me| {
        let m = r
            .match_handle()
            .ok_or_else(|| ApiError::bad("err.room.no_match"))?;
        Ok((m, me))
    })?;
    let ready = tokio::task::spawn_blocking(move || m.order_ready(me))
        .await
        .map_err(|e| ApiError::bad(format!("order_ready task: {e}").as_str()))?
        .map_err(|e| ApiError::bad(e.as_str()))?;
    if let Some(room) = s.room(&id) {
        room.lock().unwrap().notify();
    }
    Ok(Json(serde_json::json!({ "ok": true, "ready": ready })))
}

/// `POST /api/rooms/{id}/act` -- a match command (`NetMessage` with `act` set).
/// A refused command (400) writes no state -- the blob is byte-identical --
/// but is recorded as `Input::Act { ok: false }`, which is what lets a replay
/// reproduce the refusal instead of drifting.
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

/// The caller's member id in `room`: the session's own memory of the seat
/// first (the room may already be dissolved), then the room's live token map.
fn caller_member(s: &Server, sess: &Session, id: &str) -> Option<i32> {
    match &sess.room {
        Some((rid, member)) if rid == id => Some(*member),
        _ => s
            .room(id)
            .and_then(|r| r.lock().unwrap().member_of(&sess.token)),
    }
}

/// A record download: the sealed bytes as stored, named for the browser.
/// Deliberately **no** `Content-Encoding` -- a browser would transparently
/// decode zstd and inconsistently; the client decodes in wasm
/// (`from_record_bytes`).
fn download(id: &str, match_id: Option<&str>, bytes: Vec<u8>) -> Response {
    let ctype = if game_core::record::is_zstd(&bytes) {
        "application/zstd"
    } else {
        "application/json"
    };
    let name = match match_id {
        Some(m) => format!("bdrec-{id}-{m}.bdrec"),
        None => format!("bdrec-{id}-{}.bdrec", crate::state::now_file_stamp()),
    };
    let mut b = Response::builder()
        .header(header::CONTENT_TYPE, ctype)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        );
    if let Some(m) = match_id {
        // The archived match is named back so a client can ask for the
        // neighbours (`GET .../records`).
        b = b.header("x-match-id", HeaderValue::from_str(m).unwrap_or(HeaderValue::from_static("")));
    }
    b.body(Body::from(bytes))
        .unwrap_or_else(|e| ApiError::bad(e.to_string().as_str()).into_response())
}

/// The store's last-record cache, or -- when it has expired / been replaced --
/// the durable archive's newest match the caller sat in.
///
/// `Ok(Ok((bytes, match_id)))` serves it; `Ok(Err(exists))` is the refusal,
/// and `exists` tells the caller whether `403` or `404` is the honest answer.
/// The outer `Err` is a store failure.
fn caller_record(
    s: &Server,
    me: Option<i32>,
    id: &str,
) -> Result<Result<(Vec<u8>, Option<String>), bool>, ApiError> {
    let mut exists = false;
    if let Some(rec) = s
        .store
        .record_get(id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
    {
        exists = true;
        if me.is_some_and(|m| rec.members.contains(&m)) {
            let mid = rec.meta.as_ref().map(|m| m.id.clone());
            return Ok(Ok((rec.record, mid)));
        }
    }
    let list = s
        .store
        .archive_list(id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?;
    let Some(me) = me else {
        return Ok(Err(exists || !list.is_empty()));
    };
    for meta in &list {
        if !meta.members.contains(&me) {
            continue;
        }
        if let Some(rec) = s
            .store
            .archive_get(id, &meta.id)
            .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        {
            return Ok(Ok((rec.record, Some(rec.meta.id))));
        }
    }
    Ok(Err(exists || !list.is_empty()))
}

/// `GET /api/rooms/{id}/record` -- the last finished match's `.bdrec`.
///
/// The full record reveals every hand and the deck order, so it is only served
/// **after** the match ends, and only to the people who sat down for it
/// (`docs/REPLAY.md` §1). `409 err.record.live` while the room is playing,
/// `404 err.record.none` when there is nothing sealed yet, `403
/// err.record.forbidden` to anyone the room's tokens do not put in the match.
///
/// The store's 24 h copy is used first; past it (or after the next match in
/// the room replaced it) the durable archive serves the newest match the caller
/// sat in.
pub async fn record(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    // `playing` is the store's flag (any instance may have started the match).
    let playing = s
        .store
        .room_get(&id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        .is_some_and(|rec| rec.info.playing);
    if playing {
        return Err(ApiError::new(StatusCode::CONFLICT, "err.record.live"));
    }
    let me = caller_member(&s, &sess, &id);
    match caller_record(&s, me, &id)? {
        Ok((bytes, mid)) => Ok(download(&id, mid.as_deref(), bytes)),
        Err(false) => Err(ApiError::not_found("err.record.none")),
        Err(true) => Err(ApiError::forbidden("err.record.forbidden")),
    }
}

/// `GET /api/rooms/{id}/records` -- the room's archived matches, newest first,
/// narrowed to the ones the caller sat in. Metadata only (no hands); the
/// download for one is `GET /api/rooms/{id}/records/{match}`. An empty archive
/// answers `{ "records": [] }`; an archive the caller never sat in is `403`.
pub async fn records(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let me = caller_member(&s, &sess, &id);
    let list = s
        .store
        .archive_list(&id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?;
    if list.is_empty() {
        return Ok(Json(serde_json::json!({ "records": [] })));
    }
    let Some(me) = me else {
        return Err(ApiError::forbidden("err.record.forbidden"));
    };
    let mine: Vec<&crate::archive::ArchiveMeta> =
        list.iter().filter(|m| m.members.contains(&me)).collect();
    if mine.is_empty() {
        return Err(ApiError::forbidden("err.record.forbidden"));
    }
    Ok(Json(serde_json::json!({ "records": mine })))
}

/// `GET /api/rooms/{id}/records/{match}` -- one archived match, participants
/// only. `404 err.record.none` for an unknown id.
pub async fn archived_record(
    State(s): S,
    Auth(sess): Auth,
    Path((id, match_id)): Path<(String, String)>,
) -> ApiResult<Response> {
    let me = caller_member(&s, &sess, &id);
    let rec = s
        .store
        .archive_get(&id, &match_id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        .ok_or_else(|| ApiError::not_found("err.record.none"))?;
    if !me.is_some_and(|m| rec.meta.members.contains(&m)) {
        return Err(ApiError::forbidden("err.record.forbidden"));
    }
    Ok(download(&id, Some(&rec.meta.id), rec.record))
}
