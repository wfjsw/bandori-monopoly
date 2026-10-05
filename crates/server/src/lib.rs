//! Dedicated server: HTTP GET/POST for commands, Server-Sent Events for updates.
//!
//! | Method | Path | Body | Response |
//! |---|---|---|---|
//! | GET  | `/api/health` | | `{game, version, rooms}` |
//! | POST | `/api/session` | `{player, character?, cnId?}` | `{token, player}` + cookie |
//! | GET  | `/api/session` | | `{token, player, room, member}` |
//! | GET  | `/api/rooms` | | `RoomInfo[]` |
//! | POST | `/api/rooms` | `{name, ranked, maxPlayers, password, weights?}` | `{room, you}` |
//! | POST | `/api/rooms/{id}/join` | `{password, version?}` | `{room, you}` or 403/409 `{error, reason}` |
//! | POST | `/api/rooms/{id}/ready` | `{on}` | `RoomInfo` |
//! | POST | `/api/rooms/{id}/bots` | `{op: "add"\|"remove", member?}` | `RoomInfo` (host) |
//! | POST | `/api/rooms/{id}/weights` | `ScoreWeights` | `RoomInfo` (host) |
//! | POST | `/api/rooms/{id}/start` | `{force}` | `RoomInfo` (host) |
//! | POST | `/api/rooms/{id}/leave` | | `{ok}` |
//! | GET  | `/api/rooms/{id}/state` | | `{room, you, match: {state, hand, handNotes, you, player_id}}` |
//! | POST | `/api/rooms/{id}/act` | `NetMessage` (`act`, `card`, `cards`, `value`, `prompt`, ...) | `{ok}` or 400 `{error}` |
//! | GET  | `/api/rooms/{id}/stream` | | SSE, see [`sse`] |
//!
//! Errors are `{error, reason}` with the original game's Chinese messages; `reason`
//! is set for join rejections (`password`, `full`, `playing`, `version`).

pub mod api;
pub mod auth;
pub mod error;
pub mod room;
pub mod sse;
pub mod state;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;

pub use state::Server;

/// Match ticks per second.
pub const TICK_HZ: u64 = 20;

pub fn router(server: Arc<Server>, data_dir: Option<PathBuf>, static_dir: Option<PathBuf>) -> Router {
    let mut app = Router::new()
        .route("/api/health", get(api::health))
        .route("/api/session", post(api::create_session).get(api::get_session))
        .route("/api/rooms", get(api::list_rooms).post(api::create_room))
        .route("/api/rooms/{id}/join", post(api::join_room))
        .route("/api/rooms/{id}/ready", post(api::ready))
        .route("/api/rooms/{id}/bots", post(api::bots))
        .route("/api/rooms/{id}/weights", post(api::weights))
        .route("/api/rooms/{id}/start", post(api::start))
        .route("/api/rooms/{id}/leave", post(api::leave))
        .route("/api/rooms/{id}/state", get(api::room_state))
        .route("/api/rooms/{id}/act", post(api::act))
        .route("/api/rooms/{id}/stream", get(sse::stream))
        .layer(DefaultBodyLimit::max(game_core::net::MAX_MESSAGE))
        .with_state(server);
    if let Some(dir) = data_dir {
        app = app.nest_service("/data", ServeDir::new(dir));
    }
    if let Some(dir) = static_dir {
        // The web client routes with the History API (/menu, /room/<id>, /play/solo):
        // any page path that is not a file gets index.html so a refresh lands back
        // on the same screen.
        let index = get(spa_index).with_state(dir.join("index.html"));
        app = app.fallback_service(ServeDir::new(dir).fallback(index));
    }
    app
}

/// `index.html` for client-side routes; a real 404 for missing files and API paths.
async fn spa_index(State(index): State<PathBuf>, uri: Uri) -> Response {
    let path = uri.path();
    let file_like = path.rsplit('/').next().is_some_and(|last| last.contains('.'));
    if file_like || path.starts_with("/api/") || path.starts_with("/data/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(&index).await {
        Ok(html) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], html).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Advance every room by `dt` and apply presence time-outs.
pub fn tick_all(server: &Server, dt: f32) {
    let rooms: Vec<(String, Arc<std::sync::Mutex<room::Room>>)> =
        server.rooms.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut cleared = vec![];
    let mut dead = vec![];
    for (id, room) in rooms {
        let mut r = room.lock().unwrap();
        cleared.extend(r.tick(dt, server.presence_timeout));
        if r.dissolved.is_some() {
            dead.push(id);
        }
    }
    for token in cleared {
        server.set_room(&token, None);
    }
    if !dead.is_empty() {
        let mut rooms = server.rooms.lock().unwrap();
        for id in dead {
            rooms.remove(&id);
        }
    }
}

/// Run [`tick_all`] at [`TICK_HZ`] forever.
pub fn spawn_ticker(server: Arc<Server>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(Duration::from_millis(1000 / TICK_HZ));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last = Instant::now();
        loop {
            iv.tick().await;
            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.5);
            last = now;
            tick_all(&server, dt * server.time_scale);
        }
    })
}
