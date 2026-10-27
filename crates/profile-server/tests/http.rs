//! End-to-end over real HTTP: name sign-in, profile sync and the game-server
//! directory. There is no password -- the server is deliberately not an IdP.

use std::sync::Arc;

use profile_server::{App, Store};
use serde_json::{json, Value};

const SECRET: &[u8] = b"test-secret";

async fn spawn(dir: &std::path::Path) -> String {
    let app = Arc::new(App {
        store: Store::open(dir).unwrap(),
        secret: SECRET.to_vec(),
        started: profile_server::now(),
    });
    let router = profile_server::router(app, None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

async fn sign_in(base: &str, name: &str) -> String {
    let r = reqwest::Client::new()
        .post(format!("{base}/api/session"))
        .json(&json!({ "player": name, "persona": "高松灯" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    v["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn sign_in_is_stable_by_name() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let t1 = sign_in(&base, "灯").await;
    let t2 = sign_in(&base, "灯").await;
    // Both tickets are valid and name the same account.
    for t in [&t1, &t2] {
        let c = profile_token::verify(SECRET, t).unwrap();
        assert_eq!(c.name, "灯");
    }
    assert_eq!(profile_token::verify(SECRET, &t1).unwrap().sub, profile_token::verify(SECRET, &t2).unwrap().sub);
}

#[tokio::test]
async fn profile_round_trips_and_is_per_account() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let http = reqwest::Client::new();
    let a = sign_in(&base, "A").await;
    let b = sign_in(&base, "B").await;

    let r = http
        .put(format!("{base}/api/profile"))
        .bearer_auth(&a)
        .json(&json!({ "profile": "{\"coins\":7}" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let v: Value = http
        .get(format!("{base}/api/profile"))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["profile"], "{\"coins\":7}");

    let v: Value = http
        .get(format!("{base}/api/profile"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(v["profile"].is_null(), "B has no profile yet");
}

#[tokio::test]
async fn an_upload_id_traverses_to_the_data_root_but_no_further() {
    // Handler-level: a URL client normalizes `..` away before sending, so a
    // request to `/uploads/../accounts.json` never reaches this handler.
    let dir = tempdir();
    let app = std::sync::Arc::new(App {
        store: Store::open(&dir).unwrap(),
        secret: SECRET.to_vec(),
        started: profile_server::now(),
    });
    app.store.ensure_account("Owner", "", "", profile_server::now()).unwrap();
    // `uploads/..` needs `uploads/` to exist for path resolution.
    std::fs::create_dir_all(dir.join("uploads")).unwrap();

    let r = profile_server::serve_upload(
        axum::extract::State(app.clone()),
        axum::extract::Path(("..".to_string(), "accounts.json".to_string())),
    )
    .await;
    assert_eq!(r.status(), 200);
    assert!(r.headers().contains_key("content-type"));

    let r = profile_server::serve_upload(
        axum::extract::State(app),
        axum::extract::Path(("nope".to_string(), "accounts.json".to_string())),
    )
    .await;
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn concurrent_first_sign_ins_share_one_account() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let b = base.clone();
        set.spawn(async move { sign_in(&b, "灯").await });
    }
    let mut ids = std::collections::HashSet::new();
    while let Some(r) = set.join_next().await {
        let token = r.unwrap();
        ids.insert(profile_token::verify(SECRET, &token).unwrap().sub);
    }
    assert_eq!(ids.len(), 1, "one account per name");
}

#[tokio::test]
async fn requires_a_ticket() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let r = reqwest::Client::new()
        .get(format!("{base}/api/profile"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn server_directory_add_list_remove() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let http = reqwest::Client::new();
    let token = sign_in(&base, "Admin").await;

    let v: Value = http
        .post(format!("{base}/api/servers"))
        .bearer_auth(&token)
        .json(&json!({ "name": "本地", "url": "http://127.0.0.1:8080/", "note": "dev" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(v["url"], "http://127.0.0.1:8080", "trailing slash trimmed");

    let list: Value = http.get(format!("{base}/api/servers")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Anonymous writes are refused; reads stay public.
    let anon = http
        .post(format!("{base}/api/servers"))
        .json(&json!({ "name": "x", "url": "http://anon.example" }))
        .send()
        .await
        .unwrap();
    assert_eq!(anon.status(), 401);
    let anon = http.delete(format!("{base}/api/servers/{id}")).send().await.unwrap();
    assert_eq!(anon.status(), 401);

    let r = http.delete(format!("{base}/api/servers/{id}")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let list: Value = http.get(format!("{base}/api/servers")).send().await.unwrap().json().await.unwrap();
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn rejects_a_bad_server_url_and_duplicates() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let http = reqwest::Client::new();
    let token = sign_in(&base, "Admin").await;
    let bad = http
        .post(format!("{base}/api/servers"))
        .bearer_auth(&token)
        .json(&json!({ "name": "x", "url": "127.0.0.1:1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);

    for _ in 0..2 {
        let r = http
            .post(format!("{base}/api/servers"))
            .bearer_auth(&token)
            .json(&json!({ "name": "x", "url": "http://a.example" }))
            .send()
            .await
            .unwrap();
        if r.status() == 200 {
            continue;
        }
        assert_eq!(r.status(), 400);
    }
}

#[tokio::test]
async fn uploads_require_a_ticket() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let r = reqwest::Client::new()
        .post(format!("{base}/api/uploads?name=a.wasm&kind=ruleset"))
        .body("bytes")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn upload_round_trips_and_only_the_owner_removes_it() {
    let dir = tempdir();
    let base = spawn(&dir).await;
    let http = reqwest::Client::new();
    let owner = sign_in(&base, "Owner").await;
    let other = sign_in(&base, "Other").await;

    let r = http
        .post(format!("{base}/api/uploads?name=../Build%201.wasm&kind=ruleset"))
        .bearer_auth(&owner)
        .body(b"\0asm\x01\0\0\0".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(v["name"], "Build1.wasm", "path and space stripped");
    assert_eq!(v["size"], 8);
    assert_eq!(v["sha256"].as_str().unwrap().len(), 64);

    let bytes = http.get(format!("{base}{}", v["url"].as_str().unwrap())).send().await.unwrap();
    assert_eq!(bytes.status(), 200);
    assert_eq!(bytes.bytes().await.unwrap().as_ref(), b"\0asm\x01\0\0\0");

    let list: Value = http.get(format!("{base}/api/uploads")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    let r = http.delete(format!("{base}/api/uploads/{id}")).bearer_auth(&other).send().await.unwrap();
    assert_eq!(r.status(), 404, "not the owner");
    let r = http.delete(format!("{base}/api/uploads/{id}")).bearer_auth(&owner).send().await.unwrap();
    assert_eq!(r.status(), 200);

    let gone = http.get(format!("{base}/uploads/{id}/Build1.wasm")).send().await.unwrap();
    assert_eq!(gone.status(), 404);
}

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("bm-profile-{}-{}", std::process::id(), profile_server::random_secret()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
