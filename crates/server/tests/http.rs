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
