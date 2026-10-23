//! Unit `mujica`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op + covers + mutant.
//! Specs live in `crate::spec::mujica` and come from the clause docs only.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Isolation assumes are named here and `kani::assume`d.
//! Narration (`ctx::log`) is not part of the frame.
//!
//! **arm gaps.** `host.rs` has no `arm_turn_key` / `arm_stay_of` / `arm_stun_of`
//! (those three stay inlined in `host::dispatch`). The thin wrappers below
//! supply the same reads and delegate the rest to the per-body subset. Do not
//! edit host.rs. `tile_forward` / `dist` share `arm_dist`.

use crate::host;
use crate::invoke;
use crate::spec::mujica as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-card host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `Mujica:黑色生日` play — others / money bracket latch / pay_to.
/// `player_out` rides `player_count` under cfg(kani) (facade bound check).
crate::host_subset!(black_bday_dispatch {
    others_count => arm_others_count,
    others_at => arm_others_at,
    slot => arm_slot,
    set_slot => arm_set_slot,
    money => arm_money,
    pay_to => arm_pay_to,
    player_count => arm_player_count,
    player_out => arm_player_out,
    log | effect => arm_log,
});

/// `Mujica:心の雨` play — window scan + stay / fallback stun+gain.
crate::host_subset!(heart_rain_dispatch {
    player_pos => arm_player_pos,
    others_count => arm_others_count,
    others_at => arm_others_at,
    tile_forward | dist => arm_dist,
    roll => arm_roll,
    apply_abnormal => arm_apply_abnormal,
    apply_abnormal => arm_apply_abnormal,
    gain | gain_typed | gain_fixed => arm_gain,
    log | effect => arm_log,
});

/// `Mujica:骰子已经掷下` play body (via `cast`) — place + once-per-turn latch.
/// GAP: `arm_turn_key` is still inlined in `host::dispatch` — see
/// [`dice_cast_dispatch`].
crate::host_subset!(dice_cast_subset {
    set_dest => arm_set_dest,
    slot => arm_slot,
    set_slot => arm_set_slot,
    place_card_at => arm_place_card_at,
    log | effect => arm_log,
});

/// Same reads as `host::dispatch` `Op::turn_key`; rest is the loud subset.
fn dice_cast_dispatch(op: card_sdk::kani_host::Op, a: card_sdk::kani_host::Args) -> i64 {
    match op {
        card_sdk::kani_host::Op::turn_key => world::with_world(|w| w.turn_key) as i64,
        other => dice_cast_subset(other, a),
    }
}

/// `Mujica:燃尽前的线香花火` play — place + 2 crystals.
crate::host_subset!(sparkler_play_dispatch {
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
});

/// `Mujica:燃尽前的线香花火` `on_crystals_changed` — discard + 2 stun.
crate::host_subset!(sparkler_changed_dispatch {
    set_dest => arm_set_dest,
    apply_abnormal => arm_apply_abnormal,
    log | effect => arm_log,
});

/// `Mujica:人偶的箱庭` play — window scan + pay / move branches.
/// GAP: `arm_stay_of` / `arm_stun_of` are still inlined in `host::dispatch` —
/// see [`doll_garden_dispatch`]. `tile_forward` shares `arm_dist`.
crate::host_subset!(doll_garden_subset {
    others_count => arm_others_count,
    others_at => arm_others_at,
    tile_forward | dist => arm_dist,
    player_pos => arm_player_pos,
    ask => arm_ask,
    opt_str | opt_int | opt_tile => arm_opt,
    pay_to => arm_pay_to,
    roll => arm_roll,
    // Isolated move branch: `ctx::plan::set_steps` / `set_resolve` (stubs).
    set_steps | set_resolve => arm_stub_zero,
    card_move => arm_card_move,
    log | effect => arm_log,
});

/// Same reads as `host::dispatch` `Op::stay_of` / `Op::stun_of`; rest is the
/// loud subset.
fn doll_garden_dispatch(op: card_sdk::kani_host::Op, a: card_sdk::kani_host::Args) -> i64 {
    match op {
        card_sdk::kani_host::Op::stay_of => {
            let p = a[0];
            world::with_world(|w| {
                if p >= 0 && (p as usize) < MAX_PLAYERS {
                    w.stay[p as usize]
                } else {
                    0
                }
            }) as i64
        }
        card_sdk::kani_host::Op::stun_of => {
            let p = a[0];
            world::with_world(|w| {
                if p >= 0 && (p as usize) < MAX_PLAYERS {
                    w.stun[p as usize]
                } else {
                    0
                }
            }) as i64
        }
        other => doll_garden_subset(other, a),
    }
}

/// `Mujica:（喵梦）` play — place on own field.
crate::host_subset!(nyamu_dispatch {
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    log | effect => arm_log,
});

/// `Mujica:（海铃）` play — place on the next actor + take band skills.
crate::host_subset!(umiri_dispatch {
    player_count => arm_player_count,
    player_out => arm_player_out,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    set_slot => arm_set_slot,
    group_skills => arm_band_skills,
    add_group_skill => arm_add_band_skill,
    log | effect => arm_log,
});

/// `Mujica:（睦/mortis）表演的本能` play — inert copy (narration only).
crate::host_subset!(mortis_dispatch {
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

/// In-place 2-seat world on `dispatch`, then `f` for the card-specific fields.
/// No `reset_world` full-struct copy (SSA cliff).
fn sparse2(dispatch: card_sdk::kani_host::Backend, user: i32, f: impl FnOnce(&mut world::World)) {
    host::install_backend(dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.money[p] = 0;
            w.pos[p] = 0;
            w.stay[p] = 0;
            w.stun[p] = 0;
        }
        w.trig_player = user;
        f(w);
    });
}

// ===========================================================================
// C-MUJICA-黑色生日-01 / -02
// 「每名你以外的资金不多于1000的玩家支付你800资金，
//  每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
// Solvent payers only (G-BANKRUPT covers shortfalls).
// Seat-symmetric (the user is the payee of every other seat).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn black_bday_low_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let other = 1 - user;
    sparse2(black_bday_dispatch, user, |w| {
        w.money[user as usize] = 500;
        w.money[other as usize] = 1_000;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::black_birthday::black_birthday_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "low bracket exercised");
    crate::kassert!(
        spec::black_bday_low_effects(effects, other, user),
        "C-MUJICA-黑色生日-01: exactly one PayTo(other→user, 800) plus the latch"
    );
    crate::kassert!(
        spec::black_bday_low_post_ok(
            &post,
            other,
            user,
            pre.money[other as usize],
            pre.money[user as usize]
        ),
        "C-MUJICA-黑色生日-01: money moved 800"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand");
}

/// Low-bracket iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn black_bday_low_pays_800_iff() {
    black_bday_low_iff_at(0);
}

/// Low-bracket iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn black_bday_low_user1_pays_800_iff() {
    black_bday_low_iff_at(1);
}

/// Shared high-bracket iff at a **concrete** user seat.
fn black_bday_high_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let other = 1 - user;
    sparse2(black_bday_dispatch, user, |w| {
        w.money[user as usize] = 500;
        w.money[other as usize] = 1_001;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::black_birthday::black_birthday_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "high bracket exercised");
    crate::kassert!(
        spec::black_bday_high_effects(effects, other, user),
        "C-MUJICA-黑色生日-02: exactly two PayTo(other→user, 200) plus the latch"
    );
    crate::kassert!(
        spec::black_bday_high_post_ok(
            &post,
            other,
            user,
            pre.money[other as usize],
            pre.money[user as usize]
        ),
        "C-MUJICA-黑色生日-02: money moved 400"
    );
}

/// High-bracket iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn black_bday_high_pays_400_iff() {
    black_bday_high_iff_at(0);
}

/// High-bracket iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn black_bday_high_user1_pays_400_iff() {
    black_bday_high_iff_at(1);
}

/// MUTANT: the low-bracket reference wants 801. The card does 800.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn black_bday_low_mutant_is_detected() {
    let user = 0i32;
    let other = 1i32;
    sparse2(black_bday_dispatch, user, |w| {
        w.money[user as usize] = 500;
        w.money[other as usize] = 1_000;
    });
    invoke::unwrap_done(card_mujica::black_birthday::black_birthday_kani(user));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // Card matches the true spec (800) and therefore must NOT match 801.
    crate::kassert!(
        !spec::black_bday_low_effects_mutant(effects, other, user),
        "mutant: card must not match the 801 spec"
    );
}

/// MUTANT companion (should_panic): asserting the wrong spec must fail.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn black_bday_low_mutant_panics() {
    let user = 0i32;
    let other = 1i32;
    sparse2(black_bday_dispatch, user, |w| {
        w.money[user as usize] = 500;
        w.money[other as usize] = 1_000;
    });
    invoke::unwrap_done(card_mujica::black_birthday::black_birthday_kani(user));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        spec::black_bday_low_effects_mutant(effects, other, user),
        "mutant: card must not match the 801 spec"
    );
}

// ===========================================================================
// C-MUJICA-心の雨-01 / -02 / -03
// 「使你前方20格内的所有玩家各自骰1d20」
// 「骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。」
// 「若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
// Seat-symmetric (the user is the caster).
// ===========================================================================

/// Pure stay-layer arithmetic (the book's 「变为」 reading: ≤3 is 2, not 1+2).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn heart_rain_stay_layers_iff() {
    let roll: i32 = kani::any();
    kani::assume((1..=20).contains(&roll));
    let layers = spec::heart_rain_stay_layers(roll);
    kani::cover!(roll <= 3, "roll ≤ 3 → 2 layers");
    kani::cover!(roll >= 4 && roll <= 12, "roll 4–12 → 1 layer");
    kani::cover!(roll >= 13, "roll ≥ 13 → 0 layers");
    crate::kassert!(
        (layers == 2 && roll <= 3)
            || (layers == 1 && (4..=12).contains(&roll))
            || (layers == 0 && roll >= 13),
        "C-MUJICA-心の雨-02: ≤3 → 2, ≤12 → 1, else 0"
    );
}

/// MUTANT: ≤3 stacks 3 layers.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn heart_rain_stay_layers_mutant_is_detected() {
    let roll: i32 = kani::any();
    kani::assume((1..=3).contains(&roll));
    crate::kassert!(
        spec::heart_rain_stay_layers(roll) != spec::heart_rain_stay_layers_mutant(roll),
        "mutant: 2 layers must differ from 3"
    );
}

/// Fallback branch: nobody in the 20-tile window (or all rolls ≥ 13).
/// One other player standing on the user's own tile is not 「前方」.
fn heart_rain_fallback_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let other = 1 - user;
    sparse2(heart_rain_dispatch, user, |w| {
        w.money[user as usize] = 2_000;
        w.money[other as usize] = 2_000;
        // Other player shares the user's tile → tile_forward = 0 → not 「前方」.
        w.pos[user as usize] = 0;
        w.pos[other as usize] = 0;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::heart_rain::heart_rain_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "empty window → fallback");
    crate::kassert!(
        spec::heart_rain_fallback_effects(effects, user),
        "C-MUJICA-心の雨-03: fallback is GiveStun(user,1) + GainMoney(user,1000)"
    );
    crate::kassert!(
        post.stun[user as usize] == pre.stun[user as usize] + 1,
        "user stun layer"
    );
    crate::kassert!(
        post.money[user as usize] == pre.money[user as usize] + 1000,
        "user +1000"
    );
    crate::kassert!(
        post.money[other as usize] == pre.money[other as usize],
        "frame: other money"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Fallback iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn heart_rain_fallback_stun_and_money_iff() {
    heart_rain_fallback_iff_at(0);
}

/// Fallback iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn heart_rain_fallback_user1_stun_and_money_iff() {
    heart_rain_fallback_iff_at(1);
}

/// In-window player with a symbolic roll: stay layers match the book, and
/// the fallback fires only when the roll granted none.
fn heart_rain_in_window_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let other = 1 - user;
    sparse2(heart_rain_dispatch, user, |w| {
        w.money[user as usize] = 2_000;
        w.money[other as usize] = 2_000;
        // Other player 5 tiles clockwise ahead of the user (inside 1..=20).
        w.pos[user as usize] = 10;
        w.pos[other as usize] = 15;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::heart_rain::heart_rain_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // Recover the roll from the stay delta (0 / 1 / 2 layers).
    let gained = post.stay[other as usize] - pre.stay[other as usize];
    kani::cover!(gained == 2, "roll ≤ 3");
    kani::cover!(gained == 1, "roll 4–12");
    kani::cover!(gained == 0, "roll ≥ 13 (fallback)");
    crate::kassert!(
        gained == 0 || gained == 1 || gained == 2,
        "C-MUJICA-心の雨-02: stay gain is 0, 1, or 2"
    );
    if gained > 0 {
        crate::kassert!(
            spec::heart_rain_stay_effect(effects, other, gained),
            "C-MUJICA-心の雨-02: exactly one GiveStay for the in-window player"
        );
        crate::kassert!(
            spec::stay_frame_ok(&pre, &post, &[other], pre.n_players),
            "frame: only the in-window player's stay moves"
        );
    } else {
        crate::kassert!(
            spec::heart_rain_fallback_effects(effects, user),
            "C-MUJICA-心の雨-03: no stay gained → fallback stun+1000"
        );
    }
}

/// In-window iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn heart_rain_in_window_roll_stay_iff() {
    heart_rain_in_window_iff_at(0);
}

/// In-window iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn heart_rain_in_window_user1_roll_stay_iff() {
    heart_rain_in_window_iff_at(1);
}

// ===========================================================================
// C-MUJICA-骰子已经掷下-01
// 「将此卡放置于自身场上」
// Seat-symmetric (places on the user's own field).
// ===========================================================================

fn dice_cast_place_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(dice_cast_dispatch, user, |w| {
        w.turn_key = 7;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::dice_cast::play_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "play places the card");
    crate::kassert!(
        spec::dice_cast_place_effects(effects, user, 2),
        "C-MUJICA-骰子-01: PlaceCard(user, Field) plus the latch SetState"
    );
    crate::kassert!(
        world::arr8b_eq(post.hand, pre.hand),
        "frame: hand (play leaves hand outside this body)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Dice-cast place iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn dice_cast_places_on_own_field_iff() {
    dice_cast_place_iff_at(0);
}

/// Dice-cast place iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn dice_cast_user1_places_on_own_field_iff() {
    dice_cast_place_iff_at(1);
}

/// MUTANT: demands a GainMoney as well — the card must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn dice_cast_mutant_extra_money_is_detected() {
    let user = 0i32;
    sparse2(dice_cast_dispatch, user, |w| {
        w.turn_key = 7;
    });
    invoke::unwrap_done(card_mujica::dice_cast::play_kani(user));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::GainMoney { .. })),
        "mutant: play must not mint money"
    );
}

// ===========================================================================
// C-MUJICA-燃尽前的线香花火-01
// 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
// Seat-symmetric.
// ===========================================================================

fn sparkler_place_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(sparkler_play_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::sparkler::sparkler_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "play places with 2 crystals");
    crate::kassert!(
        spec::sparkler_place_effects(effects, user, 2),
        "C-MUJICA-花火-01: PlaceCard + AddCounter(+2)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// Sparkler place iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_places_two_crystals_iff() {
    sparkler_place_iff_at(0);
}

/// Sparkler place iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_user1_places_two_crystals_iff() {
    sparkler_place_iff_at(1);
}

/// MUTANT: demands 3 crystals.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_crystal_count_mutant_is_detected() {
    let user = 0i32;
    sparse2(sparkler_play_dispatch, user, |_| {});
    invoke::unwrap_done(card_mujica::sparkler::sparkler_kani(user));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::AddCounter { n, .. } if *n == 3)),
        "mutant: crystals must be 2, not 3"
    );
}

/// Last-crystal body: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
/// Direct body call (`on_crystals_changed_kani`). Seat-symmetric.
fn sparkler_last_crystal_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(sparkler_changed_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::sparkler::on_crystals_changed_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "last-crystal body");
    crate::kassert!(
        spec::sparkler_last_crystal_effects(effects, user),
        "C-MUJICA-花火-03: GiveStun(user, 2)"
    );
    let dest = world::with_world(|ww| ww.dest);
    crate::kassert!(
        spec::sparkler_last_crystal_dest(dest),
        "C-MUJICA-花火-03: dest becomes Graveyard"
    );
    crate::kassert!(
        post.stun[user as usize] == pre.stun[user as usize] + 2,
        "user gains 2 stun layers"
    );
}

/// Last-crystal iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_last_crystal_stun_two_iff() {
    sparkler_last_crystal_iff_at(0);
}

/// Last-crystal iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_last_crystal_user1_stun_two_iff() {
    sparkler_last_crystal_iff_at(1);
}

/// MUTANT: last crystal grants 1 stun, not 2.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn sparkler_last_crystal_mutant_is_detected() {
    let user = 0i32;
    sparse2(sparkler_changed_dispatch, user, |_| {});
    invoke::unwrap_done(card_mujica::sparkler::on_crystals_changed_kani(user));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    crate::kassert!(
        spec::sparkler_last_crystal_effects(effects, user),
        "body grants 2 stun"
    );
    crate::kassert!(
        !spec::sparkler_last_crystal_effects_mutant(effects, user),
        "mutant: 2 ≠ 1"
    );
}

// ===========================================================================
// C-MUJICA-人偶的箱庭-01 (pay branch)
// 「向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。」
// Isolation: the other player is [无法移动] (stay > 0) so only the pay
// branch is offered (clause 02). X is the clockwise walk length.
// Seat-symmetric (the user is the payee).
// ===========================================================================

/// Shared pay-branch iff at a **concrete** user seat.
/// Isolation: other at tile 10, user at tile 20 → clockwise X = 10; other
/// has stay = 1 so the prompt / move branch is not opened (ops still declared).
fn doll_garden_pay_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let other = 1 - user;
    // X = clockwise tiles from other(10) to user(20) = 10.
    let x = 10i32;
    sparse2(doll_garden_dispatch, user, |w| {
        w.money[user as usize] = 5_000;
        w.money[other as usize] = 5_000;
        w.pos[user as usize] = 20;
        w.pos[other as usize] = 10;
        // [无法移动] so the prompt has only the pay branch.
        w.stay[other as usize] = 1;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::doll_garden::doll_garden_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "pay branch");
    crate::kassert!(
        spec::doll_garden_pay_effects(effects, other, user, x),
        "C-MUJICA-人偶的箱庭-01: PayTo(other→user, X*20)"
    );
    let amount = spec::doll_garden_pay_amount(x);
    crate::kassert!(
        post.money[other as usize] == pre.money[other as usize] - amount,
        "payer debited X*20"
    );
    crate::kassert!(
        post.money[user as usize] == pre.money[user as usize] + amount,
        "user credited X*20"
    );
}

/// Doll-garden pay iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn doll_garden_pay_amount_iff() {
    doll_garden_pay_iff_at(0);
}

/// Doll-garden pay iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn doll_garden_user1_pay_amount_iff() {
    doll_garden_pay_iff_at(1);
}

/// MUTANT: X*20 + 10.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn doll_garden_pay_amount_mutant_is_detected() {
    let user = 0i32;
    let other = 1i32;
    let x = 10i32;
    sparse2(doll_garden_dispatch, user, |w| {
        w.money[user as usize] = 5_000;
        w.money[other as usize] = 5_000;
        w.pos[user as usize] = 20;
        w.pos[other as usize] = 10;
        w.stay[other as usize] = 1;
    });
    invoke::unwrap_done(card_mujica::doll_garden::doll_garden_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[other as usize] != 5_000 - spec::doll_garden_pay_amount_mutant(x),
        "mutant: card pays X*20, not X*20+10"
    );
}

// ===========================================================================
// C-MUJICA-欢迎-02
// 「转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）」
// 「状态」 = skillState 1 ↔ 2.
//
// The play body designates a player via `ctx::target(who)`. Under the Kani
// host `Op::target` always returns 0 (structure stub), so the designated
// seat cannot be modeled — BLOCKED-HOST (`target`). What is proved here is
// the clause's determinate toggle arithmetic and the required no-op when
// the target has no 状态2. The engine's two known gaps (user not offered;
// non-状态2 seat still toggled) are the contract findings in the unit table.
// Pure-formula harness — no body call.
// ===========================================================================

/// Pure toggle: 1↔2 when 状态2 exists; otherwise no-op.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn welcome_toggle_arithmetic_iff() {
    let cur: i32 = kani::any();
    kani::assume((0..=2).contains(&cur));
    let has_state2: bool = kani::any();
    kani::cover!(cur == 1 && has_state2, "1 → 2");
    kani::cover!(cur == 2 && has_state2, "2 → 1");
    kani::cover!(!has_state2, "no 状态2 → no-op");
    let to = spec::welcome_next_state(cur, has_state2);
    if has_state2 {
        crate::kassert!(
            to == if cur == 1 {
                2
            } else if cur == 2 {
                1
            } else {
                cur
            },
            "C-MUJICA-欢迎-02: skillState toggles 1↔2"
        );
    } else {
        crate::kassert!(to == cur, "C-MUJICA-欢迎-02: no 状态2 → no effect");
    }
}

/// MUTANT: toggle always flips even without 状态2 (the known engine gap).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn welcome_no_state2_mutant_is_detected() {
    crate::kassert!(spec::welcome_next_state(0, false) == 0, "no 状态2 → no-op");
    crate::kassert!(
        spec::welcome_next_state_mutant(0, false) == 2,
        "mutant would force 状态2"
    );
    crate::kassert!(
        spec::welcome_next_state(0, false) != spec::welcome_next_state_mutant(0, false),
        "mutant: forced toggle must differ from the required no-op"
    );
}

// ===========================================================================
// C-MUJICA-（喵梦）-01
// 「（1）将此卡放置于场上」
// Seat-symmetric.
// ===========================================================================

fn nyamu_place_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(nyamu_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::nyamu_card::nyamu_card_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "place");
    crate::kassert!(
        spec::nyamu_place_effects(effects, user, 2),
        "C-MUJICA-喵梦-01: PlaceCard(user, Field)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// Nyamu place iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn nyamu_places_on_field_iff() {
    nyamu_place_iff_at(0);
}

/// Nyamu place iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn nyamu_user1_places_on_field_iff() {
    nyamu_place_iff_at(1);
}

// ===========================================================================
// C-MUJICA-（海铃）-01
// 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
// Isolation: `n_players = 2`, both living → next actor is the other seat
// (under cfg(kani) `player_count()` is the constant 4; seats 2..3 read out).
// Seat-symmetric (user=0 places on 1, user=1 places on 0).
// ===========================================================================

fn umiri_place_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let holder = 1 - user;
    sparse2(umiri_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::umiri_card::umiri_card_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // Next living seat after `user` in a 2-seat world is the other seat.
    let next = spec::umiri_next_actor(user, 2);
    kani::cover!(next == holder, "next actor is the other seat");
    crate::kassert!(
        spec::umiri_place_effects(effects, holder, 2),
        "C-MUJICA-海铃-01: PlaceCard on the next actor's field"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// Umiri place iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn umiri_places_on_next_actors_field_iff() {
    umiri_place_iff_at(0);
}

/// Umiri place iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn umiri_user1_places_on_next_actors_field_iff() {
    umiri_place_iff_at(1);
}

// ===========================================================================
// C-MUJICA-（睦/mortis）表演的本能-01 (inert path)
// 「此卡打出时效果为场上其他玩家打出的上一张卡
//  （不计入效果带有放置于场上的卡）」
// With no prior copyable card the play is inert.
// Seat-symmetric.
// ===========================================================================

fn mortis_inert_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2(mortis_dispatch, user, |_| {});
    let pre = world::snapshot();
    invoke::unwrap_done(card_mujica::mortis_instinct::play_kani(user));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(true, "no copyable target");
    crate::kassert!(
        spec::mortis_inert_is_noop(effects),
        "C-MUJICA-睦-01: no prior copyable card → empty effect log"
    );
    crate::kassert!(post == pre, "frame untouched");
}

/// Mortis inert iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn mortis_nothing_to_copy_is_inert() {
    mortis_inert_iff_at(0);
}

/// Mortis inert iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn mortis_user1_nothing_to_copy_is_inert() {
    mortis_inert_iff_at(1);
}

// ===========================================================================
// C-MUJICA-祥移动-01 (destination arithmetic)
// 「强制一名玩家向你选择的方向移动3格」
// The walk itself is `card_move` + `plan::set_steps(3)` (host stubs) —
// PROTOCOL-LEVEL for the motion. The 3-tile destination arithmetic is the
// text's determinate part. Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn saki_move_dest_is_three_tiles() {
    let from: i32 = kani::any();
    kani::assume((0..60).contains(&from));
    let dir: i32 = kani::any();
    kani::assume(dir == 1 || dir == -1);
    let dest = spec::saki_move_dest(from, dir, 60);
    kani::cover!(dir == 1, "clockwise 3");
    kani::cover!(dir == -1, "counter-clockwise 3");
    crate::kassert!(
        spec::saki_move_steps() == 3,
        "C-MUJICA-祥移动-01: walk length is exactly 3"
    );
    let expected = if dir >= 0 {
        (from + 3) % 60
    } else {
        (((from - 3) % 60) + 60) % 60
    };
    crate::kassert!(
        dest == expected,
        "C-MUJICA-祥移动-01: dest is from±3 mod 60"
    );
}

/// MUTANT: 4-tile walk.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn saki_move_dest_mutant_is_detected() {
    let from: i32 = kani::any();
    kani::assume((0..60).contains(&from));
    let dir = 1i32;
    crate::kassert!(
        spec::saki_move_dest(from, dir, 60) != spec::saki_move_dest_mutant(from, dir, 60),
        "mutant: 3-tile dest ≠ 4-tile dest"
    );
}

// ===========================================================================
// C-MUJICA-祥移动-03 arithmetic
// 「触发结算时进行的支付价格减半」
// OPEN-取整 on odd prices; this spec uses floor.
// Pure-formula harness — no body call.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn saki_move_halve_pay_amount() {
    let amount: i32 = kani::any();
    kani::assume((0..=2_000).contains(&amount));
    kani::cover!(amount == 320, "even 320 → 160");
    kani::cover!(amount == 321, "odd 321 → 160 (OPEN-取整 floor)");
    crate::kassert!(
        spec::saki_move_halve_pay(amount) == amount / 2,
        "C-MUJICA-祥移动-03: payment price halved"
    );
}
