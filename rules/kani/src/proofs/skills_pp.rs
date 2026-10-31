//! Unit `skills_pp`: Pastel✽Palettes character + band skill clause proofs.
//!
//! Spec source is `crate::spec::skills_pp` (sheet text only).
//! Isolation: no `skillBlock` token; band id is symbolic where the clause
//! needs both branches.
//!
//! Concrete-user pattern (TRIAGE-LOG `ppcost`): symbolic player index is a
//! BMC cliff. `n_players = 2`, iff harnesses are `*_iff_at(user)` called from
//! seat-0 and seat-1 proofs. Direct `*_kani` body calls. Per-body host-op
//! subset (outside → loud FAIL).

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const FANS_UP: &str = "ppFansUp";
const FANS_DOWN: &str = "ppFansDown";
const FANS_BAND: &str = "skill:Pastel✽Palettes:与偶像一起";

/// Place the Pastel band skill on `owner`'s field (fans marker owner).
fn place_band_skill(owner: i32) -> i32 {
    world::with_world_mut(|w| {
        let uid = w.next_uid;
        w.next_uid += 1;
        let fi = w.field_n as usize;
        if fi < world::MAX_FIELD {
            w.field[fi] = world::FieldCard {
                uid,
                card: crate::intern::intern(FANS_BAND).0,
                player: owner,
                tile: 0,
                face_down: false,
                immune: false,
                crystals: 0,
                props: world::NameMap::new(),
            };
            w.field_n += 1;
        }
        uid
    })
}

fn crystals_at(uid: i32) -> i32 {
    world::with_world(|w| w.field_crystals(uid))
}

// Ops the aya / pastel skill bodies reach.
crate::host_subset!(skills_pp_dispatch {
    log | effect => arm_log,
    gain | gain_typed | gain_fixed => arm_gain,
    money => arm_money,
    add_held => arm_add_held,
    count_held | count_held_name => arm_count_held,
    set_held_name => arm_set_held_name,
    place_card_at => arm_place_card,
    set_dest => arm_set_dest,
    add_group_pool => arm_add_band_crystals,
    group_pool => arm_band_crystals,
    counter_self => arm_counter_self,
    add_counter_self => arm_add_counter_self,
    // `cover_crystals` / `spend_crystals` walk the field for `Sumimi:#L11`
    // (none in this world: the cover arm is not reached).
    field_instances => arm_field_instances,
    player_count => arm_player_count,
    player_out => arm_player_out,
    persona_is => arm_character_is,
    in_group => arm_in_band,
    ask => arm_ask,
    hand_count => arm_hand_count,
    take_card => arm_take_card,
    draw => arm_draw,
    add_to_dest => arm_add_to_dest,
    state_get => arm_state_get,
    state_set => arm_state_set,
    state_add => arm_inc_slot,  // same keyed-add as inc_slot (real)
    set_self_prop => arm_set_self_prop,
    placed_cards => arm_placed_cards,
    raise => arm_raise,
    gate => arm_gate,
    // FansFlip raise → pastel `on_fans_flip` (band clause （2）) via `arm_raise`.
    // Generic send/msg_* stay for any residual `ctx::send` (mark:cp / tests).
    send => arm_send,
    msg_name => arm_msg_name,
    msg_a => arm_msg_a,
    msg_b => arm_msg_b,
    msg_c => arm_msg_c,
    msg_seat => arm_msg_seat,
    msg_tile => arm_msg_tile,
    msg_reply => arm_msg_reply,
    msg_sender_uid => arm_msg_sender_uid,
    msg_sender_seat => arm_msg_sender_seat,
    msg_text => arm_msg_text,
    trig_value => arm_trig_value,
    set_steps | set_resolve | set_stop_at | schedule_turn_end | linger
    | card_offer_build | can_build_on => arm_protocol_stub,
    card_move => arm_card_move,
});

/// Sparse 2-seat world + skills_pp subset. `user` ∈ {0, 1} concrete.
fn skills_pp_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(skills_pp_dispatch);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    let up = crate::intern::intern(FANS_UP);
    let down = crate::intern::intern(FANS_DOWN);
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.symbolic_answers = true;
        // Pin owner as Pastel✽Palettes + 丸山彩 so group_is guards admit
        // (the sparse world leaves character/band at 0). Overwritten per-harness
        // when a different character is under proof.
        w.group_id[user as usize] = crate::intern::intern("Pastel✽Palettes").0;
        w.persona_id[user as usize] = crate::intern::intern("丸山彩").0;
        w.hand_resonance = [0; MAX_PLAYERS];
        for p in 0..2 {
            w.key_set(p as i32, up, w.fans_pos[p]);
            w.key_set(p as i32, down, w.fans_neg[p]);
        }
    });
    // Band skill on both used seats (fans marker owner / clause （2）).
    place_band_skill(0);
    place_band_skill(1);
}

fn effects_slice() -> ([Effect; 24], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

/// Shared iff body at a concrete seat (ppcost).
fn aya_01_five_fans_iff_at(user: i32) {
    let me = user;

    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    // Pin the owner as a Pastel✽Palettes character so the hook's guard admits.
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    w.persona_id[me as usize] = crate::intern::intern("丸山彩").0;
    let pre_pos = w.fans_pos[me as usize];
    world::reset_world(w);

    let gid = crate::guard_id!("group_is(owner, 'Pastel✽Palettes')");
    let pre_w = world::with_world(|ww| {
        // re-read a shallow view: fill_from_world takes &World
        // We already reset; build ctx from the live world via a snapshot pass.
        ()
    });
    let _ = pre_w;
    // Build the IrCtx from the live world by reconstructing the fields the
    // guard reads (owner band / character).
    let w2 = {
        let mut t = world::World::new();
        t.n_players = world::with_world(|ww| ww.n_players);
        for p in 0..MAX_PLAYERS {
            t.group_id[p] = world::with_world(|ww| ww.group_id[p]);
            t.persona_id[p] = world::with_world(|ww| ww.persona_id[p]);
            t.money[p] = world::with_world(|ww| ww.money[p]);
            t.out[p] = world::with_world(|ww| ww.out[p]);
            t.fans_pos[p] = world::with_world(|ww| ww.fans_pos[p]);
            t.fans_neg[p] = world::with_world(|ww| ww.fans_neg[p]);
        }
        t.trig_player = me;
        t
    };
    let ctx = crate::ir_fill::fill_for_guard(
        &w2,
        gid,
        me,
        0,
        false,
        -1,
        &crate::ir_fill::TriggerOverlay::default(),
    );
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (owner is PP)");
    if !admitted {
        return;
    }

    invoke::unwrap_done(skill_personas::aya::at_start_kani(me));
    let post = world::snapshot();
    crate::kassert!(
        post.fans_pos[me as usize] == spec::skills_pp::char_start_five_fans(pre_pos),
        "C-PP-aya-with-01: exactly +5 positive fans"
    );
    let (logbuf, log_n) = world::with_world(|ww| (ww.log, ww.log_n));
    let effects = &logbuf[..log_n];
    // The body may also `place_card` the granted (2) on non-PP players — that
    // is the second half of (1) and is allowed. The *fan* half is one +5.
    let fan_delta: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::AddToken { player: p, key, n }
                if *p == me && *key == crate::intern::intern(FANS_UP).0 =>
            {
                Some(*n)
            }
            _ => None,
        })
        .sum();
    crate::kassert!(
        fan_delta == 5,
        "C-PP-aya-with-01: exactly one +5 AddToken on the owner"
    );
}
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn aya_01_five_fans_iff() {
    aya_01_five_fans_iff_at(0);
}

/// Seat 1 (seat-symmetric 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn aya_01_five_fans_user1_iff() {
    aya_01_five_fans_iff_at(1);
}

/// MUTANT: +4 fans.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn aya_01_mutant_is_detected() {
    let me = 0i32;
    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let pre_pos = w.fans_pos[me as usize];
    world::reset_world(w);
    invoke::unwrap_done(skill_personas::aya::at_start_kani(me));
    let post = world::snapshot();
    crate::kassert!(
        post.fans_pos[me as usize] != spec::skills_pp::char_start_five_fans_mutant(pre_pos),
        "mutant: card must not match +4"
    );
}

// ===========================================================================
// C-PP-aya-with-02 pay discount
// 「（2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前
//  资金减少Y×100（最少0）；…」
// Isolation: the actor is a PP character (flip_others branch); no skillBlock.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost).
fn aya_02_pay_discount_iff_at(user: i32) {
    let me = user;

    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let up = w.fans_pos[me as usize];
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 5_000);
    w.trig_player = me;
    w.trig_value = pre_amount;
    w.pay_amount = pre_amount;
    world::reset_world(w);

    invoke::unwrap_done(skill_personas::aya::on_pay_kani(me));
    let post = world::snapshot();
    let y = up - post.fans_pos[me as usize]; // fans flipped reverse
    kani::cover!(y > 0, "chose some Y");
    kani::cover!(y == 0, "declined (Y = 0)");
    crate::kassert!(
        spec::skills_pp::aya_02_y_ok(y, up),
        "C-PP-aya-with-02: Y in 0..=up"
    );
    let expected = spec::skills_pp::aya_02_amount(pre_amount, y);
    crate::kassert!(
        post.pay_amount == expected,
        "C-PP-aya-with-02: pay amount = max(pre - Y*100, 0)"
    );
    crate::kassert!(
        post.fans_neg[me as usize] >= y,
        "C-PP-aya-with-02: Y positives flipped reverse"
    );
}
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_02_pay_discount_iff() {
    aya_02_pay_discount_iff_at(0);
}

/// Seat 1 (seat-symmetric 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_02_pay_discount_user1_iff() {
    aya_02_pay_discount_iff_at(1);
}

/// MUTANT: discount Y*99.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_02_mutant_is_detected() {
    let me = 0i32;
    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let up = w.fans_pos[me as usize];
    kani::assume(up >= 1);
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 200);
    w.trig_player = me;
    w.trig_value = pre_amount;
    w.pay_amount = pre_amount;
    world::reset_world(w);
    invoke::unwrap_done(skill_personas::aya::on_pay_kani(me));
    let post = world::snapshot();
    let y = up - post.fans_pos[me as usize];
    if y >= 1 {
        crate::kassert!(
            post.pay_amount != spec::skills_pp::aya_02_amount_mutant(pre_amount, y),
            "mutant: card must not use Y*99"
        );
    }
}

// ===========================================================================
// C-PP-band-idol-02 overflow formula (pure) + body via `on_fans_flip`.
// 「（2）你因任意原因受到“将X个反面[P✽P粉丝]变正”的效果且X大于拥有的反面的
//  [P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。」
//
// SPEC NOTE (loud): the previous `band_idol_02_body_never_overflows` encoded
// the OLD body (direct add_tok flip capped at `have`, so `X > down` was
// unreachable and overflow never landed — LESS than the book). The new
// `on_fans_flip` dict body **does** apply clause （2） to the effect X, so the
// harness below proves `min(X-down, 10-held)` against the unmodified body
// driven by `fans::flip_up` (FansFlip raise → `on_fans_flip_kani`).
// 「最多10个」 caps the card's TOTAL at 10 (user ruling 2026-10-17).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn band_idol_02_overflow_formula() {
    let x: i32 = kani::any();
    let down: i32 = kani::any();
    let held: i32 = kani::any();
    kani::assume(x >= 0 && x <= 30);
    kani::assume(down >= 0 && down <= 20);
    kani::assume(held >= 0 && held <= 10);
    let over = spec::skills_pp::band_idol_02_overflow(x, down, held);
    kani::cover!(x > down, "overflow applies");
    kani::cover!(x <= down, "no overflow");
    kani::cover!(x > down && held < 10, "room under the total cap");
    kani::cover!(x > down && held >= 10, "card full → adds 0");
    if x > down {
        crate::kassert!(
            over == (x - down).min(10 - held).max(0),
            "overflow added = min(X-down, 10-held)"
        );
    } else {
        crate::kassert!(over == 0, "no overflow when X <= down");
    }
    // Mutant caps the total at 11 vs the book's 10 — they disagree only when
    // x-down > 10-held. Class B: the old check asserted disagreement in the
    // agreeing range.
    let m = spec::skills_pp::band_idol_02_overflow_mutant(x, down, held);
    kani::cover!(
        x > down && (x - down) > 10 - held,
        "reachable disagreement (cap 11)"
    );
    if x > down && (x - down) > 10 - held {
        crate::kassert!(m != over, "mutant cap 11 vs 10");
    }
}

/// Non-PP `flip_all_pp` half: effect X = 1 per PP seat. When that seat holds
/// 0 face-downs and the card has room, clause （2） grants 1 overflow crystal.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn skill2_all_pp_overflow_formula() {
    let down: i32 = kani::any();
    let held: i32 = kani::any();
    kani::assume(down >= 0 && down <= 5);
    kani::assume(held >= 0 && held <= 15);
    let over = spec::skills_pp::skill2_all_pp_overflow(down, held);
    kani::cover!(
        down == 0 && held < 10,
        "X=1 > down=0, room → 1 overflow crystal"
    );
    kani::cover!(down == 0 && held >= 10, "X=1 but card full → 0");
    kani::cover!(down >= 1, "X=1 <= down → no overflow");
    if down == 0 && held < 10 {
        crate::kassert!(over == 1, "non-PP branch: 1 overflow crystal when down==0");
    } else {
        crate::kassert!(over == 0, "non-PP branch: no overflow when down>=1 or full");
    }
}

/// Body check: the `on_fans_flip` dict body applies clause （2） to the
/// **effect X** (not the clamped flip count). `X > down` grants
/// `min(X - down, 10 - pre_cry)` crystals on this instance (total capped at
/// 10); `X <= down` grants none.
///
/// Isolation: the flip target is the band-card owner (`seat == player_id`),
/// so 「你」 admits. Direction is down→up only (clause （2） is one-way).
/// Drive: unmodified `fans::flip_up` raises `FansFlip` **before** the dict
/// writes; `arm_raise` dispatches to `on_fans_flip_kani` with
/// `trigger::value() == X` while `fans::down` is still the pre-flip count.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn band_idol_02_on_fans_overflow_iff() {
    let me = 0i32;
    skills_pp_world(me);
    let pre_up = world::with_world(|w| w.fans_pos[me as usize]);
    let down = world::with_world(|w| w.fans_neg[me as usize]);
    let x: i32 = kani::any();
    kani::assume(x >= 0 && x <= 30);
    let band_uid = world::with_world(|w| {
        (0..w.field_n as usize)
            .map(|i| w.field[i])
            .find(|f| f.player == me && f.card == crate::intern::intern(FANS_BAND).0)
            .map(|f| f.uid)
            .unwrap_or(1)
    });
    let pre_cry = crystals_at(band_uid);

    // Unmodified `fans::flip_up`: FansFlip raise (→ `on_fans_flip`) then the
    // dict writes. Cancelled / aborted raise leaves both counters untouched.
    let _ = skill_groups::fans::flip_up(me, x);

    let post_cry = crystals_at(band_uid);
    let post_up = world::with_world(|w| w.fans_pos[me as usize]);
    let post_dn = world::with_world(|w| w.fans_neg[me as usize]);
    let flip = x.min(down).max(0);
    let over = spec::skills_pp::band_idol_02_overflow(x, down, pre_cry);
    kani::cover!(x > down, "overflow applies");
    kani::cover!(x <= down, "no overflow");
    crate::kassert!(
        post_cry == pre_cry + over,
        "C-PP-band-idol-02: overflow crystals = min(X-down, 10-held) when X > down"
    );
    crate::kassert!(
        post_up == pre_up + flip && post_dn == down - flip,
        "C-PP-band-idol-02: flip min(X, down) face-downs positive"
    );
}

// ===========================================================================
// C-PP-band-idol-04 crystal tick
// 「（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。」
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost).
fn band_idol_04_tick_iff_at(user: i32) {
    let me = user;

    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let pre = kani::any();
    kani::assume(pre >= 0 && pre <= 10);
    // Crystals live on the on-card `crystals` counter (`field_crystals`),
    // not the player key `"奇迹水晶"` (class B triage3).
    w.self_crystals = pre;
    w.trig_player = me;
    world::reset_world(w);
    invoke::unwrap_done(skill_groups::pastel::at_turn_start_kani(me));
    let post = world::snapshot();
    let _ = post;
    let cry = world::with_world(|ww| ww.field_crystals(ww.last_uid));
    // Automatic +1. The optional -5 / draw is a prompt; with symbolic answers
    // both branches run in this harness.
    kani::cover!(cry == pre + 1, "option declined (just +1)");
    kani::cover!(cry == pre + 1 - 5, "option taken (+1 -5)");
    crate::kassert!(
        cry == spec::skills_pp::band_idol_04_crystals_after_tick(pre)
            || cry == spec::skills_pp::band_idol_04_crystals_after_spend(pre),
        "C-PP-band-idol-04: +1, optionally -5 and draw 1"
    );
}
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_idol_04_tick_iff() {
    band_idol_04_tick_iff_at(0);
}

/// Seat 1 (seat-symmetric 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_idol_04_tick_user1_iff() {
    band_idol_04_tick_iff_at(1);
}

// ===========================================================================
// C-PP-hina-lottery-02 borrow map (pure)
// 「结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；结果为4是大和麻弥。」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn hina_02_borrow_map() {
    let roll: i32 = kani::any();
    kani::assume(spec::skills_pp::hina_02_roll_ok(roll));
    let b = spec::skills_pp::hina_02_borrow(roll);
    match roll {
        1 => crate::kassert!(b == 0, "1 -> 丸山彩"),
        2 => crate::kassert!(b == 1, "2 -> 白鹭千圣"),
        3 => crate::kassert!(b == 2, "3 -> 若宫伊芙"),
        4 => crate::kassert!(b == 3, "4 -> 大和麻弥"),
        _ => {}
    }
}

// ===========================================================================
// C-PP-maya-dawn-02 watch size (pure)
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn maya_02_watch_formula() {
    let y: i32 = kani::any();
    let deck: i32 = kani::any();
    kani::assume(y >= 0 && y <= 10);
    kani::assume(deck >= 0 && deck <= 8);
    let watch = spec::skills_pp::maya_02_watch(y, deck);
    crate::kassert!(
        watch == (y + 1).min(deck),
        "C-PP-maya-dawn-02: watch min(Y+1, deck)"
    );
    crate::kassert!(
        spec::skills_pp::maya_02_hand_n() == 1,
        "exactly one card joins the hand"
    );
}

// ===========================================================================
// C-PP-eve-unify-02 extra dice (pure; the roll hook is BLOCKED-HOST
// `trig_move_roll`).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn eve_02_extra_dice_formula() {
    let y: i32 = kani::any();
    kani::assume(y >= 0 && y <= 6);
    let n = spec::skills_pp::eve_02_extra_dice(y);
    crate::kassert!(n == y, "C-PP-eve-unify-02: +Yd4");
    let sum: i32 = kani::any();
    kani::assume(sum >= 0 && sum <= 24);
    if spec::skills_pp::eve_02_extra_sum_ok(sum, y) {
        crate::kassert!(sum >= y && sum <= 4 * y, "Yd4 sum range");
    }
}

// ===========================================================================
// C-PP-chisato-frank-02 income boost (pure formula)
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn chisato_02_formula() {
    let pre: i32 = kani::any();
    let y: i32 = kani::any();
    let up: i32 = kani::any();
    kani::assume(pre >= 0 && pre <= 5_000);
    kani::assume(y >= 0 && y <= 20);
    kani::assume(up >= 0 && up <= 20);
    crate::kassert!(
        spec::skills_pp::chisato_02_amount(pre, y) == pre + y * 100,
        "receive += Y*100"
    );
    crate::kassert!(
        spec::skills_pp::chisato_02_flip_pos(up, y) == up.min(y),
        "flip min(Y, up) positives reverse"
    );
}

// ===========================================================================
// C-PP-band-idol-01 fan grant (pure count) + body hook.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost).
fn band_idol_01_start_grant_iff_at(user: i32) {
    let me = user;

    skills_pp_world(me);
    let mut w = world::with_world(|ww| ww.clone());
    w.group_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    // Place the band card so `placed_cards` finds it (first copy).
    world::reset_world(w);
    // The body requires the card on someone's field. Place it via the host.
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("skill:Pastel✽Palettes:与偶像一起").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: 0,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.next_uid = 2;
    });
    let pre_down = world::with_world(|ww| ww.fans_neg);
    invoke::unwrap_done(skill_groups::pastel::at_start_kani(me));
    let post_down = world::with_world(|ww| ww.fans_neg);
    for p in 0..MAX_PLAYERS {
        let is_pp =
            world::with_world(|ww| ww.group_id[p] == crate::intern::intern("Pastel✽Palettes").0);
        if is_pp || p >= world::with_world(|ww| ww.n_players) as usize {
            crate::kassert!(
                post_down[p] == pre_down[p],
                "C-PP-band-idol-01: PP players / out-of-range untouched"
            );
        } else {
            crate::kassert!(
                post_down[p] == pre_down[p] + 1,
                "C-PP-band-idol-01: each non-PP player gains 1 reverse fan"
            );
        }
    }
}
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_idol_01_start_grant_iff() {
    band_idol_01_start_grant_iff_at(0);
}

/// Seat 1 (seat-symmetric 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_idol_01_start_grant_user1_iff() {
    band_idol_01_start_grant_iff_at(1);
}

// ===========================================================================
// C-PP-band-idol-03 no CiRCLE reward — the clause is a prohibition on the
// reward step (PROTOCOL-LEVEL / plan flag). Formula check only.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn band_idol_03_no_reward_formula() {
    let empty: [Effect; 0] = [];
    crate::kassert!(
        spec::skills_pp::band_idol_03_no_reward(&empty),
        "empty reward log is the required shape"
    );
}
// ===========================================================================
// C-PP-first-accident-02 (moved from proofs/pp.rs — needs `skills-personas`
// for the 丸山彩 (2) body; the pp unit's features do not compile that crate,
// so the harness was cfg'd out and `cargo kani` "Failed to match" it).
// 「[持续]：[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。」
// ===========================================================================

/// Shared body at a concrete seat.
fn first_accident_02_iff_at(me: i32) {
    skills_pp_world(me);
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 1 && pre_amount <= 5_000);
    let pre_up = world::with_world(|w| w.fans_pos[me as usize]);
    world::with_world_mut(|w| {
        w.trig_player = me;
        w.trig_value = pre_amount;
        w.pay_amount = pre_amount;
    });
    // 1) play 初次演出事故 — sets the skillBlock token
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:初次演出事故"), me));
    let blocked = world::with_world(|ww| {
        ww.key_get(me, crate::intern::intern("skillBlock:Pastel✽Palettes")) > 0
            || ww.key_get(me, crate::intern::intern("skillBlock")) > 0
    });
    kani::cover!(blocked, "skillBlock token written");
    crate::kassert!(
        blocked,
        "C-PP-first-accident-02: skillBlock token must be set"
    );

    // 2) 丸山彩 (2) must refuse
    let pre_amount2 = world::with_world(|ww| ww.pay_amount);
    invoke::unwrap_done(invoke::entry_idx(
        crate::card_id!("skill:丸山彩:With~"),
        1,
        me,
    ) /* Hook PayCombinedAdd */);
    let post = world::snapshot();
    crate::kassert!(
        post.pay_amount == pre_amount2,
        "C-PP-first-accident-02: PP (2) must not rewrite the pay amount"
    );
    crate::kassert!(
        post.fans_pos[me as usize] == pre_up,
        "C-PP-first-accident-02: PP (2) must not flip fans"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn first_accident_02_blocks_pp_skill_2_iff() {
    first_accident_02_iff_at(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn first_accident_02_blocks_pp_skill_2_user1_iff() {
    first_accident_02_iff_at(1);
}
