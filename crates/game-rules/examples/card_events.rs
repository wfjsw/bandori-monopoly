//! Card-activation counts per seeded real-rules game: 4 named personas,
//! seeds `1 2 3 5 8`, 40 rounds. A before/after pair for a guard→condition
//! batch -- `card` events may drop, every other kind must be identical.
//!
//!   CARGO_TARGET_DIR=C:/cond-target CARGO_PROFILE_DEV_DEBUG=0 \
//!     cargo run -p game-rules --example card_events -- [out.txt] [max_rounds]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use game_core::data::{GameData, Roster};
use game_core::engine::{CardRules, Match};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchEvent};
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

/// The four seats: the owners of the two cards this batch touched, plus the
/// first two roster personas.
const CHARS: [&str; 4] = ["朝日六花", "纯田真奈", "户山香澄", "花园多惠"];
const SEEDS: [u64; 5] = [1, 2, 3, 5, 8];

fn load_data(roster: Roster) -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .expect("game data"),
    )
}

fn load_ruleset() -> Ruleset {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    Ruleset::load_dir(&dir)
        .expect("read dist/cards")
        .expect("run `node tools/build-ruleset.mjs` first")
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let out = raw
        .iter()
        .find(|a| a.ends_with(".txt") || a.contains('/') || a.contains('\\'))
        .cloned()
        .unwrap_or_else(|| "target/scratch/card_events.txt".to_string());
    let max_rounds = raw
        .iter()
        .filter_map(|a| a.parse::<i32>().ok())
        .next()
        .unwrap_or(40);

    let ruleset = load_ruleset();
    let data = load_data(ruleset.roster());
    let rules: Arc<dyn CardRules> = Arc::new(WasmRules::new(ruleset, data.clone()));

    let mut lines: Vec<String> = Vec::new();
    let mut total: BTreeMap<String, usize> = BTreeMap::new();
    for seed in SEEDS {
        let members: Vec<RoomMember> = (1..=4)
            .map(|i| RoomMember {
                id: i,
                player: format!("P{i}"),
                bot: true,
                mentality: BotMentality::Standard,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(
            data.clone(),
            rules.clone(),
            &members,
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        // Seat `k` plays `CHARS[k]` (Table::with_seed's convention).
        for (k, c) in CHARS.iter().enumerate() {
            m.set_persona(k as i32 + 1, c);
        }
        m.quick_start();
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut card_by_kind: BTreeMap<String, usize> = BTreeMap::new();
        let mut last_event = 0i32;
        let mut last_round = -1i32;
        while !m.ended() {
            m.tick(0.25);
            for e in m.events_since(last_event) {
                last_event = e.id;
                count(&mut kinds, &e, &mut card_by_kind);
            }
            let st = m.state();
            if st.round > max_rounds {
                m.finish();
                for e in m.events_since(last_event) {
                    last_event = e.id;
                    count(&mut kinds, &e, &mut card_by_kind);
                }
                break;
            }
            last_round = st.round.max(last_round);
        }
        let st = m.state();
        lines.push(format!(
            "seed={seed} END round={} reason={} kinds={kinds:?}",
            st.round, st.end_reason
        ));
        lines.push(format!("seed={seed} cardTriggers={card_by_kind:?}"));
        for (k, v) in kinds {
            *total.entry(k).or_default() += v;
        }
    }
    lines.push(format!("TOTAL kinds={total:?}"));
    lines.push(format!("TOTAL chars={CHARS:?} seeds={SEEDS:?} max_rounds={max_rounds}"));

    let path = Path::new(&out);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, lines.join("\n") + "\n").expect("write out");
    println!("wrote {} lines to {out}", lines.len());
    println!("TOTAL kinds={total:?}");
}

fn count(kinds: &mut BTreeMap<String, usize>, e: &MatchEvent, card_by_kind: &mut BTreeMap<String, usize>) {
    *kinds.entry(e.r#type.clone()).or_default() += 1;
    if e.r#type == "card" {
        *card_by_kind.entry(e.kind.clone()).or_default() += 1;
        // 「无事发生」: the activation's body changed nothing. Counted so a
        // before/after pair can see how often a flash was pure noise.
        if e.results.iter().any(|r| r.key() == "log.effect_nothing") {
            *kinds.entry("card:effect_nothing".to_string()).or_default() += 1;
        }
    }
}