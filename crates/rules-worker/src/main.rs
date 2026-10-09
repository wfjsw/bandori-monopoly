//! Stateless match worker -- one long-running process, many requests, and no
//! shared mutable state between them. See `lib.rs` for the protocol.
//!
//! ```json
//! {"id":1,"op":"new","members":[...],"seed256":"<hex64>","mode":0,"weights":{...}}
//! {"id":2,"op":"act","state":"...","member":1,"cmd":{...}}
//! ```
//!
//! Run `cargo run -p rules-worker` and pipe those in to poke at it.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rules_worker::{handle, Ctx};
use serde_json::Value;

fn arg(name: &str, env: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| std::env::var(env).ok())
        .unwrap_or_else(|| default.to_string())
}

fn main() {
    let data_dir = PathBuf::from(arg("--data", "BM_DATA", "data"));
    let rules_dir = PathBuf::from(arg("--rules", "BM_RULES", "dist/cards"));
    let ctx = Ctx::load(&data_dir, &rules_dir);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            // stdin closed: a clean exit, not a crash.
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let out = match serde_json::from_str::<Value>(&line) {
            Ok(req) => handle(&ctx, req),
            // A bad request is answered, not fatal: the process stays up and
            // keeps serving the pool.
            Err(e) => serde_json::json!({"ok": false, "error": format!("bad request json: {e}")}),
        };
        let _ = writeln!(stdout, "{out}");
        let _ = stdout.flush();
    }
}
