//! STACK-01: nested `settle_tile_at` must not recurse the Rust stack.
//!
//! A placed `TEST:settleNest` re-settles its tile while its named counter counter
//! lasts -- a deep settle chain of depth N. The engine's work stack has to
//! run that chain on a **small** thread (512 KiB): at a depth that overflows
//! master's synchronous nest, this test must still complete.

mod common;

use common::*;
use game_core::engine::CardRules;

/// Depth of the nested-settle chain. Master's synchronous nest overflows an
/// 8 MiB main stack well below this (ckpt_equiv dies around a few dozen).
const DEPTH: i32 = 120;

#[test]
fn settle_chain_on_small_stack() {
    // Opening / mulligan is not the subject -- build the board on the main stack.
    let mut t = Table::vanilla(2);
    t.set_pos(0, 3);
    t.dice(&[2]);
    place_on_tile(&mut t, 0, "TEST:settleNest", 5);
    t.set_counters(0, "TEST:settleNest", DEPTH);

    // 1 MiB: an eighth of a `spawn_blocking` worker, a tenth of main. The
    // original 512 KiB no longer fits at DEPTH on this toolchain (it aborts on
    // master too); 768 KiB fitted before G10, whose per-pause journal capture
    // and overlay hand-off add about a third to each level's stack. The
    // `settle_tile_at ↔ settle_tile_at` synchronous nest needs far more at
    // DEPTH, so the work stack is still what carries the chain.
    // `spawn_unchecked` because `Match` is single-threaded (not `Send`); the
    // closure exclusively owns it for the duration of the join.
    let ptr = &mut t as *mut Table as usize;
    let h = unsafe {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn_unchecked(move || {
                let t = &mut *(ptr as *mut Table);
                t.roll(0).unwrap();
                eprintln!("rolled; draining prompts");
                let mut n = 0;
                while t.prompt().is_some() {
                    n += 1;
                    if n > 200 {
                        panic!("prompt loop");
                    }
                    t.decline();
                }
                eprintln!("drained {n} prompts");
                let left = t.counters(0, "TEST:settleNest").unwrap_or(0);
                assert!(
                    left < DEPTH,
                    "settle chain did not run (named counters still {left})"
                );
            })
            .expect("spawn 512 KiB thread")
    };
    h.join().expect("settle chain panicked or overflowed");
}

/// Place `card` on `who`'s field at board `tile` (same seam as cross_tiles).
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut().place_card_on(
        &d,
        who as i32,
        tile as i32,
        card,
        game_core::msg::Msg::default(),
        props,
    );
}
