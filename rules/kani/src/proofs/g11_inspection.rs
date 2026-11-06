//! G11 inspection / reveal host-model obligations (design
//! `spec/abi/G11-information-and-deck.md` §9).
//!
//! These pin the bounded **host model** of the four new ops -- the frame is
//! the state the whole feature hangs on, so its move discipline is proved
//! directly. The `inspected` raises are not modelled (Kani dispatches no
//! hooks), and the model keeps one frame with one owner flag; that is enough
//! for every order / refill / no-touch obligation here.
//!
//! | harness | obligation |
//! |---|---|
//! | `inspection_begin_holds_at_most_count_and_preserves_deck_order` | I4/I5 |
//! | `inspection_end_restores_the_deck` | I5 (order unwound) |
//! | `inspection_end_shuffle_keeps_the_multiset` | I5 (a shuffling return keeps the multiset) |
//! | `peek_flip_leaves_the_card_in_the_discard` | I2 (no refill while held) |
//! | `refill_is_suppressed_iff_a_frame_holds_the_seat_open` | I2 |
//! | `reveal_touches_no_pile` | I7 |
//!
//! `rules/kani/tests/conf_model.rs` checks the same arms against the engine.

use card_sdk::kani_host::{Args, Op};

use crate::host;
use crate::intern::{intern, NameId};
use crate::world::{self, World, MAX_CARDS};

fn install_empty() {
    host::install();
    world::reset_world(World::new());
}

fn call(op: Op, a: Args) -> i64 {
    host::dispatch(op, a)
}

/// A guest name handle for the interned card id (`ctx::s` tags its handles;
/// the arms decode through the same facade).
fn handle(id: u16) -> i32 {
    card_sdk::intern::encode_handle(NameId(id))
}

fn ids() -> [u16; 4] {
    [
        intern("PP:[标题偶像]").0,
        intern("PP:[强韧之花]").0,
        intern("PP:[虽然很突然]").0,
        intern("PP:[练习生解密指南]").0,
    ]
}

/// Fill the deck's front with `n` distinct cards.
fn fill_deck(n: usize) {
    let ids = ids();
    world::with_world_mut(|w| {
        let mut i = 0usize;
        while i < n {
            world::pile_push(&mut w.deck_pile, &mut w.deck_n, ids[i]);
            i += 1;
        }
    });
}

fn deck_snapshot() -> ([u16; MAX_CARDS], u8) {
    world::with_world(|w| {
        let mut a = [u16::MAX; MAX_CARDS];
        let mut i = 0usize;
        while i < w.deck_n as usize {
            a[i] = w.deck_pile[i];
            i += 1;
        }
        (a, w.deck_n)
    })
}

/// Count of `card` among `pile[..n]`.
fn count_in(pile: &[u16; MAX_CARDS], n: u8, card: u16) -> usize {
    let mut c = 0usize;
    let mut i = 0usize;
    while i < n as usize {
        if pile[i] == card {
            c += 1;
        }
        i += 1;
    }
    c
}

/// `inspection_begin` turns at most `count` cards, top-first, and touches
/// nothing else: the held prefix is the old top in order and the deck
/// remainder is exactly what followed it.
#[kani::proof]
#[kani::unwind(20)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn inspection_begin_holds_at_most_count_and_preserves_deck_order() {
    install_empty();
    let deck: u8 = kani::any();
    kani::assume(deck <= 4);
    let count: i32 = kani::any();
    kani::assume((0..=4).contains(&count));
    fill_deck(deck as usize);
    let (before, before_n) = deck_snapshot();
    let got = call(Op::inspection_begin, [0, 0, count, 0, 0, 0]) as usize;
    let moved = (count.max(0) as usize).min(deck as usize);
    kani::cover!(moved == 0, "empty or zero count");
    kani::cover!(moved == deck as usize && deck > 0, "whole deck held");
    kani::cover!(moved > 0 && moved < deck as usize, "partial hold");
    crate::kassert!(got == moved, "begin answers the held count");
    world::with_world(|w| {
        crate::kassert!(
            w.inspection_n as usize == moved,
            "the frame holds exactly the clamped count"
        );
        crate::kassert!(
            w.deck_n as usize == before_n as usize - moved,
            "the held prefix left the deck"
        );
        let mut i = 0usize;
        while i < moved {
            crate::kassert!(
                w.inspection_pile[i] == before[i],
                "the frame is the old top, in order"
            );
            i += 1;
        }
        let mut i = 0usize;
        while i < w.deck_n as usize {
            crate::kassert!(
                w.deck_pile[i] == before[i + moved],
                "the deck remainder is untouched"
            );
            i += 1;
        }
    });
}

/// Returning the frame unwinds it onto the deck's front in the original
/// order (no shuffle).
#[kani::proof]
#[kani::unwind(20)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn inspection_end_restores_the_deck() {
    install_empty();
    let deck: u8 = kani::any();
    kani::assume(deck <= 4);
    let count: i32 = kani::any();
    kani::assume((0..=4).contains(&count));
    fill_deck(deck as usize);
    let (before, before_n) = deck_snapshot();
    let moved = call(Op::inspection_begin, [0, 0, count, 0, 0, 0]) as usize;
    let released = call(Op::inspection_end, [0, 0, 0, 0, 0, 0]) as usize;
    kani::cover!(moved > 0, "a look with something held");
    kani::cover!(moved == 0, "an empty look");
    crate::kassert!(released == moved, "end answers the released count");
    world::with_world(|w| {
        crate::kassert!(w.inspection_n == 0, "the frame is gone");
        crate::kassert!(
            w.deck_n as usize == before_n as usize,
            "every held card is back"
        );
        let mut i = 0usize;
        while i < before_n as usize {
            crate::kassert!(w.deck_pile[i] == before[i], "order restored exactly");
            i += 1;
        }
    });
}

/// A shuffling return keeps the multiset (the model stub keeps the order too;
/// the engine's real shuffle is checked by the conformance suite). A concrete
/// deck keeps the O(n^2) multiset check small enough for the solver.
#[kani::proof]
#[kani::unwind(20)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn inspection_end_shuffle_keeps_the_multiset() {
    install_empty();
    fill_deck(4);
    let (before, before_n) = deck_snapshot();
    let count: i32 = kani::any();
    kani::assume((0..=4).contains(&count));
    let moved = call(Op::inspection_begin, [0, 0, count, 0, 0, 0]) as usize;
    let released = call(Op::inspection_end, [0, 1, 0, 0, 0, 0]) as usize;
    kani::cover!(moved > 0, "a look with something held");
    crate::kassert!(released == moved, "end answers the released count");
    world::with_world(|w| {
        crate::kassert!(w.inspection_n == 0, "the frame is gone");
        crate::kassert!(
            w.deck_n as usize == before_n as usize,
            "every held card is back"
        );
        let mut i = 0usize;
        while i < before_n as usize {
            crate::kassert!(
                count_in(&w.deck_pile, w.deck_n, before[i])
                    == count_in(&before, before_n, before[i]),
                "multiset restored"
            );
            i += 1;
        }
    });
}

/// With a frame holding the seat's deck, filing a card must not refill -- the
/// flipped card stays in the discard instead of being shuffled straight back.
#[kani::proof]
#[kani::unwind(16)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn peek_flip_leaves_the_card_in_the_discard() {
    install_empty();
    fill_deck(2);
    let ids = ids();
    call(Op::inspection_begin, [0, 0, 2, 0, 0, 0]);
    // Flip the held card by name (`ctx::to_discard` from the frame).
    let held = world::with_world(|w| w.inspection_pile[0]);
    call(Op::to_discard, [0, handle(held), 0, 0, 0, 0]);
    world::with_world(|w| {
        crate::kassert!(w.deck_n == 0, "the peek emptied the deck");
        crate::kassert!(w.discard_n == 1, "the flipped card stayed in the discard");
        crate::kassert!(
            count_in(&w.deck_pile, w.deck_n, held) == 0,
            "no refill shuffled it back"
        );
    });
}

/// The refill gate is exactly the open frame: closed, the same write refills.
#[kani::proof]
#[kani::unwind(16)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn refill_is_suppressed_iff_a_frame_holds_the_seat_open() {
    install_empty();
    let ids = ids();
    let frame: bool = kani::any();
    if frame {
        fill_deck(1);
        call(Op::inspection_begin, [0, 0, 1, 0, 0, 0]);
    }
    call(Op::to_discard, [0, handle(ids[1]), 0, 0, 0, 0]);
    let refilled = world::with_world(|w| w.deck_n > 0);
    kani::cover!(frame, "held");
    kani::cover!(!frame, "not held");
    crate::kassert!(
        refilled == !frame,
        "refill happens exactly when no frame holds the deck"
    );
}

/// `reveal_pile` / `reveal_card` publish ids only: no pile, no frame count and
/// no order moves.
#[kani::proof]
#[kani::unwind(16)]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn reveal_touches_no_pile() {
    install_empty();
    fill_deck(2);
    call(Op::inspection_begin, [0, 0, 1, 0, 0, 0]);
    call(Op::to_discard, [0, handle(ids()[2]), 0, 0, 0, 0]);
    let (deck, deck_n) = deck_snapshot();
    let (disp, disp_n) = world::with_world(|w| {
        let mut a = [u16::MAX; MAX_CARDS];
        let mut i = 0usize;
        while i < w.discard_n as usize {
            a[i] = w.discard_pile[i];
            i += 1;
        }
        (a, w.discard_n)
    });
    let (hold, hold_n) = world::with_world(|w| {
        let mut a = [u16::MAX; MAX_CARDS];
        let mut i = 0usize;
        while i < w.inspection_n as usize {
            a[i] = w.inspection_pile[i];
            i += 1;
        }
        (a, w.inspection_n)
    });
    // Reveal a pile and one card.
    call(Op::reveal_pile, [0, 2, 0, 0, 0, 0]);
    call(Op::reveal_card, [0, handle(ids()[0]), 0, 0, 0, 0]);
    world::with_world(|w| {
        crate::kassert!(w.deck_n == deck_n, "deck count unchanged");
        crate::kassert!(w.discard_n == disp_n, "discard count unchanged");
        crate::kassert!(w.inspection_n == hold_n, "frame count unchanged");
        let mut i = 0usize;
        while i < deck_n as usize {
            crate::kassert!(w.deck_pile[i] == deck[i], "deck order unchanged");
            i += 1;
        }
        let mut i = 0usize;
        while i < disp_n as usize {
            crate::kassert!(w.discard_pile[i] == disp[i], "discard order unchanged");
            i += 1;
        }
        let mut i = 0usize;
        while i < hold_n as usize {
            crate::kassert!(w.inspection_pile[i] == hold[i], "frame order unchanged");
            i += 1;
        }
    });
}
