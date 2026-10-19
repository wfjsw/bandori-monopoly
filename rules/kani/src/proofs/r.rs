//! Unit `r`: clause proofs (Roselia cards).
//!
//! Per clause (KANI-FOUNDATION.md): iff + no-op/cover + mutant. Specs live in
//! `crate::spec::r` and come from the 规则书 clause docs only.
//!
//! ## Concrete-user pattern (TRIAGE-LOG `ppcost` FLOOR bisect)
//!
//! A symbolic player index (`any_player_in`) is a BMC cliff: CBMC never
//! returns. With `n_players = 2` and a **concrete** user seat the proof
//! verifies in minutes. Iff harnesses are `*_iff_at(user)` called from two
//! `#[kani::proof]`s with `user = 0` and `user = 1` (every clause here is
//! seat-symmetric 「自己的」 / self-facing). Mutants use a single concrete
//! user (disagreement is also seat-symmetric).
//!
//! Per-body host-op subset (`host_subset!`) covers only the ops that body
//! reaches; anything else hits `outside_subset` (loud FAIL). Direct
//! `*_kani` body calls keep CBMC's call graph at ONE body (no `call_body_idx`
//! trampoline IF-chain).

use crate::host;
use crate::harness::{self, SparsePlay};
use crate::invoke;
use crate::spec::r as spec;
use crate::world::{self, Effect, MAX_LOG};

// ---------------------------------------------------------------------------
// Per-body host-op subsets (outside → loud FAIL).
//
// Ops listed are what the unmodified body reaches (aliases folded:
// `add_tok`→add_held, `set_crystals`/`decay`→set/add_counter_self,
// `ask_*`→ask, `place_card`→place_card_at, `is_deed`→tile_is).
// Dead-path ops under the harness isolation are listed too, so an isolation
// break surfaces as a spec failure rather than an undeclared-op panic.
// ---------------------------------------------------------------------------

/// `R:[衍生] 觉悟` play — `set_dest` + `gain`.
crate::host_subset!(resolve_dispatch {
    set_dest => arm_set_dest,
    gain | gain_typed | gain_fixed => arm_gain,
});

/// `R:曲奇时间` play — discard size, shuffle, log, gain.
crate::host_subset!(cookie_dispatch {
    discard_size => arm_discard_size,
    shuffle_into_deck => arm_shuffle_into_deck,
    log | effect => arm_log,
    gain | gain_typed | gain_fixed => arm_gain,
});

/// `R:Sprechchor` play body only (`cant_play` is not invoked).
crate::host_subset!(sprechchor_dispatch {
    roll | roll_ask => arm_roll,
    gain | gain_typed | gain_fixed => arm_gain,
});

/// `R:（亚子）黑暗大魔姬亚子` counteract — stun, cancel pay, off-hand draw.
/// `play_from_hand` is a protocol stub (always 0 = off-hand).
crate::host_subset!(ako_dark_dispatch {
    apply_abnormal => arm_apply_abnormal,
    ,
    log | effect => arm_log,
    trig_set_pay_amount => arm_trig_set_pay_amount,
    play_from_hand => arm_protocol_stub,  // 0 = not-from-hand (dispatch polarity); TODO(规则书) if a proof needs 1
    draw => arm_draw,
});

/// `R:Fire bird` play — pay 1600, ask X, place, set crystals.
crate::host_subset!(fire_bird_play_dispatch {
    pay => arm_pay,
    ask => arm_ask,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card,
    set_counter_self => arm_set_counter_self,
    log | effect => arm_log,
});

/// `R:Fire bird` TurnEnd — pay 400 + crystal decay.
crate::host_subset!(fire_bird_burn_dispatch {
    pay => arm_pay,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
});

/// `R:NFO` play — roll + face 1..=6 (6 runs 1..=5). Face 2's move/split and
/// face 4's replay are listed (the sparse world usually early-returns them;
/// if isolation breaks the body runs and the face-1 slice is unaffected).
crate::host_subset!(nfo_play_dispatch {
    roll | roll_ask => arm_roll,
    schedule_turn_end => arm_schedule_turn_end,
    log | effect => arm_log,
    player_out => arm_player_out,
    gain | gain_typed | gain_fixed => arm_gain,
    tile_count => arm_tile_count,
    player_pos => arm_player_pos,
    others_count => arm_others_count,
    others_at => arm_others_at,
    players_on_count => arm_players_on_count,
    players_on_at => arm_players_on_at,
    tile_forward | dist => arm_dist,
    set_steps | set_resolve => arm_protocol_stub,
    card_move => arm_card_move,
    tile_steps_ahead => arm_tile_steps_ahead,
    pay_total | pay_leg => arm_pay_to,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card,
    cards_in => arm_cards_in,
    card_replayable | play_card => arm_protocol_stub,
    ask | opt_str | opt_int | opt_tile | opt_price | opt_ai => arm_ask,
    take_card => arm_take_card,
    to_discard => arm_to_discard,
    add_to_hand => arm_add_to_hand,
    add_to_deck | add_to_deck_at => arm_add_to_deck_at,
    add_held => arm_add_held,
});

/// `R:NFO` AtEnd — one [停留] layer.
crate::host_subset!(nfo_at_end_dispatch {
    apply_abnormal => arm_apply_abnormal,
    log | effect => arm_log,
});

/// `R:学生会的检查` counteract — insert 觉悟, no-build prop, optional place.
/// Isolation (no owned deed with a house cost) keeps the body at the insert;
/// the post-return ops are listed so an isolation break is a spec failure.
crate::host_subset!(council_dispatch {
    add_to_deck | add_to_deck_at => arm_add_to_deck_at,
    log | effect => arm_log,
    set_self_prop => arm_set_self_prop,
    linger => arm_protocol_stub,
    player_pos => arm_player_pos,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    dist => arm_dist,
    build_cost => arm_build_cost,
    ask => arm_ask,
    pay => arm_pay,
    set_dest => arm_set_dest,
    place_card_on => arm_place_card_on,
    set_slot => arm_set_slot,
});

/// `R:（燐子）Ringing Bloom` play — place + (2) house-count override.
crate::host_subset!(ringing_bloom_dispatch {
    set_dest => arm_set_dest,
    place_card_at => arm_place_card,
    log | effect => arm_log,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    tile_is => arm_tile_is,
    houses_of => arm_houses_of,
    set_self_prop => arm_set_self_prop,
    set_tile_prop => arm_set_tile_prop,
});

// ---------------------------------------------------------------------------
// shared sparse 2-seat world (concrete user; see module docs)
// ---------------------------------------------------------------------------

/// Sparse 2-seat world **in place**. `user` is a concrete seat in `{0, 1}`.
/// Install the per-body `host_subset!` backend first.
fn sparse_r_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.symbolic_answers = true;
    });
}

fn log_slice() -> ([Effect; MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects_of(logbuf: &[Effect; MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

// ===========================================================================
// C-R-觉悟-01 / C-R-觉悟-02 -- `R:[衍生] 觉悟`
// 「[特]：此卡[移除]时获得1000资金。」 「[手]：[移除]此卡。」
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn resolve_play_iff_at(user: i32) {
    host::install_backend(resolve_dispatch);
    sparse_r_world(user);

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::resolve::resolve_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(log_n == 1, "one effect");
    crate::kassert!(
        spec::resolve_play_log_ok(effects, user),
        "C-R-觉悟-01: exactly GainMoney 1000"
    );
    crate::kassert!(
        spec::resolve_play_post_ok(&pre, &post, user),
        "C-R-觉悟-01: only the user's money moves, +1000"
    );
}

/// C-R-觉悟-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn resolve_play_iff() {
    resolve_play_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn resolve_play_user1_iff() {
    resolve_play_iff_at(1);
}

/// MUTANT companion: the off-by-one / wrong-amount spec must NOT match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn resolve_play_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(resolve_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::resolve::resolve_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        !spec::resolve_play_log_ok_mutant(effects, user),
        "mutant: card does not pay 2000"
    );
}

/// MUTANT (should_panic): the wrong-amount spec fails against the real body.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn resolve_play_mutant_2000() {
    let user = 0i32;
    host::install_backend(resolve_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::resolve::resolve_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        spec::resolve_play_log_ok_mutant(effects, user),
        "mutant: not a 2000 spec"
    );
}

// ===========================================================================
// C-R-曲奇时间-02 -- +500·X
// Clause 01's shuffle is present in the body (`shuffle_into_deck`) but the
// pile-multiset clause is not the assertion here; X is the pre-discard size.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn cookie_pay_iff_at(user: i32) {
    host::install_backend(cookie_dispatch);
    sparse_r_world(user);
    let n: u8 = kani::any();
    kani::assume(n >= 1 && n <= 8);
    world::with_world_mut(|w| {
        w.discard_n = n;
        let mut i = 0usize;
        while i < n as usize {
            w.discard_pile[i] = kani::any();
            i += 1;
        }
    });
    let x = spec::cookie_x(n as i32);

    let pre = world::snapshot();
    // Gate: discard non-empty (the book's condition). The `cant_play` gate is
    // covered by the non-empty discard assumption; we call the play body.
    invoke::unwrap_done(card_roselia::cookie_time::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(x == 1, "X=1");
    kani::cover!(x == 3, "X=3");
    kani::cover!(x == 8, "X=8");
    crate::kassert!(
        spec::cookie_pay_log_ok(effects, user, x),
        "C-R-曲奇时间-02: exactly GainMoney 500*X"
    );
    crate::kassert!(
        spec::cookie_pay_post_ok(&pre, &post, user, x),
        "C-R-曲奇时间-02: money +500*X only"
    );
}

/// C-R-曲奇时间-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn cookie_pay_iff() {
    cookie_pay_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己弃牌堆」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn cookie_pay_user1_iff() {
    cookie_pay_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn cookie_pay_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(cookie_dispatch);
    sparse_r_world(user);
    let n: u8 = kani::any();
    kani::assume(n >= 1 && n <= 8);
    world::with_world_mut(|w| w.discard_n = n);
    let x = spec::cookie_x(n as i32);
    invoke::unwrap_done(card_roselia::cookie_time::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let wrong = spec::cookie_pay_amount_mutant(x);
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GainMoney { n, .. }) if *n == wrong),
        "mutant: gain is not 500*(X+1)"
    );
}

// ===========================================================================
// C-R-Sprechchor-02 -- 1000 + X*120
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn sprechchor_pay_iff_at(user: i32) {
    host::install_backend(sprechchor_dispatch);
    sparse_r_world(user);

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::sprechchor::sprechchor_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Recover the rolled face from the gain amount: n = 1000 + X*120.
    crate::kassert!(log_n == 1, "C-R-Sprechchor-02: exactly one effect");
    let n = match effects.first() {
        Some(Effect::GainMoney { n, .. }) => *n,
        _ => {
            crate::kassert!(false, "C-R-Sprechchor-02: effect must be GainMoney");
            0
        }
    };
    let x = (n - 1000) / 120;
    kani::cover!(x == 1, "face 1");
    kani::cover!(x == 20, "face 20");
    crate::kassert!(
        spec::sprechchor_face_ok(x),
        "C-R-Sprechchor-02: face in 1..=20"
    );
    crate::kassert!(
        spec::sprechchor_log_ok(effects, user, x),
        "C-R-Sprechchor-02: gain is exactly 1000+X*120"
    );
    crate::kassert!(
        spec::sprechchor_post_ok(&pre, &post, user, x),
        "C-R-Sprechchor-02: only the user's money moves"
    );
}

/// C-R-Sprechchor-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn sprechchor_pay_iff() {
    sprechchor_pay_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn sprechchor_pay_user1_iff() {
    sprechchor_pay_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn sprechchor_pay_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(sprechchor_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::sprechchor::sprechchor_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let n = match effects.first() {
        Some(Effect::GainMoney { n, .. }) => *n,
        _ => -1,
    };
    let x = (n - 1000) / 120;
    crate::kassert!(
        n != spec::sprechchor_gain_mutant(x),
        "mutant: gain is not 1000+(X+1)*120"
    );
}

// ===========================================================================
// C-R-黑暗大魔姬亚子-02 / -03 -- stun + off-hand draw
// `play_from_hand` is a protocol stub (always 0) -- the host always reports
// off-hand, so this harness proves the off-hand activation. The from-hand
// no-draw half is BLOCKED-HOST (see the unit table).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
/// `trig_target` is the other living seat (the pay's counterparty).
fn ako_dark_offhand_iff_at(user: i32) {
    host::install_backend(ako_dark_dispatch);
    sparse_r_world(user);
    let pay_amount = kani::any();
    kani::assume(pay_amount > 0 && pay_amount <= 10_000);
    world::with_world_mut(|w| {
        w.pay_amount = pay_amount;
        w.trig_value = pay_amount;
        // The other seat in a 2-player world (clause is self-facing; which
        // other player is the pay target does not change the body).
        w.trig_target = 1 - user;
    });

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::ako_dark::counteract_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(log_n == 3, "stun + cancel + draw");
    crate::kassert!(
        spec::ako_dark_offhand_log_ok(effects, user),
        "C-R-黑暗大魔姬亚子-02/03: GiveStun{1} + SetPayAmount{0} + Draw{1}"
    );
    crate::kassert!(
        spec::ako_dark_post_ok(&pre, &post, user),
        "C-R-黑暗大魔姬亚子-02: +1 stun, pay zeroed, money unchanged"
    );
}

/// C-R-黑暗大魔姬亚子-02/03 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ako_dark_offhand_iff() {
    ako_dark_offhand_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己」; target is the other seat).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ako_dark_offhand_user1_iff() {
    ako_dark_offhand_iff_at(1);
}

/// MUTANT companion: not a 2-stun spec.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ako_dark_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(ako_dark_dispatch);
    sparse_r_world(user);
    world::with_world_mut(|w| {
        w.pay_amount = 500;
        w.trig_value = 500;
        w.trig_target = 1;
    });
    invoke::unwrap_done(card_roselia::ako_dark::counteract_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let two = spec::ako_dark_stun_n_mutant();
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GiveStun { n, .. }) if *n == two),
        "mutant: first effect is not 2 stun layers"
    );
}

// ===========================================================================
// C-R-Fire bird-01 -- pay 1600, place, X crystals
// OPEN-CHARGE-X: the body offers 1..=10 and pins `x.max(1)`.
// NOTE: `ctx::ask_number` under Kani only supports ranges of width ≤ 4 and
// `kani::assert`s on 1..=10. That SDK limit fails this harness closed (no
// #[ignore]); it is not a body or spec gap.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn fire_bird_place_iff_at(user: i32) {
    host::install_backend(fire_bird_play_dispatch);
    sparse_r_world(user);
    // Condition from the clause doc: the user has ≥1600 money.
    world::with_world_mut(|w| {
        let m = w.money[user as usize];
        if m < spec::fire_bird_cost() {
            w.money[user as usize] = spec::fire_bird_cost();
        }
    });

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::fire_bird::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let card = crate::world::card_slot_of("R:Fire bird");

    kani::cover!(log_n >= 2, "pay + place");
    crate::kassert!(
        spec::fire_bird_place_log_ok(effects, user, card),
        "C-R-Fire bird-01: PayMoney 1600 + PlaceCard"
    );
    crate::kassert!(
        spec::fire_bird_place_post_ok(&pre, &post, user),
        "C-R-Fire bird-01: money -1600"
    );
}

/// C-R-Fire bird-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn fire_bird_place_iff() {
    fire_bird_place_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己场地」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn fire_bird_place_user1_iff() {
    fire_bird_place_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn fire_bird_place_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(fire_bird_play_dispatch);
    sparse_r_world(user);
    world::with_world_mut(|w| {
        w.money[user as usize] = spec::fire_bird_cost();
    });
    invoke::unwrap_done(card_roselia::fire_bird::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let card = crate::world::card_slot_of("R:Fire bird");
    crate::kassert!(
        !spec::fire_bird_place_log_ok_mutant(effects, user, card),
        "mutant: first effect is not a 1600 gain"
    );
}

// ===========================================================================
// C-R-Fire bird-02 -- turn-end burn 400 + 1 crystal
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn fire_bird_burn_iff_at(user: i32) {
    host::install_backend(fire_bird_burn_dispatch);
    sparse_r_world(user);
    world::with_world_mut(|w| {
        let m = w.money[user as usize];
        if m < spec::fire_bird_burn_pay() {
            w.money[user as usize] = spec::fire_bird_burn_pay();
        }
        // Crystals present for the decay half (pay is unconditional).
        w.key_set(user, crate::intern::intern("crystals"), 1);
    });

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::fire_bird::turn_end_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(log_n >= 1, "pay 400");
    crate::kassert!(
        spec::fire_bird_burn_log_ok(effects, user),
        "C-R-Fire bird-02: PayMoney 400 (+ crystal decay)"
    );
    crate::kassert!(
        spec::only_player_money_delta(&pre, &post, user, -spec::fire_bird_burn_pay(), 1),
        "C-R-Fire bird-02: money -400"
    );
}

/// C-R-Fire bird-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_bird_burn_iff() {
    fire_bird_burn_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_bird_burn_user1_iff() {
    fire_bird_burn_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_bird_burn_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(fire_bird_burn_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::fire_bird::turn_end_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        !spec::fire_bird_burn_log_ok_mutant(effects, user),
        "mutant: not a 400 gain"
    );
}

// ===========================================================================
// C-R-NFO-03 -- face 1 = +1500
// The roll is symbolic 1..=6 inside the body; cover each face and assert the
// face-1 slice exactly. Faces 2-6 have their own clauses (unit table).
// C-R-NFO-02 is the separate `AtEnd` entry.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn nfo_face1_iff_at(user: i32) {
    host::install_backend(nfo_play_dispatch);
    sparse_r_world(user);

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::nfo::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Recover the face from the gain amount when face 1 fired.
    let face1 = log_n == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == user && n == spec::nfo_face1_gain());
    kani::cover!(face1, "face 1 fires");
    kani::cover!(!face1, "other face");
    if face1 {
        crate::kassert!(
            spec::nfo_face1_log_ok(effects, user),
            "C-R-NFO-03: exactly GainMoney 1500"
        );
        crate::kassert!(
            spec::only_player_money_delta(&pre, &post, user, spec::nfo_face1_gain(), 1),
            "C-R-NFO-03: money +1500 only"
        );
    }
}

/// C-R-NFO-03 (face-1 slice) iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn nfo_face1_iff() {
    nfo_face1_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn nfo_face1_user1_iff() {
    nfo_face1_iff_at(1);
}

/// MUTANT companion for face 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn nfo_face1_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(nfo_play_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::nfo::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let face1 = log_n == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == user && n == spec::nfo_face1_gain());
    if face1 {
        let wrong = spec::nfo_face1_gain_mutant();
        crate::kassert!(
            !matches!(effects[0], Effect::GainMoney { n, .. } if n == wrong),
            "mutant: face-1 gain is not 1400"
        );
    }
}

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn nfo_stay_iff_at(user: i32) {
    host::install_backend(nfo_at_end_dispatch);
    sparse_r_world(user);

    let pre = world::snapshot();
    invoke::unwrap_done(card_roselia::nfo::at_end_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::nfo_stay_log_ok(effects, user),
        "C-R-NFO-02: exactly GiveStay{1}"
    );
    crate::kassert!(
        post.stay[user as usize] == pre.stay[user as usize] + 1,
        "C-R-NFO-02: +1 [停留]"
    );
}

/// C-R-NFO-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn nfo_stay_iff() {
    nfo_stay_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自己」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn nfo_stay_user1_iff() {
    nfo_stay_iff_at(1);
}

/// MUTANT companion for the stay layer.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn nfo_stay_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(nfo_at_end_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::nfo::at_end_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let two = spec::nfo_stay_n_mutant();
    crate::kassert!(
        !matches!(effects.first(), Some(Effect::GiveStay { n, .. }) if *n == two),
        "mutant: stay is not 2 layers"
    );
}

// ===========================================================================
// C-R-学生会的检查-02 -- add one `R:[衍生] 觉悟` to the draw pile
// 「向抽牌堆中加入一张"觉悟"」
// Isolation: no owned deed with a house build cost, so the optional
// placement branch is not offered (the insert is unconditional).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn student_council_add_resolve_iff_at(user: i32) {
    host::install_backend(council_dispatch);
    sparse_r_world(user);
    // Isolation: no owned tiles (sparse world leaves `owner = -1`).
    world::with_world_mut(|w| w.deck_n = 0);

    let pre = world::snapshot();
    let pre_deck_n = world::with_world(|w| w.deck_n as i32);
    invoke::unwrap_done(card_roselia::council_check::counteract_kani(user));
    let post = world::snapshot();
    let post_deck_n = world::with_world(|w| w.deck_n as i32);

    kani::cover!(post_deck_n == 1, "one card in deck");
    crate::kassert!(
        spec::student_council_add_resolve_delta(pre_deck_n, post_deck_n),
        "C-R-学生会的检查-02: deck gains exactly one 觉悟"
    );
    let _ = pre;
    let _ = post;
    // No effect-log entry: `add_to_deck` is a silent pile write.
    let (logbuf, log_n) = log_slice();
    let _ = logbuf;
    crate::kassert!(log_n == 0, "C-R-学生会的检查-02: insert is not an effect-log row");
}

/// C-R-学生会的检查-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn student_council_add_resolve_iff() {
    student_council_add_resolve_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「抽牌堆」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn student_council_add_resolve_user1_iff() {
    student_council_add_resolve_iff_at(1);
}

/// MUTANT companion: the deck must not grow by 2.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn student_council_add_resolve_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(council_dispatch);
    sparse_r_world(user);
    world::with_world_mut(|w| w.deck_n = 0);
    invoke::unwrap_done(card_roselia::council_check::counteract_kani(user));
    let post_deck_n = world::with_world(|w| w.deck_n as i32);
    crate::kassert!(
        post_deck_n != 2,
        "mutant: deck did not grow by 2"
    );
}

// ===========================================================================
// C-R-Ringing Bloom-01 -- place on own field
// 「（1）将此卡放置于自身场上」
// Clauses 02/03 are the rent pipeline (PROTOCOL-LEVEL).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn ringing_bloom_place_iff_at(user: i32) {
    host::install_backend(ringing_bloom_dispatch);
    sparse_r_world(user);

    let _pre = world::snapshot();
    invoke::unwrap_done(card_roselia::ringing_bloom::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let card = crate::world::card_slot_of("R:（燐子）Ringing Bloom");

    kani::cover!(log_n == 1, "place");
    crate::kassert!(
        spec::ringing_bloom_place_log_ok(effects, user, card),
        "C-R-Ringing Bloom-01: exactly PlaceCard on the user's field"
    );
}

/// C-R-Ringing Bloom-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ringing_bloom_place_iff() {
    ringing_bloom_place_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「自身场上」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ringing_bloom_place_user1_iff() {
    ringing_bloom_place_iff_at(1);
}

/// MUTANT companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn ringing_bloom_place_mutant_is_detected() {
    let user = 0i32;
    host::install_backend(ringing_bloom_dispatch);
    sparse_r_world(user);
    invoke::unwrap_done(card_roselia::ringing_bloom::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        !spec::ringing_bloom_place_log_ok_mutant(effects, user),
        "mutant: not a two-PlaceCard body"
    );
}