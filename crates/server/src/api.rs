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
use crate::state::{random_hex, random_u64, room_code, Server, Session};

type S = State<Arc<Server>>;

// ------------------------------------------------------------------ session

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionReq {
    pub player: String,
    pub character: String,
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
pub async fn create_session(
    State(s): S,
    Json(req): Json<SessionReq>,
) -> ApiResult<impl IntoResponse> {
    let player = net::clean_name(&req.player);
    if player.is_empty() {
        return Err(ApiError::bad("err.name_required"));
    }
    let session = Session {
        token: random_hex(32),
        player,
        character: req.character,
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
    let rooms = s.rooms.lock().unwrap().len();
    Json(serde_json::json!({ "game": net::GAME, "version": net::VERSION, "rooms": rooms }))
}

// ------------------------------------------------------------------ rooms

/// `GET /api/rooms` -- the lobby list (passwords never leave the server).
pub async fn list_rooms(State(s): S) -> Json<Vec<RoomInfo>> {
    let rooms: Vec<Arc<Mutex<Room>>> = s.rooms.lock().unwrap().values().cloned().collect();
    let mut list: Vec<RoomInfo> = rooms
        .iter()
        .filter_map(|r| {
            let r = r.lock().unwrap();
            r.dissolved.is_none().then(|| r.info.clone())
        })
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
}

impl Default for CreateRoom {
    fn default() -> Self {
        Self {
            name: String::new(),
            ranked: false,
            max_players: 6,
            password: String::new(),
            weights: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Joined {
    pub room: RoomInfo,
    pub you: i32,
}

fn rules_weights(s: &Server) -> ScoreWeights {
    ScoreWeights::from_rules(&s.data.match_rules)
}

/// Leave whatever room the session is in.
fn leave_current(s: &Server, sess: &Session) {
    let Some((rid, member)) = &sess.room else {
        return;
    };
    if let Some(room) = s.room(rid) {
        // Mid-match, leaving forfeits: run that on the handle first, with the
        // room lock dropped, then take it again for the room bookkeeping.
        let m = room.lock().unwrap().match_handle();
        if let Some(m) = m {
            let _ = m.act(*member, &game_core::net::NetMessage::act("leave"));
        }
        let dissolved = {
            let mut r = room.lock().unwrap();
            r.leave(*member);
            r.dissolved.is_some()
        };
        if dissolved {
            s.rooms.lock().unwrap().remove(rid);
        }
    }
    s.set_room(&sess.token, None);
}

/// `POST /api/rooms` -- create a room and join it as host.
pub async fn create_room(
    State(s): S,
    Auth(sess): Auth,
    Json(req): Json<CreateRoom>,
) -> ApiResult<Json<Joined>> {
    leave_current(&s, &sess);
    let id = loop {
        let id = room_code();
        if s.room(&id).is_none() {
            break id;
        }
    };
    // An empty name is shown by clients as "<host>'s room" in their own language.
    let name = req.name.trim().to_string();
    let defaults = rules_weights(&s);
    let weights = req.weights.unwrap_or(defaults).sanitized(defaults);
    let mut room = Room::new(
        id.clone(),
        &name,
        req.ranked,
        req.max_players,
        &req.password,
        weights,
        s.engine.clone(),
        s.store.clone(),
    );
    // Advanced bots need the bot-service; without one they play as standard
    // (`docs/BOT.md` B5, `Room::start`).
    room.bot_search = s.bots.is_some();
    let who = NewMember {
        token: &sess.token,
        player: &sess.player,
        character: &sess.character,
        cn_id: &sess.cn_id,
    };
    let you = room.join(who, &req.password, "")?;
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
}

/// `POST /api/rooms/{id}/join` -- join (or rejoin) a room.
pub async fn join_room(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(req): Json<JoinReq>,
) -> ApiResult<Json<Joined>> {
    let room = s
        .room(&id)
        .ok_or_else(|| ApiError::not_found("err.room.gone"))?;
    if sess.room.as_ref().is_some_and(|r| r.0 != id) {
        leave_current(&s, &sess);
    }
    let (you, info) = {
        let mut r = room.lock().unwrap();
        if r.dissolved.is_some() {
            return Err(ApiError::not_found("err.room.gone"));
        }
        let who = NewMember {
            token: &sess.token,
            player: &sess.player,
            character: &sess.character,
            cn_id: &sess.cn_id,
        };
        let you = r.join(who, &req.password, &req.version)?;
        (you, r.info.clone())
    };
    s.set_room(&sess.token, Some((id, you)));
    Ok(Json(Joined { room: info, you }))
}

/// The session's room and member id, checked against `id`.
pub fn member_room(s: &Server, sess: &Session, id: &str) -> ApiResult<(Arc<Mutex<Room>>, i32)> {
    let (rid, member) = sess
        .room
        .clone()
        .ok_or_else(|| ApiError::bad("err.room.not_member"))?;
    if rid != id {
        return Err(ApiError::bad("err.room.not_member"));
    }
    let room = s
        .room(id)
        .ok_or_else(|| ApiError::not_found("err.room.gone"))?;
    if room.lock().unwrap().member_of(&sess.token) != Some(member) {
        return Err(ApiError::bad("err.room.not_member"));
    }
    Ok((room, member))
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
    let (room, me) = member_room(&s, &sess, &id)?;
    let mut r = room.lock().unwrap();
    r.set_ready(me, req.on)?;
    Ok(Json(r.info.clone()))
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
    let (room, me) = member_room(&s, &sess, &id)?;
    let mut r = room.lock().unwrap();
    match req.op.as_str() {
        "add" => {
            let m = game_core::state::BotMentality::parse(&req.mentality)
                .ok_or_else(|| ApiError::bad("err.bad_mentality"))?;
            r.add_bot(me, &s.data.match_rules.bot_names, m)?
        }
        "remove" => r.remove_bot(me, req.member)?,
        _ => return Err(ApiError::bad("err.bad_bot_op")),
    }
    Ok(Json(r.info.clone()))
}

pub async fn weights(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(w): Json<ScoreWeights>,
) -> ApiResult<Json<RoomInfo>> {
    let (room, me) = member_room(&s, &sess, &id)?;
    let defaults = rules_weights(&s);
    let mut r = room.lock().unwrap();
    r.set_weights(me, w, defaults)?;
    Ok(Json(r.info.clone()))
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
    let (room, me) = member_room(&s, &sess, &id)?;
    // The tick quantum this match will run on, sealed into its record header
    // so a replay reproduces the same `tick(dt)` f32s (docs/REPLAY.md §1).
    let step = crate::TICK_QUANTUM * s.time_scale;
    let info = tokio::task::spawn_blocking(move || {
        let mut r = room.lock().unwrap();
        r.start(me, req.force, random_u64(), step)?;
        Ok::<_, ApiError>(r.info.clone())
    })
    .await
    .map_err(|e| ApiError::bad(format!("start task: {e}").as_str()))??;
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
}

/// Ask the worker for one member's view. Blocking; see `pool`.
pub fn match_view(m: &crate::room::MatchHandle, member: i32) -> ApiResult<MatchView> {
    let v = m.view(member).map_err(|e| ApiError::bad(e.as_str()))?;
    serde_json::from_value(v).map_err(|e| ApiError::bad(format!("view: {e}").as_str()))
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
    let (room, me) = member_room(&s, &sess, &id)?;
    // Check the match out and drop the room lock: the round-trip must not hold
    // it. The view is a worker call, so it also runs off the async runtime.
    let (info, m) = {
        let r = room.lock().unwrap();
        (r.info.clone(), r.match_handle())
    };
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

/// `POST /api/rooms/{id}/act` -- a match command (`NetMessage` with `act` set).
pub async fn act(
    State(s): S,
    Auth(sess): Auth,
    Path(id): Path<String>,
    Json(msg): Json<NetMessage>,
) -> ApiResult<Json<serde_json::Value>> {
    let (room, me) = member_room(&s, &sess, &id)?;
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
    // Check the match out, drop the room lock, then run the command.
    let m = {
        let r = room.lock().unwrap();
        r.match_handle()
            .ok_or_else(|| ApiError::bad("err.room.no_match"))?
    };
    let err = tokio::task::spawn_blocking(move || m.act(me, &msg))
        .await
        .map_err(|e| ApiError::bad(format!("act task: {e}").as_str()))?
        .map_err(|e| ApiError::bad(e.as_str()))?;
    if let Some(e) = err {
        return Err(ApiError::bad(e));
    }
    room.lock().unwrap().notify();
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
    let room = s.room(&id);
    if let Some(room) = &room {
        if room.lock().unwrap().info.playing {
            return Err(ApiError::new(StatusCode::CONFLICT, "err.record.live"));
        }
    }
    let rec = s
        .store
        .record_get(&id)
        .map_err(|e| ApiError::bad(e.to_string().as_str()))?
        .ok_or_else(|| ApiError::not_found("err.record.none"))?;
    // "whoever the room's tokens put in this match" -- a member id, checked
    // against the seats the record was sealed for.
    let me = room
        .as_ref()
        .and_then(|r| r.lock().unwrap().member_of(&sess.token));
    let me = me.or_else(|| match &sess.room {
        Some((rid, member)) if *rid == id => Some(*member),
        _ => None,
    });
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
