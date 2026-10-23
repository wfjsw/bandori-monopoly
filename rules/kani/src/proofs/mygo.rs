//! Unit `mygo`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op + covers + mutant.
//! Specs live in `crate::spec::mygo` and come from the clause docs only.
//!
//! **ppcost pattern.** Concrete user seats (`n_players = 2`, user=0/1 for
//! seat-symmetric clauses). Direct body call via the cfg(kani) `NAME_kani`
//! wrapper. Per-harness host-op subset (`host_subset!`) — undeclared op
//! FAILS loudly. Isolation assumes are named here and `kani::assume`d.
//! Narration (`ctx::log`) is not part of the frame.
//!
//! Crystals of the just-placed instance live on `field[last_uid]` /
//! `self_crystals` (`arm_set_counter_self` routes the `crystals` key there),
//! **not** on the per-player key map.

use crate::host;
use crate::invoke;
use crate::spec::mygo as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-card host-op subsets (ops the unmodified body reaches, including
// isolated paths — if isolation breaks the proof FAILS loudly).
// ---------------------------------------------------------------------------

/// `MyGO:轮符雨` play — +1 [停留] + schedule turn-end settle + narration.
/// `before_turn_end` is `Op::schedule_turn_end` (a plan stub).
crate::host_subset!(rinne_rain_dispatch {
    apply_abnormal => arm_apply_abnormal,
    schedule_turn_end => arm_stub_zero,
    log | effect => arm_log,
});

/// `MyGO:哪怕这旅程没有终点` play — place on current tile + 4 crystals.
/// `place_card_on` (NOT `place_card_at`: name at `(a2,a3)`, tile at `a1`).
crate::host_subset!(endless_journey_dispatch {
    set_dest => arm_set_dest,
    place_card_on => arm_place_card_on,
    player_pos => arm_player_pos,
    set_counter_self => arm_set_counter_self,
    log | effect => arm_log,
});

/// `MyGO:羽丘的不可思议女孩` play — own-tile gate, 1d20, house / draw / place.
crate::host_subset!(haneoka_dispatch {
    player_pos => arm_player_pos,
    tile_owner => arm_tile_owner,
    roll_ask => arm_roll_ask,
    mortgaged_of => arm_mortgaged_of,
    add_house => arm_add_house,
    draw => arm_draw,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    log | effect => arm_log,
});

/// `MyGO:那天的雨` play — place + 5 crystals + the inlined `rain` leg
/// (1d10 → agent tiles → +1 [停留] to everyone standing in the zone).
/// Placement proofs pin the die to face 1 (主要街道) so the rain `Vec`
/// walk is one agent — the stay leg is PROTOCOL-adjacent (`rainday_02`).
/// Not a silent-zero: a real face of 1 on the 1d10.
crate::host_subset!(that_day_rain_dispatch {
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    set_counter_self => arm_set_counter_self,
    roll => arm_roll_face_one,
    tile_count => arm_tile_count,
    tile_named => arm_tile_named,
    player_count => arm_player_count,
    player_out => arm_player_out,
    player_pos => arm_player_pos,
    apply_abnormal => arm_apply_abnormal,
    log | effect => arm_log,
});

/// `MyGO:壱雫空` play — clear [停留]/[眩晕] then the money legs.
///
/// GAP: `stay_of` / `stun_of` have **no** `arm_*` yet (still inline in
/// `host::dispatch`). Handled locally below so the subset still covers every
/// op the body reaches; the main agent should extract `arm_stay_of` /
/// `arm_stun_of` into `host.rs` and swap this to `host_subset!`.
fn hitoshizuku_dispatch(op: card_sdk::kani_host::Op, a: card_sdk::kani_host::Args) -> i64 {
    use card_sdk::kani_host::Op;
    match op {
        Op::player_count => host::arm_player_count(a),
        Op::player_out => host::arm_player_out(a),
        // Local — see GAP above.
        Op::stay_of => {
            let [a0, ..] = a;
            world::with_world(|w| w.stay[a0 as usize]) as i64
        }
        Op::stun_of => {
            let [a0, ..] = a;
            world::with_world(|w| w.stun[a0 as usize]) as i64
        }
        Op::slot => host::arm_slot(a),
        Op::give_stay => host::arm_apply_abnormal(a),
        Op::give_stun => host::arm_apply_abnormal(a),
        Op::state_get => host::arm_state_get(a),
        Op::state_set => host::arm_state_set(a),
        Op::pay_to => host::arm_pay_to(a),
        Op::gain | Op::gain_typed | Op::gain_fixed => host::arm_gain(a),
        Op::log | Op::effect => host::arm_log(a),
        other => host::outside_subset(other),
    }
}

/// `MyGO:（灯）不再迷茫` play — 2 locked [停留] + place + discard hand + X+1
/// crystals. Isolation: empty hand (the `cards_in` String loop is a no-op).
crate::host_subset!(tomori_no_longer_dispatch {
    apply_abnormal => arm_apply_abnormal,
    state_add | inc_slot => arm_state_add,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    cards_in => arm_cards_in,
    discard_from_hand => arm_discard_from_hand,
    set_counter_self => arm_set_counter_self,
    log | effect => arm_log,
});

// ---------------------------------------------------------------------------
// In-place 2-seat worlds. No `base_world`, no `reset_world` full copy.
// ---------------------------------------------------------------------------

/// Sparse 2-seat play world + concrete `trig_player = user`.
fn sparse2_user(dispatch: card_sdk::kani_host::Backend, user: i32) {
    host::install_backend(dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
    });
}

/// Miracle crystals of the just-placed instance (`set_counter_self` routes the
/// `crystals` key to `field[last_uid]`, falling back to `self_crystals`).
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

fn effects_of() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

// ===========================================================================
// C-MYGO-rinpuu-01
// 「使自己获得一层[停留]」 — `MyGO:轮符雨` On::Play.
// Seat-symmetric. Isolation: `before_turn_end` is a scheduling stub.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn rinpuu_01_give_stay_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2_user(rinne_rain_dispatch, user);
    let pre = world::snapshot();
    // DIRECT body call (ppcost): one body in CBMC's call graph.
    invoke::unwrap_done(card_mygo::rinne_rain::rinne_rain_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    crate::kassert!(
        spec::rinpuu_01_log_ok(effects, user),
        "C-MYGO-rinpuu-01: effect log must be exactly one GiveStay(user, 1)"
    );
    crate::kassert!(
        spec::rinpuu_01_post_ok(&pre, &post, user),
        "C-MYGO-rinpuu-01: post must be pre + 1 stay on the user, frame unchanged"
    );

    kani::cover!(pre.stay[user as usize] == 0, "pre stay was 0");
    kani::cover!(pre.stay[user as usize] > 0, "pre stay was nonzero");
}

/// Iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rinpuu_01_give_stay_iff() {
    rinpuu_01_give_stay_iff_at(0);
}

/// Iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rinpuu_01_user1_give_stay_iff() {
    rinpuu_01_give_stay_iff_at(1);
}

/// Mutant companion: a two-layer spec must be distinguishable from the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rinpuu_01_mutant_is_detected() {
    let user = 0i32;
    sparse2_user(rinne_rain_dispatch, user);
    invoke::unwrap_done(card_mygo::rinne_rain::rinne_rain_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    // The card does n=1; the mutant wants n=2. They must disagree.
    crate::kassert!(
        !spec::rinpuu_01_log_ok_mutant(effects, user),
        "mutant: two-layer spec must not match the one-layer card"
    );
}

/// `#[kani::should_panic]` twin of the mutant companion.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn rinpuu_01_mutant_two_layers() {
    let user = 0i32;
    sparse2_user(rinne_rain_dispatch, user);
    invoke::unwrap_done(card_mygo::rinne_rain::rinne_rain_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::rinpuu_01_log_ok_mutant(effects, user),
        "mutant: card must not match the two-layer spec"
    );
}

// ===========================================================================
// C-MYGO-rinpuu-02 is PROTOCOL-LEVEL: 「并在回合结束时额外进行一次[触发结算]」
// is engine scheduling (`On::AtEnd` + `card_settle_at`, a protocol stub).
// Documented here so the unit table can cite a harness name.
// ===========================================================================

// ===========================================================================
// C-MYGO-journey-01
// 「[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶」
// `MyGO:哪怕这旅程没有终点` On::Play.
// Seat-symmetric. Host note: `place_card_on` logs `PlaceCard { dest }` with
// `dest` = the landing **tile index** (not `Dest::Field`); `set_crystals(4)`
// is `set_counter_self` and logs one `AddCounter { n: 4 }` on the instance.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn journey_01_place_four_crystals_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2_user(endless_journey_dispatch, user);
    let pre = world::snapshot();
    let tile = pre.pos[user as usize];
    invoke::unwrap_done(card_mygo::endless_journey::endless_journey_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();
    let field_n = world::with_world(|ww| ww.field_n);

    // Host `card_slot` maps unknown card names to slot 2 (pilot CARD_OTHER).
    crate::kassert!(
        spec::journey_01_log_ok(effects, user, 2, tile),
        "C-MYGO-journey-01: PlaceCard on the current tile + AddCounter(4)"
    );
    crate::kassert!(
        post_c == 4,
        "C-MYGO-journey-01: card must hold exactly 4 crystals"
    );
    crate::kassert!(
        field_n == 1,
        "C-MYGO-journey-01: exactly one field instance"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money unchanged"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    crate::kassert!(post.stay == pre.stay, "frame: stay unchanged");
    crate::kassert!(
        world::arr8b_eq(post.hand, pre.hand),
        "frame: hand unchanged (play body does not draw)"
    );

    kani::cover!(pre.stay[user as usize] == 0, "clean pre");
}

/// Iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn journey_01_place_four_crystals_iff() {
    journey_01_place_four_crystals_iff_at(0);
}

/// Iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn journey_01_user1_place_four_crystals_iff() {
    journey_01_place_four_crystals_iff_at(1);
}

/// Mutant companion: a two-PlaceCard spec must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn journey_01_mutant_is_detected() {
    let user = 0i32;
    sparse2_user(endless_journey_dispatch, user);
    let tile = world::with_world(|w| w.pos[user as usize]);
    invoke::unwrap_done(card_mygo::endless_journey::endless_journey_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::journey_01_log_ok_mutant(effects, user, 2, tile),
        "mutant: two-PlaceCard spec must not match the one-place card"
    );
}

// ===========================================================================
// C-MYGO-haneoka-02/03/04/05
// `MyGO:羽丘的不可思议女孩` On::Play. Gated on owning the current tile
// (`cant_play` / body early-out). Roll 1d20 is fully symbolic under Kani.
// Seat-symmetric (「属于自己的格子」). Isolation: the tile is owned by the
// user and unmortgaged so `add_house` fires on the >=10 band. `pos` stays at
// the sparse default 0 — the user owns tile 0.
// ===========================================================================

/// Own-tile + unmortgaged world for the haneoka thresholds.
fn haneoka_world(user: i32) {
    sparse2_user(haneoka_dispatch, user);
    world::with_world_mut(|w| {
        let pos = w.pos[user as usize] as usize;
        w.tiles[pos].owner = user;
        w.tiles[pos].mortgaged = false;
        w.tiles[pos].houses = kani::any();
        kani::assume(w.tiles[pos].houses >= 0 && w.tiles[pos].houses <= 4);
    });
}

/// Shared iff at a **concrete** user seat, over every 1d20 face:
/// r<10 empty log; r>=10 SetHouses; r>=15 +Draw; r>=20 +PlaceCard.
fn haneoka_thresholds_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    haneoka_world(user);
    let pre = world::snapshot();
    let pos = pre.pos[user as usize];
    let pre_houses = world::with_world(|w| w.tiles[pos as usize].houses);
    invoke::unwrap_done(card_mygo::haneoka::haneoka_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let post_houses = world::with_world(|ww| ww.tiles[pos as usize].houses);

    // Recover the face from the effects: <10 → empty; else look at the chain.
    // We don't have the face in the log, so branch on the observed shape and
    // cover each band via the house/draw/place presence.
    let has_house = effects
        .iter()
        .any(|e| matches!(e, Effect::SetHouses { .. }));
    let has_draw = effects.iter().any(|e| matches!(e, Effect::Draw { .. }));
    let has_place = effects
        .iter()
        .any(|e| matches!(e, Effect::PlaceCard { .. }));

    kani::cover!(
        !has_house && !has_draw && !has_place,
        "r < 10 (ineffective)"
    );
    kani::cover!(has_house && !has_draw && !has_place, "10..=14 (house only)");
    kani::cover!(has_house && has_draw && !has_place, "15..=19 (house+draw)");
    kani::cover!(
        has_house && has_draw && has_place,
        "r >= 20 (house+draw+place)"
    );

    // Shape consistency: draw only with house; place only with draw.
    if has_draw {
        crate::kassert!(
            has_house,
            "C-MYGO-haneoka: draw implies house (r>=15 ⊇ r>=10)"
        );
    }
    if has_place {
        crate::kassert!(
            has_draw,
            "C-MYGO-haneoka: place implies draw (r>=20 ⊇ r>=15)"
        );
        crate::kassert!(has_house, "C-MYGO-haneoka: place implies house");
    }
    if has_house {
        crate::kassert!(
            post_houses == pre_houses.saturating_add(1),
            "C-MYGO-haneoka-02: exactly one free house"
        );
        crate::kassert!(
            world::arr4_eq(post.money, pre.money),
            "C-MYGO-haneoka-02: 免费 — money unchanged"
        );
    } else {
        crate::kassert!(
            post_houses == pre_houses,
            "C-MYGO-haneoka-05: no house on r<10"
        );
        crate::kassert!(
            world::arr4_eq(post.money, pre.money),
            "frame: money unchanged"
        );
    }
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    crate::kassert!(post.stay == pre.stay, "frame: stay unchanged");
}

/// Thresholds iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn haneoka_thresholds_iff() {
    haneoka_thresholds_iff_at(0);
}

/// Thresholds iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn haneoka_user1_thresholds_iff() {
    haneoka_thresholds_iff_at(1);
}

/// Mutant: the ineffective band must not match a one-effect spec.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn haneoka_05_mutant_is_detected() {
    let user = 0i32;
    haneoka_world(user);
    let pos = world::with_world(|w| w.pos[user as usize]);
    // Force the ineffective band by assuming the symbolic die is < 10 is NOT
    // possible to force post-hoc; instead just run and check: if the log is
    // empty the mutant (wants 1 effect) must disagree.
    invoke::unwrap_done(card_mygo::haneoka::haneoka_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    if effects.is_empty() {
        crate::kassert!(
            !spec::haneoka_effects_ok_mutant(effects, user, pos, 5, 2),
            "mutant: empty-log ineffective band must not match a 1-effect spec"
        );
    }
}

// ===========================================================================
// C-MYGO-rainday-01
// 「将此卡放置于自己场上并为其放置5个奇迹水晶」
// `MyGO:那天的雨` On::Play. The play body also runs the -02 rain; that leg's
// GiveStay effects follow the PlaceCard. This harness checks the placement +
// crystal count; `rainday_02` covers the stay leg (PROTOCOL-adjacent — the
// rolled zone is board-wide).
// Seat-symmetric.
// ===========================================================================

/// Shared iff at a **concrete** user seat: PlaceCard(Field) first; crystals
/// key == 5; rain may add GiveStays. Die is pinned to face 1 (see dispatch).
fn rainday_01_place_five_crystals_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2_user(that_day_rain_dispatch, user);
    // Pin seats so both rain covers are reachable: `user` stands on the
    // face-1 agent tile 「主要街道」 (in the colour+neighbour area); the
    // other seat stays on CiRCLE (tile 0).
    let agent_tile = crate::board::tile_named("主要街道");
    world::with_world_mut(|w| {
        w.pos[user as usize] = if agent_tile >= 0 { agent_tile } else { 0 };
        w.pos[1 - user as usize] = 0;
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_mygo::that_day_rain::that_day_rain_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();

    crate::kassert!(
        spec::rainday_01_place_ok(effects, user, 2),
        "C-MYGO-rainday-01: first effect must be PlaceCard(Field)"
    );
    crate::kassert!(
        post_c == 5,
        "C-MYGO-rainday-01: card must hold exactly 5 crystals"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money unchanged"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");

    let mut stayed = false;
    let mut i = 0usize;
    let len = effects.len().min(24);
    while i < len {
        if matches!(effects[i], Effect::GiveStay { .. }) {
            stayed = true;
        }
        i += 1;
    }
    kani::cover!(stayed, "rain leg granted stay");
    kani::cover!(!stayed, "rain leg granted no stay");
}

/// Placement iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rainday_01_place_five_crystals_iff() {
    rainday_01_place_five_crystals_iff_at(0);
}

/// Placement iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rainday_01_user1_place_five_crystals_iff() {
    rainday_01_place_five_crystals_iff_at(1);
}

/// Mutant: 4 crystals must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn rainday_01_mutant_is_detected() {
    let user = 0i32;
    sparse2_user(that_day_rain_dispatch, user);
    invoke::unwrap_done(card_mygo::that_day_rain::that_day_rain_kani(user));
    kani::cover!(true, "body returned");
    let post_c = self_crystals();
    crate::kassert!(
        post_c != spec::rainday_01_crystals_mutant(),
        "mutant: 4-crystal spec must not match the 5-crystal card"
    );
}

// ===========================================================================
// C-MYGO-hitoshizuku-01/02/03
// `MyGO:壱雫空` On::Play.
// Seat-symmetric on the user (the payee / extra-gainer).
// Isolation: `lockedStay` = 0 (locked layers survive the clear). Stay / stun
// start symbolic per seat so the clear prefix is reachable.
// Book vs body: both are per-player kinds (ruling 2026-10-06). The old
// "body charges global kinds" note is stale — see `spec::mygo`.
// ===========================================================================

/// Concrete stay / stun on the two live seats (lockedStay stays 0).
///
/// Pattern: seat 0 has stay+stun (kinds=2), seat 1 has stay only (kinds=1).
/// Concrete values keep `cleared.len()` a constant 2, so the body's
/// `for &(_,_) in &cleared` unrolls twice — a symbolic-length `Vec` iterator
/// is an SSA cliff (`Not unwinding … iteration 32` at
/// `hitoshizuku.rs:92`).
fn hitoshizuku_world(user: i32) {
    sparse2_user(hitoshizuku_dispatch, user);
    world::with_world_mut(|w| {
        w.stay = [1, 1, 0, 0];
        w.stun = [1, 0, 0, 0];
    });
}

/// Recover per-player kinds from the clear prefix: GiveStay with negative n
/// is the stay clear (1 kind); GiveStun with nonzero n is the stun clear.
/// Bounded while (slice iterators are an SSA cliff).
fn hitoshizuku_kinds(effects: &[Effect]) -> [i32; MAX_PLAYERS] {
    let mut kinds = [0i32; MAX_PLAYERS];
    let mut i = 0usize;
    let len = effects.len().min(24);
    while i < len {
        match &effects[i] {
            Effect::GiveStay { player: p, n } if *n < 0 => {
                if (*p as usize) < MAX_PLAYERS {
                    kinds[*p as usize] += 1;
                }
            }
            Effect::GiveStun { player: p, n } if *n != 0 => {
                if (*p as usize) < MAX_PLAYERS {
                    kinds[*p as usize] += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    kinds
}

fn kinds_sum(kinds: &[i32; MAX_PLAYERS]) -> i32 {
    let mut s = 0;
    let mut i = 0;
    while i < MAX_PLAYERS {
        s += kinds[i];
        i += 1;
    }
    s
}

/// Shared iff against the **book** money rule at a concrete user seat.
/// Body matches the book (per-player kinds) since the 2026-10-06 ruling.
fn hitoshizuku_money_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    hitoshizuku_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_mygo::hitoshizuku::hitoshizuku_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    let kinds = hitoshizuku_kinds(effects);
    let global_kinds = kinds_sum(&kinds);
    kani::cover!(global_kinds > 0, "something cleared");
    kani::cover!(
        global_kinds > 0 && kinds[0] != kinds[1],
        "uneven per-player kinds (book ≠ global mutant)"
    );

    if global_kinds == 0 {
        // no money leg — bounded scan, no iterator `any`
        let mut money = false;
        let mut i = 0usize;
        let len = effects.len().min(24);
        while i < len {
            if matches!(effects[i], Effect::PayTo { .. } | Effect::GainMoney { .. }) {
                money = true;
            }
            i += 1;
        }
        crate::kassert!(!money, "C-MYGO-hitoshizuku: no clear ⇒ no money");
    } else {
        let book_ok = spec::hitoshizuku_money_ok(effects, user, &kinds, pre.n_players);
        crate::kassert!(
            book_ok,
            "C-MYGO-hitoshizuku-02: book wants 1000 per type per affected player"
        );
    }

    // Stay never rises — bounded compare (zip/all is an SSA cliff).
    let mut stay_ok = true;
    let mut p = 0usize;
    while p < MAX_PLAYERS {
        if post.stay[p] > pre.stay[p] {
            stay_ok = false;
        }
        p += 1;
    }
    crate::kassert!(stay_ok, "C-MYGO-hitoshizuku-01: stay never rises");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
}

/// Book-spec iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn hitoshizuku_money_iff() {
    hitoshizuku_money_iff_at(0);
}

/// Book-spec iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn hitoshizuku_user1_money_iff() {
    hitoshizuku_money_iff_at(1);
}

/// Companion: the stale global-kinds reading is a **mutant**. When per-player
/// kinds are uneven the body must not charge `1000 * sum(kinds)` from every
/// living seat. (Keeps the old harness names so the central pass still finds
/// a row here; the assertion is now "mutant is not what the body does".)
fn hitoshizuku_body_global_pay_observed_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    hitoshizuku_world(user);
    invoke::unwrap_done(card_mygo::hitoshizuku::hitoshizuku_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let kinds = hitoshizuku_kinds(effects);
    let global_kinds = kinds_sum(&kinds);
    // Only the uneven case separates the book from the global mutant.
    let uneven = global_kinds > 0 && kinds[0] != kinds[1];
    kani::cover!(uneven, "uneven kinds (mutant distinguishable)");
    if uneven {
        let pre = world::snapshot();
        let body_global =
            spec::hitoshizuku_body_global_pay_ok(effects, user, global_kinds, pre.n_players);
        crate::kassert!(
            !body_global,
            "mutant: body must not charge 1000 * global kinds when per-player kinds differ"
        );
    }
}

/// Global-kinds mutant is not what the body does (seat 0).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn hitoshizuku_body_global_pay_observed() {
    hitoshizuku_body_global_pay_observed_at(0);
}

/// Global-kinds mutant is not what the body does (seat 1).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn hitoshizuku_user1_body_global_pay_observed() {
    hitoshizuku_body_global_pay_observed_at(1);
}

/// No-clear limb: with stay=stun=0 the body logs no money effects.
fn hitoshizuku_no_clear_is_noop_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    sparse2_user(hitoshizuku_dispatch, user);
    world::with_world_mut(|w| {
        w.stay = [0; MAX_PLAYERS];
        w.stun = [0; MAX_PLAYERS];
    });
    invoke::unwrap_done(card_mygo::hitoshizuku::hitoshizuku_kani(user));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let mut money = false;
    let mut i = 0usize;
    let len = effects.len().min(24);
    while i < len {
        if matches!(effects[i], Effect::PayTo { .. } | Effect::GainMoney { .. }) {
            money = true;
        }
        i += 1;
    }
    crate::kassert!(!money, "C-MYGO-hitoshizuku: no clear ⇒ no money");
}

/// No-clear limb at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn hitoshizuku_no_clear_is_noop() {
    hitoshizuku_no_clear_is_noop_at(0);
}

// ===========================================================================
// C-MYGO-tomori-card-01/02
// `MyGO:（灯）不再迷茫` On::Play.
// Seat-symmetric. Isolation: empty hand at pre (X=0 ⇒ crystals=1) so the
// `cards_in` String loop is a no-op. lockedStay is a slot write (no Effect);
// set_crystals(1) is `set_counter_self` and logs one AddCounter.
// ===========================================================================

/// Empty-hand world (sparse default already has `hand_resonance = 0` and an
/// empty `hand` bitset — the `cards_in(Hand)` view is empty).
fn tomori_world(user: i32) {
    sparse2_user(tomori_no_longer_dispatch, user);
}

/// Shared iff at a **concrete** user seat: GiveStay(user, 2) + PlaceCard(Field)
/// + AddCounter(1); crystals == 1 (X=0).
fn tomori_card_02_empty_hand_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    tomori_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_mygo::tomori_no_longer::tomori_no_longer_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();
    let locked = world::with_world(|ww| ww.key_get(user, crate::intern::intern("lockedStay")));

    crate::kassert!(
        spec::tomori_card_02_empty_hand_log_ok(effects, user, 2),
        "C-MYGO-tomori-card-02: empty-hand log must be GiveStay(2) + PlaceCard + AddCounter(1)"
    );
    crate::kassert!(
        post_c == spec::tomori_card_02_crystals(0),
        "C-MYGO-tomori-card-02: X=0 ⇒ exactly 1 crystal"
    );
    crate::kassert!(
        locked >= 2,
        "C-MYGO-tomori-card-01: lockedStay marks the 2 layers"
    );
    crate::kassert!(
        post.stay[user as usize] == pre.stay[user as usize].saturating_add(2),
        "C-MYGO-tomori-card-01: exactly 2 stay layers"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money unchanged"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    crate::kassert!(
        world::arr8b_eq(post.hand, pre.hand),
        "frame: hand unchanged (empty)"
    );
}

/// Empty-hand iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn tomori_card_02_empty_hand_iff() {
    tomori_card_02_empty_hand_iff_at(0);
}

/// Empty-hand iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn tomori_card_02_user1_empty_hand_iff() {
    tomori_card_02_empty_hand_iff_at(1);
}

/// Mutant: 1 stay layer must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn tomori_card_02_mutant_is_detected() {
    let user = 0i32;
    tomori_world(user);
    invoke::unwrap_done(card_mygo::tomori_no_longer::tomori_no_longer_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::tomori_card_02_empty_hand_log_ok_mutant(effects, user, 2),
        "mutant: 1-stay spec must not match the 2-stay card"
    );
}
