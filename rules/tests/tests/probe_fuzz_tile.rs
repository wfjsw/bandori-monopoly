//! Throwaway probe: fuzz-drive many seeds and watch for a `tile:` rule name
//! reaching a hand / deck / graveyard *during play* (the arranged start state
//! is scrubbed of tile names first).

mod common;
mod fuzz;

use fuzz::drive::AnswerMode;
use fuzz::rng::Rng;

fn scrub_tiles(t: &mut common::Table) {
    let w = t.m.world_mut();
    for h in w.hidden.iter_mut() {
        h.hand.retain(|c| !c.starts_with("tile:"));
        h.deck.retain(|c| !c.starts_with("tile:"));
        h.graveyard.retain(|c| !c.starts_with("tile:"));
    }
}

fn watch(t: &common::Table, seed: u64) -> bool {
    let mut found = false;
    for (i, h) in t.m.world().hidden.iter().enumerate() {
        for (name, zone) in [("hand", &h.hand), ("deck", &h.deck), ("gy", &h.graveyard)] {
            for c in zone.iter() {
                if c.starts_with("tile:") {
                    eprintln!("HIT seed={seed} p{i} zone={name} card={c}");
                    found = true;
                }
            }
        }
    }
    found
}

#[test]
fn probe_fuzz_tile_in_hand() {
    let mut hits = 0u32;
    for seed in 0..600u64 {
        let (mut t, _cards) = fuzz::gen::arrange(seed ^ 0x51e1);
        scrub_tiles(&mut t);
        let mut rng = Rng::new(seed ^ 0xA5A5);
        let mut hit = false;
        for _turn in 0..3 {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fuzz::drive::drive(&mut t, &mut rng, 1, AnswerMode::Free)
            }));
            if r.is_err() {
                eprintln!("panic on seed {seed}");
                break;
            }
            if watch(&t, seed) {
                hit = true;
                break;
            }
        }
        if hit {
            hits += 1;
            if hits >= 5 {
                break;
            }
        }
    }
    eprintln!("seeds with tile in a pile: {hits}");
}
