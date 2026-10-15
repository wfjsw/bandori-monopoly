//! Unit `sumimi`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op + covers + mutant.
//! Specs live in `crate::spec::sumimi` and come from the clause docs only.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Isolation assumes are named here and `kani::assume`d.
//! Narration (`ctx::log`) is not part of the frame.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-body host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `Sumimi:兼顾偶像与乐队` play body — money + log + gain.
/// `state_get` is the play-gate residual (`cant_play` reads `redeemed`); the
/// body itself is money / log / gain only.
crate::host_subset!(idolband_dispatch {
    money => arm_money,
    gain | gain_typed | gain_fixed => arm_gain,
    log | effect => arm_log,
    state_get => arm_state_get,
});

/// `skill:三角初华（Sumimi）:成为偶像` `on_pass` — one `gain_charge`.
crate::host_subset!(hina_pass_dispatch {
    gain_charge => arm_gain_charge,
});

/// `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` `after_move` — `gain_charge` + `set_dest`.
crate::host_subset!(childhood_after_move_dispatch {
    gain_charge => arm_gain_charge,
    set_dest => arm_set_dest,
});

/// `Sumimi:一人两个甜甜圈` play body — pos / exile / slots / dest / place / log.
/// Isolation: the multi-act `PassTile` / `SettleAfter` half (clause (2)) is
/// PROTOCOL-LEVEL and not entered here.
crate::host_subset!(two_donuts_dispatch {
    player_pos => arm_player_pos,
    give_exile => arm_give_exile,
    set_slot => arm_set_slot,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    log | effect => arm_log,
});

/// `Sumimi:Sumimi是二人一体的` play body — character flip + fire reset.
/// `find_card` walks `field_instances`; under Kani that arm is a count-only
/// stub (no uid/name buffer), so the `unplace_at` half of the swap is
/// isolated (never entered) — still declared so isolation cannot drift.
crate::host_subset!(two_in_one_dispatch {
    persona_is => arm_character_is,
    field_instances => arm_field_instances,
    unplace_at | unplace_card | unplace_card_named => arm_unplace_card,
    place_card_at => arm_place_card_at,
    state_set => arm_state_set,
    log | effect => arm_log,
});

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn log_slice() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn effects_of(logbuf: &[Effect; world::MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

/// Play gate of a `On::Play(_, Some(gate), _)` entry. `true` = playable.
/// `idx` is a `crate::card_id!` index (no string scan).
fn play_gate_allows(idx: usize, player: i32) -> bool {
    let card = invoke::by_idx(idx);
    for on in card.on.iter() {
        if let card_sdk::On::Play(_, gate, _) = on {
            return match gate {
                Some(g) => g(player).is_none(),
                None => true,
            };
        }
    }
    false
}

// ===========================================================================
// C-SUMIMI-idolband -- Sumimi:兼顾偶像与乐队
// 「当你本回合未进行过赎回操作时可打出，将你的资金重设为3000。
//  （若你的资金为3000以上则无效果）」
// Seat-symmetric (the user's own money).
// ===========================================================================

const IDOLBAND: usize = crate::card_id!("Sumimi:兼顾偶像与乐队");

/// In-place 2-seat world for the play body + its `redeemed` play gate.
fn idolband_world(user: i32, redeemed: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(idolband_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    let key = intern::intern("redeemed");
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        w.key_set(user, key, redeemed);
    });
}

/// Shared iff at a **concrete** user seat.
fn idolband_reset_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let redeemed: i32 = kani::any();
    kani::assume(redeemed == 0 || redeemed == 1);
    idolband_world(user, redeemed);
    let playable = play_gate_allows(IDOLBAND, user);

    kani::cover!(playable, "gate admits (not redeemed)");
    kani::cover!(!playable, "gate rejects (redeemed)");

    crate::kassert!(
        playable == (redeemed == 0),
        "C-SUMIMI-idolband-01: gate must allow exactly the not-redeemed case"
    );

    let pre = world::snapshot();
    if !playable {
        // Host would not call the body. Untouched.
        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.money, pre.money), "rejected play must not touch money");
        let (logbuf, log_n) = log_slice();
        crate::kassert!(log_n == 0, "rejected play must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    let pre_money = pre.money[user as usize];
    kani::cover!(pre_money < 3000, "pre money < 3000");
    kani::cover!(pre_money >= 3000, "pre money >= 3000");

    // DIRECT body call (ppcost).
    invoke::unwrap_done(card_sumimi::idol_and_band::idol_and_band_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::sumimi::idolband_money_ok(effects, user, pre_money),
        "C-SUMIMI-idolband-02/03: money path must be a set-to-3000 or a no-op"
    );
    crate::kassert!(
        post.money[user as usize] == spec::sumimi::idolband_post_money(pre_money),
        "C-SUMIMI-idolband-02: post money must be exactly 3000 when pre < 3000"
    );
    for p in 0..MAX_PLAYERS {
        if p != user as usize {
            crate::kassert!(post.money[p] == pre.money[p], "frame: other players' money");
        }
        crate::kassert!(post.charge[p] == pre.charge[p], "frame: fire");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
        crate::kassert!(post.stay[p] == pre.stay[p], "frame: stay");
    }
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand bitset");
    crate::kassert!(post.discard == pre.discard, "frame: discard bitset");
}

/// Iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn idolband_reset_iff() {
    idolband_reset_iff_at(0);
}

/// Same iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
#[kani::proof]
#[kani::unwind(32)]
pub fn idolband_reset_user1_iff() {
    idolband_reset_iff_at(1);
}

/// Mutant companion: the off-by-one target (2999) must NOT match the card.
#[kani::proof]
#[kani::unwind(32)]
pub fn idolband_mutant_is_detected() {
    idolband_world(0, 0);
    let user = 0i32;
    world::with_world_mut(|w| {
        // Pin money in the live region so the mutant is reachable.
        w.money[0] = kani::any();
        kani::assume(w.money[0] >= 0 && w.money[0] < 3000);
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_sumimi::idol_and_band::idol_and_band_kani(user));
    let post = world::snapshot();
    let pre_money = pre.money[user as usize];
    let wrong = spec::sumimi::idolband_post_money_mutant(pre_money);
    kani::cover!(pre_money < 3000, "mutant region live");
    crate::kassert!(
        post.money[user as usize] != wrong,
        "mutant: card must not match the 2999 target"
    );
}

// ===========================================================================
// C-SUMIMI-hina-01 -- pass CiRCLE → +1 [火罐]
// `skill:三角初华（Sumimi）:成为偶像` `On::Hook(Pass)` with the single-source
// guard `"actor == owner && is_start(tile.id)"`.
// Isolation: the turn-start pot placement (clause (2)) is a different entry.
// Seat-symmetric (the user's own fire).
// ===========================================================================

/// In-place 2-seat world for the Pass leaf: fire pots symbolic under the
/// hold-cap model (`charge_max = 2` = `FIRE_CAP_SUMIMI`).
fn hina_pass_world(user: i32, tile: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(hina_pass_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.trig_tile = tile;
        w.out[user as usize] = false;
        for p in 0..MAX_PLAYERS {
            w.charge[p] = kani::any();
            kani::assume(w.charge[p] >= 0 && w.charge[p] <= 8);
            w.charge_max[p] = spec::sumimi::FIRE_CAP_SUMIMI;
        }
    });
}

/// Shared iff at a **concrete** user seat.
fn hina_01_pass_circle_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    // Board slot 0 is 「CiRCLE」 (board.json index 1). Symbolic over the
    // 60-tile board so both the circle and non-circle covers are live.
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    hina_pass_world(user, tile);

    // single-source guard: the real CEL string, lowered once
    let gid = crate::guard_id!("actor == owner && is_start(tile.id)");
    let ov = ir_fill::TriggerOverlay::default();
    let admitted = world::with_world(|w| {
        let ctx = ir_fill::fill_for_guard(w, gid, user, 0, true, -1, &ov);
        crate::guard!(gid, &ctx)
    });
    kani::cover!(admitted, "guard admits (on a CiRCLE tile)");
    kani::cover!(!admitted, "guard rejects (not CiRCLE / not owner)");

    let pre = world::snapshot();
    if !admitted {
        // Host would not call the body. Untouched.
        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "rejected hook must not touch fire");
        let (logbuf, log_n) = log_slice();
        crate::kassert!(log_n == 0, "rejected hook must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    let pre_fire = pre.charge[user as usize];
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    kani::cover!(pre_fire < cap, "under cap (gain applies)");
    kani::cover!(pre_fire >= cap, "at/over cap (global hold-cap forbids)");

    // DIRECT body call (ppcost).
    invoke::unwrap_done(skill_personas::uika_idol::on_pass_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::sumimi::pass_circle_fire_ok(effects, user, pre_fire, cap),
        "C-SUMIMI-hina-01: effect log must be exactly +1 fire under the cap"
    );
    crate::kassert!(
        post.charge[user as usize] == spec::sumimi::pass_circle_fire_post(pre_fire, cap),
        "C-SUMIMI-hina-01: post fire must be pre+1 under the cap and unchanged at the cap"
    );
    for p in 0..MAX_PLAYERS {
        if p != user as usize {
            crate::kassert!(post.charge[p] == pre.charge[p], "frame: other players' fire");
        }
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
    }
}

/// Iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn hina_01_pass_circle_iff() {
    hina_01_pass_circle_iff_at(0);
}

/// Same iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn hina_01_user1_pass_circle_iff() {
    hina_01_pass_circle_iff_at(1);
}

/// Mutant: +2 instead of +1 (under the cap).
#[kani::proof]
#[kani::unwind(32)]
pub fn hina_01_mutant_is_detected() {
    hina_pass_world(0, 0); // CiRCLE
    let user = 0i32;
    world::with_world_mut(|w| w.charge[0] = 0);
    let pre = world::snapshot();
    invoke::unwrap_done(skill_personas::uika_idol::on_pass_kani(user));
    let post = world::snapshot();
    let wrong = spec::sumimi::pass_circle_fire_post_mutant(
        pre.charge[user as usize],
        spec::sumimi::FIRE_CAP_SUMIMI,
    );
    crate::kassert!(
        post.charge[user as usize] != wrong,
        "mutant: card must not match the +2 spec"
    );
}

// ===========================================================================
// C-SUMIMI-childhood-02 -- after-move +1 [火罐]
// `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` `On::Hook(MoveAfter)`.
// Guard: `"actor == owner && move.main && card.placed"`.
// C-SUMIMI-childhood-01 (move start = 小豆岛) is PROTOCOL-LEVEL (plan stub).
// Seat-symmetric (the user's own fire).
// ===========================================================================

/// In-place 2-seat world for the after-move leaf.
fn childhood_after_move_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(childhood_after_move_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        for p in 0..MAX_PLAYERS {
            w.charge[p] = kani::any();
            kani::assume(w.charge[p] >= 0 && w.charge[p] <= 8);
            w.charge_max[p] = spec::sumimi::FIRE_CAP_SUMIMI;
        }
    });
}

/// Shared iff at a **concrete** user seat.
fn childhood_02_fire_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    childhood_after_move_world(user);

    let gid = crate::guard_id!("actor == owner && move.main && card.placed");
    let mut ov = ir_fill::TriggerOverlay::default();
    ov.move_main = true;
    let admitted = world::with_world(|w| {
        let ctx = ir_fill::fill_for_guard(w, gid, user, 0, true, -1, &ov);
        crate::guard!(gid, &ctx)
    });
    kani::cover!(admitted, "guard admits");
    kani::cover!(!admitted, "guard rejects");

    let pre = world::snapshot();
    if !admitted {
        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "rejected hook must not touch fire");
        let (logbuf, log_n) = log_slice();
        crate::kassert!(log_n == 0, "rejected hook must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    let pre_fire = pre.charge[user as usize];
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    kani::cover!(pre_fire < cap, "under cap");
    kani::cover!(pre_fire >= cap, "at cap");

    // DIRECT body call (ppcost).
    invoke::unwrap_done(card_sumimi::childhood_cheer::after_move_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::sumimi::childhood_fire_ok(effects, user, pre_fire, cap),
        "C-SUMIMI-childhood-02: after-move must be exactly +1 fire under the cap"
    );
    crate::kassert!(
        post.charge[user as usize] == spec::sumimi::childhood_fire_post(pre_fire, cap),
        "C-SUMIMI-childhood-02: post fire"
    );
    for p in 0..MAX_PLAYERS {
        if p != user as usize {
            crate::kassert!(post.charge[p] == pre.charge[p], "frame: other fire");
        }
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
    }
}

/// Iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn childhood_02_fire_iff() {
    childhood_02_fire_iff_at(0);
}

/// Same iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn childhood_02_user1_fire_iff() {
    childhood_02_fire_iff_at(1);
}

/// Mutant: after-move is +1, not +2.
#[kani::proof]
#[kani::unwind(32)]
pub fn childhood_02_mutant_is_detected() {
    childhood_after_move_world(0);
    let user = 0i32;
    world::with_world_mut(|w| w.charge[0] = 0);
    invoke::unwrap_done(card_sumimi::childhood_cheer::after_move_kani(user));
    let post = world::snapshot();
    let wrong = spec::sumimi::childhood_fire_post(0, spec::sumimi::FIRE_CAP_SUMIMI) + 1;
    crate::kassert!(
        post.charge[user as usize] != wrong,
        "mutant: after-move is +1, not +2"
    );
}

// ===========================================================================
// C-SUMIMI-twodonuts-01 -- 「（1）获得[除外]…」
// `Sumimi:一人两个甜甜圈` play body.
// (2)/(3) are PROTOCOL-LEVEL (multi-act pass window).
// Seat-symmetric (the user's own exile grant).
// ===========================================================================

/// In-place 2-seat world for the play body. Position symbolic so the exile
/// target tile is live.
fn two_donuts_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(two_donuts_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        let pos: i32 = kani::any();
        kani::assume(pos >= 0 && pos < 60);
        w.pos[user as usize] = pos;
    });
}

/// Shared iff at a **concrete** user seat.
fn twodonuts_01_exile_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    two_donuts_world(user);
    let pre = world::snapshot();
    // DIRECT body call (ppcost).
    invoke::unwrap_done(card_sumimi::two_donuts::two_donuts_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::sumimi::twodonuts_exile_ok(effects, user),
        "C-SUMIMI-twodonuts-01: exactly one [除外] grant and no money/pos/fire side effects"
    );
    for p in 0..MAX_PLAYERS {
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
        crate::kassert!(post.charge[p] == pre.charge[p], "frame: fire");
    }
}

/// Iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn twodonuts_01_exile_iff() {
    twodonuts_01_exile_iff_at(0);
}

/// Same iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn twodonuts_01_user1_exile_iff() {
    twodonuts_01_exile_iff_at(1);
}

/// Mutant: no exile grant at all. The card does grant — disagreement is live.
#[kani::proof]
#[kani::unwind(32)]
pub fn twodonuts_01_mutant_is_detected() {
    two_donuts_world(0);
    let user = 0i32;
    invoke::unwrap_done(card_sumimi::two_donuts::two_donuts_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let wrong = !spec::sumimi::twodonuts_exile_grant_in(effects, user);
    crate::kassert!(!wrong, "mutant: card does grant [除外]");
}

// ===========================================================================
// C-SUMIMI-twoinone-01b -- fire becomes the other character's 初始火罐数
// `Sumimi:Sumimi是二人一体的` play body.
// C-SUMIMI-twoinone-01 (character identity) is a known engine gap (the
// bound skill is swapped but `player.persona` stays) — the fire half is
// what this harness pins. C-SUMIMI-twoinone-02 is PROTOCOL-LEVEL.
// Seat-symmetric (the user's own character / fire).
// ===========================================================================

/// In-place 2-seat world for the play body. Character identity is the
/// residual the swap reads (`persona_is`).
fn two_in_one_world(user: i32, is_hina: bool) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(two_in_one_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    let name = if is_hina {
        "三角初华（Sumimi）"
    } else {
        "纯田真奈"
    };
    let cid = intern::intern(name).0;
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.out[user as usize] = false;
        w.persona_id[user as usize] = cid;
    });
}

/// Shared iff at a **concrete** user seat.
fn twoinone_01b_fire_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let is_hina: bool = kani::any();
    two_in_one_world(user, is_hina);

    let to = spec::sumimi::twoinone_fire_target(is_hina);
    let fire_key = intern::intern(card_sdk::abi::state_key::CHARGE).0;
    let pre = world::snapshot();

    // DIRECT body call (ppcost).
    invoke::unwrap_done(card_sumimi::two_in_one::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(is_hina, "from 三角初华（Sumimi） → 纯田真奈 fire 1");
    kani::cover!(!is_hina, "from 纯田真奈 → 三角初华（Sumimi） fire 2");

    crate::kassert!(
        spec::sumimi::twoinone_fire_effects_ok(effects, user, fire_key, to),
        "C-SUMIMI-twoinone-01b: fire must be set to the other character's 初始火罐数"
    );
    // Money / positions are outside the swap's frame.
    for p in 0..MAX_PLAYERS {
        crate::kassert!(post.money[p] == pre.money[p], "frame: money");
        crate::kassert!(post.pos[p] == pre.pos[p], "frame: pos");
    }
}

/// Iff at seat 0.
#[kani::proof]
#[kani::unwind(32)]
pub fn twoinone_01b_fire_iff() {
    twoinone_01b_fire_iff_at(0);
}

/// Same iff at seat 1.
#[kani::proof]
#[kani::unwind(32)]
pub fn twoinone_01b_user1_fire_iff() {
    twoinone_01b_fire_iff_at(1);
}

/// Mutant: fire is not set to 初始+1.
#[kani::proof]
#[kani::unwind(32)]
pub fn twoinone_01b_mutant_is_detected() {
    let is_hina: bool = kani::any();
    two_in_one_world(0, is_hina);
    let user = 0i32;
    let to = spec::sumimi::twoinone_fire_target(is_hina);
    let wrong = to + 1;
    let fire_key = intern::intern(card_sdk::abi::state_key::CHARGE).0;
    invoke::unwrap_done(card_sumimi::two_in_one::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let mut saw_wrong = false;
    for e in effects {
        if let Effect::SetState { player: p, key, n } = e {
            if *p == user && *key == fire_key && *n == wrong {
                saw_wrong = true;
            }
        }
    }
    crate::kassert!(!saw_wrong, "mutant: fire is not set to 初始+1");
}

// ===========================================================================
// Cheap pure-formula harnesses (spec math, no host invoke) — run these first.
// ===========================================================================

/// C-SUMIMI-idolband money formula.
#[kani::proof]
#[kani::unwind(8)]
pub fn idolband_money_formula() {
    let pre: i32 = kani::any();
    kani::assume(pre >= 0 && pre <= 10_000);
    let post = spec::sumimi::idolband_post_money(pre);
    if pre >= 3000 {
        crate::kassert!(post == pre, "no-effect region keeps money");
    } else {
        crate::kassert!(post == 3000, "under-3000 region sets exactly 3000");
    }
    let wrong = spec::sumimi::idolband_post_money_mutant(pre);
    if pre < 3000 {
        crate::kassert!(wrong == 2999 && post != wrong, "mutant 2999 is distinguishable");
    }
    kani::cover!(pre < 3000, "under 3000");
    kani::cover!(pre >= 3000, "at/over 3000");
}

/// C-SUMIMI-mana-01 / hina-01 fire formula under the hold cap.
#[kani::proof]
#[kani::unwind(8)]
pub fn pass_circle_fire_formula() {
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 0 && pre_fire <= 8);
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    let post = spec::sumimi::pass_circle_fire_post(pre_fire, cap);
    if pre_fire < cap {
        crate::kassert!(post == pre_fire + 1, "under cap: +1");
    } else {
        crate::kassert!(post == pre_fire, "at cap: no-op (global hold-cap)");
    }
    let wrong = spec::sumimi::pass_circle_fire_post_mutant(pre_fire, cap);
    crate::kassert!(post != wrong, "mutant +2 / past-cap is distinguishable");
    kani::cover!(pre_fire < cap, "under cap");
    kani::cover!(pre_fire >= cap, "at cap");
}

/// C-SUMIMI-band-02 rent-bonus formula.
#[kani::proof]
#[kani::unwind(8)]
pub fn band_02_rent_bonus_formula() {
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    let post = spec::sumimi::band_rent_bonus_amount(pre_amount, crystals);
    crate::kassert!(post == pre_amount + crystals * 100, "bonus is +100 per crystal");
    let wrong = spec::sumimi::band_rent_bonus_amount_mutant(pre_amount, crystals);
    if crystals != 1 {
        crate::kassert!(post != wrong, "mutant +100-total distinguishable when c != 1");
    }
    kani::cover!(crystals == 0, "c=0");
    kani::cover!(crystals >= 2, "c>=2");
}