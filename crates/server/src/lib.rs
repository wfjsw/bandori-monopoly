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
//! | GET  | `/api/rooms/{id}/record` | | the last finished match's `.bdrec`, participants only |
//! | GET  | `/api/rooms/{id}/stream` | | SSE, see [`sse`] |
//!
//! Errors are `{error, reason}` with the original game's Chinese messages; `reason`
//! is set for join rejections (`password`, `full`, `playing`, `version`).

pub mod api;
pub mod auth;
pub mod error;
pub mod pool;
pub mod room;
pub mod sse;
pub mod state;
pub mod store;

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

pub fn router(
    server: Arc<Server>,
    data_dir: Option<PathBuf>,
    static_dir: Option<PathBuf>,
) -> Router {
    let mut app = Router::new()
        .route("/api/health", get(api::health))
        .route(
            "/api/session",
            post(api::create_session).get(api::get_session),
        )
        .route("/api/rooms", get(api::list_rooms).post(api::create_room))
        .route("/api/rooms/{id}/join", post(api::join_room))
        .route("/api/rooms/{id}/ready", post(api::ready))
        .route("/api/rooms/{id}/bots", post(api::bots))
        .route("/api/rooms/{id}/weights", post(api::weights))
        .route("/api/rooms/{id}/start", post(api::start))
        .route("/api/rooms/{id}/leave", post(api::leave))
        .route("/api/rooms/{id}/state", get(api::room_state))
        .route("/api/rooms/{id}/act", post(api::act))
        .route("/api/rooms/{id}/record", get(api::record))
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
    let file_like = path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'));
    if file_like || path.starts_with("/api/") || path.starts_with("/data/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(&index).await {
        Ok(html) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            html,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Advance every room by one tick quantum and apply presence time-outs.
/// Advance every room. The match clock is a worker round-trip, so each room is
/// checked out of its lock first and the round-trip runs with nothing held --
/// then the rooms are fanned out over `spawn_blocking` and run in parallel.
///
/// `dt` is the game time this call advances and `k` how many whole quanta of
/// `header.step` that is (see [`spawn_ticker`]); the record logs `k`, so a
/// replay recomputes the identical f32.
pub async fn tick_all(server: &Arc<Server>, dt: f32, k: u8) {
    let rooms: Vec<(String, Arc<std::sync::Mutex<room::Room>>)> = server
        .rooms
        .lock()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let timeout = server.presence_timeout;
    let mut cleared: Vec<String> = vec![];
    let mut dead = vec![];
    let mut jobs = Vec::with_capacity(rooms.len());
    for (id, room) in rooms {
        jobs.push(tokio::task::spawn_blocking(move || {
            // 1. presence and check-out -- the room lock is held only here.
            let (m, playing, dropped, removed) = {
                let mut r = room.lock().unwrap();
                let m = r.match_handle();
                let playing = r.info.playing;
                let (dropped, removed) = r.tick_presence(timeout);
                (m, playing, dropped, removed)
            };
            // 2. the round-trips, with no lock held.
            let mut changed = false;
            let mut ended = false;
            if let Some(m) = &m {
                for d in &dropped {
                    if let Err(e) = m.member_left(*d, true) {
                        eprintln!("room {id} member_left failed: {e}");
                    }
                    changed = true;
                }
                if playing {
                    match m.tick(dt, k) {
                        Ok(c) => changed |= c,
                        Err(e) => eprintln!("room {id} tick failed: {e}"),
                    }
                    ended = m.ended().unwrap_or(false);
                }
            }
            // 3. bookkeeping -- short lock again.
            let mut r = room.lock().unwrap();
            if ended {
                r.info.playing = false;
                changed = true;
            }
            if changed {
                r.notify();
            }
            (id, removed, r.dissolved.is_some())
        }));
    }
    for job in jobs {
        match job.await {
            Ok((id, removed, dissolved)) => {
                cleared.extend(removed);
                if dissolved {
                    dead.push(id);
                }
            }
            Err(e) => eprintln!("tick task failed: {e}"),
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

/// One tick quantum of **wall** time, the unit the accumulator counts in. At
/// `TICK_HZ` that is one interval, so `k` is 1 unless the loop stalled.
pub const TICK_QUANTUM: f32 = 0.05;

/// Drive every room's match on a fixed-step accumulator (`docs/REPLAY.md` §1).
///
/// `k = floor(acc / TICK_QUANTUM).min(10)` whole quanta are owed to the engine;
/// `k == 0` skips the round-trip entirely. Each quantum is
/// `TICK_QUANTUM * time_scale` of game time, so the record's `step` is that
/// same number and a replay feeds the engine the identical `tick(dt)` f32s.
/// The `min(10)` bound is what keeps one long stall from making a single
/// enormous step.
pub fn spawn_ticker(server: Arc<Server>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(Duration::from_millis(1000 / TICK_HZ));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last = Instant::now();
        let mut acc = 0.0f32;
        loop {
            iv.tick().await;
            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.5);
            last = now;
            acc += dt;
            let k = (acc / TICK_QUANTUM).floor().clamp(0.0, 10.0) as u8;
            if k == 0 {
                continue;
            }
            acc -= k as f32 * TICK_QUANTUM;
            let step = TICK_QUANTUM * server.time_scale;
            tick_all(&server, k as f32 * step, k).await;
        }
    })
}
