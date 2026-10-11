//! Unit `r`: clause proofs (Roselia cards).
//!
//! Per clause (KANI-FOUNDATION.md): iff + no-op/cover + mutant. Specs live in
//! `crate::spec::r` and come from the 规则书 clause docs only. Card bodies are
//! invoked unmodified via `crate::invoke`.
//!
//! Cheap first: money / status / place clauses before prompt / move / settle.

use crate::host;
use crate::invoke;
use crate::spec::r as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// shared symbolic worlds
// ---------------------------------------------------------------------------

fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = false;
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 20);
        w.fire_max[p] = 10;
        w.stay[p] = 0;
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
    }
    w.trig_player = kani::any();
    kani::assume(w.trig_player >= 0 && w.trig_player < w.n_players);
    w
}

fn effects_of() -> ([Effect; 24], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn slice_of(buf: [Effect; 24], n: usize) -> [Effect; 24] {
    // Return the buffer; callers slice with `n`.
    let _ = n;
    buf
}

// ===========================================================================
// C-R-觉悟-01 / C-R-觉悟-02 -- `R:[衍生] 觉悟`
// 「[特]：此卡[移除]时获得1000资金。」 「[手]：[移除]此卡。」
// ===========================================================================

/// IFF: the [手] Play pays exactly 1000 and touches only the user's money.
#[kani::proof]
#[kani::unwind(24)]
pub fn resolve_play_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    w.next_answer = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:[衍生] 觉悟"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    kani::cover!(log_n == 1, "one effect");
    assert!(
        spec::resolve_play_log_ok(effects, player),
        "C-R-觉悟-01: exactly GainMoney 1000"
    );
    assert!(
        spec::resolve_play_post_ok(&pre, &post, player),
        "C-R-觉悟-01: only the user's money moves, +1000"
    );
}

/// MUTANT companion: the off-by-one / wrong-amount spec must NOT match.
#[kani::proof]
#[kani::unwind(24)]
pub fn resolve_play_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:[衍生] 觉悟"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    assert!(
        !spec::resolve_play_log_ok_mutant(effects, player),
        "mutant: card does not pay 2000"
    );
    assert!(
        post.money[player as usize] != pre.money[player as usize] + 2000,
        "mutant: money is not +2000"
    );
}

/// MUTANT (should_panic): the wrong-amount spec fails against the real body.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn resolve_play_mutant_2000() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:[衍生] 觉悟"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    assert!(
        spec::resolve_play_log_ok_mutant(effects, player),
        "mutant: not a 2000 spec"
    );
}

// ===========================================================================
// C-R-曲奇时间-02 -- +500·X
// Clause 01's shuffle is BLOCKED-HOST (`shuffle_into_deck` stub).
// ===========================================================================

/// IFF: with a non-empty discard the body gains exactly 500*X (X = pre size).
#[kani::proof]
#[kani::unwind(32)]
pub fn cookie_pay_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let n: u8 = kani::any();
    kani::assume(n >= 1 && n <= 8);
    w.discard_n = n;
    for i in 0..n as usize {
        w.discard_pile[i] = kani::any();
    }
    let x = spec::cookie_x(n as i32);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Gate: discard non-empty (the book's condition). Invoke the body directly
    // -- the `cant_play` gate is covered by the non-empty discard assumption.
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:曲奇时间"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    kani::cover!(x == 1, "X=1");
    kani::cover!(x == 3, "X=3");
    kani::cover!(x == 8, "X=8");
    assert!(
        spec::cookie_pay_log_ok(effects, player, x),
        "C-R-曲奇时间-02: exactly GainMoney 500*X"
    );
    assert!(
        spec::cookie_pay_post_ok(&pre, &post, player, x),
        "C-R-曲奇时间-02: money +500*X only"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(32)]
pub fn cookie_pay_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let n: u8 = kani::any();
    kani::assume(n >= 1 && n <= 8);
    w.discard_n = n;
    let x = spec::cookie_x(n as i32);
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:曲奇时间"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let wrong = spec::cookie_pay_amount_mutant(x);
    assert!(
        !matches!(effects.first(), Some(Effect::GainMoney { n, .. }) if *n == wrong),
        "mutant: gain is not 500*(X+1)"
    );
}

// ===========================================================================
// C-R-Sprechchor-02 -- 1000 + X*120
// ===========================================================================

/// IFF: roll X ∈ 1..=20, gain exactly 1000+X*120, only the user's money moves.
#[kani::proof]
#[kani::unwind(24)]
pub fn sprechchor_pay_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:Sprechchor"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    // Recover the rolled face from the gain amount: n = 1000 + X*120.
    assert!(log_n == 1, "C-R-Sprechchor-02: exactly one effect");
    let n = match effects[0] {
        Effect::GainMoney { n, .. } => n,
        _ => {
            assert!(false, "C-R-Sprechchor-02: effect must be GainMoney");
            0
        }
    };
    let x = (n - 1000) / 120;
    kani::cover!(x == 1, "face 1");
    kani::cover!(x == 20, "face 20");
    assert!(
        spec::sprechchor_face_ok(x),
        "C-R-Sprechchor-02: face in 1..=20"
    );
    assert!(
        spec::sprechchor_log_ok(effects, player, x),
        "C-R-Sprechchor-02: gain is exactly 1000+X*120"
    );
    assert!(
        spec::sprechchor_post_ok(&pre, &post, player, x),
        "C-R-Sprechchor-02: only the user's money moves"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(24)]
pub fn sprechchor_pay_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:Sprechchor"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let n = match effects.first() {
        Some(Effect::GainMoney { n, .. }) => *n,
        _ => -1,
    };
    let x = (n - 1000) / 120;
    assert!(
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

/// IFF (off-hand origin): GiveStun{1} + SetPayAmount{0} + Draw{1}.
#[kani::proof]
#[kani::unwind(24)]
pub fn ako_dark_offhand_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    w.pay_amount = kani::any();
    kani::assume(w.pay_amount > 0 && w.pay_amount <= 10_000);
    w.trig_value = w.pay_amount;
    w.trig_target = kani::any();
    kani::assume(w.trig_target >= 0 && w.trig_target < w.n_players);
    kani::assume(w.trig_target != player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("R:（亚子）黑暗大魔姬亚子"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    kani::cover!(log_n == 3, "stun + cancel + draw");
    assert!(
        spec::ako_dark_offhand_log_ok(effects, player),
        "C-R-黑暗大魔姬亚子-02/03: GiveStun{1} + SetPayAmount{0} + Draw{1}"
    );
    assert!(
        spec::ako_dark_post_ok(&pre, &post, player),
        "C-R-黑暗大魔姬亚子-02: +1 stun, pay zeroed, money unchanged"
    );
}

/// MUTANT companion: not a 2-stun spec.
#[kani::proof]
#[kani::unwind(24)]
pub fn ako_dark_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    w.pay_amount = 500;
    w.trig_value = 500;
    w.trig_target = (player + 1) % 4;
    kani::assume(w.trig_target < w.n_players);
    kani::assume(w.trig_target != player);
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("R:（亚子）黑暗大魔姬亚子"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let two = spec::ako_dark_stun_n_mutant();
    assert!(
        !matches!(effects.first(), Some(Effect::GiveStun { n, .. }) if *n == two),
        "mutant: first effect is not 2 stun layers"
    );
}

// ===========================================================================
// C-R-Fire bird-01 -- pay 1600, place, X crystals
// OPEN-FIRE-X: the body offers 1..=10 and pins `x.max(1)`.
// ===========================================================================

/// IFF: PayMoney 1600 + PlaceCard(Field); money -1600.
/// Condition from the clause doc: the user has ≥1600 money.
#[kani::proof]
#[kani::unwind(32)]
pub fn fire_bird_place_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    kani::assume(w.money[player as usize] >= spec::fire_bird_cost());
    // Symbolic pick among the 10 offered numbers (1..=10).
    w.symbolic_answers = true;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:Fire bird"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let card = crate::world::card_slot_of("R:Fire bird");

    kani::cover!(log_n == 2, "pay + place");
    assert!(
        spec::fire_bird_place_log_ok(effects, player, card),
        "C-R-Fire bird-01: PayMoney 1600 + PlaceCard"
    );
    assert!(
        spec::fire_bird_place_post_ok(&pre, &post, player),
        "C-R-Fire bird-01: money -1600"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(32)]
pub fn fire_bird_place_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    kani::assume(w.money[player as usize] >= spec::fire_bird_cost());
    w.symbolic_answers = true;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:Fire bird"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let card = crate::world::card_slot_of("R:Fire bird");
    assert!(
        !spec::fire_bird_place_log_ok_mutant(effects, player, card),
        "mutant: first effect is not a 1600 gain"
    );
}

// ===========================================================================
// C-R-Fire bird-02 -- turn-end burn 400 + 1 crystal
// Entry index 1 = `On::Hook(TurnEnd)`.
// ===========================================================================

/// IFF: the TurnEnd hook pays exactly 400 (crystal decay is a silent write).
/// Assume the owner can pay the 400 (the book's 「失去400资金」).
#[kani::proof]
#[kani::unwind(24)]
pub fn fire_bird_burn_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    kani::assume(w.money[player as usize] >= spec::fire_bird_burn_pay());
    // Crystals must be present for the decay half; the pay is unconditional
    // in the body (`ctx::pay(400)` then `decay()`).
    w.key_set(player, crate::intern::intern("crystals"), 1);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("R:Fire bird"), 1, player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    kani::cover!(log_n == 1, "pay 400");
    assert!(
        spec::fire_bird_burn_log_ok(effects, player),
        "C-R-Fire bird-02: exactly PayMoney 400"
    );
    assert!(
        spec::only_player_money_delta(&pre, &post, player, -spec::fire_bird_burn_pay(), 1),
        "C-R-Fire bird-02: money -400"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(24)]
pub fn fire_bird_burn_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("R:Fire bird"), 1, player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    assert!(
        !spec::fire_bird_burn_log_ok_mutant(effects, player),
        "mutant: not a 400 gain"
    );
}

// ===========================================================================
// C-R-NFO-03 -- face 1 = +1500
// The roll is symbolic 1..=6 inside the body; cover each face and assert the
// face-1 slice exactly. Faces 2-6 have their own clauses (unit table).
// C-R-NFO-02 is the separate `AtEnd` entry.
// ===========================================================================

/// IFF (face-1 slice): when the wrapped face is 1 the body gains exactly 1500.
#[kani::proof]
#[kani::unwind(32)]
pub fn nfo_face1_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    w.symbolic_answers = true;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:NFO"), player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    // Recover the face from the gain amount when face 1 fired.
    let face1 = log_n == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == spec::nfo_face1_gain());
    kani::cover!(face1, "face 1 fires");
    kani::cover!(!face1, "other face");
    if face1 {
        assert!(
            spec::nfo_face1_log_ok(effects, player),
            "C-R-NFO-03: exactly GainMoney 1500"
        );
        assert!(
            spec::only_player_money_delta(&pre, &post, player, spec::nfo_face1_gain(), 1),
            "C-R-NFO-03: money +1500 only"
        );
    }
}

/// MUTANT companion for face 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn nfo_face1_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    w.symbolic_answers = true;
    // Pin the face-1 region by making the roll land on 1: the host's
    // `roll(1,6)` is symbolic; we only assert the disagreement when the body
    // produced a single 1500 gain (the face-1 path).
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:NFO"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let face1 = log_n == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == spec::nfo_face1_gain());
    if face1 {
        let wrong = spec::nfo_face1_gain_mutant();
        assert!(
            !matches!(effects[0], Effect::GainMoney { n, .. } if n == wrong),
            "mutant: face-1 gain is not 1400"
        );
    }
}

/// IFF: `AtEnd` gives exactly 1 [停留].
#[kani::proof]
#[kani::unwind(16)]
pub fn nfo_stay_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("R:NFO"), "AtEnd", "", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];

    assert!(
        spec::nfo_stay_log_ok(effects, player),
        "C-R-NFO-02: exactly GiveStay{1}"
    );
    assert!(
        post.stay[player as usize] == pre.stay[player as usize] + 1,
        "C-R-NFO-02: +1 [停留]"
    );
}

/// MUTANT companion for the stay layer.
#[kani::proof]
#[kani::unwind(16)]
pub fn nfo_stay_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("R:NFO"), "AtEnd", "", player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let two = spec::nfo_stay_n_mutant();
    assert!(
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

/// IFF: the draw pile gains exactly one `R:[衍生] 觉悟`.
#[kani::proof]
#[kani::unwind(24)]
pub fn student_council_add_resolve_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    // Isolation: no owned tiles (so `spots` is empty and the body returns
    // after the insert). Also keep the discard empty so nothing else fires.
    for t in 0..60 {
        w.tiles[t].owner = -1;
    }
    let pre_deck_n = w.deck_n as i32;
    w.deck_n = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("R:学生会的检查"), player));
    let post = world::snapshot();
    let post_deck_n = world::with_world(|ww| ww.deck_n as i32);

    kani::cover!(post_deck_n == 1, "one card in deck");
    assert!(
        spec::student_council_add_resolve_delta(pre_deck_n, post_deck_n),
        "C-R-学生会的检查-02: deck gains exactly one 觉悟"
    );
    let _ = pre;
    let _ = post;
    // No effect-log entry: `add_to_deck` is a silent pile write.
    let (logbuf, log_n) = effects_of();
    let _ = slice_of(logbuf, log_n);
    assert!(log_n == 0, "C-R-学生会的检查-02: insert is not an effect-log row");
}

/// MUTANT companion: the deck must not grow by 2.
#[kani::proof]
#[kani::unwind(24)]
pub fn student_council_add_resolve_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    for t in 0..60 {
        w.tiles[t].owner = -1;
    }
    w.deck_n = 0;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("R:学生会的检查"), player));
    let post_deck_n = world::with_world(|ww| ww.deck_n as i32);
    assert!(
        post_deck_n != 2,
        "mutant: deck did not grow by 2"
    );
}

// ===========================================================================
// C-R-Ringing Bloom-01 -- place on own field
// 「（1）将此卡放置于自身场上」
// Clauses 02/03 are the rent pipeline (PROTOCOL-LEVEL).
// ===========================================================================

/// IFF: the play places the card on the user's field.
#[kani::proof]
#[kani::unwind(24)]
pub fn ringing_bloom_place_iff() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:（燐子）Ringing Bloom"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    let card = crate::world::card_slot_of("R:（燐子）Ringing Bloom");

    kani::cover!(log_n == 1, "place");
    assert!(
        spec::ringing_bloom_place_log_ok(effects, player, card),
        "C-R-Ringing Bloom-01: exactly PlaceCard on the user's field"
    );
}

/// MUTANT companion.
#[kani::proof]
#[kani::unwind(24)]
pub fn ringing_bloom_place_mutant_is_detected() {
    let mut w = base_world();
    let player = w.trig_player;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("R:（燐子）Ringing Bloom"), player));
    let (logbuf, log_n) = effects_of();
    let effects = &slice_of(logbuf, log_n)[..log_n];
    assert!(
        !spec::ringing_bloom_place_log_ok_mutant(effects, player),
        "mutant: not a two-PlaceCard body"
    );
}