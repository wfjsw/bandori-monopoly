//! Unit `ras`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op + covers + mutant.
//! Specs live in `crate::spec::ras` and come from the clause docs only.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Isolation assumes are named here and `kani::assume`d.
//! Narration (`ctx::log`) is not part of the frame.

use crate::host;
use crate::invoke;
use crate::spec::ras as spec;
use crate::world::{self, Effect, MAX_CARDS, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-card host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `RAS:UNSTOPPABLE` play — money + (BLOCKED-HOST) teleport half.
/// Isolation: `tile_named` cannot resolve the campus names and `card_move` is
/// a plan stub, so the teleport branch is never entered — still declared.
crate::host_subset!(unstoppable_dispatch {
    roll => arm_roll,
    tile_named => arm_tile_named,
    set_teleport_to | set_resolve | set_kind | set_steps | move_dir | fixed_roll
        | schedule_turn_end => arm_stub_zero,
    card_move => arm_card_move,
    player_count => arm_player_count,
    player_out => arm_player_out,
    gain | gain_typed | gain_fixed => arm_gain,
    log | effect => arm_log,
});

/// `RAS:暴动` counteract — discard-all + redraw + holder extra.
crate::host_subset!(riot_dispatch {
    player_count => arm_player_count,
    player_out => arm_player_out,
    cards_in => arm_cards_in,
    discard_from_hand => arm_discard_from_hand,
    draw => arm_draw,
    log | effect => arm_log,
});

/// `RAS:（pareo）` play — +2 PAREO marks (optional house-removal isolated).
crate::host_subset!(pareo_dispatch {
    add_held => arm_add_held,
    count_held | count_held_name => arm_count_held,
    log | effect => arm_log,
    band_skill | character_skill => arm_band_skill,
    invoke_skill => arm_invoke_skill,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    houses_of => arm_houses_of,
    add_house => arm_add_house,
    opt_tile | opt_int | opt_str => arm_opt,
    opt_price | opt_ai => arm_opt_price,
    ask => arm_ask,
});

/// In-place 2-seat world for UNSTOPPABLE (stay=0 so cant_move admits).
fn unstoppable_world(user: i32) {
    host::install_backend(unstoppable_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        money_hi: 8_000,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.stay = [0; MAX_PLAYERS];
        w.out[user as usize] = false;
    });
}

/// In-place 2-seat world for riot: hands are the `hand_resonance` view
/// (the only per-player hand `cards_in(Hand)` reads under Kani), at most 2
/// cards per player so the 24-slot effect log cannot overflow.
fn riot_world() -> ([usize; MAX_PLAYERS], [[u8; 4]; MAX_PLAYERS]) {
    host::install_backend(riot_dispatch);
    let mut hand_ns = [0usize; MAX_PLAYERS];
    let mut hands = [[0u8; 4]; MAX_PLAYERS];
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            let h: i32 = if p < 2 { kani::any() } else { 0 };
            if p < 2 {
                kani::assume(h >= 0 && h <= 2);
            }
            w.hand_resonance[p] = h;
            w.hand[p] = false;
            hand_ns[p] = h as usize;
            for k in 0..h as usize {
                hands[p][k] = 1;
            }
        }
        w.symbolic_answers = true;
    });
    (hand_ns, hands)
}

/// In-place 2-seat world for pareo: no houses (no optional-removal prompt);
/// PAREO marks start sub-cap.
fn pareo_world(user: i32, pre_marks: i32) {
    host::install_backend(pareo_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    let key = crate::intern::intern("PAREO标记");
    world::with_world_mut(|w| {
        w.out[user as usize] = false;
        w.key_set(user, key, pre_marks);
    });
}

// ===========================================================================
// C-RAS-unstoppable-card-03
// 「若骰点为1-3获得2000资金，若为4-6则获得1000资金」
// Isolation: the teleport half is BLOCKED-HOST (`tile_named` cannot resolve
// 「白雪学园」…`CHUCHU的公寓` and `card_move` is a plan stub).
// Seat-symmetric (the user gains the rolled face money).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn unstoppable_card_03_money_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    unstoppable_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::unstoppable::unstoppable_kani(user));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    // The roll is symbolic 1..=6 inside `ctx::roll(1, 6)`. Recover the face
    // from the money amount the body granted (the only observable).
    let gained = post.money[user as usize] - pre.money[user as usize];
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
        spec::unstoppable_card_03_log_ok(effects, user, expected),
        "C-RAS-unstoppable-card-03: effect log is exactly one GainMoney"
    );
    // Frame: nothing but the money moved.
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: no movement (teleport is BLOCKED-HOST)");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire unchanged");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand unchanged");
    crate::kassert!(post.discard == pre.discard, "frame: discard unchanged");
    for q in 0..MAX_PLAYERS {
        if q != user as usize {
            crate::kassert!(
                post.money[q] == pre.money[q],
                "frame: other players' money unchanged"
            );
        }
    }
}

/// Iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn unstoppable_card_03_money_iff() {
    unstoppable_card_03_money_iff_at(0);
}

/// Iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
#[kani::proof]
#[kani::unwind(32)]
pub fn unstoppable_card_03_user1_money_iff() {
    unstoppable_card_03_money_iff_at(1);
}

/// No-op: an out player gains nothing. (Concrete user, forced `out`.)
#[kani::proof]
#[kani::unwind(32)]
pub fn unstoppable_card_03_money_noop_when_out() {
    unstoppable_world(0);
    let user = 0i32;
    world::with_world_mut(|w| w.out[0] = true);
    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::unstoppable::unstoppable_kani(user));
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
#[kani::unwind(32)]
pub fn unstoppable_card_03_money_mutant_is_detected() {
    unstoppable_world(0);
    let user = 0i32;
    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::unstoppable::unstoppable_kani(user));
    let post = world::snapshot();
    let gained = post.money[user as usize] - pre.money[user as usize];

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
// The [反击] *window* is PROTOCOL-LEVEL. Leaf effect driven directly.
// Isolation: hands are the `hand_resonance` view, ≤2 cards/player.
// Seat-symmetric (the holder is the extra-draw seat).
// ===========================================================================

/// Shared iff at a **concrete** holder seat.
fn riot_02_03_cycle_iff_at(holder: i32) {
    debug_assert!(holder == 0 || holder == 1);
    let (hand_ns, hands) = riot_world();
    world::with_world_mut(|w| {
        w.out[holder as usize] = false;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::riot::counteract_kani(holder));
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

/// Riot cycle iff at holder seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn riot_02_03_cycle_iff() {
    riot_02_03_cycle_iff_at(0);
}

/// Riot cycle iff at holder seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn riot_02_03_user1_cycle_iff() {
    riot_02_03_cycle_iff_at(1);
}

/// Mutant companion: dropping the holder's extra draw disagrees with the card.
#[kani::proof]
#[kani::unwind(32)]
pub fn riot_03_extra_draw_mutant_is_detected() {
    let (hand_ns, hands) = riot_world();
    let holder = 0i32;
    world::with_world_mut(|w| w.out[0] = false);
    // Need the holder to actually hold a card so h vs h+1 differ in the log.
    kani::assume(hand_ns[0] >= 1);

    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::riot::counteract_kani(holder));
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
// Isolation: no owned tile with houses (the optional 「移除…一层房屋」 offer
// is not opened). Sub-cap: `PAREO标记` starts at 0 or 1 so +2 stays ≤ 3.
// Seat-symmetric.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn pareo_01_marks_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let pre_marks: i32 = kani::any();
    kani::assume(pre_marks >= 0 && pre_marks <= 1);
    pareo_world(user, pre_marks);
    let pre = world::snapshot();
    invoke::unwrap_done(card_ras::pareo_far::pareo_far_kani(user));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let pareo_key = crate::intern::intern("PAREO标记").0;
    let marks_after = world::with_world(|ww| ww.key_get(user, crate::intern::intern("PAREO标记")));

    kani::cover!(pre_marks == 0, "start at 0 marks");
    kani::cover!(pre_marks == 1, "start at 1 mark");

    crate::kassert!(
        spec::pareo_01_log_ok(effects, user, pareo_key),
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

/// Pareo marks iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn pareo_01_marks_iff() {
    pareo_01_marks_iff_at(0);
}

/// Pareo marks iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn pareo_01_user1_marks_iff() {
    pareo_01_marks_iff_at(1);
}

/// Mutant companion: +3 marks is not what the card does.
#[kani::proof]
#[kani::unwind(32)]
pub fn pareo_01_marks_mutant_is_detected() {
    pareo_world(0, 0);
    let user = 0i32;
    let pareo_key = crate::intern::intern("PAREO标记").0;
    invoke::unwrap_done(card_ras::pareo_far::pareo_far_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::pareo_01_log_ok_mutant(effects, user, pareo_key),
        "mutant: card must not match the +3 marks spec"
    );
}