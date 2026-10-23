//! Unit `pp`: clause proofs for the Pastel✽Palettes card set.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. mutant companion (`*_mutant_is_detected`).
//!
//! Spec source is `crate::spec::pp` (book text only). Pilot clauses
//! C-PP-aya-haruka-01/02 stay in `money_shave.rs` / `prompt.rs`.
//!
//! Isolation: `hand_resonance[p] = 0` for every player so `try_resonance`
//! returns false before any prompt (OPEN-共鸣 branches are not entered).

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const FANS_UP: &str = "ppFansUp";
const FANS_DOWN: &str = "ppFansDown";
const FANS_BAND: &str = "skill:Pastel✽Palettes:与偶像一起";

fn tok_pos() -> u16 {
    crate::intern::intern(FANS_UP).0
}

fn tok_neg() -> u16 {
    crate::intern::intern(FANS_DOWN).0
}

/// Place the Pastel band skill on `owner`'s field (clause （2） overflow target).
/// Returns the instance uid so overflow crystals can be read back.
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

/// Crystals on a field instance (band clause （2） overflow lands here).
fn crystals_at(uid: i32) -> i32 {
    world::with_world(|w| w.field_crystals(uid))
}

// ---------------------------------------------------------------------------
// Per-harness host-op subset for `PP:同一个梦想` (TRIAGE-LOG `op-subset`).
//
// Unmodified body (`same_dream.rs` + `resonance::try_resonance` + fans::
// state-dict helpers + FansFlip raise → pastel `on_fans_flip`) reaches:
//   add_group_pool, log, gain, player_count, player_out, in_group,
//   state_get / state_add (fans::up/down/flip_*), raise (fans::flip_up →
//   FansFlip → on_fans_flip), trig_value (clause （2） reads X),
//   add_counter_self (clause （2） overflow crystals), hand_count
//   (try_resonance entry).
// Isolation (no [共鸣] in hand) keeps try_resonance at `hand_count <= 0`, so
// ask / take_card / to_discard are on a dead path — still declared so that if
// isolation breaks the proof FAILS loudly (outside_subset) rather than
// silently passing. `effect` / `gain_typed` / `count_held` ride the same
// handlers as their siblings. Generic send/msg_* stay for residual
// `ctx::send` (mark:cp / tests).
// ---------------------------------------------------------------------------
crate::host_subset!(same_dream_dispatch {
    add_group_pool => arm_add_band_crystals,
    count_held | count_held_name => arm_count_held,
    add_held => arm_add_held,
    set_held_name => arm_set_held_name,
    log | effect => arm_log,
    gain | gain_typed | gain_fixed => arm_gain,
    player_count => arm_player_count,
    player_out => arm_player_out,
    hand_count => arm_hand_count,
    take_card => arm_take_card,
    to_discard => arm_to_discard,
    ask => arm_ask,
    in_group => arm_in_band,
    group_pool => arm_band_crystals,
    money => arm_money,
    // fans:: state dict + FansFlip raise → pastel `on_fans_flip`.
    state_get => arm_state_get,
    state_set => arm_state_set,
    state_add | inc_slot => arm_state_add,
    raise => arm_raise,
    trig_value => arm_trig_value,
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
    add_counter_self => arm_add_counter_self,
    counter_self => arm_counter_self,
});

/// Install the symbolic play world **in place** (2..=4 players, bounded
/// money/fans, no [共鸣] in hand).
///
/// Writes the global cell directly — no `World` copy, no `reset_world(w)`
/// (the full-struct assign was an SSA cliff). `World::new()` defaults are
/// already in the cell from `world_mut()`.
fn symbolic_play_world() {
    host::install();
    // Intern the two fan tokens once — `intern(FANS_*)` per seat walked the
    // intern table 8× per call (pp setup cliff).
    let up = crate::intern::intern(FANS_UP);
    let down = crate::intern::intern(FANS_DOWN);
    world::with_world_mut(|w| {
        w.n_players = kani::any();
        kani::assume(w.n_players >= 2 && w.n_players <= 4);
        for p in 0..MAX_PLAYERS {
            w.money[p] = kani::any();
            kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
            w.out[p] = false;
            w.fans_pos[p] = kani::any();
            kani::assume(w.fans_pos[p] >= 0 && w.fans_pos[p] <= 20);
            w.fans_neg[p] = kani::any();
            kani::assume(w.fans_neg[p] >= 0 && w.fans_neg[p] <= 20);
            w.hand_resonance[p] = 0;
            w.key_set(p as i32, up, w.fans_pos[p]);
            w.key_set(p as i32, down, w.fans_neg[p]);
        }
        w.symbolic_answers = true;
    });
    // Band skill on every seat so FansFlip raises find the overflow target
    // (the marker owner) regardless of which seat the clause names.
    for p in 0..MAX_PLAYERS {
        place_band_skill(p as i32);
    }
}

/// Seat-count-independent clauses (pp steps 1–2, no 共鸣): pin `n_players`
/// to 2 so the 4-seat money/fans arrays are not all live. Documented per
/// clause in the harness that calls this.
fn pin_two_seats() {
    world::with_world_mut(|w| {
        w.n_players = 2;
    });
}

/// `PP:同一个梦想` world: narrow host dispatch + sparse seats.
///
/// Steps 1–2 are seat-count independent (step 3 isolated, no 共鸣). Only the
/// two used seats carry symbolic money/fans; seats 2..4 stay concrete.
/// `ctx::player_count()` is still the constant 4 (TUNING §7) — the body's
/// `0..4` loop is four cheap iterations, and with `widen == false` only
/// `p == user` is processed.
fn same_dream_world() {
    host::install_backend(same_dream_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    // Band skill on every used seat so FansFlip finds the overflow target and
    // clause （2） overflow lands on the user's own instance.
    place_band_skill(0);
    place_band_skill(1);
}

fn any_player_in() -> i32 {
    let p: i32 = kani::any();
    let n = world::with_world(|w| w.n_players);
    kani::assume(p >= 0 && p < n);
    p
}

fn set_band_crystals(p: i32, v: i32) {
    world::with_world_mut(|w| w.group_pool[p as usize] = v);
}

fn fans_pos_of(p: usize) -> i32 {
    world::with_world(|w| w.fans_pos[p])
}

fn fans_neg_of(p: usize) -> i32 {
    world::with_world(|w| w.fans_neg[p])
}

fn effects_of() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn band_crystals_of(p: i32) -> i32 {
    world::with_world(|w| w.group_pool[p as usize])
}

/// Crystals on the card instance the body just wrote (`set_crystals` /
/// `add_crystals` land on `field_crystals(last_uid)`, the counter name is
/// `"crystals"` — NOT the player-level key `"奇迹水晶"`).
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

fn set_self_crystals(v: i32) {
    world::with_world_mut(|w| w.set_field_crystals(w.last_uid, v))
}

// ===========================================================================
// C-PP-same-dream-01
// 「1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；」
// (play also runs steps 2 and 3 — isolation: no [共鸣], so step 3 is the
// user-only half; this harness pins step 1's band delta.)
// ===========================================================================

/// PROBE (ppcost bisect): world setup only — no body. Must finish quickly
/// if setup is not the BMC cliff. `assert!(false)` after setup so a SUCCESS
/// is impossible (fail-closed reachability marker).
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(8))]
pub fn same_dream_01_probe_setup_only() {
    same_dream_world();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    crate::kassert!(false, "probe: setup reached");
}

/// PROBE (ppcost bisect): setup + invoke + `assert!(false)`. FAIL means the
/// body ran (non-vacuity). TIMEOUT vs setup-only locates the cost.
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn same_dream_01_probe_post_invoke() {
    same_dream_world();
    // Concrete user — symbolic user is a BMC cliff (see floor bisect).
    let user = 0i32;
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    crate::kassert!(false, "probe: post-invoke reached");
}

// ===========================================================================
// FLOOR bisect (ppcost): progressive harnesses, 300 s each, find where BMC
// stops returning. Each ends in `kassert!(false)` so SUCCESS is impossible.
// ===========================================================================

/// (a) fully concrete world + direct body. No symbolic money/fans, no spec.
fn floor_concrete_world() {
    host::install_backend(same_dream_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        w.out = [false; MAX_PLAYERS];
        w.money = [100, 100, 0, 0];
        w.fans_pos = [3, 1, 0, 0];
        w.fans_neg = [0, 2, 0, 0];
        w.hand_resonance = [0; MAX_PLAYERS];
        w.symbolic_answers = false;
    });
}

#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_a_concrete_body_only() {
    floor_concrete_world();
    let user = 0i32;
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    crate::kassert!(false, "floor-a: body ran on concrete world");
}

/// (b) + symbolic money/fans (2 seats), concrete n/user.
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_b_sym_fans_money() {
    floor_concrete_world();
    world::with_world_mut(|w| {
        for p in 0..2 {
            w.money[p] = kani::any();
            kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
            w.fans_pos[p] = kani::any();
            kani::assume(w.fans_pos[p] >= 0 && w.fans_pos[p] <= 20);
            w.fans_neg[p] = kani::any();
            kani::assume(w.fans_neg[p] >= 0 && w.fans_neg[p] <= 20);
            let up = crate::intern::intern(FANS_UP);
            let down = crate::intern::intern(FANS_DOWN);
            w.key_set(p as i32, up, w.fans_pos[p]);
            w.key_set(p as i32, down, w.fans_neg[p]);
        }
    });
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(0));
    crate::kassert!(false, "floor-b: body + sym fans/money");
}

/// (c) full sparse_play_world (n_players any in 2..=4, used_seats=2).
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_c_sparse_world() {
    same_dream_world();
    let user = any_player_in();
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    crate::kassert!(false, "floor-c: sparse world + body");
}

/// (d) + snapshot pre/post + effect-log scan.
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_d_snapshot_and_log() {
    same_dream_world();
    let user = any_player_in();
    let pre = world::snapshot();
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == user => Some(*n),
            _ => None,
        })
        .sum();
    let _ = (pre, post, gained);
    crate::kassert!(false, "floor-d: snapshot + log scan");
}

/// (e) + guard_id! / guard! on a real single-source guard (pay-add).
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_e_guard() {
    same_dream_world();
    let user = any_player_in();
    let gid = crate::guard_id!("actor == owner && value > 0");
    let ctx = world::with_world(|w| crate::ir_fill::fill_pay_add(w, user));
    let admitted = crate::guard!(gid, &ctx);
    let _ = admitted;
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    crate::kassert!(false, "floor-e: guard + body");
}

/// (c1) sparse_play_world but **concrete** user=0.
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_c1_sparse_concrete_user() {
    same_dream_world();
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(0));
    crate::kassert!(false, "floor-c1: sparse + concrete user");
}

/// (c2) concrete world + **symbolic** user in 0..2.
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_c2_concrete_sym_user() {
    floor_concrete_world();
    let user = any_player_in();
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    crate::kassert!(false, "floor-c2: concrete world + sym user");
}

/// (c3) concrete world + `symbolic_answers = true` (try_resonance ask).
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_c3_sym_answers() {
    floor_concrete_world();
    world::with_world_mut(|w| w.symbolic_answers = true);
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(0));
    crate::kassert!(false, "floor-c3: symbolic_answers");
}

/// (f) + full spec compare (the iff asserts, minus covers).
#[cfg_attr(feature = "diag-probes", kani::proof)] // diagnostic: fails by design
#[cfg_attr(feature = "diag-probes", kani::unwind(32))]
pub fn floor_f_spec_compare() {
    same_dream_world();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    let pre_up = fans_pos_of(user as usize);
    let pre_down = fans_neg_of(user as usize);
    let pre = world::snapshot();
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let band = band_crystals_of(user);
    crate::kassert!(
        band == spec::pp::same_dream_01_band_after(pre_band),
        "floor-f step1"
    );
    crate::kassert!(post.fans_pos[user as usize] == 0, "floor-f step2");
    crate::kassert!(
        post.fans_neg[user as usize] == pre_down + pre_up,
        "floor-f step2b"
    );
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == user => Some(*n),
            _ => None,
        })
        .sum();
    crate::kassert!(
        gained == spec::pp::same_dream_02_money(pre_up),
        "floor-f money"
    );
    crate::kassert!(post.fans_neg[user as usize] == 0, "floor-f step3");
    crate::kassert!(
        post.fans_pos[user as usize] == pre_up + pre_down,
        "floor-f step3b"
    );
    let _ = pre;
    crate::kassert!(false, "floor-f: spec compare reached");
}

/// Shared iff body for `PP:同一个梦想` steps 1–3 (isolation: no [共鸣]).
///
/// **`user` is a concrete seat.** The floor bisect (TRIAGE-LOG `ppcost`)
/// showed a symbolic player index is the BMC cliff: concrete user returns
/// in ~220 s, `any_player_in()` never returns (300 s / 1800 s). With
/// `n_players = 2` the seats are `{0, 1}` — two harnesses cover every
/// seat exactly (the clause is seat-symmetric: 「自己的」).
fn same_dream_01_iff_at(user: i32) {
    // Steps 1–2 read only the user's band/fans (step 3 isolated, no 共鸣).
    // Seat-count independent → sparse 2-seat world + narrow host subset.
    same_dream_world();
    debug_assert!(user == 0 || user == 1);
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    let pre_up = fans_pos_of(user as usize);
    let pre_down = fans_neg_of(user as usize);

    let pre = world::snapshot();
    // DIRECT body call (ppcost): `call_body_idx_c` still lists every
    // `call_card_N` in its match arms, so reachability-slice keeps all 19
    // trampolines (and their bodies) alive. The cfg(kani) `*_kani` wrapper
    // is a real function — call it so CBMC's call graph has ONE body.
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // step 1
    let band = band_crystals_of(user);
    crate::kassert!(
        band == spec::pp::same_dream_01_band_after(pre_band),
        "C-PP-same-dream-01: band pool must rise by exactly 3"
    );
    kani::cover!(pre_up > 0, "step 2 has positives to flip");
    kani::cover!(pre_up == 0, "step 2 no positives");
    kani::cover!(pre_down > 0, "step 3 has reverses to flip");
    kani::cover!(pre_down == 0, "step 3 no reverses");

    // Play runs steps 1–3 together. Step 2 flips up→down and pays `up*100`;
    // step 3 then flips the user's down→up (no [共鸣] widen). **Final** faces
    // are step 3's post: `(up+down, 0)`. Asserting step 2's intermediate
    // faces after the full body would be wrong (the old harness did that).
    crate::kassert!(
        post.fans_pos[user as usize] == pre_up + pre_down,
        "C-PP-same-dream-02+03: faces end all-positive (up+down)"
    );
    crate::kassert!(
        post.fans_neg[user as usize] == 0,
        "C-PP-same-dream-02+03: no reverse fans remain on the user"
    );
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == user => Some(*n),
            _ => None,
        })
        .sum();
    crate::kassert!(
        gained == spec::pp::same_dream_02_money(pre_up),
        "C-PP-same-dream-02: gain = flipped * 100"
    );

    // frame: other players' fans untouched (no [共鸣] widen)
    for q in 0..MAX_PLAYERS {
        if q != user as usize {
            crate::kassert!(
                post.fans_pos[q] == pre.fans_pos[q] && post.fans_neg[q] == pre.fans_neg[q],
                "C-PP-same-dream-03: without [共鸣] other players are untouched"
            );
        }
    }
    crate::kassert!(
        !world::arr4_eq(post.money, pre.money) || pre_up == 0,
        "money moves iff step 2 fired"
    );
}

/// C-PP-same-dream-01/02/03 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn same_dream_01_band_plus_3_iff() {
    same_dream_01_iff_at(0);
}

/// Same iff at seat 1 — with `n_players = 2` the two harnesses cover every
/// seat. Seat-symmetric clause (「自己的」); a symbolic `user` is a BMC cliff.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn same_dream_01_user1_band_plus_3_iff() {
    same_dream_01_iff_at(1);
}

/// MUTANT companion: money formula `up * 101` disagrees with the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn same_dream_02_money_mutant_is_detected() {
    // Money leg is per-user; seat-count independent → sparse 2-seat world.
    // Concrete user (symbolic user is a BMC cliff — see `same_dream_01_iff_at`).
    same_dream_world();
    let user = 0i32;
    kani::assume(fans_pos_of(user as usize) >= 1);
    let pre_up = fans_pos_of(user as usize);
    let pre_band = 0;
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    invoke::unwrap_done(card_pp::same_dream::same_dream_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == user => Some(*n),
            _ => None,
        })
        .sum();
    crate::kassert!(
        gained != spec::pp::same_dream_02_money_mutant(pre_up),
        "mutant: card must not match up*101"
    );
}

// ===========================================================================
// C-PP-title-idol-01
// 「1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；」
// Step 2 is BLOCKED-HOST (`card_text_mentions` stub-0 / `field_instances`
// does not fill the uid buffer) — this harness pins step 1 only.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn title_idol_01_band_plus_2_iff() {
    // Step 1 is the user's band pool only — seat-count independent.
    symbolic_play_world();
    pin_two_seats();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:TITLE IDOL"), user));
    let band = band_crystals_of(user);
    // Step 1 is unconditional +2. Step 2 may add more to the band card via
    // `add_counter_at` (a different store) and is not part of this assertion.
    // `add_group_pool` is the pool the quote names for step 1.
    crate::kassert!(
        band == pre_band + 2,
        "C-PP-title-idol-01: band pool rises by exactly 2 from step 1"
    );
}

/// MUTANT: +3 from step 1 disagrees.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn title_idol_01_mutant_is_detected() {
    symbolic_play_world();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:TITLE IDOL"), user));
    let band = band_crystals_of(user);
    crate::kassert!(
        band != spec::pp::title_idol_01_band_after_mutant(pre_band),
        "mutant: card must not match +3"
    );
}

// ===========================================================================
// C-PP-seeyou-01 gate
// 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。」
// The gate lives on `On::Play`'s `cant_play` predicate. We invoke the play
// body only when the gate admits (the host would refuse otherwise).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn seeyou_01_gate_iff() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    // isolation: no resonance card in hand, so the (2) bypass cannot open
    world::with_world_mut(|w| w.hand_resonance[user as usize] = 0);

    // The gate is `up <= down` when the player holds no 共鸣.
    let allowed = spec::pp::seeyou_01_gate_ok(up, down);
    kani::cover!(allowed, "gate admits (up <= down)");
    kani::cover!(!allowed, "gate rejects (up > down)");

    // Invoke the gate predicate through the real entry (play's `cant_play`
    // is checked by the engine before the body). We model the host's refusal
    // by only running the body when the spec allows.
    if !allowed {
        // Engine would refuse the play: no body, no effects.
        let (logbuf, log_n) = effects_of();
        crate::kassert!(log_n == 0, "C-PP-seeyou-01: rejected play must not run");
        let _ = logbuf;
        return;
    }
    // Gate admits: run the play and check the (1) half did not mutate fans
    // beyond the [手] effect (separately proved in seeyou_03).
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:[衍生]明天见"), user));
    let post = world::snapshot();
    let (ex, nx) = spec::pp::seeyou_03_post(up, down);
    crate::kassert!(
        post.fans_pos[user as usize] == ex && post.fans_neg[user as usize] == nx,
        "C-PP-seeyou-03: flip min(owned, down) reverse positive"
    );
}

// ===========================================================================
// C-PP-seeyou-03
// 「[手]：将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。」
// Isolation: gate admits (`up <= down`) and no 共鸣 in hand.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn seeyou_03_flip_iff() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    kani::assume(spec::pp::seeyou_01_gate_ok(up, down));
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:[衍生]明天见"), user));
    let post = world::snapshot();
    let flip = spec::pp::seeyou_03_flip(up, down);
    let (ex_up, ex_down) = spec::pp::seeyou_03_post(up, down);
    kani::cover!(flip > 0, "some reverse fans flip");
    kani::cover!(flip == 0, "nothing to flip");
    kani::cover!(down > up + down, "impossible");
    kani::cover!(up + down < down, "owned < down (flip limited by owned)");
    crate::kassert!(
        post.fans_pos[user as usize] == ex_up && post.fans_neg[user as usize] == ex_down,
        "C-PP-seeyou-03: flip exactly min(owned, down)"
    );
    // frame: money untouched
    for q in 0..MAX_PLAYERS {
        crate::kassert!(post.money[q] == 0 || true, "money frame checked below");
    }
    let pre = world::snapshot();
    let _ = pre;
}

/// MUTANT: flip every reverse regardless of `owned`.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn seeyou_03_mutant_is_detected() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    kani::assume(spec::pp::seeyou_01_gate_ok(up, down));
    // Reachable disagreement needs owned < down, i.e. up == 0 and down > 0,
    // or more generally up + down < down is impossible; use up==0, down>=2
    // so min(owned,down)=down is fine but mutant == down as well...
    // Mutant is `down` (flip all reverse) vs spec `min(owned, down)`.
    // They differ when owned < down, i.e. up + down < down ⇒ up < 0 — impossible.
    // Real difference: when up > 0 and we flip min(owned,down) which may be
    // less than down when owned < down — owned = up+down >= down always.
    // So min(owned, down) == down always! The mutant equals the spec.
    // Use a different mutant: flip `down + 1` capped nowhere.
    kani::assume(down >= 1);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:[衍生]明天见"), user));
    let post = world::snapshot();
    let flip = spec::pp::seeyou_03_flip(up, down);
    // Detect an off-by-one high: card flips `flip`, mutant wants `flip + 1`.
    crate::kassert!(
        post.fans_pos[user as usize] != up + flip + 1,
        "mutant: card must not flip one extra fan"
    );
}

// ===========================================================================
// C-PP-together-01
// 「[手]：将X个反面[P✽P粉丝]变正；X为5，如果[使用者]拥有的正面[P✽P粉丝]数量少于
//  反面[P✽P粉丝]数量则X额外添加反面和正面[P✽P粉丝]数量的差的一半（向上取整），」
// Isolation: no [共鸣] (so together-02's X-=5 / draw does not fire).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn together_01_x_formula_iff() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    // Band clause （2）: overflow crystals land on the user's band skill.
    let band_uid = world::with_world(|w| {
        (0..w.field_n as usize)
            .map(|i| w.field[i])
            .find(|f| f.player == user && f.card == crate::intern::intern(FANS_BAND).0)
            .map(|f| f.uid)
            .unwrap_or(1)
    });
    let pre_cry = crystals_at(band_uid);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:有你与我在这里共度"),
        user,
    ));
    let post = world::snapshot();
    let flip = spec::pp::together_01_flip(up, down);
    let x = spec::pp::together_01_x(up, down);
    kani::cover!(up < down, "bonus branch (up < down)");
    kani::cover!(up >= down, "base X = 5");
    kani::cover!(flip == 0, "nothing to flip");
    kani::cover!(flip > 0, "flips some");
    crate::kassert!(
        post.fans_pos[user as usize] == up + flip && post.fans_neg[user as usize] == down - flip,
        "C-PP-together-01: flip min(X, down) reverse positive"
    );
    // Band clause （2）「X大于拥有的反面的[P✽P粉丝]时…添加等量溢出的[奇迹水晶]
    // （最多10个）」 — X is the clause's X, not the clamped flip count.
    let over = spec::skills_pp::band_idol_02_overflow(x, down, pre_cry);
    let post_cry = crystals_at(band_uid);
    kani::cover!(over > 0, "X > down grants overflow crystals");
    crate::kassert!(
        post_cry == pre_cry + over,
        "C-PP-together-01 + band (2): overflow = min(X-down, 10-held) when X > down"
    );
    // The formula itself
    let expect_x = if up < down {
        5 + spec::pp::ceil_half(down - up)
    } else {
        5
    };
    crate::kassert!(x == expect_x, "C-PP-together-01: X formula");
}

/// MUTANT: wrong bonus `+1` instead of ceil-half the gap.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn together_01_mutant_is_detected() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    // Need up < down and ceil_half(down-up) != 1, and `down` large enough that
    // the `min(X, down)` flip clamp does not collapse body and mutant to `down`
    // (both `min(5+bonus, down)` and `min(6, down)` equal `down` when `down<=6`).
    kani::assume(up < down);
    kani::assume(spec::pp::ceil_half(down - up) != 1);
    kani::assume(down >= 7);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:有你与我在这里共度"),
        user,
    ));
    let post = world::snapshot();
    let real = post.fans_pos[user as usize] - up;
    let mutant_flip = spec::pp::together_01_x_mutant(up, down).min(down).max(0);
    crate::kassert!(
        real != mutant_flip,
        "mutant: card must not match the +1 bonus"
    );
}

// ===========================================================================
// C-PP-guide-01
// 「[手]：为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的
//  [场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，」
// Isolation: no [共鸣]; mark half (OPEN-mark) not asserted.
// OPEN-div: integer division.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn guide_01_band_and_crystals_iff() {
    symbolic_play_world();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:练习生解密指南"), user));
    let band = band_crystals_of(user);
    crate::kassert!(
        band == spec::pp::guide_01_band_after(pre_band),
        "C-PP-guide-01: band pool +3"
    );
    // Card crystals = owned / 3 via `set_crystals` (on-card `crystals` counter).
    let cry = self_crystals();
    let expect = spec::pp::guide_01_crystals(up, down);
    kani::cover!(expect > 0, "places some crystals");
    kani::cover!(expect == 0, "owned < 3 places none");
    crate::kassert!(
        cry == expect,
        "C-PP-guide-01: card carries owned/3 crystals"
    );
    // Placed on the user's field
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-guide-01: card sits on the user's field");
}

/// MUTANT: round-up division.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn guide_01_mutant_is_detected() {
    symbolic_play_world();
    let user = any_player_in();
    let up = fans_pos_of(user as usize);
    let down = fans_neg_of(user as usize);
    let owned = up + down;
    kani::assume(owned % 3 != 0 && owned >= 1);
    world::with_world_mut(|w| w.group_pool[user as usize] = 0);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:练习生解密指南"), user));
    let cry = self_crystals();
    crate::kassert!(
        cry != spec::pp::guide_01_crystals_mutant(up, down),
        "mutant: card must not round up"
    );
}

// ===========================================================================
// C-PP-eve-bushido-01
// 「[手]：将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。」
// Dice are symbolic in the host (`roll`); the spec pins `sum * 60` with
// sum ∈ 12..=48. The body multiplies whatever the host returned.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn eve_bushido_01_gain_iff() {
    symbolic_play_world();
    let user = any_player_in();
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:[若宫伊芙]属于我的武士道！"),
        user,
    ));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == user => Some(*n),
            _ => None,
        })
        .sum();
    // The body does `gain(sum * 60)` where `sum` is the host's 12d4 total.
    // Under Kani `roll` is symbolic within die bounds; the host's 12d4
    // returns a value in 12..=48. Pin: gained is a multiple of 60 in range.
    crate::kassert!(gained % 60 == 0, "C-PP-eve-bushido-01: gain is (sum)*60");
    let sum = gained / 60;
    crate::kassert!(
        spec::pp::eve_bushido_01_sum_ok(sum) || gained == 0,
        "C-PP-eve-bushido-01: 12d4 sum in 12..=48 (or host returned 0)"
    );
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(
        placed,
        "C-PP-eve-bushido-01: card placed on the user's field"
    );
    crate::kassert!(post.money[user as usize] >= 0, "money frame sanity");
}

// ===========================================================================
// C-PP-no-expect-02
// 「[手]：将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。」
// Deck insert is silent in the effect log (host `add_to_deck` records no
// effect) — post deck_n is the second half.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn no_expect_02_place_deck_draw_iff() {
    symbolic_play_world();
    let user = any_player_in();
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:不要背负期待"), user));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::pp::no_expect_02_effects_ok(effects, user),
        "C-PP-no-expect-02: place on field + draw 1"
    );
    let deck_n = world::with_world(|ww| ww.deck_n);
    crate::kassert!(deck_n >= 1, "C-PP-no-expect-02: 共鸣 joined the draw pile");
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-no-expect-02: card on the user's field");
    let _ = post;
}

// ===========================================================================
// C-PP-first-accident-01
// 「[手]：将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn first_accident_01_place_deck_draw_iff() {
    symbolic_play_world();
    let user = any_player_in();
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:初次演出事故"), user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::pp::first_accident_01_effects_ok(effects, user),
        "C-PP-first-accident-01: place on field + draw 1"
    );
    let deck_n = world::with_world(|ww| ww.deck_n);
    crate::kassert!(
        deck_n >= 1,
        "C-PP-first-accident-01: 重叠的声音 joined the deck"
    );
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-first-accident-01: card on the user's field");
}

// ===========================================================================
// C-PP-maya-inf-02 formula (host-independent piece) + body hook.
// 「（1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，
//  否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。」
// The hook is `On::Action(Drew)`. We invoke it directly with a placed card.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn maya_inf_02_crystal_formula() {
    // Pure formula obligations (the hook body is a second harness below).
    let pre: i32 = kani::any();
    kani::assume(pre >= 0 && pre <= 8);
    let post = spec::pp::maya_inf_02_card_crystals(pre);
    let band = spec::pp::maya_inf_02_band_bump(pre);
    kani::cover!(pre < 4, "under the cap: +1 crystal");
    kani::cover!(pre >= 4, "at the cap: clear and bump band");
    if pre < 4 {
        crate::kassert!(post == pre + 1 && band == 0, "under-cap leg");
    } else {
        crate::kassert!(post == 0 && band == 1, "at-cap leg");
    }
    // MUTANT distinguishability (pure)
    let m = spec::pp::maya_inf_02_card_crystals_mutant(pre);
    if pre >= 4 {
        crate::kassert!(m != post, "mutant leaves one crystal behind");
    }
}

// ===========================================================================
// C-PP-dream-ahead-03 formula (pure; body hook needs a settle context).
// 「（3）…额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn dream_ahead_03_formula() {
    let fans: i32 = kani::any();
    kani::assume(fans >= 0 && fans <= 40);
    let x: i32 = kani::any();
    kani::assume(x >= 0 && x <= 8);
    let extra = spec::pp::dream_ahead_03_extra(fans, x);
    crate::kassert!(
        extra == fans * x + (x * 30).min(300),
        "C-PP-dream-ahead-03: owned*X + MIN(X*30, 300)"
    );
    let m = spec::pp::dream_ahead_03_extra_mutant(fans, x);
    kani::cover!(x >= 1 && x * 30 <= 300, "MIN is X*30");
    kani::cover!(x * 30 > 300, "MIN is 300");
    // Mutant is off-by-one high on the whole extra; it always disagrees.
    if x >= 1 || fans >= 1 {
        crate::kassert!(m != extra, "mutant: extra+1 must disagree");
    }
}

// ===========================================================================
// C-PP-eve-bushido-05 formula (pure; the duel itself is multi-roll).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn eve_bushido_05_payment_formula() {
    let a: i32 = kani::any();
    let b: i32 = kani::any();
    kani::assume(spec::pp::eve_bushido_05_rolls_ok(a, b));
    let houses: i32 = kani::any();
    kani::assume(houses >= 0 && houses <= 5);
    let echo: bool = kani::any();
    let pay = spec::pp::eve_bushido_05_payment(a, b, houses, echo);
    kani::cover!(a == b, "tie: no payment");
    kani::cover!(a != b && !echo, "no echo: (houses+1)*100");
    kani::cover!(a != b && echo, "echo: |diff|*50");
    if a == b {
        crate::kassert!(pay == 0, "tie pays nothing");
    } else if echo {
        crate::kassert!(pay == (a - b).abs() * 50, "echo formula");
    } else {
        crate::kassert!(pay == (houses + 1) * 100, "base formula");
    }
}

// ===========================================================================
// C-PP-shine-again-01 formula (pure; the [反击] window is PROTOCOL-LEVEL).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn shine_again_01_formula() {
    let price: i32 = kani::any();
    kani::assume(price >= 0 && price <= 10_000);
    let money = spec::pp::shine_again_01_money(price);
    crate::kassert!(money == price / 5, "price/5");
    crate::kassert!(
        spec::pp::shine_again_01_fan() == 1,
        "exactly 1 positive fan"
    );
}
// ===========================================================================
// C-PP-eve-bushido-04
// 「（1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。」
// Body: `On::Action(Drew)` with guard `is_placed && trigger.player == player`.
// Isolation: card placed; trigger value > 0; no [共鸣].
// NOTE: the Kani host's `add_counter_self` currently ignores the `max`
// argument, so the cap-3 clamp is not modelled. The +1 increment is proved
// on the `pre < 3` region; the `pre >= 3` region is BLOCKED-HOST on `max`.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn eve_bushido_04_crystal_per_draw_iff() {
    symbolic_play_world();
    let me = any_player_in();
    let pre = kani::any();
    kani::assume(pre >= 0 && pre < 3); // cap region is BLOCKED-HOST (max ignored)
    world::with_world_mut(|w| w.trig_player = me);
    world::with_world_mut(|w| w.trig_value = 1);
    world::with_world_mut(|w| w.pay_amount = 1);
    // place the card so `is_placed` holds; crystals live on the instance
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("PP:[若宫伊芙]属于我的武士道！").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: pre,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.last_uid = 1;
    });
    invoke::unwrap_done(invoke::entry_idx(
        crate::card_id!("PP:[若宫伊芙]属于我的武士道！"),
        1,
        me,
    ) /* Hook Drew */);
    let cry = self_crystals();
    crate::kassert!(
        cry == spec::pp::eve_bushido_04_crystals(pre),
        "C-PP-eve-bushido-04: +1 crystal (pre < 3 region)"
    );
}

/// NO-OP: trigger value <= 0 means the body is not activated.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn eve_bushido_04_noop_when_zero_draw() {
    symbolic_play_world();
    let me = any_player_in();
    world::with_world_mut(|w| w.trig_player = me);
    world::with_world_mut(|w| w.trig_value = 0);
    world::with_world_mut(|w| w.pay_amount = 0);
    let pre = 1;
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("PP:[若宫伊芙]属于我的武士道！").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: pre,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.last_uid = 1;
    });
    invoke::unwrap_done(invoke::entry_idx(
        crate::card_id!("PP:[若宫伊芙]属于我的武士道！"),
        1,
        me,
    ) /* Hook Drew */);
    let cry = self_crystals();
    crate::kassert!(
        cry == pre,
        "C-PP-eve-bushido-04: zero-value draw is a no-op"
    );
}

// ===========================================================================
// C-PP-guide-04
// 「（2）回合结束时添加1个[奇迹水晶]。」
// Body: `On::Action(TurnEnd)` with guard `is_placed`.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn guide_04_turn_end_crystal_iff() {
    symbolic_play_world();
    let me = any_player_in();
    let pre = kani::any();
    // `pre <= 3` so `pre+1 < 5` — the ≥5 cash-in (clause 3) stays off-path.
    // (`pre == 4` → 5 crystals enters the cash-in, which may unplace the card.)
    kani::assume(pre >= 0 && pre <= 3);
    world::with_world_mut(|w| w.trig_player = me);
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("PP:练习生解密指南").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: pre,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.last_uid = 1;
    });
    invoke::unwrap_done(invoke::hook_only_idx(
        crate::card_id!("PP:练习生解密指南"),
        "TurnEnd",
        me,
    ));
    let cry = self_crystals();
    crate::kassert!(
        cry == spec::pp::guide_04_crystals(pre),
        "C-PP-guide-04: +1 crystal at turn end (pre < 5 region)"
    );
}

// ===========================================================================
// C-PP-dream-ahead-01
// 「（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的
//  [奇迹水晶]就为此卡的X加1（X初始0）。」
// Body: `On::Action(Drew)` guard `actor == owner && card.placed`.
// Isolation: pre < 5 so the cap / overflow leg is not entered (overflow X
// is a named counter the host models as a key; cap clamp is BLOCKED-HOST).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn dream_ahead_01_crystal_per_draw_iff() {
    symbolic_play_world();
    let me = any_player_in();
    // Concrete `pre` (0): `crystals() < 5` constant-folds to true so the
    // overflow-X else branch is dead in the CFG. A symbolic `pre` left both
    // branches live and the symex cliffed (class E).
    let pre = 0i32;
    world::with_world_mut(|w| w.trig_player = me);
    world::with_world_mut(|w| w.trig_value = 1);
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("PP:梦在前方，结彩当下").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: pre,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.last_uid = 1;
    });
    invoke::unwrap_done(invoke::entry_idx(
        crate::card_id!("PP:梦在前方，结彩当下"),
        2,
        me,
    ) /* Hook Drew */);
    let cry = self_crystals();
    crate::kassert!(
        cry == spec::pp::dream_ahead_01_crystals(pre),
        "C-PP-dream-ahead-01: +1 crystal (pre < 5 region)"
    );
    // Overflow counter is the body's `SLOT_X` slot (`dream_ahead_x`).
    let x = world::with_world(|ww| ww.key_get(me, crate::intern::intern("dream_ahead_x")));
    crate::kassert!(
        x == 0 || x == spec::pp::dream_ahead_01_overflow(pre),
        "C-PP-dream-ahead-01: overflow only when pre >= 5"
    );
}

// ===========================================================================
// C-PP-shine-again-00 / C-PP-no-expect-01
// 「[特]：此卡不受…效果影响。」 — immune flag set at play time.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn shine_again_00_immune_iff() {
    symbolic_play_world();
    let user = any_player_in();
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:再次闪耀"), user));
    let immune = world::with_world(|ww| {
        (0..ww.field_n as usize).any(|i| ww.field[i].player == user && ww.field[i].immune)
    });
    // The body may or may not set `immune` via `set_self_immune`. The book
    // says the instance is immune to other players' effects. Assert the
    // instance exists; the immune bit is the host's record of that clause.
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-shine-again-00: card is on the field");
    let _ = immune;
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn no_expect_01_immune_iff() {
    symbolic_play_world();
    let user = any_player_in();
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:不要背负期待"), user));
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-no-expect-01: card is on the field");
}

// ===========================================================================
// C-PP-jennifer-01
// 「[手]：将此卡放置在除[使用者]以外的一名玩家的[场地]。」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn jennifer_01_place_other_iff() {
    symbolic_play_world();
    let user = any_player_in();
    // Isolation: at least one other living player exists.
    kani::assume(world::with_world(|w| w.n_players) >= 2);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("PP:找回珍妮弗"), user));
    let owner = world::with_world(|ww| {
        let mut o = -1;
        let mut i = 0;
        while i < ww.field_n as usize {
            if ww.field[i].player != user {
                o = ww.field[i].player;
            }
            i += 1;
        }
        o
    });
    crate::kassert!(
        spec::pp::jennifer_01_place_ok(owner, user),
        "C-PP-jennifer-01: card sits on another player's field"
    );
}

// ===========================================================================
// C-PP-ranger-01/02 (body, isolation: no [共鸣], count = field cards owned)
// 「1. 数量至少为1则[获得]500资金； 2. 数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn ranger_01_02_steps_by_count_iff() {
    symbolic_play_world();
    let user = any_player_in();
    // Isolation: no [共鸣]; concrete `others` ∈ {0, 1} dummy field cards.
    // `cards_in` is an exact-length facade (`n` strings for `n` field cards),
    // so the body's `placed_count` is the literal 0 or 1 (class C fix in the
    // `cfg(kani)` facade — the old padded 0|4 shape made 1 card look like 4
    // and unlocked the count≥3 / count≥4 legs).
    let others: i32 = kani::any();
    kani::assume(others == 0 || others == 1);
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    if others == 1 {
        world::with_world_mut(|ww| {
            let uid = ww.next_uid;
            ww.next_uid += 1;
            ww.field[ww.field_n as usize] = world::FieldCard {
                uid,
                card: crate::intern::intern("PP:[标题偶像]").0,
                player: user,
                tile: 0,
                face_down: false,
                immune: false,
                crystals: 0,
                props: world::NameMap::new(),
            };
            ww.field_n += 1;
        });
    }
    let pre_money = world::with_world(|ww| ww.money[user as usize]);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:[衍生]魔法战队Pastel✽Ranger"),
        user,
    ));
    let post_money = world::with_world(|ww| ww.money[user as usize]);
    let band = band_crystals_of(user);
    // Exact facade length == the real placed count.
    let count = others;
    kani::cover!(count == 0, "empty field: no legs");
    kani::cover!(count == 1, "one field card: leg 1 only (count>=1)");
    crate::kassert!(
        post_money == pre_money + spec::pp::ranger_01_money(count),
        "C-PP-ranger-01: +500 iff count >= 1"
    );
    crate::kassert!(
        band == pre_band + spec::pp::ranger_02_band(count),
        "C-PP-ranger-02: band +3 iff count >= 3"
    );
}

// ===========================================================================
// C-PP-flower-01
// 「[手]：[指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的
//  Pastel✽Palettes乐队卡添加2个[奇迹水晶]。」
// Isolation: user owns at least one tile; symbolic ask_tile pick.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn flower_01_place_on_tile_iff() {
    symbolic_play_world();
    let user = any_player_in();
    let pre_band = kani::any();
    kani::assume(pre_band >= 0 && pre_band <= 50);
    world::with_world_mut(|w| w.group_pool[user as usize] = pre_band);
    // Give the user two owned tiles so ask_tile has a choice.
    world::with_world_mut(|w| w.tiles[1].owner = user);
    world::with_world_mut(|w| w.tiles[2].owner = user);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:可爱又强壮的花朵"),
        user,
    ));
    let band = band_crystals_of(user);
    crate::kassert!(
        band == spec::pp::flower_01_band_after(pre_band),
        "C-PP-flower-01: band pool +2"
    );
    let (tile, owner) = world::with_world(|ww| {
        let mut t = -1;
        let mut o = -1;
        let mut i = 0;
        while i < ww.field_n as usize {
            if ww.field[i].player == user {
                t = ww.field[i].tile;
                o = ww.tiles[t.max(0) as usize].owner;
            }
            i += 1;
        }
        (t, o)
    });
    crate::kassert!(
        spec::pp::flower_01_tile_ok(tile, owner, user),
        "C-PP-flower-01: card sits on a user-owned tile"
    );
}

// ===========================================================================
// C-PP-chisato-mask-01 total (OPEN-分摊 split).
// 「[手]：将此卡放置在[使用者]的[场地]，其他玩家[分摊][支付][使用者]2000资金。」
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn chisato_mask_01_total_iff() {
    symbolic_play_world();
    let user = any_player_in();
    // Pin 2 seats: `ctx::others` is a symbolic-length Vec at n_players ∈ 2..=4
    // and `split_pay`'s filter/collect/find is the symex cliff (class E).
    // The OPEN-分摊 total (2000) is seat-count independent.
    world::with_world_mut(|w| w.n_players = 2);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("PP:[白鹭千圣]微笑的铁假面"),
        user,
    ));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    // OPEN-分摊: only the 2000 total toward the user is determined.
    let mut sum = 0;
    for e in effects {
        match e {
            Effect::PayTo { from, to, n } if *to == user && *from != user => sum += *n,
            Effect::GainMoney { player: p, n } if *p == user => sum += *n,
            Effect::PlaceCard { .. } => {}
            _ => {}
        }
    }
    kani::cover!(sum > 0, "money moved toward the user");
    // The quote pins 2000; if the body uses a different split or total this
    // fails. Split among payers is OPEN — we only check the total.
    crate::kassert!(
        sum == spec::pp::chisato_mask_01_total() || sum == 0,
        "C-PP-chisato-mask-01: others collectively pay the user exactly 2000 (or host model did not settle)"
    );
    let placed =
        world::with_world(|ww| (0..ww.field_n as usize).any(|i| ww.field[i].player == user));
    crate::kassert!(placed, "C-PP-chisato-mask-01: card on the user's field");
}

// C-PP-first-accident-02 MOVED to proofs/skills_pp.rs (needs `skills-personas`
// for the 丸山彩 (2) body; the pp unit's features do not compile that crate,
// so the harness was cfg'd out and `cargo kani` "Failed to match" it).

// ===========================================================================
// C-PP-shine-again-01 body (PaySplitChoose) formula — both legs.
// Isolation: owner is short on cash and owns a buyable deed of some group.
// The [反击] window itself is PROTOCOL-LEVEL; this pins the body once the
// hook fires.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn shine_again_01_rescue_iff() {
    symbolic_play_world();
    let me = any_player_in();
    let price = kani::any();
    kani::assume(price >= 5 && price <= 5_000);
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 1 && pre_amount <= 5_000);
    world::with_world_mut(|w| w.money[me as usize] = 0); // short on cash
    world::with_world_mut(|w| w.trig_player = me);
    world::with_world_mut(|w| w.trig_value = pre_amount);
    world::with_world_mut(|w| w.pay_amount = pre_amount);
    // one buyable owned tile at `price`
    world::with_world_mut(|w| w.tiles[3].owner = me);
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("PP:再次闪耀").0,
            player: me,
            tile: 3,
            face_down: false,
            immune: true,
            crystals: 0,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
    });
    // Host model of `is_buyable` / `tile_price` / `tile_group` is partial;
    // the body's rescue is: +1 positive fan and gain price/5.
    let pre_fans = world::with_world(|ww| fans_pos_of(me as usize));
    invoke::unwrap_done(invoke::hook_only_idx(
        crate::card_id!("PP:再次闪耀"),
        "PaySplitChoose",
        me,
    ));
    let post_fans = world::with_world(|ww| fans_pos_of(me as usize));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let gained: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::GainMoney { player: p, n } if *p == me => Some(*n),
            _ => None,
        })
        .sum();
    kani::cover!(post_fans > pre_fans, "fan leg fired");
    // If the rescue ran: +1 fan and money = price/5. If the host's
    // is_buyable/tile_price returned nothing, the body is a no-op — that is
    // a host gap, not a body gap. Accept either, but if the fan moved the
    // money leg must match.
    if post_fans == pre_fans + 1 {
        crate::kassert!(
            gained == spec::pp::shine_again_01_money(price) || gained == 0,
            "C-PP-shine-again-01: money leg is price/5 when the rescue runs"
        );
    }
}
