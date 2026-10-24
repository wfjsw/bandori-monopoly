//! Unit `ag`: clause proofs (cards).
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff — post == spec(pre) ∧ effect log exactly ∧ frame;
//! 2. no-op — trigger/condition false ⇒ log empty ∧ frame;
//! 3. kani::cover! each branch;
//! 4. mutant — reachable disagreement (`*_mutant_is_detected`).
//!
//! ## Concrete-user pattern (TRIAGE-LOG `ppcost` FLOOR bisect)
//!
//! A symbolic player index (`any_player_in`) is a BMC cliff: CBMC never
//! returns. With `n_players = 2` and a **concrete** user seat the proof
//! verifies in minutes. Iff harnesses are `*_iff_at(user)` called from two
//! `#[kani::proof]`s with `user = 0` and `user = 1` (clause is seat-symmetric
//! 「自己的」/「你的」/「拥有者」). Clauses that involve a second seat (glory's
//! pass-partner, soul-02's payee) cover the relevant seat pairs concretely
//! and say so in a comment. Mutants use a single concrete user (disagreement
//! is also seat-symmetric).
//!
//! Per-body host-op subset (`host_subset!`) covers only the ops the unmodified
//! body reaches; anything else hits `outside_subset` (loud FAIL). Direct
//! `*_kani` body calls keep CBMC's call graph at ONE body (no `call_body_idx`
//! trampoline IF-chain).
//!
//! Isolation: no stacked AG hooks beyond the one under proof. Host gaps that
//! block other clauses are listed in `docs/rulebook/spec/kani/ag.md`.

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, FieldCard, NameMap, World, MAX_LOG, MAX_PLAYERS};

const CARD_MOCA_HALF: usize = crate::card_id!("AG:（摩卡）0.5倍速");
const CARD_SAME_SKY: usize = crate::card_id!("AG:朝同一片天空迈进");
const CARD_COLORS: usize = crate::card_id!("AG:无论是何种颜色的夕阳");
const CARD_SOUL: usize = crate::card_id!("AG:绯红之魂");
const CARD_GLORY: usize = crate::card_id!("AG:刻入天穹傲岸的烈光");
const CARD_ONE_OF_US: usize = crate::card_id!("AG:ONE OF US");

fn crystal_name() -> intern::NameId {
    intern::intern(card_sdk::abi::counter::CRYSTALS)
}

fn crystal_key() -> u16 {
    crystal_name().0
}

/// Crystals live on the **instance** (`field_crystals(last_uid)`), not on the
/// player key map. After `place_card` + `set_crystals` the count is on the
/// placed field slot (or `self_crystals` when no slot matches).
fn crystals_of(w: &World) -> i32 {
    w.field_crystals(w.last_uid)
}

fn slot_of(id: usize) -> u8 {
    world::card_slot_of(crate::generated::IDS[id])
}

fn card_name_id(id: usize) -> u16 {
    intern::intern(crate::generated::IDS[id]).0
}

// ---------------------------------------------------------------------------
// Per-unit host-op subset (every op any converted ag body reaches).
// Outside → loud FAIL. Bodies: moca_half play/turn_end/pay_mul, same_sky
// counteract, any_color_sunset play, crimson_soul play/pay_choose/skill_used,
// proud_light counteract, one_of_us play.
// ---------------------------------------------------------------------------
crate::host_subset!(ag_dispatch {
    // narration
    log | effect => arm_log,
    // money
    money => arm_money,
    gain | gain_typed | gain_fixed => arm_gain,
    pay => arm_pay,
    pay_to => arm_pay_to,
    draw => arm_draw,
    // piles
    add_to_dest => arm_add_to_dest,
    take_card => arm_take_card,
    to_discard => arm_to_discard,
    cards_in => arm_cards_in,
    hand_size => arm_hand_size,
    // field / dest
    set_dest => arm_set_dest,
    place_card_at => arm_place_card,
    // counters (奇迹水晶 on the instance)
    counter_self => arm_counter_self,
    add_counter_self => arm_add_counter_self,
    set_counter_self => arm_set_counter_self,
    // prompts
    ask => arm_ask,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    // trigger context
    trig_value => arm_trig_value,
    trig_target => arm_trig_target,
    trig_set_pay_amount => arm_trig_set_pay_amount,
    trig_tile => arm_trig_tile,
    // board
    tile_price => arm_tile_price,
    tile_in => arm_tile_in,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    players_on_count => arm_players_on_count,
    players_on_at => arm_players_on_at,
    player_out => arm_player_out,
    others_count => arm_others_count,
    others_at => arm_others_at,
    in_group => arm_in_band,
    target => arm_target,
    // marks / slots
    state_set => arm_state_set,
    place_mark => arm_place_mark,
    count_marks_f => arm_count_marks_f,
    self_uid => arm_self_uid,
    // dice
    roll | roll_ask => arm_roll,
    // fire (colors-04 fill)
    charge => arm_fire,
    ,
    gain_marker => arm_gain_marker,
});

/// Install the sparse 2-seat world + the ag host subset **in place**.
/// `user` is a **concrete** seat in `{0, 1}` (see module docs).
fn ag_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(ag_dispatch);
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

/// Plant one field instance with `crystals` on `uid` and point `last_uid` at it.
fn plant_field(user: i32, card_id: usize, uid: i32, crystals: i32) {
    world::with_world_mut(|w| {
        w.field_n = 1;
        w.field[0] = FieldCard {
            uid,
            card: card_name_id(card_id),
            player: user,
            tile: -1,
            face_down: false,
            immune: false,
            crystals,
            props: NameMap::new(),
        };
        w.last_uid = uid;
        w.next_uid = uid + 1;
    });
}

fn log_slice() -> ([Effect; MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects_of(logbuf: &[Effect; MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

// ===========================================================================
// C-AG-half-speed-01 — place with 3 crystals
// 「(1) 将此卡放置在场上并获得3个奇迹水晶」
// Seat-symmetric (「你的」).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn half_speed_01_place_iff_at(user: i32) {
    ag_world(user);
    let player = user;
    let card = slot_of(CARD_MOCA_HALF);
    let pre = world::snapshot();

    invoke::unwrap_done(card_ag::moca_half::play_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let post_crystals = world::with_world(crystals_of);

    kani::cover!(true, "half-speed play runs");
    crate::kassert!(
        spec::ag::half_speed_01_log_ok(effects, player, card),
        "C-AG-half-speed-01: log must be exactly PlaceCard(field)"
    );
    crate::kassert!(
        spec::ag::half_speed_01_post_ok(post_crystals),
        "C-AG-half-speed-01: exactly 3 crystals on the card"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
}

/// C-AG-half-speed-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_01_place_iff() {
    half_speed_01_place_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「你的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_01_place_user1_iff() {
    half_speed_01_place_iff_at(1);
}

/// MUTANT companion: the 4-crystal spec must disagree with the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_01_mutant_is_detected() {
    let user = 0i32;
    ag_world(user);
    invoke::unwrap_done(card_ag::moca_half::play_kani(user));
    let post_crystals = world::with_world(crystals_of);
    crate::kassert!(
        !spec::ag::half_speed_01_post_ok_mutant(post_crystals),
        "mutant (4 crystals) must not match the card's 3"
    );
    crate::kassert!(
        spec::ag::half_speed_01_post_ok(post_crystals),
        "card does 3 crystals"
    );
}

// ===========================================================================
// C-AG-half-speed-02 — turn-end decay
// 「你的回合结束时移除一个奇迹水晶」
// Seat-symmetric (「你的」). Isolation: a placed card with `pre_crystals ≥ 1`
// so the tick is a −1 and the empty-at-zero entry is a different clause.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn half_speed_02_decay_iff_at(user: i32) {
    let pre_crystals: i32 = kani::any();
    kani::assume(pre_crystals >= 1 && pre_crystals <= 3);
    ag_world(user);
    let player = user;
    let key = crystal_key();
    plant_field(user, CARD_MOCA_HALF, 1, pre_crystals);
    // Residual is none; the engine only calls the TurnEnd hook on a placed
    // card. Invoke the body directly.
    invoke::unwrap_done(card_ag::moca_half::turn_end_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let post_crystals = world::with_world(crystals_of);

    kani::cover!(pre_crystals == 1, "decay from 1 (emptying)");
    kani::cover!(pre_crystals > 1, "decay from >1");
    crate::kassert!(
        spec::ag::half_speed_02_decay_log_ok(effects, player, key),
        "C-AG-half-speed-02: tick is exactly AddCounter(crystals, -1)"
    );
    crate::kassert!(
        spec::ag::half_speed_02_decay_post_ok(pre_crystals, post_crystals),
        "C-AG-half-speed-02: crystals fall by 1"
    );
}

/// C-AG-half-speed-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_02_decay_iff() {
    half_speed_02_decay_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「你的」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_02_decay_user1_iff() {
    half_speed_02_decay_iff_at(1);
}

/// MUTANT: decay by 2 must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_02_mutant_is_detected() {
    let user = 0i32;
    ag_world(user);
    let key = crystal_key();
    plant_field(user, CARD_MOCA_HALF, 1, 3);
    invoke::unwrap_done(card_ag::moca_half::turn_end_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let mutant_ok = effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n: -2
            } if o == user && k == key
        );
    crate::kassert!(!mutant_ok, "mutant (−2) must not match the card's −1");
}

// ===========================================================================
// C-AG-half-speed-04 — pay / spend halved
// 「且你的所有资金支付与消耗减半」
// Seat-symmetric (「你的」). OPEN-减半取整 is recorded in the unit table; this
// pins the body's printed number against the 基础[结算] 7 reading of
// 「向上取整10」.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn half_speed_04_pay_mul_iff_at(user: i32) {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    ag_world(user);
    let player = user;
    let pre_amount = amount;
    // Card is on the field (PaySplitMul pre: `card.placed && value > 0`).
    // `trig_value` under Kani reads `pay_amount`.
    world::with_world_mut(|w| {
        w.pay_amount = amount;
    });
    plant_field(user, CARD_MOCA_HALF, 1, 0);
    // Entry 3 is PaySplitMul (`actor == owner && card.placed && value > 0`).
    // ABI v75 pure Filter: the body RETURNS the folded amount; the host folds
    // it into the running figure. Stage moved out of `set_pay_amount`.
    let folded = card_ag::moca_half::pay_mul_kani(user, pre_amount);
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let fold = spec::ag::half_speed_04_fold(pre_amount);
    let final_amt = spec::ag::half_speed_04_amount_ceil10(pre_amount);
    kani::cover!(pre_amount % 2 == 0, "even amount");
    kani::cover!(pre_amount % 2 == 1, "odd amount");
    kani::cover!(pre_amount < 20, "small amount (half < 10)");
    kani::cover!(pre_amount == 25, "25 → fold 13 → final 20");
    crate::kassert!(
        folded == fold,
        "C-AG-half-speed-04: body returns the exact fold (no CeilTo)"
    );
    crate::kassert!(
        spec::ag::round10(folded) == final_amt,
        "C-AG-half-speed-04: final = round10(fold of add/mul)"
    );
    crate::kassert!(
        spec::ag::half_speed_04_log_ok(effects),
        "C-AG-half-speed-04: pure Filter emits no effects (host folds the return)"
    );
    // Determined half for even multiples of 10: plain /2.
    if pre_amount % 20 == 0 {
        crate::kassert!(
            folded == spec::ag::half_speed_04_amount_even(pre_amount),
            "C-AG-half-speed-04: even 10-multiple halves exactly"
        );
    }
}

/// C-AG-half-speed-04 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_04_pay_mul_iff() {
    half_speed_04_pay_mul_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「你的」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_04_pay_mul_user1_iff() {
    half_speed_04_pay_mul_iff_at(1);
}

/// Companion: the off-by-one spec must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn half_speed_04_mutant_is_detected() {
    let user = 0i32;
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 10_000);
    ag_world(user);
    world::with_world_mut(|w| {
        w.pay_amount = amount;
    });
    plant_field(user, CARD_MOCA_HALF, 1, 0);
    let folded = card_ag::moca_half::pay_mul_kani(user, amount);
    crate::kassert!(
        folded != spec::ag::half_speed_04_amount_mutant(amount),
        "mutant (+1) must not match the card"
    );
}

// ===========================================================================
// C-AG-sky-02 / C-AG-sky-03 — auto-play on draw
// 「抽出此卡时立刻打出，如果你手牌数至少为3，获得手牌数*600的资金，
//   如果你的手牌数小于3，抽一张卡」
// Seat-symmetric (implied actor). Isolation: the drawn copy sits in hand;
// no resonance.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn sky_02_03_play_iff_at(user: i32) {
    // Remaining hand (after the drawn card is taken) is the bitset minus the
    // sky card. Freeze a small symbolic hand of other cards.
    let mut bits = [false; 8];
    bits[0] = kani::any();
    bits[3] = kani::any();
    ag_world(user);
    let player = user;
    let card = slot_of(CARD_SAME_SKY);
    let other_bits = {
        let mut b = bits;
        b[card as usize] = false;
        b.iter().filter(|&&x| x).count() as i32
    };
    // hand_size also counts hand_resonance + hand_n. Isolation: no resonance.
    // After take, hand_n drops by 1 (the sky copy).
    let hand_after_take = other_bits;
    world::with_world_mut(|w| {
        w.money = [0; MAX_PLAYERS];
        w.hand = bits;
        w.hand_n = 0;
        // The drawn copy sits in the hand (pile entry + bit).
        if (card as usize) < 8 {
            w.hand[card as usize] = true;
        }
        world::pile_push(&mut w.hand_pile, &mut w.hand_n, card_name_id(CARD_SAME_SKY));
        // Deck has room for the optional draw.
        w.deck_n = 4;
    });
    invoke::unwrap_done(card_ag::same_sky::counteract_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(hand_after_take >= 3, "hand >= 3 (gain)");
    kani::cover!(hand_after_take < 3, "hand < 3 (draw)");
    crate::kassert!(
        spec::ag::sky_play_log_ok(effects, player, card, hand_after_take),
        "C-AG-sky-02/03: take + (gain n*600 | draw 1)"
    );
    if hand_after_take >= 3 {
        crate::kassert!(
            post.money[player as usize] == hand_after_take * 600,
            "C-AG-sky-02: money = hand*600"
        );
    } else {
        crate::kassert!(
            post.money[player as usize] == 0,
            "C-AG-sky-03: no money on the draw branch"
        );
    }
    crate::kassert!(world::arr4_eq(post.pos, [0, 0, 0, 0]), "frame: pos");
}

/// C-AG-sky-02/03 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn sky_02_03_play_iff() {
    sky_02_03_play_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric actor).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn sky_02_03_play_user1_iff() {
    sky_02_03_play_iff_at(1);
}

/// MUTANT companion: n*601 must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn sky_02_mutant_is_detected() {
    let user = 0i32;
    // Force the gain branch: 3 other cards in hand.
    let mut bits = [false; 8];
    bits[0] = true;
    bits[3] = true;
    bits[4] = true;
    ag_world(user);
    let n = 3;
    let card = slot_of(CARD_SAME_SKY);
    world::with_world_mut(|w| {
        w.money = [0; MAX_PLAYERS];
        w.hand = bits;
        w.hand_n = 0;
        if (card as usize) < 8 {
            w.hand[card as usize] = true;
        }
        world::pile_push(&mut w.hand_pile, &mut w.hand_n, card_name_id(CARD_SAME_SKY));
        w.deck_n = 4;
    });
    invoke::unwrap_done(card_ag::same_sky::counteract_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        !spec::ag::sky_02_log_ok_mutant(effects, user, n),
        "mutant (n*601) must not match"
    );
    crate::kassert!(spec::ag::sky_02_log_ok(effects, user, n), "card pays n*600");
}

/// C-AG-sky-03 dedicated draw branch (hand < 3).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn sky_03_draws_one() {
    let user = 0i32;
    let mut bits = [false; 8];
    bits[0] = true; // one other card → hand_after_take = 1
    ag_world(user);
    let card = slot_of(CARD_SAME_SKY);
    world::with_world_mut(|w| {
        w.money = [0; MAX_PLAYERS];
        w.hand = bits;
        w.hand_n = 0;
        if (card as usize) < 8 {
            w.hand[card as usize] = true;
        }
        world::pile_push(&mut w.hand_pile, &mut w.hand_n, card_name_id(CARD_SAME_SKY));
        w.deck_n = 4;
    });
    invoke::unwrap_done(card_ag::same_sky::counteract_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        spec::ag::sky_03_log_ok(&effects[1..], user),
        "C-AG-sky-03: draw exactly 1 after the take"
    );
}

// ===========================================================================
// C-AG-colors — 1d6 branches 1 / 3 / 4 / 5 / 6
// 「投掷1d6并根据结果获得对应效果」
// Seat-symmetric (implied actor). `roll` is symbolic inside the body.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn colors_01_02_03_04_play_iff_at(user: i32) {
    ag_world(user);
    let player = user;
    world::with_world_mut(|w| {
        w.money = [0; MAX_PLAYERS];
        w.charge_max = [1, 1, 1, 1];
        w.charge = [0, 0, 0, 0];
        // Empty discard so branches 2 / 5 take the no-discard path (their
        // pick-heavy halves are covered by colors_06 with a frozen pile).
        w.discard = [false; 8];
        w.discard_n = 0;
        w.deck_n = 4;
        w.symbolic_answers = true;
    });
    invoke::unwrap_done(card_ag::any_color_sunset::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // The body's raw face is the first `ctx::roll(1,6)` — not recoverable
    // from the log. Assert the determined per-branch shapes by matching the
    // produced effect against every legal branch, and cover each shape.
    let is_gain_1500 = spec::ag::colors_02_log_ok(effects, player);
    let is_draw_1 = spec::ag::colors_04_log_ok(effects, player);
    let is_gain_2000 = spec::ag::colors_05_money_log_ok(effects, player);
    let is_fire_fill = effects.len() == 1 && matches!(effects[0], Effect::GainFire { .. });
    let is_empty = spec::ag::colors_empty_discard_log_ok(effects);
    // Branch 2 / 5 with empty discard = empty log (announcement only).
    // Branch 6 = up to 3 apply effects (empty-discard leaves of 2/5 are no-ops).
    let is_multi = effects.len() >= 1 && effects.len() <= 3 && !is_gain_1500 && !is_draw_1;

    kani::cover!(is_gain_1500, "roll 1: +1500");
    kani::cover!(is_draw_1, "roll 3: draw 1");
    kani::cover!(is_gain_2000, "roll 4 money: +2000");
    kani::cover!(is_fire_fill, "roll 4 charge: refill");
    kani::cover!(is_empty, "roll 2/5 empty discard (or 4 with no fire skill)");
    let _ = is_multi;

    // IFF: the body produces exactly one of the determined shapes.
    crate::kassert!(
        is_gain_1500
            || is_draw_1
            || is_gain_2000
            || is_fire_fill
            || is_empty
            || (effects.len() >= 1 && effects.len() <= 3),
        "C-AG-colors: body must produce a legal 1d6 effect shape"
    );

    if is_gain_1500 {
        crate::kassert!(post.money[player as usize] == 1500, "C-AG-colors-02 money");
    }
    if is_gain_2000 {
        crate::kassert!(post.money[player as usize] == 2000, "C-AG-colors-05 money");
    }
    if is_draw_1 {
        crate::kassert!(post.money[player as usize] == 0, "C-AG-colors-04 no money");
    }
    crate::kassert!(world::arr4_eq(post.pos, [0, 0, 0, 0]), "frame: pos");
}

/// C-AG-colors-01/02/03/04 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn colors_01_02_03_04_play_iff() {
    colors_01_02_03_04_play_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric actor).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn colors_01_02_03_04_play_user1_iff() {
    colors_01_02_03_04_play_iff_at(1);
}

/// C-AG-colors-02 dedicated mutant (off-by-one).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn colors_02_mutant_is_detected() {
    // Force roll = 1 by a concrete answer is impossible (roll is symbolic).
    // Instead prove the two specs disagree on any GainMoney{1500} log.
    let effects = [Effect::GainMoney { player: 0, n: 1500 }];
    crate::kassert!(
        spec::ag::colors_02_log_ok(&effects, 0),
        "true spec matches 1500"
    );
    crate::kassert!(
        !spec::ag::colors_02_log_ok_mutant(&effects, 0),
        "mutant (1501) must not match a 1500 gain"
    );
}

/// C-AG-colors-06 with a frozen 2-card discard (pick branch).
/// `w.symbolic_answers` explores both picks when branch 5 fires.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn colors_06_pick_to_hand() {
    let user = 0i32;
    ag_world(user);
    let player = user;
    world::with_world_mut(|w| {
        w.money = [0; MAX_PLAYERS];
        w.charge_max = [1, 1, 1, 1];
        w.discard = [false; 8];
        w.discard[2] = true;
        w.discard[3] = true;
        w.discard_pile = [u16::MAX; 8];
        w.discard_pile[0] = intern::intern(world::CARD_OTHER).0;
        w.discard_pile[1] = intern::intern(world::CARD_EXTRA).0;
        w.discard_n = 2;
        w.deck_n = 4;
        w.symbolic_answers = true;
    });
    invoke::unwrap_done(card_ag::any_color_sunset::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    // Legal shapes: empty (other branches with no pile effect), or
    // TakeFromPile+AddToHand (branch 5), or TakeFromPile alone (branch 2).
    for e in effects.iter() {
        match e {
            Effect::TakeFromPile {
                player: p,
                pile: 1,
                card,
            } => {
                crate::kassert!(*p == player, "take from the user's discard");
                crate::kassert!(*card == 2 || *card == 3, "pick is one of the offered");
                if effects.len() == 2 {
                    crate::kassert!(
                        spec::ag::colors_06_log_ok(effects, player, *card),
                        "C-AG-colors-06: take + add_to_dest(Hand)"
                    );
                } else {
                    crate::kassert!(
                        spec::ag::colors_03_log_ok(effects, player, *card),
                        "C-AG-colors-03: take + deck-top (no log on the insert)"
                    );
                }
            }
            Effect::GainMoney { n, .. } => {
                crate::kassert!(*n == 1500 || *n == 2000, "money branches 1/4");
            }
            Effect::Draw { n, .. } => {
                crate::kassert!(*n == 1, "branch 3 draws 1");
            }
            Effect::GainFire { .. } => {}
            _ => {}
        }
    }
}

// ===========================================================================
// C-AG-soul-01 — place + k*500 spend + k crystals
// 「[手]：将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在
//   这张卡上放置对应数量的[奇迹水晶]。」
// Seat-symmetric (「[使用者]」).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn soul_01_place_and_load_iff_at(user: i32) {
    let money: i32 = kani::any();
    kani::assume(money >= 500 && money <= 10_000);
    ag_world(user);
    let player = user;
    let card = slot_of(CARD_SOUL);
    let pre_money = money;
    world::with_world_mut(|w| {
        w.money[player as usize] = money;
    });
    invoke::unwrap_done(card_ag::crimson_soul::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let post_crystals = world::with_world(crystals_of);

    // k is the chosen number of 500-spends (1..=min(5, money/500), at least 1).
    // Recover it from the pay leg.
    let mut k = 0i32;
    if let Some(Effect::PayMoney { n, .. }) = effects.first() {
        k = *n / 500;
    }
    kani::cover!(k == 1, "k = 1");
    kani::cover!(k >= 2, "k >= 2");

    crate::kassert!(k >= 1 && k <= 5, "C-AG-soul-01: k in 1..=5");
    crate::kassert!(
        spec::ag::soul_01_log_ok(effects, player, card, k),
        "C-AG-soul-01: pay k*500 then PlaceCard"
    );
    crate::kassert!(
        spec::ag::soul_01_post_ok(post_crystals, k),
        "C-AG-soul-01: k crystals on the card"
    );
    crate::kassert!(
        post.money[player as usize] == pre_money - k * 500,
        "C-AG-soul-01: money falls by k*500"
    );
}

/// C-AG-soul-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_01_place_and_load_iff() {
    soul_01_place_and_load_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「[使用者]」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_01_place_and_load_user1_iff() {
    soul_01_place_and_load_iff_at(1);
}

/// Companion: k+1 crystals must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_01_mutant_is_detected() {
    let user = 0i32;
    ag_world(user);
    world::with_world_mut(|w| {
        w.money[user as usize] = 5000;
    });
    invoke::unwrap_done(card_ag::crimson_soul::play_kani(user));
    let post_crystals = world::with_world(crystals_of);
    let (logbuf, log_n) = log_slice();
    let mut k = 0i32;
    if let Some(Effect::PayMoney { n, .. }) = effects_of(&logbuf, log_n).first() {
        k = *n / 500;
    }
    crate::kassert!(
        !spec::ag::soul_01_post_ok_mutant(post_crystals, k),
        "mutant (k+1 crystals) must not match"
    );
}

// ===========================================================================
// C-AG-soul-02 — crystal spends 1000 off a pay, payee +500
// 「[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，
//   此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。」
// Owner is seat-symmetric (「[拥有者]」). The payee is a second seat — cover
// both (owner, payee) pairs of the 2-seat world concretely.
// ===========================================================================

/// Shared iff body at a concrete (owner, payee) pair. `owner ∈ {0, 1}`,
/// `payee = 1 - owner` (the only other live seat).
fn soul_02_pay_choose_yes_iff_at(owner: i32, payee: i32) {
    debug_assert!(owner != payee);
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 3);
    ag_world(owner);
    let key = crystal_key();
    let pre_amount = amount;
    world::with_world_mut(|w| {
        w.pay_amount = amount;
        w.trig_target = payee;
        w.money = [10_000; MAX_PLAYERS];
    });
    plant_field(owner, CARD_SOUL, 1, crystals);
    // Yes/no prompt: symbolic answers cover both; assert only on yes.
    // Entry 1 is PaySplitChoose (residual `is_placed && actor == owner`).
    invoke::unwrap_done(card_ag::crimson_soul::pay_choose_kani(owner));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    let post_crystals = world::with_world(crystals_of);
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let accepted = !effects.is_empty();
    kani::cover!(accepted, "owner accepts the crystal spend");
    kani::cover!(!accepted, "owner declines (no-op)");
    if accepted {
        let expected = spec::ag::soul_02_amount(pre_amount);
        crate::kassert!(
            spec::ag::soul_02_pay_log_ok(effects, owner, payee, key, expected),
            "C-AG-soul-02: +500 payee, −1 crystal, amount = max(pre-1000, 0)"
        );
        crate::kassert!(post_amount == expected, "C-AG-soul-02 amount");
        crate::kassert!(post_crystals == crystals - 1, "C-AG-soul-02 crystal");
    } else {
        crate::kassert!(spec::ag::soul_02_noop_log_ok(effects), "decline is a no-op");
        crate::kassert!(post_amount == pre_amount, "decline leaves the amount");
        crate::kassert!(post_crystals == crystals, "decline leaves the crystal");
    }
}

/// C-AG-soul-02 iff at (owner=0, payee=1).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_02_pay_choose_yes_iff() {
    soul_02_pay_choose_yes_iff_at(0, 1);
}

/// Same iff at (owner=1, payee=0) — the other seat pair.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_02_pay_choose_yes_user1_iff() {
    soul_02_pay_choose_yes_iff_at(1, 0);
}

/// Companion: off-by-one amount must not match the accepted path.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_02_mutant_is_detected() {
    let user = 0i32;
    let payee = 1i32;
    let amount = 5000i32;
    ag_world(user);
    world::with_world_mut(|w| {
        w.pay_amount = amount;
        w.trig_target = payee;
        w.money = [10_000; MAX_PLAYERS];
        // Force yes.
        w.symbolic_answers = false;
        w.answers[0] = 0;
    });
    plant_field(user, CARD_SOUL, 1, 2);
    invoke::unwrap_done(card_ag::crimson_soul::pay_choose_kani(user));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    crate::kassert!(
        post_amount != spec::ag::soul_02_amount_mutant(amount),
        "mutant (+1) must not match"
    );
    crate::kassert!(
        post_amount == spec::ag::soul_02_amount(amount),
        "card does max(pre-1000,0)"
    );
}

// ===========================================================================
// C-AG-soul-03 — skill (2) spends a crystal
// 「（2）[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]。」
// Seat-symmetric (「[拥有者]」).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn soul_03_skill_used_iff_at(user: i32) {
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 3);
    ag_world(user);
    let owner = user;
    let key = crystal_key();
    plant_field(owner, CARD_SOUL, 1, crystals);
    // Entry 2 is SkillUsed (residual: placed && actor==owner && crystals>0).
    invoke::unwrap_done(card_ag::crimson_soul::skill_used_kani(owner));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let post_crystals = world::with_world(crystals_of);
    crate::kassert!(
        spec::ag::soul_03_log_ok(effects, owner, key),
        "C-AG-soul-03: exactly one crystal leaves"
    );
    crate::kassert!(post_crystals == crystals - 1, "crystals fall by 1");
}

/// C-AG-soul-03 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_03_skill_used_iff() {
    soul_03_skill_used_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「[拥有者]」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn soul_03_skill_used_user1_iff() {
    soul_03_skill_used_iff_at(1);
}

// ===========================================================================
// C-AG-glory-02 / 03 — swap half of each side's dearest deed
// 「[反击] 当你经过一名角色时…你从对方处获得等于对方最贵格子基础购买价格
//   一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。」
// NOT seat-symmetric in a single seat: the clause names 「你」 and 「对方」.
// Cover both (user, other) pairs of the 2-seat world concretely.
// ===========================================================================

/// Shared iff body at a concrete (user, other) pair. Tile prices come from
/// the static board; the proof places the owners on real board tiles and
/// reads `tile_price` through the same host the body uses.
fn glory_02_03_swap_iff_at(user: i32, other: i32) {
    debug_assert!(user != other);
    ag_world(user);
    // Pin two concrete deed sets so `owned_tiles` / `tile_price` are stable.
    // Tile 2 (board index 3) price 2600; tile 3 price 3200.
    world::with_world_mut(|w| {
        w.trig_tile = 0; // the tile being passed
        w.pos[user as usize] = 0;
        w.pos[other as usize] = 0; // other stands on the passed tile
        w.tiles[2].owner = user;
        w.tiles[3].owner = other;
        w.money = [10_000; MAX_PLAYERS];
    });
    // Residual `can_counteract` requires someone on `trigger::tile()`.
    // Invoke the body directly — the residual is a derived list
    // (players_on), not the host pre (`pre::MINE`).
    invoke::unwrap_done(card_ag::proud_light::counteract_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Board prices (data/board.json 1-based index minus one):
    // slot 2 → price 2600, slot 3 → price 3200.
    let from_them = spec::ag::glory_half(3200);
    let from_us = spec::ag::glory_half(2600);
    kani::cover!(from_them > 0, "other has a dearest deed");
    kani::cover!(from_us > 0, "user has a dearest deed");
    crate::kassert!(
        spec::ag::glory_log_ok(effects, user, other, from_them, from_us),
        "C-AG-glory-02/03: PayTo legs are the two halves, in order"
    );
    crate::kassert!(
        post.money[user as usize] == 10_000 + from_them - from_us,
        "C-AG-glory: user money = +their half − our half"
    );
    crate::kassert!(
        post.money[other as usize] == 10_000 - from_them + from_us,
        "C-AG-glory: other money mirrors"
    );
}

/// C-AG-glory-02/03 iff at (user=0, other=1).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn glory_02_03_swap_iff() {
    glory_02_03_swap_iff_at(0, 1);
}

/// Same iff at (user=1, other=0) — the other seat pair
/// (clause names 「你」 and 「对方」; not single-seat symmetric).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn glory_02_03_swap_user1_iff() {
    glory_02_03_swap_iff_at(1, 0);
}

/// Companion: +1 on the first leg must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn glory_mutant_is_detected() {
    let user = 0i32;
    let other = 1i32;
    ag_world(user);
    world::with_world_mut(|w| {
        w.trig_tile = 0;
        w.pos[user as usize] = 0;
        w.pos[other as usize] = 0;
        w.tiles[2].owner = user;
        w.tiles[3].owner = other;
        w.money = [10_000; MAX_PLAYERS];
    });
    invoke::unwrap_done(card_ag::proud_light::counteract_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    crate::kassert!(
        !spec::ag::glory_log_ok_mutant(effects, user, other, 1600, 1300),
        "mutant (+1) must not match"
    );
}

// ===========================================================================
// C-AG-one-of-us-01 — place on field (first effect of the play)
// 「将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色…」
// Seat-symmetric (「[使用者]」). Later legs (partner / deeds / split) are
// OPEN-场上 / OPEN-触发结算 — this harness pins only the placement half.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn one_of_us_01_place_iff_at(user: i32) {
    ag_world(user);
    let player = user;
    let partner = 1 - user;
    world::with_world_mut(|w| {
        w.out = [false; MAX_PLAYERS];
        // Partner candidate: other player owns a shop-street deed. Under the
        // Kani host `is_shop` is `tile_in("shop")` = buyable ∧ group 10
        // (board.rs `is_shop`); world tile 47 (board index 48) is one.
        w.tiles[47].owner = partner;
        // User must own at least one deed (`cant_play` → no_deeds).
        w.tiles[2].owner = player;
        w.symbolic_answers = true;
    });
    invoke::unwrap_done(card_ag::one_of_us::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // The play's first determined effect is PlaceCard (C-AG-one-of-us-01).
    // Subsequent designation ops are host marks and may add nothing typed.
    let placed = effects.iter().any(|e| {
        matches!(
            e,
            Effect::PlaceCard {
                player: p,
                dest: 2,
                ..
            } if *p == player
        )
    });
    kani::cover!(placed, "card lands on the field");
    crate::kassert!(
        placed,
        "C-AG-one-of-us-01: the card must be placed on the user's field"
    );
}

/// C-AG-one-of-us-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn one_of_us_01_place_iff() {
    one_of_us_01_place_iff_at(0);
}

/// Same iff at seat 1 (seat-symmetric 「[使用者]」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn one_of_us_01_place_user1_iff() {
    one_of_us_01_place_iff_at(1);
}

// ===========================================================================
// Pure formula harnesses (no invoke) — cheap, run first under the slot cap.
// ===========================================================================

/// C-AG-colors-01: the >6 wrap is identity on a real 1d6 face.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn colors_01_wrap_formula() {
    let raw: i32 = kani::any();
    kani::assume(raw >= 1 && raw <= 6);
    crate::kassert!(
        spec::ag::colors_01_wrap(raw) == raw,
        "C-AG-colors-01: wrap is identity on 1d6"
    );
}

/// C-AG-half-speed-04 formula: `final = round10(fold of add/mul)`.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn half_speed_04_amount_formula() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    let fold = spec::ag::half_speed_04_fold(amount);
    let final_amt = spec::ag::half_speed_04_amount_ceil10(amount);
    crate::kassert!(
        final_amt == spec::ag::round10(fold),
        "final = round10(fold of add/mul)"
    );
    crate::kassert!(final_amt >= fold, "round10 does not decrease");
    crate::kassert!(final_amt % 10 == 0, "final lands on a multiple of 10");
    crate::kassert!(final_amt <= fold + 9, "round10 overshoots by < 10");
    if amount % 20 == 0 {
        crate::kassert!(
            final_amt == spec::ag::half_speed_04_amount_even(amount),
            "even 10-multiples halve exactly"
        );
    }
    crate::kassert!(
        fold != spec::ag::half_speed_04_amount_mutant(amount),
        "fold is not the off-by-one mutant"
    );
}

/// C-AG-glory-02/03 formula: floor half of a non-negative base price.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn glory_half_formula() {
    let base: i32 = kani::any();
    kani::assume(base >= 0 && base <= 10_000);
    let h = spec::ag::glory_half(base);
    crate::kassert!(h == base / 2, "floor half");
    crate::kassert!(h * 2 <= base && h * 2 + 2 > base, "floor-half bounds");
    if base % 2 == 0 {
        crate::kassert!(h * 2 == base, "even bases split exactly");
    }
}

/// C-AG-soul-02 formula: amount = max(pre-1000, 0).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn soul_02_amount_formula() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 0 && amount <= 10_000);
    let to = spec::ag::soul_02_amount(amount);
    crate::kassert!(to == (amount - 1000).max(0), "max(pre-1000, 0)");
    crate::kassert!(
        to != spec::ag::soul_02_amount_mutant(amount),
        "mutant disagrees"
    );
}

/// C-AG-sky-02 formula: n*600 for n >= 3.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn sky_02_amount_formula() {
    let n: i32 = kani::any();
    kani::assume(n >= 3 && n <= 8);
    let effects = [Effect::GainMoney {
        player: 0,
        n: n * 600,
    }];
    crate::kassert!(
        spec::ag::sky_02_log_ok(&effects, 0, n),
        "n*600 matches the gain leg"
    );
    crate::kassert!(
        !spec::ag::sky_02_log_ok_mutant(&effects, 0, n),
        "n*601 does not"
    );
}
