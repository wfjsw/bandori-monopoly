//! TEMP diagnostic: what one `POST /anim-done` costs.
//!
//! Every ack is a full recorded op (store lock, meta get/put, blob get/put,
//! worker round-trip, record-log append). This measures that on a live server
//! with the ticker running, then with it stopped, so lock contention with the
//! 20 Hz tick loop is visible. Delete after the diagnosis.

use std::sync::Arc;
use std::time::{Duration, Instant};

use game_core::data::{GameData, Roster};
use game_core::engine::StubRules;
use serde_json::{json, Value};
use server::store::{dummy, CrossState};

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../game-core/tests/fixtures/roster.json"),
        )
        .expect("roster fixture"),
    )
    .expect("roster fixture parses");
    Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .unwrap(),
    )
}

async fn spawn(
    store: &dummy::Store,
) -> (String, Arc<server::Server>, tokio::task::JoinHandle<()>) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("BD_GLUE_SHA", "test-glue-sha"));
    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut srv = server::Server::new(
        d.clone(),
        rules.clone(),
        server::pool::Pool::in_process(d, rules),
        Arc::new(store.clone()),
    );
    {
        let s = Arc::get_mut(&mut srv).expect("fresh");
        s.time_scale = 0.2;
    }
    let ticker = server::spawn_ticker(srv.clone());
    let app = server::router(srv.clone(), None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), srv, ticker)
}

fn pct(v: &mut [Duration], p: f64) -> Duration {
    v.sort();
    v[((v.len() as f64 - 1.0) * p) as usize]
}

#[tokio::test]
async fn anim_done_cost() {
    let store = dummy::Store::new();
    let (base, srv, ticker) = spawn(&store).await;
    let http = reqwest::Client::new();
    let token: String = http
        .post(format!("{base}/api/session"))
        .json(&json!({ "player": "Host" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .into();
    let post = |p: String, b: Value| {
        let http = http.clone();
        let token = token.clone();
        let base = base.clone();
        async move {
            let t = Instant::now();
            let r = http
                .post(format!("{base}{p}"))
                .bearer_auth(&token)
                .json(&b)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap();
            (t.elapsed(), r)
        }
    };
    let id: String = post("/api/rooms".into(), json!({ "name": "Perf" }))
        .await
        .1["room"]["id"]
        .as_str()
        .unwrap()
        .into();
    for _ in 0..3 {
        post(format!("/api/rooms/{id}/bots"), json!({ "op": "add" })).await;
    }
    post(format!("/api/rooms/{id}/nonce"), json!({ "nonce": format!("{:064x}", 1) })).await;
    post(format!("/api/rooms/{id}/start"), json!({ "force": true })).await;
    {
        let room = srv.room(&id).unwrap();
        let r = room.lock().unwrap();
        r.game.as_ref().expect("match").quick_start().unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let blob = store.match_get(&id).unwrap().unwrap().len();
    let log0 = store.record_log_get(&id).unwrap().len();
    eprintln!("state blob = {blob} bytes, log lines = {log0}");

    // no-op ack (stale seq): the shape of every redundant client ack
    let path = format!("/api/rooms/{id}/anim-done");
    let n = 100;
    let mut with_ticker = Vec::with_capacity(n);
    for _ in 0..n {
        with_ticker.push(post(path.clone(), json!({ "seq": -1 })).await.0);
    }
    let log1 = store.record_log_get(&id).unwrap().len();
    eprintln!(
        "no-op ack WITH ticker: p50={:?} p95={:?} max={:?} | log lines {} -> {} (+{} for {} acks)",
        pct(&mut with_ticker.clone(), 0.5),
        pct(&mut with_ticker.clone(), 0.95),
        with_ticker.iter().max().unwrap(),
        log0,
        log1,
        log1 - log0,
        n
    );

    ticker.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut without = Vec::with_capacity(n);
    for _ in 0..n {
        without.push(post(path.clone(), json!({ "seq": -1 })).await.0);
    }
    eprintln!(
        "no-op ack NO ticker:  p50={:?} p95={:?} max={:?}",
        pct(&mut without.clone(), 0.5),
        pct(&mut without.clone(), 0.95),
        without.iter().max().unwrap()
    );

    // GET /state for scale
    let mut gets = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        http.get(format!("{base}/api/rooms/{id}/state"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        gets.push(t.elapsed());
    }
    eprintln!(
        "GET /state:           p50={:?} p95={:?}",
        pct(&mut gets.clone(), 0.5),
        pct(&mut gets.clone(), 0.95)
    );
}

/// Repeats what the Board does: one ack per session per seq change, plus the
/// ack of the release frame itself. Counts them against real engine commits.
#[tokio::test]
async fn ack_traffic_per_commit() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let store = dummy::Store::new();
    let (base, srv, ticker) = spawn(&store).await;
    let http = reqwest::Client::new();
    let mut sesis = Vec::new();
    for name in ["A", "B"] {
        let tok: String = http
            .post(format!("{base}/api/session"))
            .json(&json!({ "player": name }))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .into();
        sesis.push(tok);
    }
    let host = sesis[0].clone();
    let call = |tok: String, m: &'static str, p: String, b: Value| {
        let http = http.clone();
        let base = base.clone();
        async move {
            let r = http
                .request(reqwest::Method::from_bytes(m.as_bytes()).unwrap(), format!("{base}{p}"))
                .bearer_auth(tok)
                .json(&b)
                .send()
                .await
                .unwrap();
            r.json::<Value>().await.unwrap_or(Value::Null)
        }
    };
    let id: String = call(host.clone(), "POST", "/api/rooms".into(), json!({ "name": "Ack" }))
        .await["room"]["id"]
        .as_str()
        .unwrap()
        .into();
    call(sesis[1].clone(), "POST", format!("/api/rooms/{id}/join"), json!({})).await;
    for _ in 0..2 {
        call(host.clone(), "POST", format!("/api/rooms/{id}/bots"), json!({ "op": "add" })).await;
    }
    call(host.clone(), "POST", format!("/api/rooms/{id}/start"), json!({ "force": true })).await;
    {
        let room = srv.room(&id).unwrap();
        room.lock().unwrap().game.as_ref().expect("match").quick_start().unwrap();
    }

    let acks = Arc::new(AtomicUsize::new(0));
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut tasks = Vec::new();
    for tok in sesis.clone() {
        let (http, base, id, acks) = (http.clone(), base.clone(), id.clone(), acks.clone());
        tasks.push(tokio::spawn(async move {
            let mut last = -1;
            while Instant::now() < deadline {
                let v: Value = http
                    .get(format!("{base}/api/rooms/{id}/state"))
                    .bearer_auth(&tok)
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                let seq = v["match"]["state"]["seq"].as_i64().unwrap_or(-1) as i32;
                if seq != last {
                    last = seq;
                    acks.fetch_add(1, Ordering::Relaxed);
                    http.post(format!("{base}/api/rooms/{id}/anim-done"))
                        .bearer_auth(&tok)
                        .json(&json!({ "seq": seq }))
                        .send()
                        .await
                        .unwrap();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    ticker.abort();
    let blob = store.match_get(&id).unwrap().unwrap().len();
    let n = acks.load(Ordering::Relaxed);
    eprintln!(
        "8s of play, 2 seats mirroring the Board's ack rule: {n} anim-done POSTs, blob {blob} B => ~{} KiB of blob GET+PUT written",
        n * blob * 2 / 1024
    );
    let lines = store.record_log_get(&id).unwrap();
    eprintln!(
        "log: {} lines, {} AnimDone",
        lines.len(),
        lines.iter().filter(|l| l.contains("AnimDone")).count()
    );
}

/// The release frame makes **every** connected session ack at once. Measure
/// that burst, and what a spectator's ack gets.
#[tokio::test]
async fn concurrent_ack_burst() {
    let store = dummy::Store::new();
    let (base, srv, _ticker) = spawn(&store).await;
    let http = reqwest::Client::new();
    let mut sesis = Vec::new();
    for i in 0..6 {
        let tok: String = http
            .post(format!("{base}/api/session"))
            .json(&json!({ "player": format!("p{i}") }))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .into();
        sesis.push(tok);
    }
    let post = |tok: String, p: String, b: Value| {
        let http = http.clone();
        let base = base.clone();
        async move {
            let t = Instant::now();
            let r = http
                .post(format!("{base}{p}"))
                .bearer_auth(tok)
                .json(&b)
                .send()
                .await
                .unwrap();
            (t.elapsed(), r.status().as_u16())
        }
    };
    post(sesis[0].clone(), "/api/rooms".into(), json!({ "name": "Burst" })).await;
    let id: String = {
        // The helper returns the status only, so read the id from the list.
        let v: Value = http
            .get(format!("{base}/api/rooms"))
            .bearer_auth(&sesis[0])
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        v.as_array().unwrap()[0]["id"].as_str().unwrap().into()
    };
    // 2 seats, 4 spectators
    for tok in sesis.iter().skip(1).take(2) {
        post(tok.clone(), format!("/api/rooms/{id}/join"), json!({})).await;
    }
    for _ in 0..2 {
        post(sesis[0].clone(), format!("/api/rooms/{id}/bots"), json!({ "op": "add" })).await;
    }
    post(sesis[0].clone(), format!("/api/rooms/{id}/start"), json!({ "force": true })).await;
    {
        let room = srv.room(&id).unwrap();
        room.lock().unwrap().game.as_ref().expect("match").quick_start().unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut lat = Vec::new();
    let mut codes = std::collections::BTreeMap::new();
    for round in 0..50 {
        let mut set = Vec::new();
        for tok in sesis.clone() {
            set.push(post(tok, format!("/api/rooms/{id}/anim-done"), json!({ "seq": round % 3 })));
        }
        for (d, c) in futures_util::future::join_all(set).await {
            lat.push(d);
            *codes.entry(c).or_insert(0) += 1;
        }
    }
    let (n_ok, n_400) = (codes.get(&200).copied().unwrap_or(0), codes.get(&400).copied().unwrap_or(0));
    eprintln!(
        "6 concurrent ackers x50: p50={:?} p95={:?} max={:?} | 200={n_ok} 400={n_400} codes={codes:?}",
        pct(&mut lat.clone(), 0.5),
        pct(&mut lat.clone(), 0.95),
        lat.iter().max().unwrap()
    );
}

/// A refused act is the same shape as the old no-op ack: full path, no state.
#[tokio::test]
async fn refused_act_cost() {
    let store = dummy::Store::new();
    let (base, _srv, _ticker) = spawn(&store).await;
    let http = reqwest::Client::new();
    let token: String = http
        .post(format!("{base}/api/session"))
        .json(&json!({ "player": "Host" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .into();
    let post = |p: String, b: Value| {
        let (http, base, token) = (http.clone(), base.clone(), token.clone());
        async move {
            http.post(format!("{base}{p}"))
                .bearer_auth(token)
                .json(&b)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let id: String = post("/api/rooms".into(), json!({ "name": "Refuse" })).await["room"]["id"]
        .as_str()
        .unwrap()
        .into();
    for _ in 0..2 {
        post(format!("/api/rooms/{id}/bots"), json!({ "op": "add" })).await;
    }
    post(format!("/api/rooms/{id}/start"), json!({ "force": true })).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = store.match_get(&id).unwrap().unwrap();
    let mut lat = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let r = http
            .post(format!("{base}/api/rooms/{id}/act"))
            .bearer_auth(&token)
            .json(&json!({ "act": "bogus" }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 400);
        lat.push(t.elapsed());
    }
    let after = store.match_get(&id).unwrap().unwrap();
    eprintln!(
        "refused act x100: p50={:?} p95={:?} | blob unchanged: {}",
        pct(&mut lat.clone(), 0.5),
        pct(&mut lat.clone(), 0.95),
        before == after
    );
}
