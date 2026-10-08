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
pub mod botsvc;
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
        let server = server.clone();
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
            // Advanced seats the bot service should be asked about (claimed,
            // so the next tick does not double-schedule them).
            let mut drive: Vec<i32> = Vec::new();
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
                    drive = claim_advanced(&server, &id);
                }
            }
            // 3. bookkeeping -- short lock again.
            let mut r = room.lock().unwrap();
            if ended {
                r.info.playing = false;
                changed = true;
                server.bot_drive.lock().unwrap().forget_room(&id);
            }
            if changed {
                r.notify();
            }
            (id, removed, r.dissolved.is_some(), drive, m)
        }));
    }
    for job in jobs {
        match job.await {
            Ok((id, removed, dissolved, drive, m)) => {
                cleared.extend(removed);
                if dissolved {
                    server.bot_drive.lock().unwrap().forget_room(&id);
                    dead.push(id.clone());
                }
                // 4. ask the bot service -- its own tasks, never the tick.
                if let (Some(bots), Some(m)) = (server.bots.clone(), m) {
                    for member in drive {
                        spawn_probe(server.clone(), bots.clone(), id.clone(), member, m.clone());
                    }
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

// ------------------------------------------------------- advanced bots (B5)

/// Members of `room` that the bot service drives: bot seats tagged
/// [`BotMentality::Advanced`]. Claims each one whose probe time is due, so a
/// seat is only ever asked about once (`docs/BOT.md` B5).
fn claim_advanced(server: &Arc<Server>, room_id: &str) -> Vec<i32> {
    if server.bots.is_none() {
        return Vec::new();
    }
    let members: Vec<i32> = {
        let Some(room) = server.room(room_id) else {
            return Vec::new();
        };
        let r = room.lock().unwrap();
        r.info
            .members
            .iter()
            .filter(|m| m.bot && m.mentality == game_core::state::BotMentality::Advanced)
            .map(|m| m.id)
            .collect()
    };
    if members.is_empty() {
        return Vec::new();
    }
    let now = Instant::now();
    let mut drive = server.bot_drive.lock().unwrap();
    members
        .into_iter()
        .filter(|&m| drive.claim(room_id, m, now))
        .collect()
}

/// One decision probe: look at the seat's view, and if it must act, ask the
/// bot service and apply the answer (or the heuristic on failure). Runs as
/// its own task -- the tick loop is already on to the next room.
fn spawn_probe(
    server: Arc<Server>,
    bots: Arc<botsvc::BotService>,
    room_id: String,
    member: i32,
    m: Arc<room::MatchHandle>,
) {
    tokio::spawn(async move {
        let acted = probe_one(&server, &bots, &room_id, member, &m).await;
        server
            .bot_drive
            .lock()
            .unwrap()
            .release(&room_id, member, acted);
    });
}

async fn probe_one(
    server: &Arc<Server>,
    bots: &Arc<botsvc::BotService>,
    room_id: &str,
    member: i32,
    m: &Arc<room::MatchHandle>,
) -> bool {
    // The view is the exact frame the client gets (`MatchHandle::view`), so
    // the service never sees anything a player would not (§1).
    let view = {
        let m = m.clone();
        match tokio::task::spawn_blocking(move || m.view(member)).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                eprintln!("room {room_id} bot view failed: {e}");
                return false;
            }
            Err(e) => {
                eprintln!("room {room_id} bot view task: {e}");
                return false;
            }
        }
    };
    let Ok(seat) = serde_json::from_value::<bot_service::BotSeatView>(view.clone()) else {
        return false;
    };
    let state = &seat.state;
    let Some(prompt_id) = botsvc::decision_at(state, seat.player_id) else {
        // Nothing this seat must answer right now -- another seat's turn or
        // prompt. Speculate on the view instead (BOT-RESEARCH #5): non-blocking
        // and rate-limited by the idle probe cadence, one in flight per seat,
        // cancelled when this seat's own decision arrives.
        if state.phase == "play" {
            spawn_ponder(server, bots, room_id, member, view, state.seq);
        }
        return false;
    };
    // The seat's own decision supersedes any speculative search in flight.
    server.bot_drive.lock().unwrap().cancel_ponder(room_id, member);
    let budget = server
        .bot_budget_ms
        .unwrap_or_else(|| botsvc::budget_ms(state, prompt_id));
    // Search-side seed: derived from the public decision identity, never the
    // match RNG (§1).
    let seed = decision_seed(room_id, member, state.seq, prompt_id);
    // Outer deadline: the search budget plus room for one iteration overrun
    // and queue / apply (`docs/BOT.md` §5 B7 -- a real-ruleset iteration
    // finishes after the budget and must still land inside here).
    let timeout = server
        .bot_ask_timeout
        .unwrap_or_else(|| botsvc::ask_timeout(budget));
    let answer = match bots
        .decide(room_id, member, &view, prompt_id, budget, seed, timeout)
        .await
    {
        Ok(a) => {
            eprintln!(
                "room {room_id} bot {member}: {} iters {} ms (budget {budget}, reused {})",
                a.iterations, a.elapsed_ms, a.reused
            );
            a.answer
        }
        Err(e) => {
            // Timeout / crash / bad reply: the engine's own heuristic answers
            // this decision and the match keeps moving (§1).
            eprintln!("room {room_id} bot {member}: {e} -- falling back to heuristic");
            bot_service::heuristic_message_view(&server.data, &seat)
        }
    };
    let m = m.clone();
    apply_bot_answer(bots, &server.data, &seat, member, answer, move |cmd| {
        let m = m.clone();
        async move {
            match tokio::task::spawn_blocking(move || m.act(member, &cmd)).await {
                Ok(r) => r,
                Err(e) => Err(format!("act task: {e}")),
            }
        }
    })
    .await
}

/// Apply the service's answer to one seat. On an engine refusal, drop the
/// cached answer for this decision and apply the heuristic instead -- the
/// drive must never re-ask after a refusal (`docs/BOT.md` §5 B6). Without the
/// fallback the next probe re-sent the same cached refusal every 200 ms until
/// the turn bank expired (bot_cpu §6, 45–54% stall). Returns whether the seat
/// ended up acting.
///
/// `act` applies one command and returns the engine's refusal (`Ok(Some)`),
/// success (`Ok(None)`) or a failure. It runs at most twice: the service's
/// answer, then -- only on a refusal -- the heuristic.
pub async fn apply_bot_answer<F, Fut>(
    bots: &botsvc::BotService,
    data: &Arc<game_core::data::GameData>,
    seat: &bot_service::BotSeatView,
    member: i32,
    answer: game_core::net::NetMessage,
    mut act: F,
) -> bool
where
    F: FnMut(game_core::net::NetMessage) -> Fut,
    Fut: std::future::Future<Output = Result<Option<game_core::msg::Msg>, String>>,
{
    match act(answer).await {
        Ok(None) => true,
        Ok(Some(msg)) => {
            eprintln!("bot {member}: act refused: {msg:?}");
            bots.invalidate(seat.decision_key()).await;
            let fallback = bot_service::heuristic_message_view(data, seat);
            match act(fallback).await {
                Ok(None) => true,
                Ok(Some(msg2)) => {
                    eprintln!("bot {member}: heuristic refused too: {msg2:?}");
                    false
                }
                Err(e) => {
                    eprintln!("bot {member}: fallback act failed: {e}");
                    false
                }
            }
        }
        Err(e) => {
            eprintln!("bot {member}: act failed: {e}");
            false
        }
    }
}

/// Fire-and-forget `op: "ponder"` for an idle advanced seat (BOT-RESEARCH #5).
/// The tick / probe returns immediately -- the search runs on its own task and
/// only lands in the service's cache. One in flight per seat; when the seat's
/// own decision arrives the slot is cancelled / ignored and the real `decide`
/// is what counts (it then hits the service's cache when the decision key
/// matches).
fn spawn_ponder(
    server: &Arc<Server>,
    bots: &Arc<botsvc::BotService>,
    room_id: &str,
    member: i32,
    view: serde_json::Value,
    seq: i32,
) {
    let token = {
        let mut drive = server.bot_drive.lock().unwrap();
        match drive.claim_ponder(room_id, member) {
            Some(t) => t,
            None => return, // one in flight per seat
        }
    };
    let server = server.clone();
    let bots = bots.clone();
    let room_id = room_id.to_string();
    tokio::spawn(async move {
        let budget = server
            .bot_ponder_budget_ms
            .unwrap_or(botsvc::PONDER_BUDGET_MS);
        let seed = decision_seed(&room_id, member, seq, -1);
        // Nobody waits on this; the timeout only bounds the task's lifetime.
        let timeout = botsvc::ask_timeout(budget);
        match bots
            .ponder(&room_id, member, &view, budget, seed, timeout)
            .await
        {
            Ok(reused) => {
                if reused {
                    eprintln!("room {room_id} bot {member}: ponder hit the cache");
                }
            }
            // A ponder is speculative: a slow / missing reply is never a
            // match failure, so it is not logged as one.
            Err(_) => {}
        }
        server
            .bot_drive
            .lock()
            .unwrap()
            .release_ponder(&room_id, member, token);
    });
}

/// A stable search-side seed. Mixes the public decision identity with a
/// counter so two identical decisions in one match still get different
/// samples; never derived from the match's RNG.
fn decision_seed(room: &str, member: i32, seq: i32, prompt_id: i32) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    room.hash(&mut h);
    member.hash(&mut h);
    seq.hash(&mut h);
    prompt_id.hash(&mut h);
    N.fetch_add(1, Ordering::Relaxed).hash(&mut h);
    h.finish()
}
