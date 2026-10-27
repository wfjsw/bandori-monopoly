//! Name-only identity and rule-build selection on the game server.
//!
//! A game server in a split deployment shares nothing with the profile server
//! but the player name: the client opens a session here by name. Rule builds
//! are advertised and a room binds one.

use std::sync::Arc;
use std::time::Duration;

use game_core::data::{GameData, Roster};
use game_core::engine::StubRules;
use reqwest::StatusCode;
use serde_json::{json, Value};
use server::Server;

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
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), roster)
            .unwrap(),
    )
}

async fn spawn() -> String {
    let d = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut server = Server::new(
        d.clone(),
        rules.clone(),
        server::pool::Pool::in_process(d, rules),
        Arc::new(server::store::dummy::Store::new()),
    );
    {
        let s = Arc::get_mut(&mut server).expect("fresh");
        s.presence_timeout = Duration::from_secs(20);
        s.time_scale = 5.0;
    }
    server::spawn_ticker(server.clone());
    let app = server::router(server, None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

async fn sign_in(base: &str, name: &str) -> String {
    let r = reqwest::Client::new()
        .post(format!("{base}/api/session"))
        .json(&json!({ "player": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    r.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn a_session_is_opened_by_name_alone() {
    let base = spawn().await;
    // No profile ticket is involved: the name is the identifier.
    let token = sign_in(&base, "灯").await;
    let v: Value = reqwest::Client::new()
        .get(format!("{base}/api/session"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["player"], "灯");
}

#[tokio::test]
async fn the_default_build_alias_serves_the_manifest_default() {
    // A manifest may name its default anything; a client that has not picked
    // one asks for `/builds/default/...`.
    let root = std::env::temp_dir().join(format!("bm-build-alias-{}", std::process::id()));
    let rules = root.join("rules");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(rules.join("index.json"), "{\"modules\":[]}").unwrap();
    std::fs::write(data_dir.join("board.json"), "{}").unwrap();

    let d = data();
    let ruleset: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let engine = server::pool::Pool::in_process(d.clone(), ruleset.clone());
    let mut server = Server::new(
        d.clone(),
        ruleset.clone(),
        engine.clone(),
        Arc::new(server::store::dummy::Store::new()),
    );
    {
        let s = Arc::get_mut(&mut server).expect("fresh");
        s.builds.insert(
            "live".into(),
            Arc::new(
                server::state::Build::new(
                    "live".into(),
                    "Live".into(),
                    String::new(),
                    data_dir,
                    rules,
                    d,
                    ruleset,
                    0,
                )
                .with_engine(engine),
            ),
        );
        s.default_build = "live".into();
    }
    server::spawn_ticker(server.clone());
    let app = server::router(server, None, None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let index = reqwest::get(format!("http://{addr}/builds/default/rules/index.json"))
        .await
        .unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    assert_eq!(index.text().await.unwrap(), "{\"modules\":[]}");
    let board = reqwest::get(format!("http://{addr}/builds/default/data/board.json"))
        .await
        .unwrap();
    assert_eq!(board.status(), StatusCode::OK);
    let live = reqwest::get(format!("http://{addr}/builds/live/data/board.json"))
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);
}

#[tokio::test]
async fn builds_are_advertised_and_a_room_binds_one() {
    let base = spawn().await;
    let http = reqwest::Client::new();
    let token = sign_in(&base, "Host").await;

    let builds: Value = http
        .get(format!("{base}/api/builds"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list = builds.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], "default");
    assert_eq!(list[0]["default"], true);

    let r = http
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Built", "build": "default" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["room"]["build"], "default");

    let r = http
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Nope", "build": "missing" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
