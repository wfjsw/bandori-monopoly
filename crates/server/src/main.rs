//! `server [--port 8080] [--host 127.0.0.1] [--data data] [--static webui/dist] [--rules dist/cards] [--builds DIR] [--workers N] [--redis URL] [--bot-service PATH] [--bot-threads N] [--bot-search-threads N]`
//!
//! Environment: `PORT`, `BM_DATA`, `BM_STATIC`, `BM_WORKER`, `BM_WORKERS`, `BM_REDIS`,
//! `BM_BOT_SERVICE`, `BOT_THREADS`, `BOT_SEARCH_THREADS`, `BM_SSE_KEEPALIVE` (seconds, default 30).

use std::path::PathBuf;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, StubRules};
use serde::Deserialize;

/// One build from the manifest (see [`read_builds`]).
#[derive(Debug, Clone, Deserialize)]
struct BuildSpec {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    /// Ruleset directory, relative to the manifest unless absolute.
    rules: String,
    /// Game tables directory; absent = the server's `--data`.
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    default: bool,
}

/// Resolve the build list. No `--builds` = the lone `--rules` set as `default`.
fn read_builds(dir: &std::path::Path, rules: &std::path::Path, data: &std::path::Path) -> Vec<BuildSpec> {
    if dir.as_os_str().is_empty() {
        return vec![BuildSpec {
            id: "default".into(),
            name: "默认规则".into(),
            description: String::new(),
            rules: rules.to_string_lossy().into_owned(),
            data: Some(data.to_string_lossy().into_owned()),
            default: true,
        }];
    }
    let text = std::fs::read_to_string(dir.join("builds.json"))
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.join("builds.json").display()));
    let mut specs: Vec<BuildSpec> =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("bad builds.json: {e}"));
    let mut seen = std::collections::HashSet::new();
    for s in &mut specs {
        if s.id.is_empty() || !s.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            panic!("bad build id {:?} (ASCII letters/digits/-/_ only)", s.id);
        }
        if !seen.insert(s.id.clone()) {
            panic!("duplicate build id {:?}", s.id);
        }
        if s.name.is_empty() {
            s.name = s.id.clone();
        }
        if !std::path::Path::new(&s.rules).is_absolute() {
            s.rules = dir.join(&s.rules).to_string_lossy().into_owned();
        }
        s.data = Some(match &s.data {
            Some(d) if !std::path::Path::new(d).is_absolute() => dir.join(d).to_string_lossy().into_owned(),
            Some(d) => d.clone(),
            None => data.to_string_lossy().into_owned(),
        });
    }
    if specs.len() > 16 {
        panic!("more than 16 builds");
    }
    specs
}

fn load_build(spec: BuildSpec, workers: usize) -> (server::state::Build, usize) {
    let rules_path = PathBuf::from(&spec.rules);
    let data_path = PathBuf::from(spec.data.clone().expect("resolved"));
    let ruleset = match game_rules::Ruleset::load_dir(&rules_path) {
        Ok(Some(set)) => Some(set),
        Ok(None) => {
            eprintln!("no card modules in {} -- cards have no effect", rules_path.display());
            None
        }
        Err(e) => {
            eprintln!("card modules in {} failed to load: {e:?} -- falling back to no effects", rules_path.display());
            None
        }
    };
    let modules = ruleset.as_ref().map(|s| s.module_count()).unwrap_or(0);
    let roster = ruleset.as_ref().map(|s| s.roster()).unwrap_or_default();
    let data = Arc::new(
        GameData::load(|f| std::fs::read_to_string(data_path.join(f)).map_err(|e| e.to_string()), roster)
            .unwrap_or_else(|e| panic!("cannot load game data from {}: {e}", data_path.display())),
    );
    let rules: Arc<dyn CardRules> = match ruleset {
        Some(set) => Arc::new(game_rules::WasmRules::new(set, data.clone())),
        None => Arc::new(StubRules),
    };
    let engine = server::pool::Pool::start(workers, server::pool::Pool::default_exe(), data_path.clone(), rules_path.clone());
    (
        server::state::Build {
            id: spec.id,
            name: spec.name,
            description: spec.description,
            data_path,
            rules_path,
            data,
            rules,
            engine,
        },
        modules,
    )
}

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
    // Loopback by default: the dev server proxies /api and /data here, so the
    // backend need not be reachable from outside. `--host ::` / `0.0.0.0` (or
    // `BM_HOST`) exposes it, e.g. for a deploy that serves `--static` directly.
    let host = arg("--host", "BM_HOST", "127.0.0.1");
    let data_dir = PathBuf::from(arg("--data", "BM_DATA", "data"));
    let static_dir = PathBuf::from(arg("--static", "BM_STATIC", "webui/dist"));

    let rules_dir = PathBuf::from(arg("--rules", "BM_RULES", "dist/cards"));
    let builds_dir = PathBuf::from(arg("--builds", "BM_BUILDS", ""));
    let workers: usize = arg("--workers", "BM_WORKERS", "4")
        .parse()
        .expect("workers must be a number");
    // Selectable rule builds (`docs/SERVER.md` §Rule builds). A manifest
    // (`--builds DIR/builds.json`) lists them; without one, the single
    // `--rules` set is the default build. Each build loads its own roster and
    // tables and gets its own worker pool, so matches on different builds
    // never share engine identity.
    let specs = read_builds(&builds_dir, &rules_dir, &data_dir);
    let mut builds = std::collections::HashMap::new();
    let mut default_id = specs.first().map(|s| s.id.clone()).unwrap_or_default();
    for spec in &specs {
        if spec.default {
            default_id = spec.id.clone();
        }
        let (b, modules) = load_build(spec.clone(), workers);
        eprintln!(
            "build {}: {modules} card modules, {} cards (rules {}){}",
            b.id,
            b.data.cards.len(),
            b.rules_path.display(),
            if b.id == default_id { " [default]" } else { "" }
        );
        builds.insert(b.id.clone(), Arc::new(b));
    }
    if builds.is_empty() {
        panic!("no rule builds (pass --rules or --builds)");
    }
    let default = builds.get(&default_id).or_else(|| builds.values().next()).cloned().expect("a build");
    let default_id = default.id.clone();
    let (data, rules, engine) = (default.data.clone(), default.rules.clone(), default.engine.clone());
    // Card modules for the bot service and the in-process fallback load from
    // the default build.
    let rules_dir = default.rules_path.clone();
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
    {
        let s = Arc::get_mut(&mut server).expect("fresh server handle");
        s.builds = builds;
        s.default_build = default_id;
    }
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
        let search_threads: usize = arg(
            "--bot-search-threads",
            "BOT_SEARCH_THREADS",
            &threads.to_string(),
        )
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
                eprintln!("bot-service failed to start ({e}) -- advanced bots play as standard");
                None
            }
        }
    };
    {
        let s = Arc::get_mut(&mut server).expect("fresh server handle");
        s.bots = bots;
        let secs: u64 = arg("--sse-keepalive", "BM_SSE_KEEPALIVE", "30")
            .parse()
            .expect("sse-keepalive must be a number of seconds");
        s.sse_keepalive = std::time::Duration::from_secs(secs.max(1));
    }
    // Bring back anything the store still knows about, so a restart is silent.
    server.restore_rooms();
    server::spawn_ticker(server.clone());
    let statics = static_dir.is_dir().then_some(static_dir);
    let app = server::router(server, Some(data_dir), statics);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .expect("bind");
    eprintln!(
        "listening on {}",
        listener.local_addr().expect("local addr")
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
