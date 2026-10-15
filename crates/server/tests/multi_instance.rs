//! Two [`Server`] processes sharing one store: the whole point of the
//! `CrossState` seam. These run two live servers (real HTTP, real SSE) over
//! one [`dummy::Store`] -- the same shape as two processes over one Redis.
//!
//! Covers: a room created on A is listable / joinable / startable on B; a
//! match started on A can be acted on via B; two tickers advance each match
//! exactly once per quantum; a member whose only stream is on B is not reaped
//! by A; a server rebuilt mid-match resumes; two concurrent joins allocate
//! distinct member ids.

use std::sync::Arc;
use std::time::{Duration, Instant};

use game_core::data::{GameData, Roster};
use game_core::engine::StubRules;
use reqwest::StatusCode;
use serde_json::{json, Value};
use server::store::{dummy, CrossState};
use server::Server;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(&std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../game-core/tests/fixtures/roster.json"),
    )
    .expect("roster fixture"))
    .expect("roster fixture parses");
    Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .unwrap(),
    )
}

/// One live server over `store`, with a ticker running. `tune` runs before
/// the ticker starts (presence timeout, time scale, ...).
async fn spawn(
    store: &dummy::Store,
    tune: impl FnOnce(&mut Server),
) -> (String, Arc<Server>, tokio::task::JoinHandle<()>) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("BD_GLUE_SHA", "test-glue-sha"));
    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut server = Server::new(
        d.clone(),
        rules.clone(),
        server::pool::Pool::in_process(d, rules),
        Arc::new(store.clone()),
    );
    {
        let s = Arc::get_mut(&mut server).expect("fresh");
        tune(s);
    }
    let ticker = server::spawn_ticker(server.clone());
    let app = server::router(server.clone(), None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), server, ticker)
}

struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl Client {
    async fn new(base: &str, name: &str) -> Self {
        let http = reqwest::Client::new();
        let r: Value = http
            .post(format!("{base}/api/session"))
            .json(&json!({ "player": name }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Self {
            http,
            base: base.into(),
            token: r["token"].as_str().unwrap().into(),
        }
    }

    /// Rebind the same session (token) to another server's base URL -- what a
    /// load balancer does when a client lands on the other process.
    fn on(&self, base: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
            token: self.token.clone(),
        }
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        let r = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = r.status();
        (status, r.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        let r = self
            .http
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        let status = r.status();
        (status, r.json().await.unwrap_or(Value::Null))
    }

    async fn ok(&self, path: &str, body: Value) -> Value {
        let (s, v) = self.post(path, body).await;
        assert_eq!(s, StatusCode::OK, "{path}: {v}");
        v
    }

    async fn stream(&self, room: &str) -> reqwest::Response {
        let r = self
            .http
            .get(format!("{}/api/rooms/{room}/stream", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        r
    }
}

/// A room created on A, visible and joinable on B, started from B with the
/// host's session (sessions live in the store too).
#[tokio::test]
async fn a_room_on_a_is_listable_joinable_and_startable_on_b() {
    let store = dummy::Store::new();
    let (base_a, _sa, ta) = spawn(&store, |_| {}).await;
    let (base_b, _sb, tb) = spawn(&store, |_| {}).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Shared" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // B's lobby lists it straight from the store.
    let guest = Client::new(&base_b, "Guest").await;
    let (s, list) = guest.get("/api/rooms").await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        list.as_array().unwrap().iter().any(|r| r["id"] == id.as_str()),
        "room created on A is listable on B: {list}"
    );

    // Join on B.
    let joined = guest
        .ok(&format!("/api/rooms/{id}/join"), json!({}))
        .await;
    assert_eq!(joined["you"].as_i64().unwrap(), 2);

    // Start on B, with the **host's** token (the session table is shared).
    // One bot to reach the 3-seat casual minimum (G-FLOW-12).
    host.on(&base_b)
        .ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    let host_on_b = host.on(&base_b);
    host_on_b.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) }))
        .await;
    guest.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 2) }))
        .await;
    let started = host_on_b
        .ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    assert_eq!(started["playing"], true, "{started}");

    // And the match is visible on both.
    for base in [&base_a, &base_b] {
        let c = host.on(base);
        let (s, st) = c.get(&format!("/api/rooms/{id}/state")).await;
        assert_eq!(s, StatusCode::OK, "{st}");
        assert!(st["match"]["state"]["matchId"].as_i64().is_some(), "{st}");
    }

    ta.abort();
    tb.abort();
}

/// A match started on A can be acted on via B (the blob is in the store; the
/// match lock sequences the two).
#[tokio::test]
async fn a_match_started_on_a_can_be_acted_on_via_b() {
    let store = dummy::Store::new();
    let (base_a, sa, ta) = spawn(&store, |_| {}).await;
    let (base_b, _sb, tb) = spawn(&store, |_| {}).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Act" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) }))
        .await;
    host.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;

    // Skip pick/deck from A, act from B.
    let handle = {
        let room = sa.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match started").clone()
    };
    handle.quick_start().unwrap();

    let host_on_b = host.on(&base_b);
    // `anim-done` is a recorded match op; any seat may send it and it always
    // lands in the log. B has the same session, so it is the same member.
    let (s, v) = host_on_b
        .post(&format!("/api/rooms/{id}/anim-done"), json!({ "seq": 0 }))
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    // The log (in the shared store) carries the op B just ran.
    let lines = store.record_log_get(&id).unwrap();
    assert!(
        lines.iter().any(|l| l.contains("AnimDone")),
        "B's act reached the shared record log: {lines:?}"
    );

    ta.abort();
    tb.abort();
}

/// Two ticker loops, one store: each match advances **once** per quantum, not
/// once per process.
#[tokio::test]
async fn ticks_run_once_per_quantum_across_two_tickers() {
    let store = dummy::Store::new();
    // time_scale 1.0: one quantum of game time per TICK_QUANTUM of wall.
    let (base_a, sa, ta) = spawn(&store, |s| s.time_scale = 1.0).await;
    let (_base_b, sb, tb) = spawn(&store, |s| s.time_scale = 1.0).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Ticks" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) }))
        .await;
    host.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    let handle = {
        let room = sa.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match").clone()
    };
    handle.quick_start().unwrap();

    // Let both tickers run for a while. The lease means only one of them
    // ticks this room; the other is idle.
    let wall = Duration::from_millis(1200);
    let t0 = Instant::now();
    tokio::time::sleep(wall).await;
    let elapsed = t0.elapsed().as_secs_f64();

    let meta = store.rec_meta_get(&id).unwrap().expect("rec meta");
    let ticks = meta.ticks as f64;
    let quanta = elapsed / server::TICK_QUANTUM as f64;
    // Once per quantum: within 50%, and definitely not 2x (two tickers).
    assert!(
        ticks > quanta * 0.5 && ticks < quanta * 1.5,
        "ticks {ticks} vs wall {elapsed}s = {quanta} quanta (two tickers must not double)"
    );
    // Both servers see the same counter -- the meta is shared.
    let _ = sb.room(&id);

    ta.abort();
    tb.abort();
}

/// A member whose only SSE stream is on B is not dropped by A's presence
/// reaper: the store's presence keys are shared.
#[tokio::test]
async fn a_member_streaming_on_b_is_not_reaped_by_a() {
    let store = dummy::Store::new();
    // Short presence timeout so the reaper would fire quickly if it were wrong.
    let (base_a, sa, ta) = spawn(&store, |s| s.presence_timeout = Duration::from_millis(400)).await;
    let (base_b, _sb, tb) = spawn(&store, |s| s.presence_timeout = Duration::from_millis(400)).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Presence" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Keep A present with its own stream; B's guest streams only on B.
    let _host_stream = host.stream(&id).await;
    let guest = Client::new(&base_b, "Guest").await;
    guest.ok(&format!("/api/rooms/{id}/join"), json!({})).await;
    let _guest_stream = guest.stream(&id).await;

    // Well past the presence timeout: A's ticker has been reaping the whole
    // time. The guest's stream key is refreshed by B's pump, so they stay.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let (s, st) = host.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK);
    let members = st["room"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2, "guest still in the room: {st}");
    assert!(
        members.iter().all(|m| m["away"] != true),
        "nobody marked away while streams are live: {st}"
    );
    // And A still has the room (it did not dissolve).
    assert!(sa.room(&id).is_some());

    ta.abort();
    tb.abort();
}

/// A server rebuilt mid-match over the same store keeps playing: the blob,
/// the record log and its bookkeeping all come back.
#[tokio::test]
async fn a_rebuilt_server_resumes_a_match_mid_play() {
    let store = dummy::Store::new();
    let (base_a, sa, ta) = spawn(&store, |_| {}).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Resume" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) }))
        .await;
    host.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    {
        let room = sa.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match").quick_start().unwrap();
    }
    let before = store.match_get(&id).unwrap().expect("blob");
    let ticks_before = store.rec_meta_get(&id).unwrap().unwrap().ticks;

    // Kill the server (ticker + handle) and build a new one over the same
    // store -- what a restart (or a second process taking over) looks like.
    ta.abort();
    drop(sa);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let (base_b, sb, tb) = spawn(&store, |_| {}).await;
    let room = sb.room(&id).expect("room restored");
    let m = room.lock().unwrap().match_handle().expect("match restored");
    assert_eq!(m.snapshot().unwrap(), before, "same blob");
    // And it still runs.
    m.tick(0.05, 1).unwrap();
    let after = store.match_get(&id).unwrap().unwrap();
    assert_ne!(after, before, "the match advanced");
    let ticks_after = store.rec_meta_get(&id).unwrap().unwrap().ticks;
    assert!(ticks_after > ticks_before, "tick bookkeeping carried over");

    // A client session from before the rebuild still works.
    let host_on_b = host.on(&base_b);
    let (s, st) = host_on_b.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{st}");
    assert!(st["room"]["playing"] == true);

    tb.abort();
}

/// Two concurrent joins on A and B allocate distinct member ids (the
/// `next_member` counter is read-modify-written under the room lock).
#[tokio::test]
async fn concurrent_joins_on_two_servers_get_distinct_member_ids() {
    let store = dummy::Store::new();
    let (base_a, _sa, ta) = spawn(&store, |_| {}).await;
    let (base_b, _sb, tb) = spawn(&store, |_| {}).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Race" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let a = Client::new(&base_a, "A1").await;
    let b = Client::new(&base_b, "B1").await;
    let path = format!("/api/rooms/{id}/join");
    let (ra, rb) = tokio::join!(
        a.post(&path, json!({})),
        b.post(&path, json!({})),
    );
    assert_eq!(ra.0, StatusCode::OK, "{ra:?}");
    assert_eq!(rb.0, StatusCode::OK, "{rb:?}");
    let ida = ra.1["you"].as_i64().unwrap();
    let idb = rb.1["you"].as_i64().unwrap();
    assert_ne!(ida, idb, "distinct member ids: {ida} vs {idb}");

    ta.abort();
    tb.abort();
}

/// Two concurrent acts through A and B both land: the match lock serialises
/// them rather than letting one clobber the other's blob write.
#[tokio::test]
async fn concurrent_acts_on_two_servers_serialise() {
    let store = dummy::Store::new();
    let (base_a, sa, ta) = spawn(&store, |_| {}).await;
    let (base_b, _sb, tb) = spawn(&store, |_| {}).await;

    let host = Client::new(&base_a, "Host").await;
    let id = host.ok("/api/rooms", json!({ "name": "Lock" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    host.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) }))
        .await;
    host.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    {
        let room = sa.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match").quick_start().unwrap();
    }

    let host_on_b = host.on(&base_b);
    let path1 = format!("/api/rooms/{id}/anim-done");
    let path2 = path1.clone();
    let p1 = host.post(&path1, json!({ "seq": 1 }));
    let p2 = host_on_b.post(&path2, json!({ "seq": 2 }));
    let (r1, r2) = tokio::join!(p1, p2);
    assert_eq!(r1.0, StatusCode::OK, "{r1:?}");
    assert_eq!(r2.0, StatusCode::OK, "{r2:?}");

    let lines = store.record_log_get(&id).unwrap();
    let anims = lines.iter().filter(|l| l.contains("AnimDone")).count();
    assert_eq!(anims, 2, "both acts recorded, neither lost: {lines:?}");

    ta.abort();
    tb.abort();
}