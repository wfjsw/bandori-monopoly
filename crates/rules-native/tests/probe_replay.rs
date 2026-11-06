//! Throwaway probe: replay a `.bdrec` and print every event naming a `tile:`
//! card, so the moment a tile rule reaches a hand can be read off.
//!
//!   FILE=/tmp/a.bdrec cargo test -p rules-native --test probe_replay -- --nocapture

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::CardRules;
use game_core::record::{decode_record, Replayer};
use game_rules::WasmRules;

#[test]
fn probe() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            rules_native::roster(),
        )
        .expect("game data"),
    );
    let cards = std::env::var_os("RULES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards"));
    let rules = Arc::new(
        WasmRules::load_dir(data.clone(), &cards)
            .expect("cards")
            .expect("built"),
    ) as Arc<dyn CardRules>;
    let bytes = std::fs::read(std::env::var("FILE").unwrap()).unwrap();
    let rec = decode_record(&bytes).expect("decode");
    eprintln!(
        "recorded ruleset {} abi {} bundle {}",
        rec.header.engine.ruleset_sha256, rec.header.engine.abi, rec.header.engine.bundle
    );
    eprintln!("loaded ruleset {:?}", rules.ruleset_sha256());
    let mut rp = Replayer::new(data, rules, &rec, true).expect("open");
    let dump_at: usize = std::env::var("DUMP_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let mut dumped = false;
    let mut applied = 0usize;
    let total = rp.total_ticks();
    let mut last = 0i32;
    let mut n = 0usize;
    while !rp.status().ended && rp.status().tick < total {
        rp.next_input();
        for e in rp.events_since(last) {
            if e.card.starts_with("tile:") {
                eprintln!(
                    "tick={} ev#{} type={} card={} pid={} from={} to={} msg={:?}",
                    rp.status().tick,
                    e.id,
                    e.r#type,
                    e.card,
                    e.player_id,
                    e.from,
                    e.to,
                    e.msg
                );
            }
            if rp.status().tick <= 60 {
                eprintln!(
                    "  early tick={} ev#{} type={} card={:?} pid={} msg={:?}",
                    rp.status().tick,
                    e.id,
                    e.r#type,
                    e.card,
                    e.player_id,
                    e.msg
                );
            }
            last = e.id;
        }
        applied += 1;
        if !dumped && applied >= dump_at {
            dumped = true;
            let out = std::env::var("OUT").unwrap();
            std::fs::create_dir_all(&out).unwrap();
            std::fs::write(std::path::Path::new(&out).join("save.json"), rp.inner().save()).unwrap();
            eprintln!(
                "dumped save after {applied} inputs at tick {}",
                rp.status().tick
            );
            let m = rp.inner();
            for (i, p) in m.state().players.iter().enumerate() {
                let h = &m.world().hidden[i];
                eprintln!(
                    "  p{i} ready={} hand={} deck={:?}",
                    p.deck_ready,
                    h.hand.len(),
                    h.deck
                );
            }
            eprintln!("  phase={} boardDiscard={:?}", m.state().phase, m.world().st.board_discard);
        }
        let mut hit = false;
        for (i, h) in rp.inner().world().hidden.iter().enumerate() {
            for c in h.hand.iter().filter(|c| c.starts_with("tile:")) {
                eprintln!(
                    "HAND tick={} p{i} card={c} hand={:?}",
                    rp.status().tick,
                    h.hand
                );
                hit = true;
            }
        }
        if hit {
            for e in rp.events_since(last.saturating_sub(15)) {
                eprintln!(
                    "  ev#{} tick={} type={} card={} pid={} msg={:?}",
                    e.id,
                    rp.status().tick,
                    e.r#type,
                    e.card,
                    e.player_id,
                    e.msg
                );
            }
            break;
        }
        n += 1;
        if n % 500 == 0 {
            eprintln!(
                "… {n} inputs, tick {}/{total} diverged={}",
                rp.status().tick,
                rp.status().diverged
            );
        }
    }
    eprintln!(
        "done: tick {}/{total} diverged={} mismatch={:?}",
        rp.status().tick,
        rp.status().diverged,
        rp.first_mismatch()
    );
}
