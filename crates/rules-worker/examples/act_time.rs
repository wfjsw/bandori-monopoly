//! Time one act on a saved state, with the real ruleset or `StubRules`, to
//! separate engine bookkeeping from rule execution. Throwaway harness.
//!
//! cargo run --release -p rules-worker --example act_time -- <state.json> <member> <act> [stub]

use std::path::Path;
use std::time::Instant;

use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::NetMessage;
use std::sync::Arc;

fn main() {
    let mut args = std::env::args().skip(1);
    let state_path = args.next().expect("state path");
    let member: i32 = args.next().and_then(|s| s.parse().ok()).expect("member");
    let act = args.next().expect("act");
    let stub = args.next().as_deref() == Some("stub");

    let data_dir = std::env::var("BM_DATA").unwrap_or_else(|_| "data".into());
    let rules_dir = std::env::var("BM_RULES").unwrap_or_else(|_| "dist/cards".into());
    let ctx = rules_worker::Ctx::load(Path::new(&data_dir), Path::new(&rules_dir));
    let rules: Arc<dyn CardRules> = if stub {
        Arc::new(StubRules)
    } else {
        ctx.rules.clone()
    };
    let state = std::fs::read_to_string(&state_path).expect("read state");
    let t = Instant::now();
    let mut m = Match::restore(ctx.data.clone(), rules, &state).expect("restore");
    println!("restore {:?}", t.elapsed());
    let cmd = NetMessage {
        act,
        ..Default::default()
    };
    let t = Instant::now();
    let out = m.act(member, &cmd);
    println!("act {} {:?} err={}", cmd.act, t.elapsed(), out.is_err());
}
