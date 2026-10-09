//! First *event* divergence between the sandbox and native backends.
//!
//! Plays both backends in lockstep, draining `events_since` after every tick,
//! and prints the first event stream mismatch plus the surrounding events.
//!
//! ```
//! CARGO_TARGET_DIR=C:/native-target CARGO_INCREMENTAL=0 \
//!   cargo run -p rules-native --example drift_events -- 7 4
//! ```

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchEvent};
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

fn run_until(
    rules: Arc<dyn CardRules>,
    data: &Arc<GameData>,
    seed: u64,
    cap: i32,
    keep: usize,
) -> Vec<MatchEvent> {
    let mut m = Match::new(
        data.clone(),
        rules,
        &members(),
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut out: Vec<MatchEvent> = Vec::new();
    let mut last = 0i32;
    while !m.ended() {
        m.tick(0.25);
        for e in m.events_since(last) {
            last = e.id;
            out.push(e);
        }
        let st = m.state();
        if st.round > cap {
            m.finish();
            break;
        }
        if out.len() >= keep {
            break;
        }
    }
    out
}

fn brief(e: &MatchEvent) -> String {
    format!(
        "id={} type={} player={} other={} val={} from={} to={} dice={} card={:?} kind={} neg={} msg={:?}",
        e.id, e.r#type, e.player_id, e.other, e.value, e.from, e.to, e.dice, e.card, e.kind, e.negated, e.msg
    )
}

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let cap: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let keep: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(400);
    let data = data();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let wasm = Arc::new(
        WasmRules::load_dir(data.clone(), &dir)
            .expect("dist/cards")
            .expect("build the ruleset first"),
    ) as Arc<dyn CardRules>;
    let native = Arc::new(native_rules(data.clone())) as Arc<dyn CardRules>;

    println!("seed {seed} cap {cap} keep {keep}");
    let a = run_until(wasm, &data, seed, cap, keep);
    let b = run_until(native, &data, seed, cap, keep);
    println!("events: sandbox {} native {}", a.len(), b.len());

    let n = a.len().min(b.len());
    let mut first = None;
    for i in 0..n {
        if a[i] != b[i] {
            first = Some(i);
            break;
        }
    }
    match first {
        None => {
            if a.len() == b.len() {
                println!("event streams identical");
            } else {
                println!("prefix identical; length differs at {}", n);
                for e in a.iter().skip(n).take(5) {
                    println!("  sandbox only: {}", brief(e));
                }
                for e in b.iter().skip(n).take(5) {
                    println!("  native  only: {}", brief(e));
                }
            }
        }
        Some(i) => {
            println!("first divergent event at index {i}");
            let lo = i.saturating_sub(6);
            println!("--- sandbox context ---");
            for e in a.iter().take(i + 1).skip(lo) {
                println!("  {}", brief(e));
            }
            println!("--- native context ---");
            for e in b.iter().take(i + 1).skip(lo) {
                println!("  {}", brief(e));
            }
        }
    }
}