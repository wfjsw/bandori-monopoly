//! STACK-01: nested `card_settle_at` must not recurse the Rust stack.
//!
//! A placed `TEST:settleNest` re-settles its tile while its crystal counter
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
    t.set_crystals(0, "TEST:settleNest", DEPTH);

    // 512 KiB: a quarter of a `spawn_blocking` worker, an eighth of main.
    // Master's `card_settle_at ↔ settle_at` cycle aborts here at DEPTH.
    // `spawn_unchecked` because `Match` is single-threaded (not `Send`); the
    // closure exclusively owns it for the duration of the join.
    let ptr = &mut t as *mut Table as usize;
    let h = unsafe {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn_unchecked(move || {
                let t = &mut *(ptr as *mut Table);
                eprintln!("rolling on 512 KiB");
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
                let left = t.crystals(0, "TEST:settleNest").unwrap_or(0);
                assert!(
                    left < DEPTH,
                    "settle chain did not run (crystals still {left})"
                );
            })
            .expect("spawn 512 KiB thread")
    };
    h.join().expect("settle chain panicked or overflowed");
}

/// Place `card` on `who`'s field at board `tile` (same seam as rb_cross_tiles).
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
