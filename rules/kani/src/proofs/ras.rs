//! Unit `ras`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op + covers + mutant.
//! Specs live in `crate::spec::ras` and come from the clause docs only.
//!
//! Isolation assumes are named here and `kani::assume`d. Narration (`ctx::log`)
//! is not part of the frame.

use crate::host;
use crate::spec::ras as spec;
use crate::world::{self, Effect, World, MAX_CARDS, MAX_PLAYERS};

/// Call an `On` body without the registry string scan (`invoke::find` is a
/// memcmp cliff under CBMC). The unmodified `fn(i32) -> Asked` is taken from
/// the exported `CardDef`.
fn body_of(on: &card_sdk::On) -> fn(i32) -> card_sdk::Asked {
    match on {
        card_sdk::On::Play(_, _, f)
        | card_sdk::On::Counteract(_, _, _, f)
        | card_sdk::On::Hook(_, _, _, f)
        | card_sdk::On::Gate(_, _, _, f)
        | card_sdk::On::RollPlan(_, _, f)
        | card_sdk::On::AtEnd(_, _, f)
        | card_sdk::On::Settle(_, _, f)
        | card_sdk::On::Message(_, _, _, f) => *f,
    }
}

fn unwrap_done(r: card_sdk::Asked) {
    match r {
        Ok(()) => {}
        Err(_) => panic!("unexpected EXIT_NEED_INPUT"),
    }
}

// ===========================================================================
// C-RAS-unstoppable-card-03
// 「若骰点为1-3获得2000资金，若为4-6则获得1000资金」
//
// Isolation: the teleport half is BLOCKED-HOST (`tile_named` cannot resolve
// 「白雪学园」…`CHUCHU的公寓` and `card_move` is a plan stub), so the body's
// teleport branch is never entered and the effect log is the money half only.
// The money clause itself does not depend on that branch.
// ===========================================================================

fn symbolic_unstoppable_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 8_000);
        w.out[p] = kani::any();
        // stay == 0 ⇒ cant_move is false (the play gate admits)
        w.stay[p] = 0;
    }
    w
}

/// Iff: living player gains exactly the printed money for the rolled face.
#[kani::proof]
#[kani::unwind(24)]
pub fn unstoppable_card_03_money_iff() {
    let w = symbolic_unstoppable_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    unwrap_done(body_of(&card_ras::unstoppable::UNSTOPPABLE.on[0])(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    // The roll is symbolic 1..=6 inside `ctx::roll(1, 6)`. Recover the face
    // from the money amount the body granted (the only observable).
    let gained = post.money[player as usize] - pre.money[player as usize];
    kani::cover!(gained == 2000, "face 1-3 grants 2000");
    kani::cover!(gained == 1000, "face 4-6 grants 1000");

    let r = if gained == 2000 {
        1
    } else if gained == 1000 {
        4
    } else {
        // Must not happen: the body only grants those two figures.
        crate::kassert!(false, "C-RAS-unstoppable-card-03: unexpected money delta");
        0
    };
    let expected = spec::unstoppable_card_03_money(r);
    crate::kassert!(
        gained == expected,
        "C-RAS-unstoppable-card-03: money must be 2000 / 1000 by face"
    );
    crate::kassert!(
        spec::unstoppable_card_03_log_ok(effects, player, expected),
        "C-RAS-unstoppable-card-03: effect log is exactly one GainMoney"
    );
    // Frame: nothing but the money moved.
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: no movement (teleport is BLOCKED-HOST)");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire unchanged");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand unchanged");
    crate::kassert!(post.discard == pre.discard, "frame: discard unchanged");
    for q in 0..MAX_PLAYERS {
        if q != player as usize {
            crate::kassert!(
                post.money[q] == pre.money[q],
                "frame: other players' money unchanged"
            );
        }
    }
}

/// No-op: an out player gains nothing.
#[kani::proof]
#[kani::unwind(24)]
pub fn unstoppable_card_03_money_noop_when_out() {
    let w = symbolic_unstoppable_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(w.out[player as usize]);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    unwrap_done(body_of(&card_ras::unstoppable::UNSTOPPABLE.on[0])(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    crate::kassert!(
        spec::unstoppable_card_03_out_log_ok(effects),
        "C-RAS-unstoppable-card-03: out player produces no effects"
    );
    crate::kassert!(post == pre, "C-RAS-unstoppable-card-03: out player leaves the frame");
}

/// Mutant companion: the off-by-one money spec does NOT match the card.
#[kani::proof]
#[kani::unwind(24)]
pub fn unstoppable_card_03_money_mutant_is_detected() {
    let w = symbolic_unstoppable_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    unwrap_done(body_of(&card_ras::unstoppable::UNSTOPPABLE.on[0])(player));
    let post = world::snapshot();
    let gained = post.money[player as usize] - pre.money[player as usize];

    // Pin a face in the high band (the mutant only differs there).
    kani::assume(gained == 1000);
    let r = 4;
    let wrong = spec::unstoppable_card_03_money_mutant(r);
    crate::kassert!(
        gained != wrong,
        "mutant: card must not match the off-by-one money spec"
    );
}

// ===========================================================================
// C-RAS-riot-02 / C-RAS-riot-03
// 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。」
//
// The [反击] *window* (when the body runs) is PROTOCOL-LEVEL. This harness
// invokes `On::Counteract`'s unmodified body directly — the leaf effect.
//
// Isolation: hands are the per-player `hand_resonance` view (the only
// per-player hand `cards_in(Hand)` reads under Kani), at most 2 cards per
// player so the 24-slot effect log cannot overflow. The global pilot bitset
// is left empty.
// ===========================================================================

fn symbolic_riot_world() -> (World, [usize; MAX_PLAYERS], [[u8; 4]; MAX_PLAYERS]) {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    let mut hand_ns = [0usize; MAX_PLAYERS];
    let mut hands = [[0u8; 4]; MAX_PLAYERS];
    for p in 0..MAX_PLAYERS {
        w.out[p] = kani::any();
        let h: i32 = kani::any();
        kani::assume(h >= 0 && h <= 2);
        w.hand_resonance[p] = h;
        w.hand[p] = false;
        hand_ns[p] = h as usize;
        for k in 0..h as usize {
            hands[p][k] = 1;
        }
    }
    // bitset empty (isolation)
    for s in 0..MAX_CARDS {
        w.hand[s] = false;
    }
    (w, hand_ns, hands)
}

/// Iff: every living player's hand is discarded then redrawn; the holder +1.
#[kani::proof]
#[kani::unwind(32)]
pub fn riot_02_03_cycle_iff() {
    let (w, hand_ns, hands) = symbolic_riot_world();
    let holder: i32 = kani::any();
    kani::assume(holder >= 0 && holder < w.n_players);
    kani::assume(!w.out[holder as usize]);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    unwrap_done(body_of(&card_ras::riot::RIOT.on[0])(holder));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    let n = pre.n_players;
    kani::cover!(
        hand_ns[holder as usize] == 0,
        "holder hand empty (extra draw only)"
    );
    kani::cover!(
        hand_ns[holder as usize] > 0,
        "holder hand non-empty (cycle + extra)"
    );

    crate::kassert!(
        spec::riot_cycle_log_ok(effects, &hands, &hand_ns, &pre.out, n, holder),
        "C-RAS-riot-02/03: discard-all then redraw h (+1 for holder)"
    );
    crate::kassert!(
        spec::riot_cycle_frame_ok(&pre, &post),
        "C-RAS-riot-02: the cycle itself moves no money / pos / fire"
    );
}

/// Mutant companion: dropping the holder's extra draw disagrees with the card.
#[kani::proof]
#[kani::unwind(32)]
pub fn riot_03_extra_draw_mutant_is_detected() {
    let (w, hand_ns, hands) = symbolic_riot_world();
    let holder: i32 = kani::any();
    kani::assume(holder >= 0 && holder < w.n_players);
    kani::assume(!w.out[holder as usize]);
    // Need the holder to actually hold a card so h vs h+1 differ in the log.
    kani::assume(hand_ns[holder as usize] >= 1);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    unwrap_done(body_of(&card_ras::riot::RIOT.on[0])(holder));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let n = pre.n_players;

    // Rebuild the expected log with the mutant draw count (no extra).
    let mut i = 0usize;
    let mut ok = true;
    for p in 0..n {
        let p = p as usize;
        if pre.out[p] {
            continue;
        }
        for k in 0..hand_ns[p] {
            match effects.get(i) {
                Some(Effect::DiscardFromHand { player, card }) => {
                    if *player != p as i32 || *card != hands[p][k] {
                        ok = false;
                    }
                }
                _ => ok = false,
            }
            i += 1;
        }
    }
    for p in 0..n {
        let p = p as usize;
        if pre.out[p] {
            continue;
        }
        let expect = spec::riot_draw_count_mutant(hand_ns[p] as i32, p as i32 == holder);
        if expect <= 0 {
            continue;
        }
        match effects.get(i) {
            Some(Effect::Draw { player, n: dn }) => {
                if *player != p as i32 || *dn != expect {
                    ok = false;
                }
            }
            _ => ok = false,
        }
        i += 1;
    }
    if i != effects.len() {
        ok = false;
    }
    crate::kassert!(!ok, "mutant: card must not match the no-extra-draw spec");
}

// ===========================================================================
// C-RAS-pareo-01
// 「获得2个PAREO标记，…」
//
// Isolation: no owned tile with houses (the optional 「移除…一层房屋」 offer
// is not opened), and the holder is not 鳰原令王那 (`character_skill` models
// "none" under Kani, so 「视为你的房屋总数增加」 has no listener).
// Sub-cap: `PAREO标记` starts at 0 or 1 so the requested +2 stays ≤ 3.
// Full-cap (start ≥ 2) is BLOCKED-HOST: `Op::add_held` ignores the `max`
// the body names (3) and always logs +2.
// ===========================================================================

fn symbolic_pareo_world(pareo_key: u16) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.out[p] = kani::any();
        // isolation: no houses anywhere ⇒ no optional removal prompt
        w.tiles[p].houses = 0;
    }
    // `World::new` already has `houses = 0` on every tile — the old 60-trip
    // walk tripped the unwinding assertion at unwind 32 (class A).
    w
}

/// Iff (sub-cap): exactly +2 PAREO marks and nothing else.
#[kani::proof]
#[kani::unwind(24)]
pub fn pareo_01_marks_iff() {
    let pareo_key = crate::intern::intern("PAREO标记").0;
    let mut w = symbolic_pareo_world(pareo_key);
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    // sub-cap isolation (see module docs)
    let pre_marks: i32 = kani::any();
    kani::assume(pre_marks >= 0 && pre_marks <= 1);
    w.key_set(player, crate::intern::intern("PAREO标记"), pre_marks);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    unwrap_done(body_of(&card_ras::pareo_far::PAREO_FAR.on[0])(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let marks_after = world::with_world(|ww| {
        ww.key_get(player, crate::intern::intern("PAREO标记"))
    });

    kani::cover!(pre_marks == 0, "start at 0 marks");
    kani::cover!(pre_marks == 1, "start at 1 mark");

    crate::kassert!(
        spec::pareo_01_log_ok(effects, player, pareo_key),
        "C-RAS-pareo-01: effect log is exactly one +2 PAREO AddToken"
    );
    crate::kassert!(
        marks_after == pre_marks + spec::pareo_01_mark_delta(),
        "C-RAS-pareo-01: marks must be pre+2"
    );
    crate::kassert!(
        spec::pareo_01_frame_ok(&pre, &post),
        "C-RAS-pareo-01: frame untouched"
    );
}

/// Mutant companion: +3 marks is not what the card does.
#[kani::proof]
#[kani::unwind(24)]
pub fn pareo_01_marks_mutant_is_detected() {
    let pareo_key = crate::intern::intern("PAREO标记").0;
    let mut w = symbolic_pareo_world(pareo_key);
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.key_set(player, crate::intern::intern("PAREO标记"), 0);

    world::reset_world(w);
    unwrap_done(body_of(&card_ras::pareo_far::PAREO_FAR.on[0])(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::pareo_01_log_ok_mutant(effects, player, pareo_key),
        "mutant: card must not match the +3 marks spec"
    );
}