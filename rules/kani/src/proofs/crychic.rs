//! Unit `crychic`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op / cover + mutant.
//! Specs live in `crate::spec::crychic` and come from the clause docs only.
//!
//! **ppcost pattern (TRIAGE-LOG `ppcost` FLOOR bisect).** Concrete user seats
//! (`n_players = 2`, user=0/1 for seat-symmetric clauses). Direct body call
//! via the cfg(kani) `NAME_kani` wrapper. Per-harness host-op subset
//! (`host_subset!`) — undeclared op FAILS loudly. Isolation assumes are named
//! here and `kani::assume`d. Narration (`ctx::log`) is not part of the frame.
//!
//! Isolation (module-level):
//! * no stacked hooks besides the clause under test;
//! * `n_players = 2`, money `0..=10_000`, both seats living unless stated;
//! * tiles start unowned (`TileState::owner = -1`), so the soyo build leg is
//!   isolated away — its ops are still declared so an isolation break FAILS.
//!
//! Host notes that shape these harnesses:
//! * `Op::log` is narration only (not an effect-log entry).
//! * `Op::state_set` is host-faithful now (value in `a4`, logged as `n`).
//! * `Op::set_no_money_loss` / `Op::schedule_turn_end` are protocol stubs.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec::crychic as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-card host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `CRYCHIC:（soyo）回到曾经` play — 「获得500资金」.
crate::host_subset!(soyoreturn_play_dispatch {
    gain | gain_typed | gain_fixed => arm_gain,
    log | effect => arm_log,
});

/// `CRYCHIC:（soyo）回到曾经` on_drawn — the two-branch [特].
/// Isolation: no owned tiles, so `build_cost` / `pay` / `add_house` stay
/// off-path; still declared so an isolation break FAILS. `arm_build_cost` is
/// not yet extracted in this worktree's `crate::host` (sibling kani-bands-a
/// has it at host.rs:3308). `arm_tile_price` is a positive stand-in: a
/// non-zero cost makes `pay`/`add_house` fire and break the frame if the
/// isolated leg is ever reached. Swap to `arm_build_cost` once extracted.
crate::host_subset!(soyoreturn_on_drawn_dispatch {
    log | effect => arm_log,
    opt_str | opt_int | opt_tile => arm_opt,
    ask => arm_ask,
    take_card => arm_take_card,
    cards_in => arm_cards_in,
    discard_from_hand => arm_discard_from_hand,
    gain | gain_typed | gain_fixed => arm_gain,
    draw => arm_draw,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    build_cost => arm_tile_price,
    pay => arm_pay,
    add_house => arm_add_house,
});

/// `CRYCHIC:主唱太拼命了` counteract leaf — waive the pending pay.
crate::host_subset!(vocalist_dispatch {
    trig_set_pay_amount => arm_trig_set_pay_amount,
    log | effect => arm_log,
});

/// `CRYCHIC:想要抓住...` play — one [停留] + arm the PASSER latch.
crate::host_subset!(grab_dispatch {
    give_stay => arm_give_stay,
    state_set => arm_state_set,
    log | effect => arm_log,
});

/// `CRYCHIC:初演大成功` play — narration + two scheduling stubs.
crate::host_subset!(debut_play_dispatch {
    log | effect => arm_log,
    set_no_money_loss | schedule_turn_end => arm_stub_zero,
});

/// `CRYCHIC:初演大成功` at_end leaf — stun + one band crystal.
crate::host_subset!(debut_at_end_dispatch {
    give_stun => arm_give_stun,
    add_group_crystals => arm_add_band_crystals,
});

/// `CRYCHIC:如果能一直持续下去...` play — cash out band crystals, place as [持续].
crate::host_subset!(forever_play_dispatch {
    group_crystals => arm_band_crystals,
    add_group_crystals => arm_add_band_crystals,
    gain | gain_typed | gain_fixed => arm_gain,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    log | effect => arm_log,
});

/// `CRYCHIC:如果能一直持续下去...` on_drew — dest to graveyard + narration.
/// The book wants 「立即置入弃牌堆」 (`UnplaceCard`); the body only
/// `set_dest(Graveyard)` (see the FAILS(LESS) harness below).
crate::host_subset!(forever_on_drew_dispatch {
    set_dest => arm_set_dest,
    log | effect => arm_log,
});

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

fn band_crystals_of(p: i32) -> i32 {
    world::with_world(|w| w.group_crystals[p as usize])
}

fn key_get(p: i32, key: intern::NameId) -> i32 {
    world::with_world(|w| w.key_get(p, key))
}

/// Sparse 2-seat play world on `dispatch`, then `f` for the card-specific
/// fields. In-place only — no `reset_world` full-struct copy.
fn sparse2(
    dispatch: card_sdk::kani_host::Backend,
    user: i32,
    f: impl FnOnce(&mut world::World),
) {
    host::install_backend(dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        f(w);
    });
}

// ===========================================================================
// C-CRYCHIC-soyoreturn-02 — 「（2）[手] 获得500资金」
// Seat-symmetric (the user gains the 500).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn soyoreturn_02_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(soyoreturn_play_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::soyo_back::soyo_back_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::soyoreturn_02_log_ok(effects, user),
        "C-CRYCHIC-soyoreturn-02: exactly GainMoney{player, 500}"
    );
    crate::kassert!(
        post.money[user as usize] == pre.money[user as usize] + spec::soyoreturn_02_amount(),
        "C-CRYCHIC-soyoreturn-02: money +500"
    );
    // frame
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand bitset");
    crate::kassert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
    for q in 0..MAX_PLAYERS {
        if q != user as usize {
            crate::kassert!(
                post.money[q] == pre.money[q],
                "frame: other players' money unchanged"
            );
        }
    }
}

/// Iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn soyoreturn_02_hand_iff() {
    soyoreturn_02_iff_at(0);
}

/// Same iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
#[kani::proof]
#[kani::unwind(32)]
pub fn soyoreturn_02_user1_hand_iff() {
    soyoreturn_02_iff_at(1);
}

/// MUTANT companion: the 501-spec is distinguishable from the body.
#[kani::proof]
#[kani::unwind(32)]
pub fn soyoreturn_02_mutant_is_detected() {
    sparse2(soyoreturn_play_dispatch, 0, |_| {});
    let user = 0i32;
    invoke::unwrap_done(card_crychic::soyo_back::soyo_back_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::soyoreturn_02_log_ok(effects, user),
        "card matches the book"
    );
    let wrong = spec::soyoreturn_02_log_ok_mutant(effects, user);
    kani::cover!(!wrong, "off-by-one mutant disagrees");
    crate::kassert!(!wrong, "mutant 501 must NOT match the body");
}

// ===========================================================================
// C-CRYCHIC-vocalist-01 — leaf body of the [反击] waive
// The [反击] *window* / guard is PROTOCOL-LEVEL. Leaf effect driven directly.
// Seat-symmetric (the owner is the waiving seat).
// ===========================================================================

/// Shared iff at a **concrete** owner seat. Pending pay pinned to ≥ 5000
/// (the book's floor) so the leaf's waive is the observable rewrite.
fn vocalist_01_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let value: i32 = kani::any();
    kani::assume(value >= 5000 && value <= 20_000);
    sparse2(vocalist_dispatch, user, |w| {
        w.trig_target = 1 - user;
        w.trig_value = value;
        w.pay_amount = value;
    });
    kani::cover!(value == 5000, "at the 5000 floor");
    kani::cover!(value > 5000, "above the floor");

    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::vocal_too_hard::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::vocalist_01_log_ok(effects),
        "C-CRYCHIC-vocalist-01: exactly SetPayAmount{to: 0}"
    );
    crate::kassert!(
        post.pay_amount == 0,
        "C-CRYCHIC-vocalist-01: payment waived"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (waive is at rewrite time)");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Waive iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn vocalist_01_waive_iff() {
    vocalist_01_iff_at(0);
}

/// Waive iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn vocalist_01_user1_waive_iff() {
    vocalist_01_iff_at(1);
}

/// MUTANT companion: SetPayAmount{1} is not what the body does.
#[kani::proof]
#[kani::unwind(32)]
pub fn vocalist_01_mutant_is_detected() {
    sparse2(vocalist_dispatch, 0, |w| {
        w.trig_target = 1;
        w.trig_value = 5000;
        w.pay_amount = 5000;
    });
    let user = 0i32;
    invoke::unwrap_done(card_crychic::vocal_too_hard::counteract_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(spec::vocalist_01_log_ok(effects), "card matches book");
    let wrong = spec::vocalist_01_log_ok_mutant(effects);
    kani::cover!(!wrong, "mutant SetPayAmount{1} disagrees");
    crate::kassert!(!wrong, "mutant must NOT match");
}

// ===========================================================================
// C-CRYCHIC-grab-01 — 「（1）获得一层[停留]。」
// Seat-symmetric. The body also arms the PASSER latch (`state::set(PASSER,-1)`).
// `Op::state_set` is host-faithful now (value logged as `n`).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn grab_01_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(grab_dispatch, user, |_| {});
    let passer_key = intern::intern("want_to_grab_passer").0;
    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::want_to_grab::want_to_grab_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // GiveStay + the PASSER latch write. Host-faithful and book-exact.
    crate::kassert!(
        spec::grab_01_log_ok(effects, user, passer_key),
        "C-CRYCHIC-grab-01: GiveStay{{1}} + SetState(PASSER, -1)"
    );
    crate::kassert!(
        post.stay[user as usize] == pre.stay[user as usize] + 1,
        "C-CRYCHIC-grab-01: stay +1"
    );
    crate::kassert!(
        key_get(user, intern::NameId(passer_key)) == -1,
        "C-CRYCHIC-grab-01: PASSER latch is -1 (unset sentinel)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Stay iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn grab_01_stay_iff() {
    grab_01_iff_at(0);
}

/// Stay iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn grab_01_user1_stay_iff() {
    grab_01_iff_at(1);
}

/// MUTANT companion: two stay layers is not what the body does.
#[kani::proof]
#[kani::unwind(32)]
pub fn grab_01_mutant_is_detected() {
    sparse2(grab_dispatch, 0, |_| {});
    let user = 0i32;
    let passer_key = intern::intern("want_to_grab_passer").0;
    invoke::unwrap_done(card_crychic::want_to_grab::want_to_grab_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == user),
        "card gives exactly one stay"
    );
    let wrong = spec::grab_01_log_ok_mutant(effects, user, passer_key);
    kani::cover!(!wrong, "two-stay mutant disagrees");
    crate::kassert!(!wrong, "mutant GiveStay{2} must NOT match");
}

// ===========================================================================
// C-CRYCHIC-debut-01 — play leaf: narration + two scheduling stubs
// The standing no-drop claim is PROTOCOL-LEVEL.
// Seat-symmetric.
// ===========================================================================

/// Shared leaf at a **concrete** user seat: the play body itself logs no
/// effects (scheduling ops are protocol stubs).
fn debut_01_play_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(debut_play_dispatch, user, |_| {});
    invoke::unwrap_done(card_crychic::debut_success::debut_success_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    crate::kassert!(
        spec::debut_01_log_ok(&ebuf[..log_n]),
        "C-CRYCHIC-debut-01 leaf: play body produces no effects (arms stubs only)"
    );
}

/// Play leaf at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn debut_01_play_logs_nothing() {
    debut_01_play_iff_at(0);
}

/// Play leaf at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn debut_01_user1_play_logs_nothing() {
    debut_01_play_iff_at(1);
}

// ===========================================================================
// C-CRYCHIC-debut-02 — turn-end stun + band crystal
// Seat-symmetric.
// ===========================================================================

/// Shared iff at a **concrete** user seat. Band crystals start sub-cap.
fn debut_02_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let x: i32 = kani::any();
    kani::assume(x >= 0 && x <= 9);
    sparse2(debut_at_end_dispatch, user, |w| {
        w.group_crystals[user as usize] = x;
    });

    let pre_crystals = band_crystals_of(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::debut_success::at_end_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let post_crystals = band_crystals_of(user);

    crate::kassert!(
        spec::debut_02_log_ok(effects, user),
        "C-CRYCHIC-debut-02: exactly GiveStun{player, 1}"
    );
    crate::kassert!(
        post_crystals == pre_crystals + spec::debut_02_crystal_delta(),
        "C-CRYCHIC-debut-02: band crystal +1"
    );
    // `give_stun` writes the stun store (not stay).
    crate::kassert!(
        post.stun[user as usize] == pre.stun[user as usize] + 1,
        "C-CRYCHIC-debut-02: stun layer +1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
}

/// Turn-end leaf iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn debut_02_turn_end_iff() {
    debut_02_iff_at(0);
}

/// Turn-end leaf iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn debut_02_user1_turn_end_iff() {
    debut_02_iff_at(1);
}

/// MUTANT companion: GiveStay is not GiveStun.
#[kani::proof]
#[kani::unwind(32)]
pub fn debut_02_mutant_is_detected() {
    sparse2(debut_at_end_dispatch, 0, |w| {
        w.group_crystals[0] = 0;
    });
    let user = 0i32;
    invoke::unwrap_done(card_crychic::debut_success::at_end_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(spec::debut_02_log_ok(effects, user), "card matches book");
    let wrong = spec::debut_02_log_ok_mutant(effects, user);
    kani::cover!(!wrong, "GiveStay-mutant disagrees");
    crate::kassert!(!wrong, "mutant GiveStay must NOT match GiveStun");
}

// ===========================================================================
// C-CRYCHIC-ifonly-01 — cash out band crystals
// Seat-symmetric.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn ifonly_01_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let x: i32 = kani::any();
    kani::assume(x >= 0 && x <= 9);
    sparse2(forever_play_dispatch, user, |w| {
        w.group_crystals[user as usize] = x;
    });

    let pre = world::snapshot();
    let field0 = world::with_world(|w| w.field_n as usize);
    invoke::unwrap_done(card_crychic::forever::forever_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let post_crystals = band_crystals_of(user);
    let field1 = world::with_world(|w| w.field_n as usize);

    kani::cover!(x == 0, "X = 0");
    kani::cover!(x >= 1, "X >= 1");

    crate::kassert!(
        spec::ifonly_01_log_ok(effects, user, x),
        "C-CRYCHIC-ifonly-01: GainMoney{2000+500X} + PlaceCard on field"
    );
    crate::kassert!(
        post.money[user as usize] == pre.money[user as usize] + spec::ifonly_01_amount(x),
        "C-CRYCHIC-ifonly-01: money +2000+500X"
    );
    crate::kassert!(
        post_crystals == 0,
        "C-CRYCHIC-ifonly-01: all band crystals removed"
    );
    crate::kassert!(
        field1 == field0 + 1,
        "C-CRYCHIC-ifonly-01: card placed as [持续]"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Cash-out iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn ifonly_01_cashout_iff() {
    ifonly_01_iff_at(0);
}

/// Cash-out iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn ifonly_01_user1_cashout_iff() {
    ifonly_01_iff_at(1);
}

/// MUTANT companion: 2000+500*(X+1) is not what the body does.
#[kani::proof]
#[kani::unwind(32)]
pub fn ifonly_01_mutant_is_detected() {
    sparse2(forever_play_dispatch, 0, |w| {
        w.group_crystals[0] = 2;
    });
    let user = 0i32;
    invoke::unwrap_done(card_crychic::forever::forever_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        spec::ifonly_01_log_ok(effects, user, 2),
        "card matches book at X=2"
    );
    let wrong = spec::ifonly_01_log_ok_mutant(effects, user, 2);
    kani::cover!(!wrong, "X+1 mutant disagrees");
    crate::kassert!(!wrong, "mutant 2000+500*(X+1) must NOT match");
}

// ===========================================================================
// C-CRYCHIC-ifonly-02 — [持续] hand >= 7 → discard
// The hook condition `card.placed && owner.hand >= 7` is the entry's CEL.
// The body is driven directly (ppcost). Seat-symmetric.
//
// FAILS(LESS): the book says 「此卡立即置入弃牌堆」 (`UnplaceCard`); the body
// only `set_dest(Graveyard)` + narration. The assert below is the finding.
// ===========================================================================

/// Shared leaf at a **concrete** user seat.
fn ifonly_02_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(forever_on_drew_dispatch, user, |w| {
        // Pin a placed card so `card.placed` is true for the entry CEL.
        w.field_n = 1;
        w.field[0] = world::FieldCard {
            uid: 7,
            card: 0,
            player: user,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: 0,
            props: world::NameMap::new(),
        };
        w.next_uid = 8;
        w.last_uid = 7;
    });
    let field0 = world::with_world(|w| w.field_n as usize);
    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::forever::on_drew_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let field1 = world::with_world(|w| w.field_n as usize);

    // Book: 「此卡立即置入弃牌堆」 — the card leaves the field into discard.
    // Observed as UnplaceCard (host's unplace) or a field-count drop.
    let unplaced = spec::ifonly_02_log_ok(effects, 7)
        || field1 < field0
        || effects
            .iter()
            .any(|e| matches!(e, Effect::UnplaceCard { .. }));
    crate::kassert!(
        unplaced,
        "C-CRYCHIC-ifonly-02: card must go to discard (UnplaceCard / field drop)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Discard leaf iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn ifonly_02_discard_at_seven_iff() {
    ifonly_02_iff_at(0);
}

/// Discard leaf iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn ifonly_02_user1_discard_at_seven_iff() {
    ifonly_02_iff_at(1);
}

// ===========================================================================
// C-CRYCHIC-soyoreturn-01 branch 2 leaf — 「获得1000资金并将此卡加入手牌」
// The [特] draw-play has a two-option prompt. Seat-symmetric.
// Isolation: no owned tiles (the discard branch's build leg stays off-path);
// hand seeded with this card + one other so the discard cashout is reachable.
// ===========================================================================

/// Shared leaf at a **concrete** user seat. Symbolic pick covers both branches.
fn soyoreturn_01_branch2_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    // Hand: this card + one other, so `take_from_hand(ID)` succeeds and the
    // discard-all cashout is `500 * 1`. Host `card_slot` aliases a 29-byte
    // name to pilot slot 0 (AYA heuristic), so the taken card is `hand[0]`.
    sparse2(soyoreturn_on_drawn_dispatch, user, |w| {
        w.hand[0] = true;
        w.hand[2] = true;
        w.symbolic_answers = true;
    });

    let pre = world::snapshot();
    invoke::unwrap_done(card_crychic::soyo_back::on_drawn_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // Symbolic pick: branch 0 is the discard/cashout; branch 1 is the +1000
    // keep. Cover both; assert branch 2's leaf when the pick is 1.
    let picked_keep = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { n: 1000, .. }));
    let picked_discard = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { n, .. } if *n != 1000 && *n > 0));
    kani::cover!(picked_keep, "branch 2 (keep, +1000)");
    kani::cover!(picked_discard, "branch 1 (discard cashout)");
    kani::cover!(effects.is_empty(), "no money branch visible");

    if picked_keep {
        crate::kassert!(
            spec::soyoreturn_01_branch2_log_ok(effects, user),
            "C-CRYCHIC-soyoreturn-01 branch 2: exactly GainMoney{1000}"
        );
    }
    if picked_discard {
        // Discard-all cashout: this card was taken first, one other discarded,
        // then `gain(500 * 1)` + `draw(1)`. The house leg is isolated away.
        crate::kassert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::GainMoney { player: p, n: 500 } if *p == user)),
            "C-CRYCHIC-soyoreturn-01 branch 1: discard cashout is 500 * n"
        );
        crate::kassert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Draw { player: p, n: 1 } if *p == user)),
            "C-CRYCHIC-soyoreturn-01 branch 1: then draw 1"
        );
    }
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Branch-2 leaf at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn soyoreturn_01_branch2_iff() {
    soyoreturn_01_branch2_iff_at(0);
}

/// Branch-2 leaf at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn soyoreturn_01_user1_branch2_iff() {
    soyoreturn_01_branch2_iff_at(1);
}