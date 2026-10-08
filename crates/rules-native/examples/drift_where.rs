//! Where do the sandbox and native backends diverge? Prints the first
//! semantic difference between two `save()` checkpoints.
//!
//! ```
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!   cargo run -p rules-native --example drift_where -- 7 8
//! ```

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;
use game_rules::WasmRules;
use rules_native::native_rules;

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .expect("game data"),
    )
}

fn members() -> Vec<RoomMember> {
    (1..=4)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect()
}

fn run(rules: Arc<dyn CardRules>, data: &Arc<GameData>, seed: u64, cap: i32) -> Vec<String> {
    let mut m = Match::new(
        data.clone(),
        rules,
        &members(),
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut out = vec![];
    let mut last = -1;
    while !m.ended() {
        m.tick(0.25);
        let st = m.state();
        if st.round != last {
            last = st.round;
            out.push(m.save());
        }
        if st.round > cap {
            m.finish();
        }
    }
    out.push(m.save());
    out
}

fn diff(path: &str, a: &serde_json::Value, b: &serde_json::Value, out: &mut Vec<String>) {
    if out.len() > 30 {
        return;
    }
    match (a, b) {
        (serde_json::Value::Object(x), serde_json::Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let av = x.get(k);
                let bv = y.get(k);
                match (av, bv) {
                    (Some(av), Some(bv)) => diff(&format!("{path}.{k}"), av, bv, out),
                    (Some(av), None) => out.push(format!("{path}.{k} = only-sandbox {av}")),
                    (None, Some(bv)) => out.push(format!("{path}.{k} = only-native {bv}")),
                    (None, None) => {}
                }
            }
        }
        (serde_json::Value::Array(x), serde_json::Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path}: len {} vs {}", x.len(), y.len()));
            }
            for (i, (av, bv)) in x.iter().zip(y.iter()).enumerate() {
                diff(&format!("{path}[{i}]"), av, bv, out);
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path}: sandbox={a} native={b}"));
            }
        }
    }
}

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let cap: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let data = data();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let wasm = Arc::new(
        WasmRules::load_dir(data.clone(), &dir)
            .expect("dist/cards")
            .expect("build the ruleset first"),
    ) as Arc<dyn CardRules>;
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;

    println!("seed {seed} cap {cap}");
    let t0 = std::time::Instant::now();
    let a = run(wasm, &data, seed, cap);
    let t1 = std::time::Instant::now();
    let b = run(native, &data, seed, cap);
    let t2 = std::time::Instant::now();
    println!(
        "sandbox {:.1} ms/game   native {:.1} ms/game   speedup {:.1}x",
        t1.duration_since(t0).as_secs_f64() * 1e3,
        t2.duration_since(t1).as_secs_f64() * 1e3,
        t1.duration_since(t0).as_secs_f64() / t2.duration_since(t1).as_secs_f64()
    );
    println!("checkpoints: sandbox {} native {}", a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        if x == y {
            println!("checkpoint {i}: identical ({} bytes)", x.len());
            continue;
        }
        let av: serde_json::Value = serde_json::from_str(x).unwrap();
        let bv: serde_json::Value = serde_json::from_str(y).unwrap();
        let mut d = vec![];
        diff("root", &av, &bv, &mut d);
        println!("checkpoint {i}: DIVERGED, {} differences", d.len());
        for line in d.iter().take(30) {
            println!("  {line}");
        }
    }
}