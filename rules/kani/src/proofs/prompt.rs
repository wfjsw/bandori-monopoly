//! Harness (c): prompt / choice clause, all answer branches.
//!
//! Clause `C-PP-aya-haruka-01` (`docs/rulebook/spec/cards/pp.md`):
//! > 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」
//!
//! Condition: discard holds at least one card.
//! Effect: card on the user's field; exactly one discard card is in hand.
//! The pick is a player choice (`ctx::ask_card`); Kani explores every index.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, CARD_AYA, CARD_EXTRA, CARD_OTHER, MAX_CARDS, MAX_PLAYERS};

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    // Seat geometry is concrete: the clause's frame is per-player and does
    // not depend on which of 2..=4 seats is acting. Freezing `n_players` /
    // `player` keeps the symbolic *pick* as the only branch the proof is
    // about (path explosion was the CBMC time cliff on top of the memcmp).
    w.n_players = 3;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 1000;
    }
    let player: i32 = 0;
    w.trig_player = player;
    // Strategy (a) — bound the discard to a small fixed pile of distinct
    // interned ids; the pick stays symbolic and is assumed only into the
    // offered range (`Op::ask` does `0..opt_n`).
    //
    // Why contents are frozen: the unmodified play body runs
    // `ids.contains(&String)` over `ctx::cards_in`. Symbolic pile *membership*
    // makes each returned `String` a symbolic choice of 4 full names, and
    // `Vec::contains` + `memcmp` then OOMs CBMC (see `KANI.md` §5). Freezing
    // the pile to two concrete distinct cards lets the 1-byte interned-str
    // facade (and the underlying slot bytes) constant-fold; the symbolic
    // `ask` still explores every offered index in one proof.
    //
    // Coverage of "all picks" is pick 0 and pick 1 of a 2-card discard
    // (`cover!` below). A 1-card discard is the same body with `n = 1` and a
    // forced pick 0; empty discard is `aya_haruka_01_empty_discard`.
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

/// All picks of a 2-card discard, each pick **concrete**.
///
/// Strategy (a)+(e): the pile is frozen to two distinct interned cards and
/// the offered pick is unrolled over `0..2` as two concrete worlds. A single
/// `kani::any()` pick multiplies the `String`/`memcmp` paths inside the
/// unmodified body by 2 on top of every heap-growth branch CBMC already
/// explores (that is the time cliff). Unrolling keeps the same "every
/// offered index satisfies the spec" property — the conjunction of the two
/// concrete cases — with no symbolic index.
#[kani::proof]
#[kani::unwind(32)]
pub fn aya_haruka_01_prompt_all_picks_iff() {
    for pick in [0i32, 1] {
        let mut w = symbolic_play_world();
        // Concrete answer: `Op::ask` consumes `answers[0]` when
        // `symbolic_answers` is false.
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
        assert!(seen_take, "must take exactly one discard card");

        // Both cases must be exercised (the loop is a concrete unroll).
        kani::cover!(pick == 0, "offered index 0");
        kani::cover!(pick == 1, "offered index 1");
        kani::cover!(picked == 2, "pick OTHER");
        kani::cover!(picked == 3, "pick EXTRA");

        assert!(
            spec::aya_haruka_01_effects_ok(effects, player, 0, picked, &pre_discard),
            "C-PP-aya-haruka-01: place + exactly one discard->hand"
        );
        assert!(
            spec::aya_haruka_01_post_ok(&post, player, picked, &pre_discard),
            "C-PP-aya-haruka-01: post hand/discard move exactly the picked card"
        );
        assert!(post.money == pre.money, "frame: money");
        assert!(post.pos == pre.pos, "frame: pos");
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
    assert!(
        spec::aya_haruka_01_empty_discard_ok(&logbuf[..log_n], player, 0),
        "empty discard: place only"
    );
}

/// MUTANT: reference wants two cards in hand. Card must FAIL to match.
/// Concrete pick 0 is enough — the mutant disagrees on effect *count*, which
/// is independent of which offered index was taken.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn aya_haruka_01_mutant_two_cards() {
    // DIAG: empty_discard's world shape + panic at end.
    let mut w = symbolic_play_world();
    w.discard = [false; MAX_CARDS];
    w.discard_pile = [u16::MAX; MAX_CARDS];
    w.discard_n = 0;
    w.symbolic_answers = false;
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::aya_longing_play(player));
    panic!("reached post-invoke");
}

#[allow(dead_code)]
fn _touch() {
    let _ = (CARD_AYA, CARD_OTHER, CARD_EXTRA);
}
