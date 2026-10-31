//! Diagnostic: replay a `.bdrec` on the sandboxed rules (what the browser runs)
//! and report where it ends / first divergence.
//!
//!   FILE=/tmp/a.bdrec cargo test -p rules-native --test check_record -- --ignored --nocapture

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::CardRules;
use game_core::record::{decode_record, Replayer};
use game_rules::WasmRules;

#[test]
#[ignore]
fn check() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), rules_native::roster())
            .expect("game data"),
    );
    let cards = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    let rules = Arc::new(WasmRules::load_dir(data.clone(), &cards).expect("cards").expect("built")) as Arc<dyn CardRules>;
    let bytes = std::fs::read(std::env::var("FILE").unwrap()).unwrap();
    let rec = decode_record(&bytes).expect("decode");
    let mut rp = Replayer::new(data, rules, &rec, true).expect("open");
    let total = rp.total_ticks();
    while !rp.status().ended {
        let st = rp.step_ticks(8);
        if st.tick >= total {
            break;
        }
    }
    eprintln!(
        "total {total}, ended at {}, diverged {}",
        rp.status().tick,
        rp.status().diverged
    );
    if let Some(m) = rp.first_mismatch() {
        eprintln!("first mismatch: {m:?}");
    }
}
