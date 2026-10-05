//! `server [--port 8080] [--data data] [--static webui/dist] [--workers N]`
//!
//! Environment: `PORT`, `BM_DATA`, `BM_STATIC`, `BM_WORKER`, `BM_WORKERS`.

use std::path::PathBuf;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, StubRules};

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
    let port: u16 = arg("--port", "PORT", "8080")
        .parse()
        .expect("port must be a number");
    let data_dir = PathBuf::from(arg("--data", "BM_DATA", "data"));
    let static_dir = PathBuf::from(arg("--static", "BM_STATIC", "webui/dist"));

    let rules_dir = PathBuf::from(arg("--rules", "BM_RULES", "dist/cards"));
    let workers: usize = arg("--workers", "BM_WORKERS", "4")
        .parse()
        .expect("workers must be a number");
    let data = Arc::new(
        GameData::load(|f| std::fs::read_to_string(data_dir.join(f)).map_err(|e| e.to_string()))
            .unwrap_or_else(|e| panic!("cannot load game data from {}: {e}", data_dir.display())),
    );
    eprintln!(
        "loaded {} tiles, {} cards, {} characters",
        data.tiles.len(),
        data.cards.len(),
        data.characters.len()
    );

    // Card effects: one wasm module per card (tools/build-ruleset.sh), served to
    // the match shell through `WasmRules`. Without modules the match still plays,
    // every card just has no effect.
    let rules: Arc<dyn CardRules> = match game_rules::WasmRules::load_dir(data.clone(), &rules_dir)
    {
        Ok(Some(r)) => {
            eprintln!(
                "card modules: {} loaded from {}",
                r.ruleset().module_count(),
                rules_dir.display()
            );
            Arc::new(r)
        }
        Ok(None) => {
            eprintln!(
                "no card modules in {} (run tools/build-ruleset.sh) -- cards have no effect",
                rules_dir.display()
            );
            Arc::new(StubRules)
        }
        Err(e) => {
            eprintln!("card modules failed to load: {e:?} -- falling back to no effects");
            Arc::new(StubRules)
        }
    };
    // Matches run in `rules-worker` processes, not here: long-running, one per
    // slot, holding no match state between requests. See `server::pool`.
    let engine = server::pool::Pool::start(
        workers,
        server::pool::Pool::default_exe(),
        data_dir.clone(),
        rules_dir.clone(),
    );
    let server = server::Server::new(data, rules, engine);
    server::spawn_ticker(server.clone());
    let statics = static_dir.is_dir().then_some(static_dir);
    let app = server::router(server, Some(data_dir), statics);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("bind");
    eprintln!("listening on http://0.0.0.0:{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
