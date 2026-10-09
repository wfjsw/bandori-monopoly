//! End-to-end: a real server on a local port, real HTTP, real SSE.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use game_core::data::GameData;
use game_core::deck;
use game_core::engine::StubRules;
use reqwest::StatusCode;
use serde_json::{json, Value};
use server::Server;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

/// Start a server with a fast clock. Returns its base URL.
async fn spawn(presence_timeout: Duration) -> (String, Arc<Server>) {
    spawn_scaled(presence_timeout, 5.0).await
}

/// [`spawn`] with the clock speed named. The match clock is `time_scale` game
/// seconds per wall second (see `server::spawn_ticker`); a record seals the
/// quantum it ran on (`0.05 * time_scale`) into its header.
async fn spawn_scaled(presence_timeout: Duration, time_scale: f32) -> (String, Arc<Server>) {
    spawn_tuned(presence_timeout, time_scale, |_| {}).await
}

/// [`spawn_scaled`] plus a tuning hook that runs before the ticker starts
/// (the [`Server`] is inside an [`Arc`] by then). Tests that need a
/// `bot-service` or a non-default bot budget hang it here.
async fn spawn_tuned(
    presence_timeout: Duration,
    time_scale: f32,
    tune: impl FnOnce(&mut Server),
) -> (String, Arc<Server>) {
    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    // In-process engine: the tests exercise the server, not the worker pool.
    let mut server = Server::new(
        d.clone(),
        rules.clone(),
        server::pool::Pool::in_process(d, rules),
        Arc::new(server::store::dummy::Store::new()),
    );
    {
        let s = Arc::get_mut(&mut server).expect("fresh");
        s.presence_timeout = presence_timeout;
        s.time_scale = time_scale;
        tune(s);
    }
    server::spawn_ticker(server.clone());
    let app = server::router(server.clone(), None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), server)
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

    /// GET as raw bytes plus the response headers (the record endpoint serves
    /// a file, not a JSON envelope).
    async fn get_bytes(&self, path: &str) -> (StatusCode, bytes::Bytes, reqwest::header::HeaderMap) {
        let r = self
            .http
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        let status = r.status();
        let headers = r.headers().clone();
        (status, r.bytes().await.unwrap(), headers)
    }

    /// Post this seat's commit-reveal nonce (`docs/FAIRNESS.md`). The window
    /// closes and the match is created once every human has posted (or
    /// `server::room::NONCE_WINDOW` elapses).
    async fn fair_nonce(&self, id: &str) {
        // Any 32 bytes as hex; distinct per seat so the derived seed differs
        // from a no-nonce run.
        let nonce = format!(
            "{:064x}",
            self.token
                .bytes()
                .fold(0u64, |a, b| { a.wrapping_mul(131).wrapping_add(b as u64) })
                | 1
        );
        self.ok(&format!("/api/rooms/{id}/nonce"), json!({ "nonce": nonce }))
            .await;
    }

    async fn ok(&self, path: &str, body: Value) -> Value {
        let (s, v) = self.post(path, body).await;
        assert_eq!(s, StatusCode::OK, "{path}: {v}");
        v
    }

    async fn stream(&self, room: &str, last_event_id: Option<&str>) -> Sse {
        let mut req = self
            .http
            .get(format!("{}/api/rooms/{room}/stream", self.base))
            .bearer_auth(&self.token);
        if let Some(id) = last_event_id {
            req = req.header("Last-Event-ID", id);
        }
        let r = req.send().await.unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(r.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream"));
        Sse {
            body: Box::pin(r.bytes_stream()),
            buf: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Frame {
    event: String,
    id: Option<String>,
    data: Value,
}

struct Sse {
    body: std::pin::Pin<Box<dyn futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    buf: String,
}

impl Sse {
    /// Next frame (keep-alive comments skipped), or `None` on time-out / end.
    async fn next(&mut self, within: Duration) -> Option<Frame> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(end) = self.buf.find("\n\n") {
                let block: String = self.buf.drain(..end + 2).collect();
                let (mut event, mut id, mut data) = (String::from("message"), None, String::new());
                for line in block.lines() {
                    if let Some(v) = line.strip_prefix("event:") {
                        event = v.trim().into();
                    } else if let Some(v) = line.strip_prefix("id:") {
                        id = Some(v.trim().into());
                    } else if let Some(v) = line.strip_prefix("data:") {
                        data.push_str(v.strip_prefix(' ').unwrap_or(v));
                    }
                }
                if data.is_empty() {
                    continue; // comment / keep-alive
                }
                return Some(Frame {
                    event,
                    id,
                    data: serde_json::from_str(&data).unwrap_or(Value::String(data)),
                });
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            match tokio::time::timeout(left, self.body.next()).await {
                Ok(Some(Ok(chunk))) => self
                    .buf
                    .push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n")),
                _ => return None,
            }
        }
    }

    /// Read frames until `pred` matches one; returns everything read.
    async fn until(&mut self, within: Duration, pred: impl Fn(&Frame) -> bool) -> Vec<Frame> {
        let deadline = Instant::now() + within;
        let mut seen = vec![];
        while let Some(f) = self
            .next(deadline.saturating_duration_since(Instant::now()))
            .await
        {
            let hit = pred(&f);
            seen.push(f);
            if hit {
                return seen;
            }
        }
        panic!(
            "frame not seen within {within:?}; last frames: {:?}",
            seen.iter()
                .rev()
                .take(5)
                .map(|f| (&f.event, &f.data))
                .collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn auth_is_required() {
    let (base, _) = spawn(Duration::from_secs(20)).await;
    let r = reqwest::get(format!("{base}/api/rooms/X/state"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let health: Value = reqwest::get(format!("{base}/api/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["game"], "bandori-monopoly");
}

#[tokio::test]
async fn lobby_rules_match_the_original() {
    let (base, _) = spawn(Duration::from_secs(20)).await;
    let a = Client::new(&base, "Kasumi").await;
    let b = Client::new(&base, "Arisa").await;
    let c = Client::new(&base, "Tae").await;

    let made = a
        .ok(
            "/api/rooms",
            json!({ "name": "Practice", "maxPlayers": 3, "password": "pw" }),
        )
        .await;
    let id = made["room"]["id"].as_str().unwrap().to_string();
    assert_eq!(made["room"]["members"][0]["host"], true);

    let (s, v) = b
        .post(
            &format!("/api/rooms/{id}/join"),
            json!({ "password": "nope" }),
        )
        .await;
    assert_eq!(
        (s, v["reason"].as_str()),
        (StatusCode::FORBIDDEN, Some("password"))
    );
    assert_eq!(v["error"]["k"], "err.join.password");
    let (s, v) = b
        .post(
            &format!("/api/rooms/{id}/join"),
            json!({ "password": "pw", "version": "8" }),
        )
        .await;
    assert_eq!(
        (s, v["reason"].as_str()),
        (StatusCode::CONFLICT, Some("version"))
    );
    b.ok(
        &format!("/api/rooms/{id}/join"),
        json!({ "password": "pw" }),
    )
    .await;

    let (_, list) = c.get("/api/rooms").await;
    let room = &list.as_array().unwrap()[0];
    assert_eq!(
        (
            room["locked"].as_bool(),
            room["members"].as_array().unwrap().len()
        ),
        (Some(true), 2)
    );
    assert!(
        room.get("password").is_none(),
        "password never leaves the server"
    );

    // Only the host manages bots; a bot fills the room.
    let (s, _) = b
        .post(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    let (s, v) = c
        .post(
            &format!("/api/rooms/{id}/join"),
            json!({ "password": "pw" }),
        )
        .await;
    assert_eq!(
        (s, v["reason"].as_str()),
        (StatusCode::CONFLICT, Some("full"))
    );

    // Not everyone ready -> needs force; ready -> starts.
    let (s, v) = a.post(&format!("/api/rooms/{id}/start"), json!({})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["k"], "err.room.not_ready");
    b.ok(&format!("/api/rooms/{id}/ready"), json!({ "on": true }))
        .await;
    let started = a.ok(&format!("/api/rooms/{id}/start"), json!({})).await;
    assert_eq!(started["playing"], true);

    let (s, v) = c
        .post(
            &format!("/api/rooms/{id}/join"),
            json!({ "password": "pw" }),
        )
        .await;
    assert!(
        s == StatusCode::CONFLICT && ["playing", "full"].contains(&v["reason"].as_str().unwrap())
    );
}

#[tokio::test]
async fn host_leaving_hands_over_and_last_human_dissolves() {
    let (base, server) = spawn(Duration::from_secs(20)).await;
    let a = Client::new(&base, "A").await;
    let b = Client::new(&base, "B").await;
    let id = a.ok("/api/rooms", json!({})).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    b.ok(&format!("/api/rooms/{id}/join"), json!({})).await;
    let mut sse = b.stream(&id, None).await;

    a.ok(&format!("/api/rooms/{id}/leave"), json!({})).await;
    let frames = sse
        .until(Duration::from_secs(5), |f| {
            f.event == "room" && f.data["members"].as_array().unwrap().len() == 1
        })
        .await;
    let last = &frames.last().unwrap().data;
    assert_eq!(last["members"][0]["player"], "B");
    assert_eq!(last["members"][0]["host"], true, "hosting passed on");

    b.ok(&format!("/api/rooms/{id}/leave"), json!({})).await;
    sse.until(Duration::from_secs(5), |f| f.event == "dissolve")
        .await;
    assert!(server.room(&id).is_none());
}

/// Drive player `c` through pick / deck / prompts until `done` holds.
async fn drive(
    c: &Client,
    id: &str,
    sse: &mut Sse,
    d: &GameData,
    done: impl Fn(&Value) -> bool,
) -> Vec<Frame> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut all = vec![];
    let mut acted = (-1, String::new());
    loop {
        let f = sse
            .next(deadline.saturating_duration_since(Instant::now()))
            .await
            .expect("stream stalled");
        all.push(f.clone());
        if f.event != "match" {
            continue;
        }
        let st = &f.data["state"];
        if done(&f.data) {
            return all;
        }
        let player_id = f.data["playerId"].as_i64().unwrap() as usize;
        let key = (
            st["seq"].as_i64().unwrap() as i32,
            st["phase"].as_str().unwrap().to_string(),
        );
        if key == acted {
            continue;
        }
        let phase = st["phase"].as_str().unwrap();
        let my_turn = st["turn"].as_i64() == Some(player_id as i64);
        let path = format!("/api/rooms/{id}/act");
        if phase == "pick" && my_turn {
            let taken: Vec<&str> = st["players"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|s| s["character"].as_str())
                .collect();
            let pick = d
                .characters
                .iter()
                .find(|c| !c.cn_id.is_empty() && !taken.contains(&c.name.as_str()))
                .unwrap();
            c.ok(&path, json!({ "act": "pick", "character": pick.name }))
                .await;
            acted = key;
        } else if phase == "deck" && !st["players"][player_id]["deckReady"].as_bool().unwrap() {
            let ch = d
                .character(st["players"][player_id]["character"].as_str().unwrap())
                .unwrap();
            c.ok(
                &path,
                json!({ "act": "deck", "cards": deck::preset(d, ch) }),
            )
            .await;
            acted = key;
        } else if st["prompt"]["id"].as_i64().unwrap() != 0 {
            let p = &st["prompt"];
            let k = p["players"]
                .as_array()
                .unwrap()
                .iter()
                .position(|s| s.as_i64() == Some(player_id as i64));
            if k.is_some_and(|k| p["answers"][k].as_i64() == Some(-1)) {
                let msg = match p["kind"].as_str().unwrap() {
                    "mortgage" => {
                        json!({ "act": "answer", "prompt": p["id"], "cards": p["items"] })
                    }
                    "auction" => json!({ "act": "answer", "prompt": p["id"], "value": -1 }),
                    _ => json!({ "act": "answer", "prompt": p["id"], "value": p["fallback"] }),
                };
                let _ = c.post(&path, msg).await;
                acted = key;
            }
        }
    }
}

#[tokio::test]
async fn a_match_over_http_and_sse() {
    let (base, _) = spawn(Duration::from_secs(20)).await;
    let d = data();
    let a = Client::new(&base, "Kasumi").await;
    let made = a.ok("/api/rooms", json!({})).await;
    let id = made["room"]["id"].as_str().unwrap().to_string();
    let me = made["you"].as_i64().unwrap() as i32;
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    let mut sse = a.stream(&id, None).await;
    let hello = sse.next(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        (hello.event.as_str(), hello.data["you"].as_i64()),
        ("hello", Some(me as i64))
    );

    a.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    a.fair_nonce(&id).await;
    // Play until it's our move: phase play, our turn, 运营 stage (step 2), nothing pending.
    let frames = drive(&a, &id, &mut sse, &d, |v| {
        let st = &v["state"];
        st["phase"] == "play"
            && st["turn"] == v["playerId"]
            && st["step"] == 2
            && st["busy"] == false
    })
    .await;

    // Every `event` frame has an id "<match>:<n>", consecutive.
    let ids: Vec<(i64, i64)> = frames
        .iter()
        .filter(|f| f.event == "event")
        .map(|f| {
            let (m, e) = f.id.as_ref().unwrap().split_once(':').unwrap();
            (m.parse().unwrap(), e.parse().unwrap())
        })
        .collect();
    assert!(ids.len() > 5);
    assert!(
        ids.windows(2)
            .all(|w| w[0].0 == w[1].0 && w[1].1 == w[0].1 + 1),
        "{ids:?}"
    );

    // Our hand is in our frames, and matches our player's public count.
    let last = frames.iter().rev().find(|f| f.event == "match").unwrap();
    let player_id = last.data["playerId"].as_u64().unwrap() as usize;
    assert_eq!(
        last.data["hand"].as_array().unwrap().len() as i64,
        last.data["state"]["players"][player_id]["hand"]
            .as_i64()
            .unwrap()
    );

    // Commands: rejection message, then a roll that shows up on the stream.
    let path = format!("/api/rooms/{id}/act");
    let (s, v) = a.post(&path, json!({ "act": "end" })).await;
    assert_eq!(
        (s, v["error"]["k"].as_str()),
        (StatusCode::BAD_REQUEST, Some("err.roll_first"))
    );
    a.ok(&path, json!({ "act": "roll" })).await;
    sse.until(Duration::from_secs(10), |f| {
        f.event == "event"
            && f.data["type"] == "roll"
            && f.data["playerId"].as_u64() == Some(player_id as u64)
    })
    .await;

    // Resume: a new stream with Last-Event-ID continues right after it.
    let last_id = ids.last().unwrap();
    drop(sse);
    let mut again = a
        .stream(&id, Some(&format!("{}:{}", last_id.0, last_id.1)))
        .await;
    let first_event = again
        .until(Duration::from_secs(5), |f| f.event == "event")
        .await
        .pop()
        .unwrap();
    let (_, n) = first_event
        .id
        .unwrap()
        .split_once(':')
        .map(|(m, e)| (m.to_string(), e.parse::<i64>().unwrap()))
        .unwrap();
    assert_eq!(n, last_id.1 + 1, "no duplicates, no gaps");
}

#[tokio::test]
async fn silent_players_are_handed_to_the_ai_and_come_back() {
    let (base, server) = spawn(Duration::from_millis(600)).await;
    let a = Client::new(&base, "A").await;
    let b = Client::new(&base, "B").await;
    let id = a.ok("/api/rooms", json!({})).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut sse_a = a.stream(&id, None).await;
    let joined = b.ok(&format!("/api/rooms/{id}/join"), json!({})).await;
    let b_member = joined["you"].as_i64().unwrap();
    let _keep_b = b.stream(&id, None).await; // B present while the match starts
    a.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    a.fair_nonce(&id).await;
    b.fair_nonce(&id).await;
    drop(_keep_b);

    // B has no stream now: after the time-out the AI takes the player.
    let away = |f: &Frame| {
        f.event == "room"
            && f.data["members"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"].as_i64() == Some(b_member) && m["away"] == true)
    };
    sse_a.until(Duration::from_secs(10), away).await;
    {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        let m = r.game.as_ref().unwrap();
        let v = m.view(b_member as i32).unwrap();
        let pid = v["playerId"].as_i64().expect("member seated") as usize;
        assert!(
            v["state"]["players"][pid]["ai"].as_bool().expect("player"),
            "B was taken over by the AI"
        );
    }

    // B reconnects: player returned.
    let _b_again = b.stream(&id, None).await;
    let back = |f: &Frame| {
        f.event == "room"
            && f.data["members"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"].as_i64() == Some(b_member) && m["away"] == false)
    };
    sse_a.until(Duration::from_secs(10), back).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let room = server.room(&id).unwrap();
    let r = room.lock().unwrap();
    let m = r.game.as_ref().unwrap();
    let v = m.view(b_member as i32).unwrap();
    let pid = v["playerId"].as_i64().expect("member seated") as usize;
    assert!(
        !v["state"]["players"][pid]["ai"].as_bool().expect("player"),
        "B is back"
    );
}

#[tokio::test]
async fn silent_lobby_members_are_removed() {
    let (base, server) = spawn(Duration::from_millis(500)).await;
    let a = Client::new(&base, "A").await;
    let b = Client::new(&base, "B").await;
    let id = a.ok("/api/rooms", json!({})).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut sse = a.stream(&id, None).await;
    b.ok(&format!("/api/rooms/{id}/join"), json!({})).await;
    // A keeps a stream open; B never does. (The first room frame already shows A
    // alone -- from before B joined -- so wait for B to appear, then disappear.)
    let members = |f: &Frame| f.data["members"].as_array().map_or(0, |m| m.len());
    sse.until(Duration::from_secs(10), |f| {
        f.event == "room" && members(f) == 2
    })
    .await;
    sse.until(Duration::from_secs(10), |f| {
        f.event == "room" && members(f) == 1 && f.data["members"][0]["player"] == "A"
    })
    .await;
    let (_, me) = b.get("/api/session").await;
    assert!(me["room"].is_null(), "B's session was cleared");
    assert!(server.room(&id).is_some(), "A is still there");
}

#[tokio::test]
async fn client_routes_fall_back_to_index_html() {
    let dir = std::env::temp_dir().join(format!("bm-static-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>app</title>").unwrap();
    std::fs::write(dir.join("assets/a.txt"), "asset").unwrap();
    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let server = Server::new(
        d.clone(),
        rules.clone(),
        server::pool::Pool::in_process(d, rules),
        Arc::new(server::store::dummy::Store::new()),
    );
    let app = server::router(server, None, Some(dir.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let get = |p: &str| reqwest::get(format!("{base}{p}"));

    for page in ["/", "/menu", "/room/ABC123", "/play/solo"] {
        let r = get(page).await.unwrap();
        assert_eq!(r.status(), StatusCode::OK, "{page}");
        assert!(
            r.text().await.unwrap().contains("<title>app</title>"),
            "{page}"
        );
    }
    assert_eq!(
        get("/assets/a.txt").await.unwrap().text().await.unwrap(),
        "asset"
    );
    assert_eq!(
        get("/assets/missing.png").await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get("/api/nope").await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A restart must not interrupt a game: the roster and the match blob come back
/// from the store, so the client's SSE reconnects into the same match. This is
/// the restore path (`Server::restore_rooms`) against a shared store -- which is
/// exactly what the process does at startup, without needing to kill one.
#[tokio::test]
async fn a_restart_restores_the_room_and_its_match() {
    use game_core::net::{RoomInfo, RoomMember};
    use server::store::{CrossState, RoomRecord};

    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let store = Arc::new(server::store::dummy::Store::new());
    let engine = server::pool::Pool::in_process(d.clone(), rules.clone());

    // A room with one member and a match blob, as a restart would find them.
    let member = RoomMember {
        id: 1,
        player: "A".into(),
        ready: true,
        host: true,
        ..Default::default()
    };
    let rec = RoomRecord {
        info: RoomInfo {
            id: "RESTORE".into(),
            name: "restore me".into(),
            ranked: false,
            max_players: 6,
            locked: true,
            playing: true,
            theme: String::new(),
            weights: Default::default(),
            members: vec![member],
            fair: None,
        },
        password: "pw".into(),
        next_member: 2,
        tokens: vec![(1, "tok-A".into())],
    };
    store.room_put(&rec).unwrap();
    store.match_put("RESTORE", "{\"version\":1}").unwrap();
    // The session is its own bucket: the room record maps member -> token, the
    // session maps token -> player.
    store
        .session_put(&server::state::Session {
            token: "tok-A".into(),
            player: "A".into(),
            character: "".into(),
            cn_id: "".into(),
            room: Some(("RESTORE".into(), 1)),
        })
        .unwrap();

    // A fresh Server over the same store = the process coming back up.
    let server = Server::new(d.clone(), rules, engine, store);
    server.restore_rooms();

    let room = server.room("RESTORE").expect("room came back");
    let r = room.lock().unwrap();
    assert_eq!(r.info.name, "restore me");
    assert_eq!(r.info.members.len(), 1, "roster restored");
    assert!(r.info.playing, "still mid-match");

    // The match blob is reachable through the handle again.
    let m = r.match_handle().expect("match restored");
    assert!(
        m.snapshot().unwrap().contains("version"),
        "blob came from the store"
    );

    // And a session that names the room still resolves.
    assert_eq!(server.session("tok-A").unwrap().player, "A");
}

/// `POST /api/rooms/{id}/bots` takes a mentality, it rides the room record, and
/// the match carries it onto the seat when the room starts.
#[tokio::test]
async fn adding_a_chaos_bot_round_trips() {
    let (base, _) = spawn(Duration::from_secs(20)).await;
    let a = Client::new(&base, "Host").await;
    let id = a.ok("/api/rooms", json!({ "name": "Bots" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // A chaos bot, named and tagged on the roster.
    let room = a
        .ok(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "chaos" }),
        )
        .await;
    let bots: Vec<_> = room["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["bot"] == true)
        .collect();
    assert_eq!(bots.len(), 1);
    assert_eq!(bots[0]["mentality"], "chaos", "{room}");
    assert!(!bots[0]["player"].as_str().unwrap().is_empty());

    // Omitted mentality is standard; a named one round-trips too.
    let room = a
        .ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    let bots: Vec<_> = room["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["bot"] == true)
        .collect();
    assert_eq!(bots.len(), 2);
    assert_eq!(bots[1]["mentality"], "standard", "{room}");

    // An unknown tag is refused rather than silently coerced.
    let (s, v) = a
        .post(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "wild" }),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["k"], "err.bad_mentality");

    // Start (force -- only the host is here) and the seat carries the tag.
    let started = a
        .ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    assert_eq!(started["playing"], true);
    a.fair_nonce(&id).await;
    let (s, state) = a.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{state}");
    // `RoomState` serialises the view under `"match"`.
    let players = state["match"]["state"]["players"]
        .as_array()
        .unwrap_or_else(|| panic!("no match view in {state}"));
    let chaos = players
        .iter()
        .find(|p| p["mentality"] == "chaos")
        .expect("the chaos seat is in the match");
    assert_eq!(chaos["bot"], true);
    // The host's own (human) seat also reads "standard" -- the field's default
    // -- so look for the standard *bot*.
    players
        .iter()
        .find(|p| p["mentality"] == "standard" && p["bot"] == true)
        .expect("the standard bot seat is in the match");
}

/// The whole record path (`docs/REPLAY.md` §4): a match is logged as it is
/// played, sealed when it ends, and served to its participants only.
///
/// Covers the P4 test matrix: a bot room on a high clock plays to the end; the
/// endpoint answers 409 mid-match, 200 after it and 403 to an outsider; a
/// `member_left` / `member_back` pair is recorded; the worker's `replay` op
/// ends on the hash of the blob the server kept; and an in-process `Replayer`
/// gets there too.
#[tokio::test]
async fn the_record_endpoint_serves_the_last_finished_match() {
    use game_core::record::{hash_save, Input, Origin, Replayer};

    // A high clock and a short presence time-out: the match ticks fast, and A
    // is handed to the AI as soon as the stream drops.
    let (base, server) = spawn_scaled(Duration::from_millis(400), 20.0).await;
    let a = Client::new(&base, "Host").await;
    let outsider = Client::new(&base, "Outsider").await;
    let id = a
        .ok("/api/rooms", json!({ "name": "Record" }))
        .await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Nothing has been played here yet.
    let (s, v) = a.get(&format!("/api/rooms/{id}/record")).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"]["k"], "err.record.none");

    a.ok(
        &format!("/api/rooms/{id}/bots"),
        json!({ "op": "add", "mentality": "chaos" }),
    )
    .await;
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    a.ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    a.fair_nonce(&id).await;

    // Skip pick / deck so the match is in play immediately -- and so the log
    // carries a `QuickStart`.
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match started").clone()
    };
    handle.quick_start().unwrap();

    // Mid-match the record is not served at all.
    let (s, v) = a.get(&format!("/api/rooms/{id}/record")).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["k"], "err.record.live");

    // A's stream drops: the AI takes the seat (`Left`), and reopening it brings
    // the player back (`Back`).
    let mut sse = a.stream(&id, None).await;
    let _ = sse.next(Duration::from_millis(200)).await;
    drop(sse);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut sse = a.stream(&id, None).await;
    let _ = sse.next(Duration::from_millis(400)).await;
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Let the bots play, then settle the match by score (`Finish`).
    tokio::time::sleep(Duration::from_millis(1500)).await;
    handle.quick_start().unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.finish().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
        assert_eq!(s, StatusCode::OK, "{st}");
        if st["room"]["playing"] == false {
            break;
        }
        assert!(Instant::now() < deadline, "match did not end: {st}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(sse);

    // The record is there, and it is the match we just watched. It is served
    // as the sealed bytes: a zstd-framed `.bdrec`, `application/zstd`, no
    // `Content-Encoding` (the client decompresses in wasm).
    let (s, body, headers) = a.get_bytes(&format!("/api/rooms/{id}/record")).await;
    assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let ctype = headers[reqwest::header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ctype, "application/zstd", "Content-Type: {ctype}");
    assert!(
        !headers.contains_key(reqwest::header::CONTENT_ENCODING),
        "no Content-Encoding: the client decodes in wasm"
    );
    assert!(
        body.len() >= 4 && body[..4] == [0x28, 0xB5, 0x2F, 0xFD],
        "zstd magic 28 B5 2F FD, got {:02x?}",
        &body[..body.len().min(4)]
    );
    let disp = headers[reqwest::header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disp.starts_with("attachment; filename=\"bdrec-") && disp.ends_with(".bdrec\""),
        "Content-Disposition: {disp}"
    );
    let file = game_core::record::decode_record(&body).unwrap_or_else(|e| panic!("record: {e}"));
    assert_eq!(
        file.header.origin,
        Origin::Online { room: id.clone() },
        "an online record names its room"
    );
    assert!(file.header.ended);
    assert!(file.header.total_ticks > 0, "the clock ran");
    assert!(!file.header.gaps, "a clean run has no gaps");
    assert!(!file.body.checkpoints.is_empty(), "turns were checkpointed");
    assert_eq!(file.header.step, 0.05 * 20.0, "the quantum is sealed");
    // Every kind of input the design names shows up.
    let kinds: Vec<&str> = file
        .body
        .inputs
        .iter()
        .map(|i| match i {
            Input::Ticks { .. } => "ticks",
            Input::Act { .. } => "act",
            Input::QuickStart => "quick_start",
            Input::Finish => "finish",
            Input::Left { .. } => "left",
            Input::Back { .. } => "back",
        })
        .collect();
    for want in ["ticks", "quick_start", "left", "back", "finish"] {
        assert!(kinds.contains(&want), "missing {want} in {kinds:?}");
    }

    // An outsider who never sat down is refused -- the record reveals every
    // hand and the deck order.
    let (s, v) = outsider.get(&format!("/api/rooms/{id}/record")).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["k"], "err.record.forbidden");

    // The worker's `replay` op ends on the same hash as the blob the server kept
    // -- fed the raw zstd bytes (the form the server itself stores), and again
    // as the plain JSON string older callers send.
    let final_blob = handle.snapshot().unwrap();
    let want = hash_save(&final_blob);
    assert_eq!(file.body.final_hash, want, "the seal hashed the live blob");
    let (got, diverged) = server.engine.replay(&body).expect("worker replay");
    assert!(!diverged, "the record replays clean");
    assert_eq!(got, want, "worker `replay` ends on the final blob's hash");
    let json = serde_json::to_string(&file).unwrap();
    let v = server
        .engine
        .call(serde_json::json!({ "op": "replay", "record": json }))
        .expect("worker replay from JSON");
    assert_eq!(v["final_hash"], want.as_str(), "the JSON form replays too");
    assert_eq!(v["diverged"], false);

    // ... and so does a plain in-process `Replayer`.
    let mut rp = Replayer::new(data(), Arc::new(StubRules), &file, false).expect("load record");
    let mut guard = 0u32;
    while !rp.status().ended {
        rp.step_ticks(64);
        guard += 1;
        assert!(guard < 500_000, "replay stuck");
    }
    assert!(!rp.status().diverged, "every checkpoint matched");
    assert_eq!(hash_save(&rp.match_ref().save()), want, "same final hash");
}

// ------------------------------------------------------- advanced bots (B5)

/// A room with one human + `bots` Advanced bots, force-started into play.
/// Returns `(room id, match handle)`. The caller has already quick-started or
/// will.
async fn advanced_room(server: &Arc<Server>, who: &Client, bots: usize) -> String {
    let id = who
        .ok("/api/rooms", json!({ "name": "Advanced" }))
        .await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for _ in 0..bots {
        who.ok(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "advanced" }),
        )
        .await;
    }
    let started = who
        .ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    assert_eq!(started["playing"], true, "{started}");
    who.fair_nonce(&id).await;
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match started").clone()
    };
    // Skip pick / deck so the match is in play immediately.
    handle.quick_start().unwrap();
    id
}

/// Wait until the room's match is no longer `playing`, or give up.
async fn wait_ended(who: &Client, id: &str, deadline: Duration) -> bool {
    let until = Instant::now() + deadline;
    loop {
        let (s, st) = who.get(&format!("/api/rooms/{id}/state")).await;
        assert_eq!(s, StatusCode::OK, "{st}");
        if st["room"]["playing"] == false {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The seat the server drives: a bot tagged `advanced` in the match view.
fn advanced_seats(state: &Value) -> Vec<Value> {
    state["match"]["state"]["players"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|p| p["bot"] == true && p["mentality"] == "advanced")
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// With a bot-service attached, Advanced seats are **server-driven** (`ai` is
/// off) and the search answers for them; a short match runs to completion.
#[tokio::test]
async fn advanced_bots_play_with_the_bot_service() {
    let (base, server) = spawn_tuned(Duration::from_secs(30), 3.0, |s| {
        s.bots = Some(server::botsvc::BotService::in_process(
            s.data.clone(),
            s.rules.clone(),
        ));
        // Tiny budget: this test is about the plumbing, not strength.
        s.bot_budget_ms = Some(15);
        s.bot_ask_timeout = Some(Duration::from_millis(400));
    })
    .await;
    let a = Client::new(&base, "Host").await;
    let id = advanced_room(&server, &a, 2).await;

    // The seats are in the match tagged advanced and held (`ai` off) -- the
    // service, not the engine, answers for them.
    let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{st}");
    let seats = advanced_seats(&st);
    assert_eq!(seats.len(), 2, "{st}");
    for p in &seats {
        assert_eq!(p["ai"], false, "the server drives this seat: {p}");
    }

    // Let the drive loop + search play, then settle by score.
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().unwrap().clone()
    };
    let before = handle.snapshot().unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let after = handle.snapshot().unwrap();
    assert_ne!(before, after, "the search-driven seats moved the match");
    // Mid-match the seats are still tagged advanced (never rewritten -- that
    // only happens with no service).
    let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{st}");
    assert_eq!(advanced_seats(&st).len(), 2, "{st}");
    handle.finish().unwrap();
    assert!(
        wait_ended(&a, &id, Duration::from_secs(15)).await,
        "the match ended"
    );
}

/// With **no** bot-service, Advanced seats play as standard engine bots: the
/// mentality is rewritten at match start and the engine drives them.
#[tokio::test]
async fn advanced_bots_play_as_standard_without_the_service() {
    let (base, server) = spawn_scaled(Duration::from_secs(30), 3.0).await;
    assert!(server.bots.is_none(), "no service in this test");
    let a = Client::new(&base, "Host").await;
    let id = advanced_room(&server, &a, 2).await;

    // Rewritten: the match seats are standard, and the engine holds them.
    let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{st}");
    let seats = advanced_seats(&st);
    assert!(seats.is_empty(), "advanced was rewritten to standard: {st}");
    let std_bots: Vec<_> = st["match"]["state"]["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["bot"] == true && p["mentality"] == "standard")
        .collect();
    assert_eq!(std_bots.len(), 2, "{st}");
    for p in &std_bots {
        assert_eq!(p["ai"], true, "the engine drives this seat: {p}");
    }

    // The engine plays them; a short match still completes.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().unwrap().clone()
    };
    handle.finish().unwrap();
    assert!(
        wait_ended(&a, &id, Duration::from_secs(15)).await,
        "the match ended without a service"
    );
}

/// A service that never answers in time must not stall the match: the server
/// falls back to the engine's heuristic for that decision and moves on.
#[tokio::test]
async fn a_bot_service_timeout_falls_back_without_stalling() {
    let (base, server) = spawn_tuned(Duration::from_secs(30), 3.0, |s| {
        // The service is present (so the seats stay server-driven) but every
        // ask is given a deadline of zero, so every one of them times out.
        s.bots = Some(server::botsvc::BotService::in_process(
            s.data.clone(),
            s.rules.clone(),
        ));
        s.bot_budget_ms = Some(15);
        s.bot_ask_timeout = Some(Duration::from_millis(0));
    })
    .await;
    let a = Client::new(&base, "Host").await;
    let id = advanced_room(&server, &a, 2).await;

    let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
    assert_eq!(s, StatusCode::OK, "{st}");
    assert_eq!(advanced_seats(&st).len(), 2, "seats stay advanced: {st}");

    // The drive loop must keep answering (via the heuristic) even though the
    // service never does. The clock moves, the match does not hang.
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().unwrap().clone()
    };
    let before = handle.snapshot().unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = handle.snapshot().unwrap();
    assert_ne!(before, after, "the match moved despite the timeouts");

    // Every decide fell back; none was answered by the service.
    let bots = server.bots.as_ref().expect("a service is attached");
    let stats = bots.stats();
    assert!(stats.decides > 0, "the drive kept asking: {stats:?}");
    assert_eq!(stats.decide_ok, 0, "no ask ever beat the zero deadline: {stats:?}");
    assert_eq!(
        stats.decide_fallback, stats.decides,
        "every decide fell back to the heuristic: {stats:?}"
    );

    handle.finish().unwrap();
    assert!(
        wait_ended(&a, &id, Duration::from_secs(15)).await,
        "the match ended despite every ask timing out"
    );
}

/// The lobby accepts `"advanced"` and refuses anything else.
#[tokio::test]
async fn the_bot_mentality_picker_accepts_advanced() {
    let (base, _server) = spawn(Duration::from_secs(30)).await;
    let a = Client::new(&base, "Host").await;
    let id = a
        .ok("/api/rooms", json!({ "name": "Mentality" }))
        .await["room"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let room = a
        .ok(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "advanced" }),
        )
        .await;
    let bot = room["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["bot"] == true)
        .expect("one bot");
    assert_eq!(bot["mentality"], "advanced", "{room}");
    // Standard / chaos still parse; the two stay the solo choices.
    let room = a
        .ok(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "chaos" }),
        )
        .await;
    let bots: Vec<_> = room["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["bot"] == true)
        .collect();
    assert_eq!(bots[1]["mentality"], "chaos");
    let (s, v) = a
        .post(
            &format!("/api/rooms/{id}/bots"),
            json!({ "op": "add", "mentality": "quantum" }),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["k"], "err.bad_mentality");
}

// ------------------------------------------------- advanced bots: ponder (B7)

/// An idle advanced seat (someone else's turn) gets a speculative
/// `op: "ponder"`, and sending it does **not** block the tick: the ticker and
/// the HTTP surface keep running while a ponder is in flight, at most one per
/// seat.
#[tokio::test]
async fn ponder_is_sent_for_an_idle_advanced_seat_and_does_not_block_the_tick() {
    let (base, server) = spawn_tuned(Duration::from_secs(30), 3.0, |s| {
        let bots = server::botsvc::BotService::in_process(s.data.clone(), s.rules.clone());
        // Every request is slow enough to observe in flight (and to prove the
        // tick does not wait on one), while still inside the ask deadline.
        bots.set_latency(Duration::from_millis(250));
        s.bots = Some(bots);
        s.bot_budget_ms = Some(15);
        s.bot_ponder_budget_ms = Some(15);
        s.bot_ask_timeout = Some(Duration::from_millis(600));
    })
    .await;
    let a = Client::new(&base, "Host").await;
    let id = advanced_room(&server, &a, 2).await;
    let bots = server.bots.as_ref().expect("a service is attached").clone();

    // One seat acts, the other is idle -- the idle one is who we ponder for.
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().unwrap().clone()
    };
    let before = handle.snapshot().unwrap();

    // Wait until a ponder has actually been sent, and grab a sample while one
    // is still in flight (the latency knob keeps it outstanding for 250 ms).
    let mut saw_ponder = false;
    let mut health_while_pondering = None;
    let mut max_drive_pondering = 0usize;
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        let in_flight = server.bot_drive.lock().unwrap().ponders_in_flight();
        max_drive_pondering = max_drive_pondering.max(in_flight);
        if bots.stats().ponders > 0 {
            saw_ponder = true;
            if in_flight > 0 && health_while_pondering.is_none() {
                // The tick loop / HTTP surface must stay live while the
                // speculative search runs on its own task.
                let t0 = Instant::now();
                let (s, _) = a.get("/api/health").await;
                let dt = t0.elapsed();
                assert_eq!(s, StatusCode::OK, "the server answers while pondering");
                health_while_pondering = Some(dt);
            }
        }
        if saw_ponder && health_while_pondering.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(saw_ponder, "an idle advanced seat was pondered: {:?}", bots.stats());
    let dt = health_while_pondering.expect("a health probe during a ponder");
    assert!(
        dt < Duration::from_millis(150),
        "the tick / HTTP surface is not blocked by a ponder (health took {dt:?})"
    );
    // One in flight per seat (2 advanced seats here), never a pile-up.
    assert!(
        max_drive_pondering <= 2,
        "at most one ponder per seat: {max_drive_pondering}"
    );
    assert!(
        bots.stats().ponder_in_flight_max <= 2,
        "at most one ponder per seat: {:?}",
        bots.stats()
    );

    // And the tick kept moving the match while the ponders flew.
    let after = handle.snapshot().unwrap();
    assert_ne!(before, after, "the match advanced while ponders were in flight");

    handle.finish().unwrap();
    assert!(
        wait_ended(&a, &id, Duration::from_secs(15)).await,
        "the match ended"
    );
}

/// Park a match at one seat's **searchable** decision (the abstracted action
/// list is non-empty) and return the exact view frame the server sends.
fn parked_searchable_view() -> (Value, i32, i32) {
    use game_core::engine::Match;
    use game_core::net::RoomMember;
    use game_core::scoring::ScoreWeights;
    use game_core::state::{stage, BotMentality};
    use game_core::MatchMode;

    let members = vec![
        RoomMember {
            id: 1,
            player: "Searcher".into(),
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "BotA".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "BotB".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ];
    let d = data();
    let mut m = Match::new(
        d.clone(),
        Arc::new(StubRules),
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    for _ in 0..20_000 {
        if m.ended() {
            break;
        }
        let st = m.state();
        let me = st.player_of(1);
        let at = (st.prompt.id > 0 && st.prompt.waiting(me))
            || (!st.busy
                && st.turn == me
                && (st.step == stage::OPS || st.step == stage::END));
        if st.phase == "play" && at {
            let view = bot_service::BotSeatView::from_match(&m, 1);
            let acts = bot_service::bot_action::legal_actions(
                &d,
                &view.state,
                &view.hand,
                &view.playable,
                me.max(0) as usize,
            );
            if !acts.is_empty() {
                let prompt_id = if st.prompt.id > 0 && st.prompt.waiting(me) {
                    st.prompt.id
                } else {
                    0
                };
                let frame = serde_json::to_value(&view).expect("view frame");
                return (frame, 1, prompt_id);
            }
            let msg = bot_service::heuristic_message_view(&d, &view);
            let _ = m.act(1, &msg);
        }
        m.tick(0.25);
    }
    panic!("no searchable decision for member 1");
}

/// The server's `decide` after a `ponder` for the **same information-set key**
/// is answered from the service's cache (`reused: true`) and does not spend
/// the decision's budget (BOT-RESEARCH #5 / `docs/BOT.md` §3.5).
#[tokio::test]
async fn a_decide_after_a_ponder_for_the_same_key_is_answered_from_cache() {
    let d = data();
    let bots = server::botsvc::BotService::in_process(d.clone(), Arc::new(StubRules));
    let (view, member, prompt_id) = parked_searchable_view();

    // Speculative search first -- the wire shape the idle probes send.
    let reused = bots
        .ponder("R", member, &view, 200, 11, Duration::from_secs(3))
        .await
        .expect("ponder answers");
    assert!(!reused, "the first ponder searches: {:?}", bots.stats());

    // The real decision for the same view must come from the cache.
    let started = Instant::now();
    let ans = bots
        .decide(
            "R",
            member,
            &view,
            prompt_id,
            200,
            12,
            Duration::from_secs(3),
        )
        .await
        .expect("decide answers");
    let dt = started.elapsed();
    assert!(ans.reused, "the decide reuses the pondered result: {ans:?}");
    assert!(!ans.answer.act.is_empty(), "a command came back: {ans:?}");
    assert!(
        dt < Duration::from_millis(150),
        "a reused decide must not search again (took {dt:?})"
    );
    let stats = bots.stats();
    assert_eq!(stats.ponders, 1, "{stats:?}");
    assert_eq!(stats.decides, 1, "{stats:?}");
    assert_eq!(stats.decide_reused, 1, "{stats:?}");
    assert_eq!(stats.decide_fallback, 0, "{stats:?}");

    // A different information set misses.
    let (other, _, _) = parked_searchable_view();
    if other != view {
        let ans2 = bots
            .decide("R", member, &other, prompt_id, 50, 13, Duration::from_secs(3))
            .await
            .expect("decide answers");
        assert!(!ans2.reused, "a different key must not hit the cache");
    }
}

/// The commit-reveal openings must never appear in any client-facing payload
/// while the match runs (`docs/FAIRNESS.md` threat model): not in the start
/// reply, not in `RoomState`, not in an SSE frame, not in a seat view -- and
/// the same frame is what the bot service gets. The sealed record, served
/// after the match to participants, is where the reveal lives.
#[tokio::test]
async fn the_openings_never_reach_a_client_payload() {
    let (base, server) = spawn(Duration::from_secs(30)).await;
    let a = Client::new(&base, "Host").await;
    let made = a.ok("/api/rooms", json!({})).await;
    let id = made["room"]["id"].as_str().unwrap().to_string();
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    a.ok(&format!("/api/rooms/{id}/bots"), json!({ "op": "add" }))
        .await;
    let mut sse = a.stream(&id, None).await;
    let _hello = sse.next(Duration::from_secs(5)).await.unwrap();

    // Start: the commitment is public the moment the room starts.
    let started = a
        .ok(&format!("/api/rooms/{id}/start"), json!({ "force": true }))
        .await;
    assert_eq!(started["playing"], true, "{started}");
    let commit = started["fair"]["commit"].as_str().expect("commit public");
    assert_eq!(commit.len(), 64, "{commit}");
    assert_eq!(started["fair"]["collecting"], true, "{started}");

    let (seed_hex, salt_hex) = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.debug_fair_openings().expect("openings held server-side")
    };
    assert_eq!(seed_hex.len(), 64);
    assert_eq!(salt_hex.len(), 64);

    let mut payloads: Vec<String> = vec![started.to_string()];

    // The nonce exchange and the match that follows.
    a.fair_nonce(&id).await;
    {
        let (s, v) = a.get(&format!("/api/rooms/{id}/state")).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["room"]["fair"]["collecting"], false, "{v}");
        payloads.push(v.to_string());
    }

    // Drink the live SSE stream (room + match frames).
    for _ in 0..8 {
        if let Some(f) = sse.next(Duration::from_millis(150)).await {
            payloads.push(serde_json::to_string(&f.data).unwrap());
        }
    }

    // The seat view -- exactly what the bot service is handed (`docs/BOT.md`
    // B5): no seed, no salt, no RNG state.
    {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        let m = r.game.as_ref().expect("match started");
        for member in r.info.members.iter().map(|x| x.id) {
            let v = m.view(member).unwrap();
            payloads.push(v.to_string());
        }
    }

    for p in &payloads {
        assert!(!p.contains(&seed_hex), "seed leaked into a payload: {p}");
        assert!(!p.contains(&salt_hex), "salt leaked into a payload: {p}");
    }

    // End the match: now -- and only now -- the sealed record reveals the
    // openings (to the participants who may download it).
    let handle = {
        let room = server.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match started").clone()
    };
    handle.quick_start().unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.finish().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (s, st) = a.get(&format!("/api/rooms/{id}/state")).await;
        assert_eq!(s, StatusCode::OK, "{st}");
        if st["room"]["playing"] == false {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "match did not end: {}",
            serde_json::to_string(&st).unwrap()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (s, body, _headers) = a.get_bytes(&format!("/api/rooms/{id}/record")).await;
    assert_eq!(s, StatusCode::OK);
    let json = game_core::record::expand_record(&body).expect("record decompresses");
    let text = String::from_utf8_lossy(&json).to_string();
    assert!(text.contains(&seed_hex), "the reveal carries the seed");
    assert!(text.contains(&salt_hex), "the reveal carries the salt");
    assert!(text.contains(commit), "the record repeats the commitment");
    // And the derived seed in the setup is what the openings produce.
    let file = game_core::record::decode_record(&body).expect("record parses");
    let fair = file.header.fair.as_ref().expect("fairness material");
    let seed = game_core::fair::unhex32(&fair.seed).unwrap();
    let nonces: Vec<(i32, [u8; 32])> = fair
        .nonces
        .iter()
        .filter_map(|n| game_core::fair::unhex32(&n.nonce).ok().map(|b| (n.member, b)))
        .collect();
    let derived = game_core::fair::derive_match_seed(&seed, &nonces);
    match &file.body.init {
        game_core::record::Init::Seed(setup) => {
            assert_eq!(setup.seed256.map(|s| s.0), Some(derived), "derived seed")
        }
        other => panic!("expected a seeded record: {other:?}"),
    }
}
