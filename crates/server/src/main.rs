//! `server [--port 8080] [--data data] [--static webui/dist] [--workers N] [--redis URL] [--bot-service PATH] [--bot-threads N] [--bot-search-threads N]`
//!
//! Environment: `PORT`, `BM_DATA`, `BM_STATIC`, `BM_WORKER`, `BM_WORKERS`, `BM_REDIS`,
//! `BM_BOT_SERVICE`, `BOT_THREADS`, `BOT_SEARCH_THREADS`.

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
    // Cross-endpoint state: Redis to share/retain it across processes, the
    // in-memory store otherwise (one process, lost on restart).
    let redis_url = arg("--redis", "BM_REDIS", "");
    let store: Arc<dyn server::store::CrossState> = if redis_url.is_empty() {
        eprintln!("cross-endpoint state: in-memory (pass --redis URL to share or retain it)");
        Arc::new(server::store::dummy::Store::new())
    } else {
        eprintln!("cross-endpoint state: redis at {redis_url}");
        Arc::new(server::store::redis::Store::connect(&redis_url).expect("redis store"))
    };
    let mut server = server::Server::new(data, rules, engine, store);
    // Advanced-bot decisions (`docs/BOT.md` B5): optional. Absent (no
    // `--bot-service` / `BM_BOT_SERVICE`) means advanced bots play as standard
    // -- logged once, here.
    let bot_exe = arg("--bot-service", "BM_BOT_SERVICE", "");
    let bots = if bot_exe.is_empty() {
        eprintln!(
            "bot-service: not configured -- advanced bots play as standard \
             (pass --bot-service PATH or BM_BOT_SERVICE)"
        );
        None
    } else {
        let threads: usize = arg("--bot-threads", "BOT_THREADS", "4")
            .parse()
            .unwrap_or(4);
        // Root-parallel searches per decision. Default = the request-worker
        // count; the outer deadline (`budget + 1.5 s`, `docs/BOT.md` §5 B7)
        // leaves room for one iteration overrun at this fan-out.
        let search_threads: usize = arg("--bot-search-threads", "BOT_SEARCH_THREADS", &threads.to_string())
            .parse()
            .unwrap_or(threads);
        match server::botsvc::BotService::start(
            PathBuf::from(&bot_exe),
            data_dir.clone(),
            rules_dir.clone(),
            threads,
            search_threads,
        ) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!(
                    "bot-service failed to start ({e}) -- advanced bots play as standard"
                );
                None
            }
        }
    };
    {
        let s = Arc::get_mut(&mut server).expect("fresh server handle");
        s.bots = bots;
    }
    // Bring back anything the store still knows about, so a restart is silent.
    server.restore_rooms();
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
