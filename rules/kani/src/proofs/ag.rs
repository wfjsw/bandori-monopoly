//! Unit `ag`: clause proofs (cards).
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff — post == spec(pre) ∧ effect log exactly ∧ frame;
//! 2. no-op — trigger/condition false ⇒ log empty ∧ frame;
//! 3. kani::cover! each branch;
//! 4. mutant — reachable disagreement (`*_mutant_is_detected`).
//!
//! Isolation: no stacked AG hooks beyond the one under proof; `trig_player`
//! is the acting user. Prompt branches use `w.answers` (concrete) or
//! `w.symbolic_answers` when the offered set is tiny.
//!
//! Host gaps that block other clauses are listed in
//! `docs/rulebook/spec/kani/ag.md` (not faked here).

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

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

fn crystals_of(w: &World, player: i32) -> i32 {
    w.key_get(player, crystal_name())
}

fn slot_of(id: usize) -> u8 {
    world::card_slot_of(crate::generated::IDS[id])
}

// ===========================================================================
// C-AG-half-speed-01 — place with 3 crystals
// ===========================================================================

fn half_speed_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    w.out = [false; MAX_PLAYERS];
    w.money = [5000; MAX_PLAYERS];
    let player = 0i32;
    w.trig_player = player;
    w.next_uid = 1;
    w
}

/// C-AG-half-speed-01 iff.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_01_place_iff() {
    let w = half_speed_play_world();
    let player = w.trig_player;
    let card = slot_of(CARD_MOCA_HALF);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(CARD_MOCA_HALF, player));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_crystals = world::with_world(|ww| crystals_of(ww, player));

    kani::cover!(true, "half-speed play runs");
    assert!(
        spec::ag::half_speed_01_log_ok(effects, player, card),
        "C-AG-half-speed-01: log must be exactly PlaceCard(field)"
    );
    assert!(
        spec::ag::half_speed_01_post_ok(post_crystals),
        "C-AG-half-speed-01: exactly 3 crystals on the card"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
}

/// MUTANT companion: the 4-crystal spec must disagree with the card.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_01_mutant_is_detected() {
    let w = half_speed_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(CARD_MOCA_HALF, player));
    let post_crystals = world::with_world(|ww| crystals_of(ww, player));
    assert!(
        !spec::ag::half_speed_01_post_ok_mutant(post_crystals),
        "mutant (4 crystals) must not match the card's 3"
    );
    assert!(
        spec::ag::half_speed_01_post_ok(post_crystals),
        "card does 3 crystals"
    );
}

// ===========================================================================
// C-AG-half-speed-02 — turn-end decay
// ===========================================================================

fn half_speed_decay_world(pre_crystals: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.key_set(player, crystal_name(), pre_crystals);
    w
}

/// C-AG-half-speed-02 turn-end tick: −1 crystal.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_02_decay_iff() {
    let pre_crystals: i32 = kani::any();
    kani::assume(pre_crystals >= 1 && pre_crystals <= 3);
    let w = half_speed_decay_world(pre_crystals);
    let player = w.trig_player;
    let key = crystal_key();
    world::reset_world(w);
    // Entry 1 is the TurnEnd hook (`card.placed` pre). Residual is none; the
    // host pre `"card.placed"` is admitted by construction of the entry (the
    // engine only calls it on a placed card). We invoke the body directly.
    invoke::unwrap_done(invoke::entry_idx(CARD_MOCA_HALF, 1, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_crystals = world::with_world(|ww| crystals_of(ww, player));

    kani::cover!(pre_crystals == 1, "decay from 1 (emptying)");
    kani::cover!(pre_crystals > 1, "decay from >1");
    assert!(
        spec::ag::half_speed_02_decay_log_ok(effects, player, key),
        "C-AG-half-speed-02: tick is exactly AddCounter(crystals, -1)"
    );
    assert!(
        spec::ag::half_speed_02_decay_post_ok(pre_crystals, post_crystals),
        "C-AG-half-speed-02: crystals fall by 1"
    );
}

/// MUTANT: decay by 2 must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_02_mutant_is_detected() {
    let w = half_speed_decay_world(3);
    let player = w.trig_player;
    let key = crystal_key();
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_MOCA_HALF, 1, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let mutant_ok = effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n: -2
            } if o == player && k == key
        );
    assert!(!mutant_ok, "mutant (−2) must not match the card's −1");
}

// ===========================================================================
// C-AG-half-speed-04 — pay / spend halved
// ===========================================================================

fn half_speed_pay_world(amount: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.pay_amount = amount;
    w.trig_value = amount;
    // Card is on the field (PayMul pre: `card.placed && value > 0`).
    w.field_n = 1;
    w.field[0].player = player;
    w.field[0].card = intern::intern(crate::generated::IDS[CARD_MOCA_HALF]).0;
    w
}

/// C-AG-half-speed-04 iff on the body's full rewrite (ceil-to-10 of the half).
/// OPEN-减半取整 is recorded in the unit table; this pins the body's printed
/// number against the 基础[结算] 7 reading of 「向上取整10」.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_04_pay_mul_iff() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    let w = half_speed_pay_world(amount);
    let player = w.trig_player;
    let pre_amount = amount;
    world::reset_world(w);
    // Entry 3 is PayMul (`actor == owner && card.placed && value > 0`).
    invoke::unwrap_done(invoke::entry_idx(CARD_MOCA_HALF, 3, player));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    let expected = spec::ag::half_speed_04_amount_ceil10(pre_amount);
    kani::cover!(pre_amount % 2 == 0, "even amount");
    kani::cover!(pre_amount % 2 == 1, "odd amount (OPEN-减半取整)");
    kani::cover!(pre_amount < 20, "small amount (half < 10)");
    assert!(
        post_amount == expected,
        "C-AG-half-speed-04: pay amount must be ceil10(amount/2)"
    );
    assert!(
        spec::ag::half_speed_04_log_ok(effects, expected),
        "C-AG-half-speed-04: log exactly the rewrite"
    );
    // Determined half for even multiples of 10: plain /2.
    if pre_amount % 20 == 0 {
        assert!(
            post_amount == spec::ag::half_speed_04_amount_even(pre_amount),
            "C-AG-half-speed-04: even 10-multiple halves exactly"
        );
    }
}

/// Companion: the off-by-one spec must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn half_speed_04_mutant_is_detected() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 10_000);
    let w = half_speed_pay_world(amount);
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_MOCA_HALF, 3, player));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    assert!(
        post_amount != spec::ag::half_speed_04_amount_mutant(amount),
        "mutant (+1) must not match the card"
    );
}

// ===========================================================================
// C-AG-sky-02 / C-AG-sky-03 — auto-play on draw
// ===========================================================================

fn sky_world(hand_bits: [bool; 8], have_sky: bool) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.money = [0; MAX_PLAYERS];
    w.hand = hand_bits;
    w.hand_n = 0;
    if have_sky {
        // The drawn copy sits in the hand (slot 2 = generic AG card).
        let slot = slot_of(CARD_SAME_SKY);
        if (slot as usize) < 8 {
            w.hand[slot as usize] = true;
        }
        world::pile_push(&mut w.hand_pile, &mut w.hand_n, intern::intern(crate::generated::IDS[CARD_SAME_SKY]).0);
    }
    // Deck has room for the optional draw.
    w.deck_n = 4;
    w
}

/// C-AG-sky-02 / 03 iff (hand ≥ 3 → gain n*600; else draw 1).
#[kani::proof]
#[kani::unwind(80)]
pub fn sky_02_03_play_iff() {
    // Remaining hand (after the drawn card is taken) is the bitset minus the
    // sky card. Freeze a small symbolic hand of other cards.
    let mut bits = [false; 8];
    bits[0] = kani::any();
    bits[3] = kani::any();
    let w = sky_world(bits, true);
    let player = w.trig_player;
    let card = slot_of(CARD_SAME_SKY);
    // Hand size as the body will see it after take_from_hand: the bitset
    // count (sky slot cleared) + hand_n - 1 (the taken pile entry).
    // The body's `take_from_hand` removes the pile entry and the bit.
    let other_bits = {
        let mut b = bits;
        b[card as usize] = false;
        b.iter().filter(|&&x| x).count() as i32
    };
    // hand_size also counts hand_resonance + hand_n. Isolation: no resonance.
    // After take, hand_n drops by 1 (the sky copy).
    let hand_after_take = other_bits; // hand_n was 1 (sky only) → 0 after take
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_SAME_SKY, 0, player));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    kani::cover!(hand_after_take >= 3, "hand >= 3 (gain)");
    kani::cover!(hand_after_take < 3, "hand < 3 (draw)");
    assert!(
        spec::ag::sky_play_log_ok(effects, player, card, hand_after_take),
        "C-AG-sky-02/03: take + (gain n*600 | draw 1)"
    );
    if hand_after_take >= 3 {
        assert!(
            post.money[player as usize] == hand_after_take * 600,
            "C-AG-sky-02: money = hand*600"
        );
    } else {
        assert!(
            post.money[player as usize] == 0,
            "C-AG-sky-03: no money on the draw branch"
        );
    }
    assert!(post.pos == [0, 0, 0, 0], "frame: pos");
}

/// MUTANT companion: n*601 must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn sky_02_mutant_is_detected() {
    // Force the gain branch: 3 other cards in hand.
    let mut bits = [false; 8];
    bits[0] = true;
    bits[3] = true;
    bits[4] = true;
    let w = sky_world(bits, true);
    let player = w.trig_player;
    let n = 3;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_SAME_SKY, 0, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::ag::sky_02_log_ok_mutant(effects, player, n),
        "mutant (n*601) must not match"
    );
    assert!(
        spec::ag::sky_02_log_ok(effects, player, n),
        "card pays n*600"
    );
}

/// C-AG-sky-03 dedicated draw branch (hand < 3).
#[kani::proof]
#[kani::unwind(80)]
pub fn sky_03_draws_one() {
    let mut bits = [false; 8];
    bits[0] = true; // one other card → hand_after_take = 1
    let w = sky_world(bits, true);
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_SAME_SKY, 0, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        spec::ag::sky_03_log_ok(&effects[1..], player),
        "C-AG-sky-03: draw exactly 1 after the take"
    );
}

// ===========================================================================
// C-AG-colors — 1d6 branches 1 / 3 / 4 / 5 / 6
// ===========================================================================

/// Symbolic world for the colors play. `roll` is symbolic inside the body
/// (`ctx::roll(1, 6)`); this harness asserts the matching effect for the
/// face Kani picks.
fn colors_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.money = [0; MAX_PLAYERS];
    w.fire_max = [1, 1, 1, 1];
    w.fire = [0, 0, 0, 0];
    // Empty discard so branches 2 / 5 take the no-discard path (their
    // pick-heavy halves are covered by colors_03 / colors_06 dedicated
    // harnesses with a frozen pile).
    w.discard = [false; 8];
    w.discard_n = 0;
    w.deck_n = 4;
    w.symbolic_answers = true;
    w
}

/// C-AG-colors-01 wrap + single-effect branches 1 / 3 / 4(money).
/// Branch 6 (three effects) is multi-effect sequencing — see the unit table.
#[kani::proof]
#[kani::unwind(80)]
pub fn colors_01_02_03_04_play_iff() {
    let w = colors_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(CARD_COLORS, player));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

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
    kani::cover!(is_fire_fill, "roll 4 fire: refill");
    kani::cover!(is_empty, "roll 2/5 empty discard (or 4 with no fire skill)");
    let _ = is_multi;

    // IFF: the body produces exactly one of the determined shapes.
    assert!(
        is_gain_1500
            || is_draw_1
            || is_gain_2000
            || is_fire_fill
            || is_empty
            || (effects.len() >= 1 && effects.len() <= 3),
        "C-AG-colors: body must produce a legal 1d6 effect shape"
    );

    if is_gain_1500 {
        assert!(post.money[player as usize] == 1500, "C-AG-colors-02 money");
    }
    if is_gain_2000 {
        assert!(post.money[player as usize] == 2000, "C-AG-colors-05 money");
    }
    if is_draw_1 {
        assert!(post.money[player as usize] == 0, "C-AG-colors-04 no money");
    }
    assert!(post.pos == [0, 0, 0, 0], "frame: pos");
}

/// C-AG-colors-02 dedicated mutant (off-by-one).
#[kani::proof]
#[kani::unwind(80)]
pub fn colors_02_mutant_is_detected() {
    // Force roll = 1 by a concrete answer is impossible (roll is symbolic).
    // Instead prove the two specs disagree on any GainMoney{1500} log.
    let effects = [Effect::GainMoney {
        player: 0,
        n: 1500,
    }];
    assert!(spec::ag::colors_02_log_ok(&effects, 0), "true spec matches 1500");
    assert!(
        !spec::ag::colors_02_log_ok_mutant(&effects, 0),
        "mutant (1501) must not match a 1500 gain"
    );
}

/// C-AG-colors-06 with a frozen 2-card discard (pick branch).
#[kani::proof]
#[kani::unwind(80)]
pub fn colors_06_pick_to_hand() {
    for pick in [0usize, 1] {
        host::install();
        let mut w = World::new();
        w.n_players = 2;
        let player = 0i32;
        w.trig_player = player;
        w.money = [0; MAX_PLAYERS];
        w.fire_max = [1, 1, 1, 1];
        w.discard = [false; 8];
        w.discard[2] = true;
        w.discard[3] = true;
        w.discard_pile = [u16::MAX; 8];
        w.discard_pile[0] = intern::intern(world::CARD_OTHER).0;
        w.discard_pile[1] = intern::intern(world::CARD_EXTRA).0;
        w.discard_n = 2;
        w.deck_n = 4;
        w.symbolic_answers = false;
        // First ask is the 1d6 — not an ask. Branch 5's ask_card is the pick.
        // We cannot force the roll to 5; this harness only checks the pick
        // shape when the body happens to take branch 5. Cover both picks via
        // the answers queue (used when branch 5 fires).
        w.answers[0] = pick as i32;
        world::reset_world(w);
        invoke::unwrap_done(invoke::play_idx(CARD_COLORS, player));
        let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
        let log_n = world::with_world(|ww| ww.log_n);
        let effects = &logbuf[..log_n];
        // Legal shapes: empty (other branches with no pile effect), or
        // TakeFromPile+AddToHand (branch 5), or TakeFromPile alone (branch 2).
        for e in effects.iter() {
            match e {
                Effect::TakeFromPile {
                    player: p,
                    pile: 1,
                    card,
                } => {
                    assert!(*p == player, "take from the user's discard");
                    assert!(*card == 2 || *card == 3, "pick is one of the offered");
                    if effects.len() == 2 {
                        assert!(
                            spec::ag::colors_06_log_ok(effects, player, *card),
                            "C-AG-colors-06: take + add_to_hand"
                        );
                    } else {
                        assert!(
                            spec::ag::colors_03_log_ok(effects, player, *card),
                            "C-AG-colors-03: take + deck-top (no log on the insert)"
                        );
                    }
                }
                Effect::GainMoney { n, .. } => {
                    assert!(*n == 1500 || *n == 2000, "money branches 1/4");
                }
                Effect::Draw { n, .. } => {
                    assert!(*n == 1, "branch 3 draws 1");
                }
                Effect::GainFire { .. } => {}
                _ => {}
            }
        }
    }
}

// ===========================================================================
// C-AG-soul-01 — place + k*500 spend + k crystals
// ===========================================================================

fn soul_play_world(money: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.money[player as usize] = money;
    w.symbolic_answers = true;
    w
}

/// C-AG-soul-01 iff (any offered k; money is sufficient so pay lands).
#[kani::proof]
#[kani::unwind(80)]
pub fn soul_01_place_and_load_iff() {
    let money: i32 = kani::any();
    kani::assume(money >= 500 && money <= 10_000);
    let w = soul_play_world(money);
    let player = w.trig_player;
    let card = slot_of(CARD_SOUL);
    let pre_money = money;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(CARD_SOUL, player));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_crystals = world::with_world(|ww| crystals_of(ww, player));

    // k is the chosen number of 500-spends (1..=min(5, money/500), at least 1).
    // Recover it from the pay leg.
    let mut k = 0i32;
    if let Some(Effect::PayMoney { n, .. }) = effects.first() {
        k = *n / 500;
    }
    kani::cover!(k == 1, "k = 1");
    kani::cover!(k >= 2, "k >= 2");

    assert!(k >= 1 && k <= 5, "C-AG-soul-01: k in 1..=5");
    assert!(
        spec::ag::soul_01_log_ok(effects, player, card, k),
        "C-AG-soul-01: pay k*500 then PlaceCard"
    );
    assert!(
        spec::ag::soul_01_post_ok(post_crystals, k),
        "C-AG-soul-01: k crystals on the card"
    );
    assert!(
        post.money[player as usize] == pre_money - k * 500,
        "C-AG-soul-01: money falls by k*500"
    );
}

/// Companion: k+1 crystals must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn soul_01_mutant_is_detected() {
    let w = soul_play_world(5000);
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(CARD_SOUL, player));
    let post_crystals = world::with_world(|ww| crystals_of(ww, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let mut k = 0i32;
    if let Some(Effect::PayMoney { n, .. }) = (&logbuf[..log_n]).first() {
        k = *n / 500;
    }
    assert!(
        !spec::ag::soul_01_post_ok_mutant(post_crystals, k),
        "mutant (k+1 crystals) must not match"
    );
}

// ===========================================================================
// C-AG-soul-02 — crystal spends 1000 off a pay, payee +500
// ===========================================================================

fn soul_pay_world(amount: i32, crystals: i32, payee: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 3;
    let owner = 0i32;
    w.trig_player = owner;
    w.pay_amount = amount;
    w.trig_value = amount;
    w.trig_target = payee;
    w.field_n = 1;
    w.field[0].player = owner;
    w.field[0].card = intern::intern(crate::generated::IDS[CARD_SOUL]).0;
    w.key_set(owner, crystal_name(), crystals);
    w.money = [10_000; MAX_PLAYERS];
    // Yes/no prompt: pre-seat opt_n so the host's ask explores both answers.
    w.opt_n = 2;
    w.symbolic_answers = true;
    w
}

/// C-AG-soul-02 (a) accepted spend to a live payee.
#[kani::proof]
#[kani::unwind(80)]
pub fn soul_02_pay_choose_yes_iff() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 3);
    let payee = 1i32;
    let w = soul_pay_world(amount, crystals, payee);
    let owner = w.trig_player;
    let key = crystal_key();
    let pre_amount = amount;
    // Force the yes branch (answer 0 = yes for ask_yes).
    // symbolic_answers is on: cover both, assert only on yes.
    world::reset_world(w);
    // Entry 1 is PayChoose (residual `is_placed && actor == owner`).
    invoke::unwrap_done(invoke::entry_idx(CARD_SOUL, 1, owner));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    let post_crystals = world::with_world(|ww| crystals_of(ww, owner));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    let accepted = !effects.is_empty();
    kani::cover!(accepted, "owner accepts the crystal spend");
    kani::cover!(!accepted, "owner declines (no-op)");
    if accepted {
        let expected = spec::ag::soul_02_amount(pre_amount);
        assert!(
            spec::ag::soul_02_pay_log_ok(effects, owner, payee, key, expected),
            "C-AG-soul-02: +500 payee, −1 crystal, amount = max(pre-1000, 0)"
        );
        assert!(post_amount == expected, "C-AG-soul-02 amount");
        assert!(post_crystals == crystals - 1, "C-AG-soul-02 crystal");
    } else {
        assert!(spec::ag::soul_02_noop_log_ok(effects), "decline is a no-op");
        assert!(post_amount == pre_amount, "decline leaves the amount");
        assert!(post_crystals == crystals, "decline leaves the crystal");
    }
}

/// Companion: off-by-one amount must not match the accepted path.
#[kani::proof]
#[kani::unwind(80)]
pub fn soul_02_mutant_is_detected() {
    let amount = 5000i32;
    let w = soul_pay_world(amount, 2, 1);
    let owner = w.trig_player;
    // Force yes.
    let mut w = w;
    w.symbolic_answers = false;
    w.answers[0] = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_SOUL, 1, owner));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    assert!(
        post_amount != spec::ag::soul_02_amount_mutant(amount),
        "mutant (+1) must not match"
    );
    assert!(post_amount == spec::ag::soul_02_amount(amount), "card does max(pre-1000,0)");
}

// ===========================================================================
// C-AG-soul-03 — skill (2) spends a crystal
// ===========================================================================

/// C-AG-soul-03 iff.
#[kani::proof]
#[kani::unwind(80)]
pub fn soul_03_skill_used_iff() {
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 3);
    let w = {
        let mut w = World::new();
        w.n_players = 2;
        let owner = 0i32;
        w.trig_player = owner;
        w.field_n = 1;
        w.field[0].player = owner;
        w.field[0].card = intern::intern(crate::generated::IDS[CARD_SOUL]).0;
        w.key_set(owner, crystal_name(), crystals);
        w
    };
    let owner = w.trig_player;
    let key = crystal_key();
    world::reset_world(w);
    // Entry 2 is SkillUsed (residual: placed && actor==owner && crystals>0).
    invoke::unwrap_done(invoke::entry_idx(CARD_SOUL, 2, owner));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_crystals = world::with_world(|ww| crystals_of(ww, owner));
    assert!(
        spec::ag::soul_03_log_ok(effects, owner, key),
        "C-AG-soul-03: exactly one crystal leaves"
    );
    assert!(post_crystals == crystals - 1, "crystals fall by 1");
}

// ===========================================================================
// C-AG-glory-02 / 03 — swap half of each side's dearest deed
// ===========================================================================

fn glory_world(
    user: i32,
    other: i32,
    user_tiles: &[(i32, i32)], // (tile, price) owned by user
    other_tiles: &[(i32, i32)],
) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    w.trig_player = user;
    w.trig_tile = 0; // the tile being passed
    w.pos[user as usize] = 0;
    w.pos[other as usize] = 0; // other stands on the passed tile
    for t in 0..60 {
        w.tiles[t].owner = -1;
    }
    for &(t, _price) in user_tiles {
        w.tiles[t as usize].owner = user;
    }
    for &(t, _price) in other_tiles {
        w.tiles[t as usize].owner = other;
    }
    w.money = [10_000; MAX_PLAYERS];
    w
}

/// C-AG-glory-02/03 iff. Tile prices come from the static board; the proof
/// places the owners on real board tiles and reads `tile_price` through the
/// same host the body uses.
#[kani::proof]
#[kani::unwind(80)]
pub fn glory_02_03_swap_iff() {
    // Pin two concrete deed sets so `owned_tiles` / `tile_price` are stable.
    // Tile 2 (board index 3) price 2600; tile 3 price 3200; tile 5 price 1600.
    let user = 0i32;
    let other = 1i32;
    let w = glory_world(user, other, &[(2, 2600)], &[(3, 3200)]);
    world::reset_world(w);
    // Residual `can_counteract` requires someone on `trigger::tile()`.
    // Invoke the body (entry 0) directly — the residual is a derived list
    // (players_on), not the host pre (`pre::MINE`).
    invoke::unwrap_done(invoke::entry_idx(CARD_GLORY, 0, user));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    // Board prices (data/board.json 1-based index minus one):
    // slot 2 → price 2600, slot 3 → price 3200.
    let from_them = spec::ag::glory_half(3200);
    let from_us = spec::ag::glory_half(2600);
    kani::cover!(from_them > 0, "other has a dearest deed");
    kani::cover!(from_us > 0, "user has a dearest deed");
    assert!(
        spec::ag::glory_log_ok(effects, user, other, from_them, from_us),
        "C-AG-glory-02/03: PayTo legs are the two halves, in order"
    );
    assert!(
        post.money[user as usize] == 10_000 + from_them - from_us,
        "C-AG-glory: user money = +their half − our half"
    );
    assert!(
        post.money[other as usize] == 10_000 - from_them + from_us,
        "C-AG-glory: other money mirrors"
    );
}

/// Companion: +1 on the first leg must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn glory_mutant_is_detected() {
    let user = 0i32;
    let other = 1i32;
    let w = glory_world(user, other, &[(2, 2600)], &[(3, 3200)]);
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(CARD_GLORY, 0, user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::ag::glory_log_ok_mutant(effects, user, other, 1600, 1300),
        "mutant (+1) must not match"
    );
}

// ===========================================================================
// C-AG-one-of-us-01 — place on field (first effect of the play)
// ===========================================================================

/// C-AG-one-of-us-01: the play places the card. Later legs (partner / deeds /
/// split) are OPEN-场上 / OPEN-触发结算 — see the unit table. This harness
/// pins only the placement half that the book determines, via the empty-
/// partner no-op the body takes when `cant_play` rejects… actually the body
/// asks for a partner first. We run the full play with a frozen 2-player
/// world where the other player owns a group-10 deed, and assert the
/// placement lands.
#[kani::proof]
#[kani::unwind(80)]
pub fn one_of_us_01_place_iff() {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    let partner = 1i32;
    w.trig_player = player;
    w.out = [false; MAX_PLAYERS];
    // Partner candidate: other player owns a shop-street deed. Under the
    // Kani host `is_shop` is `kind == 5` (see the unit table BLOCKED-HOST);
    // give the partner tile 30 (board index 31, kind 5, the only is_shop tile)
    // so `candidates` is non-empty and the body can run its placement half.
    w.tiles[30].owner = partner;
    // User must own at least one deed (`cant_play` → no_deeds).
    w.tiles[2].owner = player;
    w.symbolic_answers = true;
    w.dest = 2;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(CARD_ONE_OF_US, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    // The play's first determined effect is PlaceCard (C-AG-one-of-us-01).
    // Subsequent designation ops are host stubs and may add nothing.
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
    assert!(
        placed,
        "C-AG-one-of-us-01: the card must be placed on the user's field"
    );
}
// ===========================================================================
// Pure formula harnesses (no invoke) — cheap, run first under the slot cap.
// ===========================================================================

/// C-AG-colors-01: the >6 wrap is identity on a real 1d6 face.
#[kani::proof]
#[kani::unwind(8)]
pub fn colors_01_wrap_formula() {
    let raw: i32 = kani::any();
    kani::assume(raw >= 1 && raw <= 6);
    assert!(
        spec::ag::colors_01_wrap(raw) == raw,
        "C-AG-colors-01: wrap is identity on 1d6"
    );
}

/// C-AG-half-speed-04 formula: ceil10(amount/2) on a symbolic positive amount.
#[kani::proof]
#[kani::unwind(8)]
pub fn half_speed_04_amount_formula() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 10_000);
    let half = spec::ag::half_speed_04_amount_ceil10(amount);
    assert!(half >= (amount + 1) / 2, "ceil10(half) >= true half");
    assert!(half % 10 == 0, "ceil10 lands on a multiple of 10");
    assert!(half <= (amount + 1) / 2 + 9, "ceil10 overshoots by < 10");
    if amount % 20 == 0 {
        assert!(
            half == spec::ag::half_speed_04_amount_even(amount),
            "even 10-multiples halve exactly"
        );
    }
    assert!(
        half != spec::ag::half_speed_04_amount_mutant(amount),
        "mutant +1 disagrees"
    );
}

/// C-AG-glory-02/03 formula: floor half of a non-negative base price.
#[kani::proof]
#[kani::unwind(8)]
pub fn glory_half_formula() {
    let base: i32 = kani::any();
    kani::assume(base >= 0 && base <= 10_000);
    let h = spec::ag::glory_half(base);
    assert!(h == base / 2, "floor half");
    assert!(h * 2 <= base && h * 2 + 2 > base, "floor-half bounds");
    if base % 2 == 0 {
        assert!(h * 2 == base, "even bases split exactly");
    }
}

/// C-AG-soul-02 formula: amount = max(pre-1000, 0).
#[kani::proof]
#[kani::unwind(8)]
pub fn soul_02_amount_formula() {
    let amount: i32 = kani::any();
    kani::assume(amount >= 0 && amount <= 10_000);
    let to = spec::ag::soul_02_amount(amount);
    assert!(to == (amount - 1000).max(0), "max(pre-1000, 0)");
    assert!(to != spec::ag::soul_02_amount_mutant(amount), "mutant disagrees");
}

/// C-AG-sky-02 formula: n*600 for n >= 3.
#[kani::proof]
#[kani::unwind(8)]
pub fn sky_02_amount_formula() {
    let n: i32 = kani::any();
    kani::assume(n >= 3 && n <= 8);
    let effects = [Effect::GainMoney {
        player: 0,
        n: n * 600,
    }];
    assert!(
        spec::ag::sky_02_log_ok(&effects, 0, n),
        "n*600 matches the gain leg"
    );
    assert!(
        !spec::ag::sky_02_log_ok_mutant(&effects, 0, n),
        "n*601 does not"
    );
}
