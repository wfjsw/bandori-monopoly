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
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 1000;
    }
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    w.discard[2] = kani::any(); // OTHER
    w.discard[3] = kani::any(); // EXTRA
    w.discard[0] = false;
    w.discard[1] = false;
    w.symbolic_answers = true;
    w
}

#[kani::proof]
#[kani::unwind(32)]
pub fn aya_haruka_01_prompt_all_picks_iff() {
    let w = symbolic_play_world();
    kani::assume(w.discard[2] || w.discard[3]);
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

/// Empty discard: place still applies; the add-to-hand half has no target.
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_01_empty_discard() {
    let mut w = symbolic_play_world();
    w.discard = [false; MAX_CARDS];
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
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn aya_haruka_01_mutant_two_cards() {
    let w = symbolic_play_world();
    kani::assume(w.discard[2] || w.discard[3]);
    let player = w.trig_player;
    let pre_discard = w.discard;
    world::reset_world(w);
    invoke::unwrap_done(invoke::aya_longing_play(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    assert!(
        spec::aya_haruka_01_effects_ok_mutant(&logbuf[..log_n], player, 0, 2, &pre_discard),
        "mutant: card must not match the two-card spec"
    );
}

#[allow(dead_code)]
fn _touch() {
    let _ = (CARD_AYA, CARD_OTHER, CARD_EXTRA);
}
