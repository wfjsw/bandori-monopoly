//! `profile-server [--port 8081] [--host 127.0.0.1] [--data data/profile]
//! [--static webui/dist] [--gamedata data] [--secret HEX]`
//!
//! Environment: `PORT`, `BM_HOST`, `BM_PROFILE_DATA`, `BM_STATIC`,
//! `BM_GAME_DATA`, `BM_PROFILE_SECRET`.
//!
//! The secret signs this service's own session tickets. Game servers never see
//! it -- they identify players by name -- so it only has to be stable here.
//! With no secret one is generated for the life of the process and printed;
//! that logs everyone out on restart, so a real deploy passes `--secret`.

use std::path::PathBuf;
use std::sync::Arc;

use profile_server::store::Store;
use profile_server::{random_secret, App};

fn arg(name: &str, env: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| std::env::var(env).ok())
        .unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() {
    let port: u16 = arg("--port", "PORT", "8081").parse().expect("port must be a number");
    let host = arg("--host", "BM_HOST", "127.0.0.1");
    let data_dir = PathBuf::from(arg("--data", "BM_PROFILE_DATA", "data/profile"));
    let static_dir = PathBuf::from(arg("--static", "BM_STATIC", "webui/dist"));
    // Read-only game tables for the SPA's first paint (the menu renders before
    // a game server is picked). Not used for matches.
    let game_data = PathBuf::from(arg("--gamedata", "BM_GAME_DATA", "data"));
    let mut secret = arg("--secret", "BM_PROFILE_SECRET", "");
    if secret.is_empty() {
        secret = random_secret();
        eprintln!("no --secret; generated one for this process:");
        eprintln!("BM_PROFILE_SECRET={secret}");
    }

    let store = Store::open(&data_dir).unwrap_or_else(|e| panic!("cannot open {}: {e}", data_dir.display()));
    eprintln!("profile data: {}", data_dir.display());
    let app = Arc::new(App {
        store,
        secret: secret.into_bytes(),
        started: profile_server::now(),
    });
    let statics = static_dir.is_dir().then_some(static_dir);
    let data = game_data.is_dir().then_some(game_data);
    let router = profile_server::router(app, statics, data);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .expect("bind");
    eprintln!("profile server listening on {}", listener.local_addr().expect("local addr"));
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("profile server");
}
