//! Unit `mujica`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. reachable-disagreement MUTANT companion.
//!
//! Cheap clauses first (money / place / stay / stun). Prompt and move-plan
//! clauses are tagged PROTOCOL-LEVEL in `docs/rulebook/spec/kani/mujica.md`
//! where the host's `plan::*` / `card_move` / `give_extra_turn` stubs cannot
//! record the book's motion.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn install_and_new() -> World {
    host::install();
    World::new()
}

fn effects_of() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

// ===========================================================================
// C-MUJICA-黑色生日-01 / -02
// 「每名你以外的资金不多于1000的玩家支付你800资金，
//  每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
// Solvent payers only (G-BANKRUPT covers shortfalls).
// ===========================================================================

fn black_bday_world(low_bracket: bool) -> World {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 0;
    }
    let user = 0i32;
    let other = 1i32;
    w.money[user as usize] = 500;
    // Solvent: low pays 800, high pays 400.
    w.money[other as usize] = if low_bracket { 1_000 } else { 1_001 };
    w.trig_player = user;
    w
}

/// Low bracket: the other player (money ≤ 1000) pays the user exactly 800.
#[kani::proof]
#[kani::unwind(24)]
pub fn black_bday_low_pays_800_iff() {
    let w = black_bday_world(true);
    let user = w.trig_player;
    let other = 1i32;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:黑色生日", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "low bracket exercised");
    assert!(
        spec::mujica::black_bday_low_effects(effects, other, user),
        "C-MUJICA-黑色生日-01: exactly one PayTo(other→user, 800)"
    );
    assert!(
        spec::mujica::black_bday_low_post_ok(&post, other, user, pre.money[1], pre.money[0]),
        "C-MUJICA-黑色生日-01: money moved 800"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.stay == pre.stay, "frame: stay");
    assert!(post.hand == pre.hand, "frame: hand");
}

/// High bracket: the other player (money > 1000) pays 200 twice (total 400).
#[kani::proof]
#[kani::unwind(24)]
pub fn black_bday_high_pays_400_iff() {
    let w = black_bday_world(false);
    let user = w.trig_player;
    let other = 1i32;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:黑色生日", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "high bracket exercised");
    assert!(
        spec::mujica::black_bday_high_effects(effects, other, user),
        "C-MUJICA-黑色生日-02: exactly two PayTo(other→user, 200)"
    );
    assert!(
        spec::mujica::black_bday_high_post_ok(&post, other, user, pre.money[1], pre.money[0]),
        "C-MUJICA-黑色生日-02: money moved 400"
    );
}

/// MUTANT: the low-bracket reference wants 801. The card does 800.
#[kani::proof]
#[kani::unwind(24)]
pub fn black_bday_low_mutant_is_detected() {
    let w = black_bday_world(true);
    let user = w.trig_player;
    let other = 1i32;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("Mujica:黑色生日", user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    // Card matches the true spec (800) and therefore must NOT match 801.
    assert!(
        !spec::mujica::black_bday_low_effects_mutant(effects, other, user),
        "mutant: card must not match the 801 spec"
    );
}

/// MUTANT companion (should_panic): asserting the wrong spec must fail.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn black_bday_low_mutant_panics() {
    let w = black_bday_world(true);
    let user = w.trig_player;
    let other = 1i32;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("Mujica:黑色生日", user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        spec::mujica::black_bday_low_effects_mutant(effects, other, user),
        "mutant: card must not match the 801 spec"
    );
}

// ===========================================================================
// C-MUJICA-心の雨-01 / -02 / -03
// 「使你前方20格内的所有玩家各自骰1d20」
// 「骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。」
// 「若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
// ===========================================================================

/// Pure stay-layer arithmetic (the book's 「变为」 reading: ≤3 is 2, not 1+2).
#[kani::proof]
#[kani::unwind(8)]
pub fn heart_rain_stay_layers_iff() {
    let roll: i32 = kani::any();
    kani::assume((1..=20).contains(&roll));
    let layers = spec::mujica::heart_rain_stay_layers(roll);
    kani::cover!(roll <= 3, "roll ≤ 3 → 2 layers");
    kani::cover!(roll >= 4 && roll <= 12, "roll 4–12 → 1 layer");
    kani::cover!(roll >= 13, "roll ≥ 13 → 0 layers");
    assert!(
        (layers == 2 && roll <= 3)
            || (layers == 1 && (4..=12).contains(&roll))
            || (layers == 0 && roll >= 13),
        "C-MUJICA-心の雨-02: ≤3 → 2, ≤12 → 1, else 0"
    );
}

/// MUTANT: ≤3 stacks 3 layers.
#[kani::proof]
#[kani::unwind(8)]
pub fn heart_rain_stay_layers_mutant_is_detected() {
    let roll: i32 = kani::any();
    kani::assume((1..=3).contains(&roll));
    assert!(
        spec::mujica::heart_rain_stay_layers(roll) != spec::mujica::heart_rain_stay_layers_mutant(roll),
        "mutant: 2 layers must differ from 3"
    );
}

/// Fallback branch: nobody in the 20-tile window (or all rolls ≥ 13).
/// One other player standing on the user's own tile is not 「前方」.
#[kani::proof]
#[kani::unwind(32)]
pub fn heart_rain_fallback_stun_and_money_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 2_000;
        w.pos[p] = 0;
        w.stay[p] = 0;
    }
    // Other player shares the user's tile → tile_forward = 0 → not 「前方」.
    w.pos[1] = 0;
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:心の雨", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "empty window → fallback");
    assert!(
        spec::mujica::heart_rain_fallback_effects(effects, user),
        "C-MUJICA-心の雨-03: fallback is GiveStun(user,1) + GainMoney(user,1000)"
    );
    assert!(post.stay[0] == pre.stay[0] + 1, "user stun layer (shared stay counter)");
    assert!(post.money[0] == pre.money[0] + 1000, "user +1000");
    assert!(post.money[1] == pre.money[1], "frame: other money");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// In-window player with a symbolic roll: stay layers match the book, and
/// the fallback fires only when the roll granted none.
#[kani::proof]
#[kani::unwind(32)]
pub fn heart_rain_in_window_roll_stay_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 2_000;
        w.stay[p] = 0;
    }
    // Other player 5 tiles clockwise ahead of the user (inside 1..=20).
    w.pos[0] = 10;
    w.pos[1] = 15;
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:心の雨", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // Recover the roll from the stay delta (0 / 1 / 2 layers).
    let gained = post.stay[1] - pre.stay[1];
    kani::cover!(gained == 2, "roll ≤ 3");
    kani::cover!(gained == 1, "roll 4–12");
    kani::cover!(gained == 0, "roll ≥ 13 (fallback)");
    assert!(
        gained == 0 || gained == 1 || gained == 2,
        "C-MUJICA-心の雨-02: stay gain is 0, 1, or 2"
    );
    if gained > 0 {
        assert!(
            spec::mujica::heart_rain_stay_effect(effects, 1, gained),
            "C-MUJICA-心の雨-02: exactly one GiveStay for the in-window player"
        );
        assert!(
            spec::mujica::stay_frame_ok(&pre, &post, &[1], w.n_players),
            "frame: only the in-window player's stay moves"
        );
    } else {
        assert!(
            spec::mujica::heart_rain_fallback_effects(effects, user),
            "C-MUJICA-心の雨-03: no stay gained → fallback stun+1000"
        );
    }
}

// ===========================================================================
// C-MUJICA-骰子已经掷下-01
// 「将此卡放置于自身场上」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn dice_cast_places_on_own_field_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
    }
    let user = 0i32;
    w.trig_player = user;
    w.turn_key = 7;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:骰子已经掷下", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "play places the card");
    assert!(
        spec::mujica::dice_cast_place_effects(effects, user, 2),
        "C-MUJICA-骰子-01: PlaceCard(user, Field) plus the latch SetState"
    );
    assert!(post.hand == pre.hand, "frame: hand (play leaves hand outside this body)");
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// MUTANT: demands a GainMoney as well — the card must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn dice_cast_mutant_extra_money_is_detected() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    w.turn_key = 7;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("Mujica:骰子已经掷下", user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::GainMoney { .. })),
        "mutant: play must not mint money"
    );
}

// ===========================================================================
// C-MUJICA-燃尽前的线香花火-01
// 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn sparkler_places_two_crystals_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
    }
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:燃尽前的线香花火", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "play places with 2 crystals");
    assert!(
        spec::mujica::sparkler_place_effects(effects, user, 2),
        "C-MUJICA-花火-01: PlaceCard + AddCounter(+2)"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// MUTANT: demands 3 crystals.
#[kani::proof]
#[kani::unwind(24)]
pub fn sparkler_crystal_count_mutant_is_detected() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("Mujica:燃尽前的线香花火", user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::AddCounter { n, .. } if *n == 3)),
        "mutant: crystals must be 2, not 3"
    );
}

/// Last-crystal body: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
/// Entry index 2 (`On::Hook(CounterChanged)`).
#[kani::proof]
#[kani::unwind(24)]
pub fn sparkler_last_crystal_stun_two_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.stay[p] = 0;
    }
    let user = 0i32;
    w.trig_player = user;
    // Crystals already at 0 so the residual guard's `card.counter == 0` holds
    // once the host fires CounterChanged; the body itself is what we pin.
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Entry 2 = On::Hook(CounterChanged) `on_crystals_changed`.
    invoke::unwrap_done(invoke::entry("Mujica:燃尽前的线香花火", 2, user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "last-crystal body");
    assert!(
        spec::mujica::sparkler_last_crystal_effects(effects, user),
        "C-MUJICA-花火-03: GiveStun(user, 2)"
    );
    let dest = world::with_world(|ww| ww.dest);
    assert!(
        spec::mujica::sparkler_last_crystal_dest(dest),
        "C-MUJICA-花火-03: dest becomes Graveyard"
    );
    assert!(post.stay[0] == pre.stay[0] + 2, "user gains 2 stun layers");
}

/// MUTANT: last crystal grants 1 stun, not 2.
#[kani::proof]
#[kani::unwind(24)]
pub fn sparkler_last_crystal_mutant_is_detected() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry("Mujica:燃尽前的线香花火", 2, user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    assert!(
        spec::mujica::sparkler_last_crystal_effects(effects, user),
        "body grants 2 stun"
    );
    assert!(
        !spec::mujica::sparkler_last_crystal_effects_mutant(effects, user),
        "mutant: 2 ≠ 1"
    );
}

// ===========================================================================
// C-MUJICA-人偶的箱庭-01 (pay branch)
// 「向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。」
// Isolation: the other player is [无法移动] (stay > 0) so only the pay
// branch is offered (clause 02). X is the clockwise walk length.
// ===========================================================================

fn doll_garden_pay_world() -> World {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 5_000;
        w.pos[p] = 0;
        w.stay[p] = 0;
    }
    // User at tile 0; other at tile 5 → clockwise X = 55? No: tile_forward
    // from other to user. Board is 60 tiles. Other at 5, user at 0:
    // clockwise 5 → 0 is 55 steps. Keep them close: user at 20, other at 10
    // → X = 10.
    w.pos[0] = 20;
    w.pos[1] = 10;
    // [无法移动] so the prompt has only the pay branch.
    w.stay[1] = 1;
    let user = 0i32;
    w.trig_player = user;
    w
}

#[kani::proof]
#[kani::unwind(32)]
pub fn doll_garden_pay_amount_iff() {
    let w = doll_garden_pay_world();
    let user = w.trig_player;
    let other = 1i32;
    // X = clockwise tiles from other(10) to user(20) = 10.
    let x = 10i32;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:人偶的箱庭", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "pay branch");
    assert!(
        spec::mujica::doll_garden_pay_effects(effects, other, user, x),
        "C-MUJICA-人偶的箱庭-01: PayTo(other→user, X*20)"
    );
    let amount = spec::mujica::doll_garden_pay_amount(x);
    assert!(post.money[1] == pre.money[1] - amount, "payer debited X*20");
    assert!(post.money[0] == pre.money[0] + amount, "user credited X*20");
}

/// MUTANT: X*20 + 10.
#[kani::proof]
#[kani::unwind(32)]
pub fn doll_garden_pay_amount_mutant_is_detected() {
    let w = doll_garden_pay_world();
    let user = w.trig_player;
    let x = 10i32;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("Mujica:人偶的箱庭", user));
    let post = world::snapshot();
    assert!(
        post.money[1] != 5_000 - spec::mujica::doll_garden_pay_amount_mutant(x),
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
// ===========================================================================

/// Pure toggle: 1↔2 when 状态2 exists; otherwise no-op.
#[kani::proof]
#[kani::unwind(8)]
pub fn welcome_toggle_arithmetic_iff() {
    let cur: i32 = kani::any();
    kani::assume((0..=2).contains(&cur));
    let has_state2: bool = kani::any();
    kani::cover!(cur == 1 && has_state2, "1 → 2");
    kani::cover!(cur == 2 && has_state2, "2 → 1");
    kani::cover!(!has_state2, "no 状态2 → no-op");
    let to = spec::mujica::welcome_next_state(cur, has_state2);
    if has_state2 {
        assert!(
            to == if cur == 1 { 2 } else if cur == 2 { 1 } else { cur },
            "C-MUJICA-欢迎-02: skillState toggles 1↔2"
        );
    } else {
        assert!(to == cur, "C-MUJICA-欢迎-02: no 状态2 → no effect");
    }
}

/// MUTANT: toggle always flips even without 状态2 (the known engine gap).
#[kani::proof]
#[kani::unwind(8)]
pub fn welcome_no_state2_mutant_is_detected() {
    assert!(
        spec::mujica::welcome_next_state(0, false) == 0,
        "no 状态2 → no-op"
    );
    assert!(
        spec::mujica::welcome_next_state_mutant(0, false) == 2,
        "mutant would force 状态2"
    );
    assert!(
        spec::mujica::welcome_next_state(0, false) != spec::mujica::welcome_next_state_mutant(0, false),
        "mutant: forced toggle must differ from the required no-op"
    );
}

// ===========================================================================
// C-MUJICA-（喵梦）-01
// 「（1）将此卡放置于场上」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn nyamu_places_on_field_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:（喵梦）", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "place");
    assert!(
        spec::mujica::nyamu_place_effects(effects, user, 2),
        "C-MUJICA-喵梦-01: PlaceCard(user, Field)"
    );
    assert!(post.money == pre.money, "frame: money");
}

// ===========================================================================
// C-MUJICA-（海铃）-01
// 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
// Isolation: 2 players → next actor is the other seat.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn umiri_places_on_next_actors_field_iff() {
    let mut w = install_and_new();
    w.n_players = 2;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
    }
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:（海铃）", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    let next = spec::mujica::umiri_next_actor(user, 2);
    kani::cover!(next == 1, "next actor is seat 1");
    assert!(
        spec::mujica::umiri_place_effects(effects, next, 2),
        "C-MUJICA-海铃-01: PlaceCard on the next actor's field"
    );
    assert!(post.money == pre.money, "frame: money");
}

// ===========================================================================
// C-MUJICA-（睦/mortis）表演的本能-01 (inert path)
// 「此卡打出时效果为场上其他玩家打出的上一张卡
//  （不计入效果带有放置于场上的卡）」
// With no prior copyable card the play is inert.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn mortis_nothing_to_copy_is_inert() {
    let mut w = install_and_new();
    w.n_players = 2;
    let user = 0i32;
    w.trig_player = user;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mujica:（睦/mortis）表演的本能", user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(true, "no copyable target");
    assert!(
        spec::mujica::mortis_inert_is_noop(effects),
        "C-MUJICA-睦-01: no prior copyable card → empty effect log"
    );
    assert!(post == pre || post.log_n == 0, "frame untouched (log empty)");
}
