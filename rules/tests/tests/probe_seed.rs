//! Throwaway probe: one fuzz seed, hands printed at every step.

mod common;
mod fuzz;

use fuzz::drive::AnswerMode;
use fuzz::rng::Rng;

fn dump(t: &common::Table, tag: &str) {
    let w = t.m.world();
    for (i, h) in w.hidden.iter().enumerate() {
        eprintln!("{tag} p{i} hand={:?}", h.hand);
    }
}

#[test]
fn probe_seed() {
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(13);
    let (mut t, _cards) = fuzz::gen::arrange(seed ^ 0x51e1);
    dump(&t, "AFTER_ARRANGE");
    let mut rng = Rng::new(seed ^ 0xA5A5);
    for turn in 0..4 {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fuzz::drive::drive(&mut t, &mut rng, 1, AnswerMode::Free)
        }));
        eprintln!("turn {turn} -> {}", if r.is_ok() { "ok" } else { "panic" });
        if r.is_err() {
            break;
        }
        dump(&t, "AFTER_TURN");
        let hits: Vec<(usize, String)> = t
            .m
            .world()
            .hidden
            .iter()
            .enumerate()
            .flat_map(|(i, h)| h.hand.iter().filter(|c| c.starts_with("tile:")).map(move |c| (i, c.clone())))
            .collect();
        eprintln!("turn {turn} tile-in-hand hits: {hits:?}");
        if !hits.is_empty() {
            let ev = t.m.state().events.clone();
            for e in ev.iter().rev().take(60) {
                eprintln!("  ev {} key={:?} card={:?}", e.r#type, spec_kit::event_key(e), e.card);
            }
        }
    }
}

#[test]
fn probe_deck_pool() {
    let d = common::data();
    for id in ["tile:circle", "tile:property", "event:x", "mark:cp", "status:stun"] {
        eprintln!("{id}: in cards={}", d.cards.iter().any(|c| c.id == id));
    }
    let c = d.persona("弦卷心").unwrap();
    eprintln!("preset len={} {:#?}", c.preset.len(), c.preset);
    eprintln!(
        "pool has circle={}",
        game_core::deck::pool(&d, c).iter().any(|k| k.id == "tile:circle")
    );
    eprintln!("clean keeps circle={:?}", game_core::deck::clean(&d, c, &["tile:circle"]));
}
