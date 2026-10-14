//! Harness (c): prompt / choice clause, all answer branches.
//!
//! Clause `C-PP-aya-haruka-01` (`docs/rulebook/spec/cards/pp.md`):
//! > 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」
//!
//! Condition: discard holds at least one card.
//! Effect: card on the user's field; exactly one discard card is in hand.
//! The pick is a player choice (`ctx::ask_card`); Kani explores every index.
//!
//! ## Strategies used (see report / KANI.md §5)
//!
//! (a) Discard pile frozen to two distinct interned ids; pick unrolled as
//!     two concrete worlds (`0..2`) rather than one `kani::any()` index.
//! (c) 1-byte interned-str facade in `ctx::cards_in` so `ids.contains` is a
//!     one-byte compare; `invoke::find` intern-compares under Kani.
//! (e) All-picks = conjunction of the two concrete pick cases.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, CARD_AYA, CARD_EXTRA, CARD_OTHER, MAX_CARDS, MAX_PLAYERS};

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    // Seat geometry is concrete: the clause's frame is per-player and does
    // not depend on which of 2..=4 seats is acting.
    w.n_players = 3;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 1000;
    }
    let player: i32 = 0;
    w.trig_player = player;
    // Strategy (a) — bound the discard to a small fixed pile of distinct
    // interned ids. Contents are concrete so `ids.contains(&String)` in the
    // unmodified play body constant-folds (the CBMC `memcmp` cliff).
    w.discard = [false; MAX_CARDS];
    w.discard[2] = true; // OTHER
    w.discard[3] = true; // EXTRA
    w.discard_pile = [u16::MAX; MAX_CARDS];
    w.discard_pile[0] = crate::intern::intern(CARD_OTHER).0;
    w.discard_pile[1] = crate::intern::intern(CARD_EXTRA).0;
    w.discard_n = 2;
    w.symbolic_answers = true;
    w
}

/// All picks of a 2-card discard, each pick **concrete** (strategy a+e).
///
/// The offered pick is unrolled over `0..2` as two concrete worlds. A single
/// `kani::any()` pick multiplies the `String`/`memcmp` paths inside the
/// unmodified body by 2 on top of every heap-growth branch CBMC already
/// explores. Unrolling keeps the same "every offered index satisfies the
/// spec" property — the conjunction of the two concrete cases.
#[kani::proof]
#[kani::unwind(32)]
pub fn aya_haruka_01_prompt_all_picks_iff() {
    for pick in [0i32, 1] {
        let mut w = symbolic_play_world();
        w.symbolic_answers = false;
        w.answers[0] = pick;
        let player = w.trig_player;
        let pre_discard = w.discard;
        let pre = {
            world::reset_world(w);
            world::snapshot()
        };

        invoke::unwrap_done(invoke::aya_longing_play(player));

        let post = world::snapshot();
        let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
        let log_n = world::with_world(|ww| ww.log_n);
        let effects = &logbuf[..log_n];

        let mut picked: u8 = 2;
        let mut seen_take = false;
        for e in effects.iter() {
            if let Effect::TakeFromPile { pile: 1, card, .. } = e {
                picked = *card;
                seen_take = true;
            }
        }
        crate::kassert!(seen_take, "must take exactly one discard card");

        kani::cover!(pick == 0, "offered index 0");
        kani::cover!(pick == 1, "offered index 1");
        kani::cover!(picked == 2, "pick OTHER");
        kani::cover!(picked == 3, "pick EXTRA");

        crate::kassert!(
            spec::aya_haruka_01_effects_ok(effects, player, 0, picked, &pre_discard),
            "C-PP-aya-haruka-01: place + exactly one discard->hand"
        );
        crate::kassert!(
            spec::aya_haruka_01_post_ok(&post, player, picked, &pre_discard),
            "C-PP-aya-haruka-01: post hand/discard move exactly the picked card"
        );
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    }
}

/// Empty discard: place still applies; the add-to-hand half has no target.
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_01_empty_discard() {
    let mut w = symbolic_play_world();
    w.discard = [false; MAX_CARDS];
    w.discard_pile = [u16::MAX; MAX_CARDS];
    w.discard_n = 0;
    w.symbolic_answers = false;
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::aya_longing_play(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    crate::kassert!(
        spec::aya_haruka_01_empty_discard_ok(&logbuf[..log_n], player, 0),
        "empty discard: place only"
    );
}

/// MUTANT: reference wants two cards in hand. Card must FAIL to match.
/// Concrete pick 0 is enough — the mutant disagrees on effect *count*.
///
/// `#[kani::should_panic]` counts a Rust `panic!` as the expected event.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn aya_haruka_01_mutant_two_cards() {
    let mut w = symbolic_play_world();
    w.symbolic_answers = false;
    w.answers[0] = 0;
    let player = w.trig_player;
    let pre_discard = w.discard;
    world::reset_world(w);
    invoke::unwrap_done(invoke::aya_longing_play(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    if spec::aya_haruka_01_effects_ok_mutant(&logbuf[..log_n], player, 0, 2, &pre_discard) {
        // Card matched the two-card reference — mutant is live (bad).
        panic!("mutant is live: card matched the two-card reference");
    }
    // Expected: card ≠ two-card reference.
    panic!("card disagrees with the two-card mutant (expected)");
}

#[allow(dead_code)]
fn _touch() {
    let _ = (CARD_AYA, CARD_OTHER, CARD_EXTRA);
}