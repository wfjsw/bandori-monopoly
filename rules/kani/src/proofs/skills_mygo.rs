//! Unit `skills_mygo`: clause proofs.
//!
//! Band skill `skill:MyGO!!!!!:迷途之星` and the five MyGO!!!!! character
//! skills. Per clause: iff + no-op + covers + mutant (see KANI-FOUNDATION.md).
//! Specs live in `crate::spec::skills_mygo`.
//!
//! **ppcost pattern.** Concrete user seats (n_players=2). Direct body call
//! via the cfg(kani) `NAME_kani` wrapper. Per-harness host-op subset.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Host-op subsets (body-reachable ops).
// ---------------------------------------------------------------------------

/// `after_roll` (DiceRollAfter) — add_crystals(1,1) + log.
crate::host_subset!(mygo_after_roll_dispatch {
    add_counter_self => arm_add_counter_self,
    add_card_counter => arm_add_card_counter,
    log | effect => arm_log,
});

/// `on_discarded` (Discarded) — crystals read + state + add_crystals.
crate::host_subset!(mygo_on_discarded_dispatch {
    counter_self => arm_counter_self,
    card_counter => arm_card_counter,
    add_counter_self => arm_add_counter_self,
    add_card_counter => arm_add_card_counter,
    state_get => arm_state_get,
    state_set => arm_state_set,
    log | effect => arm_log,
});

fn after_roll_world(user: i32) {
    host::install_backend(mygo_after_roll_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.stay[p] = 0;
            w.charge[p] = 0;
        }
        w.trig_player = user;
    });
}

fn on_discarded_world(user: i32, pre_c: i32) {
    host::install_backend(mygo_on_discarded_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.stay[p] = 0;
            w.charge[p] = 0;
        }
        w.trig_player = user;
        // Crystals live on the on-card counter (`field_crystals` /
        // `self_crystals` via `arm_add_counter_self` / `arm_counter_self`),
        // **not** the player key map (class B — same fix as `events.rs`
        // `key_get(0, "crystals")` and `skills_pp` band-idol-04).
        w.self_crystals = pre_c;
    });
}

/// Miracle crystals of the skill instance (`field_crystals(last_uid)`).
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

// ===========================================================================
// C-MYGO-band-02
// 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
// Hook entry index 1 (`On::Action(DiceRollAfter)`). Seat-symmetric (「为此卡」).
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn band_02_crystal_on_16_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    after_roll_world(user);
    // Seed the on-card crystal counter (see `self_crystals`).
    world::with_world_mut(|w| w.self_crystals = kani::any());
    kani::assume(self_crystals() >= 0 && self_crystals() <= 3);
    let condition = world::snapshot();
    let pre_c = self_crystals();

    invoke::unwrap_done(skill_groups::mygo::after_roll_kani(user));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();

    // `add_crystals(1, 1)` clamps at max=1. Host `add_counter_self` logs the
    // clamped delta (`now - was`) — +1 when the gain lands, 0 at the cap.
    crate::kassert!(
        spec::skills_mygo::band_02_log_ok(effects, user, pre_c),
        "C-MYGO-band-02: effect log is one AddCounter (clamped delta)"
    );
    crate::kassert!(
        post_c == spec::skills_mygo::band_02_crystals_after(pre_c),
        "C-MYGO-band-02: crystals must be min(condition+1, 1) under the roll cap"
    );
    crate::kassert!(
        world::arr4_eq(post.money, condition.money),
        "frame: money unchanged"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, condition.pos),
        "frame: pos unchanged"
    );
    crate::kassert!(post.stay == condition.stay, "frame: stay unchanged");

    kani::cover!(pre_c == 0, "condition crystals 0 (gain lands)");
    kani::cover!(
        pre_c >= 1,
        "condition crystals at cap (gain refused by max)"
    );
}

/// Band-02 iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_crystal_on_16_iff() {
    band_02_crystal_on_16_iff_at(0);
}

/// Band-02 iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_user1_crystal_on_16_iff() {
    band_02_crystal_on_16_iff_at(1);
}

/// Mutant companion: a +2 spec must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_02_mutant_is_detected() {
    after_roll_world(0);
    let user = 0i32;
    invoke::unwrap_done(skill_groups::mygo::after_roll_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::skills_mygo::band_02_log_ok_mutant(effects, user),
        "mutant: +2 spec must not match the +1 card"
    );
}

// ===========================================================================
// C-MYGO-band-04
// 「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]
//  （最多超出2个）」
// Hook entry index 2 (`On::Action(Discarded)`).
// Book: +1 crystal that may exceed the roll cap of 1, at most 2 over (ceiling 3).
// The body's `skill.mygo.over` counter is audit/display only (SetState
// bookkeeping) — the ceiling is the held total, not that key.
// ===========================================================================

/// Shared iff at a **concrete** user seat.
fn band_04_ineffective_overcap_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    let pre_c: i32 = kani::any();
    kani::assume(pre_c >= 0 && pre_c <= 3);
    on_discarded_world(user, pre_c);
    let condition = world::snapshot();
    invoke::unwrap_done(skill_groups::mygo::on_discarded_kani(user));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = self_crystals();

    let expected = spec::skills_mygo::band_04_crystals_after(pre_c);
    kani::cover!(pre_c == 1, "at roll cap (book: must still grow)");
    kani::cover!(pre_c < expected, "gain fires");
    kani::cover!(pre_c >= 3, "at the +2 ceiling (gain refused)");

    crate::kassert!(
        post_c == expected,
        "C-MYGO-band-04: ineffective discard must add one over-cap crystal (max 2 over)"
    );
    if post_c > pre_c {
        crate::kassert!(
            spec::skills_mygo::band_04_log_ok(effects, user),
            "C-MYGO-band-04: gain logs exactly one AddCounter(+1) (SetState audit tolerated)"
        );
    }
    crate::kassert!(
        world::arr4_eq(post.money, condition.money),
        "frame: money unchanged"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, condition.pos),
        "frame: pos unchanged"
    );
}

/// Band-04 iff at seat 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_ineffective_overcap_iff() {
    band_04_ineffective_overcap_iff_at(0);
}

/// Band-04 iff at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_user1_ineffective_overcap_iff() {
    band_04_ineffective_overcap_iff_at(1);
}

/// Mutant: empty log when the gain should fire.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn band_04_mutant_is_detected() {
    on_discarded_world(0, 0);
    let user = 0i32;
    let pre_c = 0; // gain must fire
    invoke::unwrap_done(skill_groups::mygo::on_discarded_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    if log_n > 0 {
        crate::kassert!(
            !spec::skills_mygo::band_04_log_ok_mutant(effects, user),
            "mutant: empty-log spec must not match a gaining card"
        );
    }
    let _ = pre_c;
}

// ===========================================================================
// G13 — `skill:高松灯:诗超绊` (2), borrowed-clause body
//
// Book:
// > （2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
// > [触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]，然后使其投掷
// > 1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；
// > 在此技能影响下的支付价格变为原本的一半。
//
// The `before_settle_applies` guard is exercised elsewhere (dist / once-per-move);
// these harnesses call the body directly after assuming it admitted: `mover !=
// owner` (the (2) arm), the mover's landing `landing`, the owner's own square
// as the destination, and both seats on the board. A borrower is represented by
// an invocation frame with `paid` set — the borrowed path must not ask or spend.
// The roll is pinned with `forced_roll`; the roll itself is not an `Effect`, so
// `roll_calls` counts it. `schedule_turn_end` / `settle_tile_at` are protocol
// stubs, recorded by the host so "exactly one" / "none" stays checkable.
// Narration (`ctx::log`) is not the frame.
// ===========================================================================

const OWED_SETTLE: &str = "skill.tomoriPoem.owedSettle";
const USED_THIS_MOVE: &str = "skill.tomoriPoem.usedThisMove";
const HALF_PRICE: &str = "skill.tomoriPoem.halfPrice";
const BORROWED: &str = "skill.tomoriPoem.borrowed";

/// Ops of a **paid** (2) press: no ask, no raise, no spend. That the body
/// cannot ask on this path is itself checked — an `ask` here is an
/// `outside_subset` FAIL.
crate::host_subset!(tomori_pull_paid_dispatch {
    trig_player => arm_trig_player,
    invoke_frame => arm_invoke_frame,
    state_set => arm_state_set,
    teleport_to => arm_teleport_to,
    player_pos => arm_player_pos,
    roll => arm_roll,
    gate => arm_gate,
    raise_after => arm_raise_after,
    skip_main_move => arm_stub_zero,
    schedule_turn_end => arm_schedule_turn_end,
    log | log_as | effect => arm_log,
});

/// The same plus the offer: still no spend (a declined answer returns first).
crate::host_subset!(tomori_pull_ask_dispatch {
    trig_player => arm_trig_player,
    invoke_frame => arm_invoke_frame,
    ask => arm_ask,
    state_set => arm_state_set,
    teleport_to => arm_teleport_to,
    player_pos => arm_player_pos,
    roll => arm_roll,
    gate => arm_gate,
    raise_after => arm_raise_after,
    skip_main_move => arm_stub_zero,
    schedule_turn_end => arm_schedule_turn_end,
    log | log_as | effect => arm_log,
});

/// The skill's own collect: `raise` opens the `ChargePaying` window,
/// `persona_skill` / `count_held` / `held_max` answer `writable`,
/// `spend_marker` is the dumb spend.
crate::host_subset!(tomori_pull_spend_dispatch {
    trig_player => arm_trig_player,
    invoke_frame => arm_invoke_frame,
    ask => arm_ask,
    raise => arm_raise,
    persona_skill => arm_persona_skill,
    count_held | count_held_name => arm_count_held,
    held_max => arm_tok_max,
    spend_marker => arm_spend_marker,
    state_set => arm_state_set,
    teleport_to => arm_teleport_to,
    player_pos => arm_player_pos,
    roll => arm_roll,
    gate => arm_gate,
    raise_after => arm_raise_after,
    skip_main_move => arm_stub_zero,
    schedule_turn_end => arm_schedule_turn_end,
    log | log_as | effect => arm_log,
});

/// A **paid** (2) body: a borrower already paid, so no ask / raise / spend;
/// the (2) destination is `player_pos(owner)` itself, so no distance scan
/// either. `outside_subset` still guards the cut.
crate::host_subset!(tomori_follow_up2_dispatch {
    trig_player => arm_trig_player,
    invoke_frame => arm_invoke_frame,
    state_set => arm_state_set,
    state_get => arm_state_get,
    state_add => arm_state_add,
    teleport_to => arm_teleport_to,
    player_pos => arm_player_pos,
    roll => arm_roll,
    gate => arm_gate,
    raise_after => arm_raise_after,
    skip_main_move => arm_stub_zero,
    schedule_turn_end => arm_schedule_turn_end,
    log | log_as | effect => arm_log,
});

/// A **paid** (3) body: no roll / stay / schedule at all; only the nearest-
/// within-5 destination scan and the teleport.
crate::host_subset!(tomori_follow_up3_dispatch {
    trig_player => arm_trig_player,
    trig_tile => arm_trig_tile,
    invoke_frame => arm_invoke_frame,
    state_set => arm_state_set,
    teleport_to => arm_teleport_to,
    player_pos => arm_player_pos,
    player_count => arm_player_count,
    player_out => arm_player_out,
    dist => arm_dist,
    log | log_as | effect => arm_log,
});

/// `owed_settle` + its `SettleAfter` body reach: the latch writes, the
/// position read and the settle stub.
crate::host_subset!(tomori_owed_dispatch {
    trig_player => arm_trig_player,
    trig_settle_redirect => arm_trig_settle_redirect,
    state_get => arm_state_get,
    state_set => arm_state_set,
    player_pos => arm_player_pos,
    settle_tile_at => arm_settle_tile_at,
});

fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

fn key_get(p: i32, key: crate::intern::NameId) -> i32 {
    world::with_world(|w| w.key_get(p, key))
}

fn roll_calls() -> u32 {
    world::with_world(|w| w.roll_calls)
}

fn schedules() -> u32 {
    world::with_world(|w| w.schedule_turn_end_n)
}

fn settles() -> u32 {
    world::with_world(|w| w.settle_calls)
}

/// 2-seat world with both seats living and concrete squares (`pos`), then the
/// mover's landing as the trigger tile. No money / fans: the body reads none,
/// so the model stays small. In-place only.
fn tomori_world(
    dispatch: card_sdk::kani_host::Backend,
    owner: i32,
    mover: i32,
    landing: i32,
    pos: [i32; MAX_PLAYERS],
) {
    let _ = owner;
    host::install_backend(dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        // Yes/no answers come from the scripted slots, not the symbolic pick.
        w.symbolic_answers = false;
        for p in 0..MAX_PLAYERS {
            w.out[p] = p >= 2;
            w.stay[p] = 0;
            w.charge[p] = 0;
            w.pos[p] = pos[p];
        }
        w.trig_player = mover;
        w.trig_tile = landing;
        w.next_answer = 0;
        w.answers[0] = 0;
    });
}

/// An open invocation frame on the mover (a borrowing card's run). `paid` is
/// what `ctx::invoke_paid()` answers.
fn borrow_frame(mover: i32, paid: bool) {
    world::with_world_mut(|w| {
        w.invoke_stack[0] = (
            crate::intern::NameId(0),
            crate::intern::NameId(0),
            mover,
            0,
            -1,
            paid,
            0,
        );
        w.invoke_n = 1;
    });
}

/// (2) press world: the mover lands on 3, the owner sits on 6 (the (2)
/// destination). `charge` is the mover's pots, `answer` the scripted offer
/// answer; the face is pinned high so no follow-up muddies the cost arm.
fn tomori_pull_world(
    dispatch: card_sdk::kani_host::Backend,
    owner: i32,
    charge: i32,
    answer: i32,
    paid: bool,
) -> i32 {
    debug_assert!(owner == 0 || owner == 1);
    let mover = 1 - owner;
    let mut pos = [0i32; MAX_PLAYERS];
    pos[owner as usize] = 6;
    pos[mover as usize] = 4;
    tomori_world(dispatch, owner, mover, 3, pos);
    borrow_frame(mover, paid);
    world::with_world_mut(|w| {
        w.charge[mover as usize] = charge;
        // A paid body must never read the answer: a refusal is scripted.
        w.answers[0] = if paid { 1 } else { answer };
        w.forced_roll = 7;
    });
    mover
}

/// The accepted shape both the borrowed and the paid press leave: latches,
/// teleport, one roll; `stay` untouched, nothing owed.
fn tomori_pull_accepted_ok(mover: i32, destination: i32, stay_before: i32, cost: i32) {
    let (post_charge, post_pos, post_stay) = world::with_world(|w| {
        (
            w.charge[mover as usize],
            w.pos[mover as usize],
            w.stay[mover as usize],
        )
    });
    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    let used_key = crate::intern::intern(USED_THIS_MOVE);
    let half_key = crate::intern::intern(HALF_PRICE);
    crate::kassert!(
        spec::skills_mygo::tomori_pull_effects_ok(
            &logbuf[..log_n],
            mover,
            destination,
            used_key.0,
            half_key.0
        ),
        "(2): exactly latch, half-latch, teleport"
    );
    crate::kassert!(post_charge == cost, "(2): the press collected `cost`");
    crate::kassert!(key_get(mover, used_key) == 1, "once-per-move latch");
    crate::kassert!(key_get(mover, half_key) == 1, "half-price latch");
    crate::kassert!(post_pos == destination, "mover teleported");
    crate::kassert!(roll_calls() == 1, "exactly one 1d20");
    crate::kassert!(schedules() == 0, "high face owes nothing");
    crate::kassert!(post_stay == stay_before, "no stay");
}

/// A **paid** frame asks nothing and spends nothing — the borrowed press only
/// runs the body. `charge` is symbolic and must come back untouched.
fn tomori_pull_paid_at(owner: i32) {
    let charge: i32 = kani::any();
    kani::assume(charge >= 0 && charge <= 8);
    let mover = tomori_pull_world(tomori_pull_paid_dispatch, owner, charge, 1, true);
    let stay_before = world::with_world(|w| w.stay[mover as usize]);
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    kani::cover!(true, "paid press runs");
    tomori_pull_accepted_ok(mover, 6, stay_before, charge);
    crate::kassert!(
        world::with_world(|w| w.invoke_n) == 1,
        "the body does not nest"
    );
}

/// The skill's own collect: an accepted answer moves exactly four pots.
fn tomori_pull_unpaid_accepted_at(owner: i32) {
    let charge: i32 = kani::any();
    kani::assume(charge >= 4 && charge <= 8);
    let mover = tomori_pull_world(tomori_pull_spend_dispatch, owner, charge, 0, false);
    let stay_before = world::with_world(|w| w.stay[mover as usize]);
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    kani::cover!(true, "accepted, payable press runs");
    tomori_pull_accepted_ok(mover, 6, stay_before, charge - 4);
}

/// A declined offer, or an accepted one the mover cannot pay for (fewer than
/// four pots, no crystal in the model), is a total no-op.
fn tomori_pull_unpaid_noop_at<const ANSWER: i32>(owner: i32) {
    let charge: i32 = kani::any();
    if ANSWER == 0 {
        // An accepted answer without the pots to back it.
        kani::assume(charge >= 0 && charge < 4);
    } else {
        // A declined answer at any balance: the body must not even read it.
        kani::assume(charge >= 0 && charge <= 8);
    }
    // A decline returns before collecting; a shortfall still walks the
    // collect path (and is refused there), so it needs the wider subset.
    let dispatch = if ANSWER == 0 {
        tomori_pull_spend_dispatch
    } else {
        tomori_pull_ask_dispatch
    };
    let mover = tomori_pull_world(dispatch, owner, charge, ANSWER, false);
    let (pos_before, stay_before) =
        world::with_world(|w| (w.pos[mover as usize], w.stay[mover as usize]));
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    kani::cover!(true, "no-op press runs");
    let (post_charge, post_pos) =
        world::with_world(|w| (w.charge[mover as usize], w.pos[mover as usize]));
    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    let used_key = crate::intern::intern(USED_THIS_MOVE);
    let half_key = crate::intern::intern(HALF_PRICE);
    crate::kassert!(log_n == 0, "refusal: no effects");
    crate::kassert!(post_charge == charge, "refusal: no pot leaves");
    crate::kassert!(post_pos == pos_before, "refusal: no teleport");
    crate::kassert!(
        key_get(mover, used_key) == 0 && key_get(mover, half_key) == 0,
        "refusal: no latch"
    );
    crate::kassert!(
        roll_calls() == 0 && schedules() == 0,
        "refusal: no roll/schedule"
    );
    crate::kassert!(
        world::with_world(|w| w.stay[mover as usize]) == stay_before,
        "no stay"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_pull_invokes_paid_iff_accepted() {
    tomori_pull_paid_at(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_pull_unpaid_accepted_spends() {
    tomori_pull_unpaid_accepted_at(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_pull_unpaid_declined_does_not_spend() {
    tomori_pull_unpaid_noop_at::<1>(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_pull_unpaid_short_pots_do_not_spend() {
    tomori_pull_unpaid_noop_at::<0>(0);
}

/// MUTANT: a paid press that still spends four pots must disagree.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
#[kani::should_panic]
pub fn tomori_pull_mutant_paid_spends() {
    let owner = 0i32;
    let mover = 1i32;
    let mut pos = [0i32; MAX_PLAYERS];
    pos[owner as usize] = 6;
    pos[mover as usize] = 4;
    tomori_world(tomori_pull_paid_dispatch, owner, mover, 3, pos);
    borrow_frame(mover, true);
    world::with_world_mut(|w| {
        w.charge[mover as usize] = 5;
        w.forced_roll = 7;
    });
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    // The mutant spec: the paid path still charged. The body leaves 5.
    crate::kassert!(
        world::with_world(|w| w.charge[mover as usize]) == 1,
        "mutant: paid press charged four"
    );
}

/// The follow-up (roll / [停留] / owed settle) belongs to the (2) arm —
/// `mover != owner` — not to the cost. `MOVER_IS_OWNER` is a const so each
/// harness folds away the arm it does not prove (the two together are the iff;
/// keeping both in one body doubles the CBMC path count).
fn tomori_follow_up_at<const MOVER_IS_OWNER: bool>(owner: i32) {
    debug_assert!(owner == 0 || owner == 1);
    let mover = if MOVER_IS_OWNER { owner } else { 1 - owner };
    let other = 1 - owner;
    let landing = 3;
    let mut pos = [0i32; MAX_PLAYERS];
    // (2): the destination is the owner's own square. (3): the other party
    // within five of the landing.
    if MOVER_IS_OWNER {
        pos[owner as usize] = landing;
        pos[other as usize] = 5;
    } else {
        pos[owner as usize] = 6;
        pos[mover as usize] = 4;
    }
    let destination = if MOVER_IS_OWNER { 5 } else { 6 };
    let dispatch = if MOVER_IS_OWNER {
        tomori_follow_up3_dispatch
    } else {
        tomori_follow_up2_dispatch
    };
    tomori_world(dispatch, owner, mover, landing, pos);
    borrow_frame(mover, true);
    world::with_world_mut(|w| w.forced_roll = 1);
    let used_key = crate::intern::intern(USED_THIS_MOVE);
    let half_key = crate::intern::intern(HALF_PRICE);
    let owed_key = crate::intern::intern(OWED_SETTLE);
    // Only the stay column is compared afterwards; a full `snapshot()` is the
    // CBMC cost cliff at this harness size.
    let stay_before = world::with_world(|w| w.stay[mover as usize]);

    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));

    let (post_pos, post_stay) =
        world::with_world(|w| (w.pos[mover as usize], w.stay[mover as usize]));
    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    let effects = &logbuf[..log_n];
    let followed = !MOVER_IS_OWNER;
    kani::cover!(followed, "mover is not the owner");
    kani::cover!(!followed, "mover is the owner");

    if followed {
        crate::kassert!(
            spec::skills_mygo::tomori_follow_up_effects_ok(
                effects,
                mover,
                destination,
                used_key.0,
                half_key.0,
                owed_key.0
            ),
            "(2): teleport then exactly one stay + owed latch"
        );
        crate::kassert!(
            post_stay == spec::skills_mygo::tomori_stay_after(stay_before, true),
            "(2): exactly one stay layer"
        );
        crate::kassert!(roll_calls() == 1, "(2): exactly one roll");
        crate::kassert!(
            key_get(mover, owed_key) == spec::skills_mygo::tomori_owed_after(true),
            "(2): owed latch"
        );
        crate::kassert!(
            schedules() == spec::skills_mygo::tomori_schedules_after(true),
            "(2): exactly one owed schedule"
        );
        crate::kassert!(
            world::with_world(|w| w.last_schedule) == (mover, 1),
            "(2): scheduled at the mover's next turn end"
        );
    } else {
        crate::kassert!(
            spec::skills_mygo::tomori_pull_effects_ok(
                effects,
                mover,
                destination,
                used_key.0,
                half_key.0
            ),
            "(3): latch, half-latch, teleport only"
        );
        crate::kassert!(
            post_stay == spec::skills_mygo::tomori_stay_after(stay_before, false),
            "(3): no stay"
        );
        crate::kassert!(roll_calls() == 0, "(3): no roll");
        crate::kassert!(key_get(mover, owed_key) == 0, "(3): no owed latch");
        crate::kassert!(schedules() == 0, "(3): no owed schedule");
    }
    crate::kassert!(post_pos == destination, "teleported to the destination");
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_follow_up_iff_the_mover_is_not_the_owner() {
    tomori_follow_up_at::<false>(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_follow_up_none_when_the_mover_is_the_owner() {
    tomori_follow_up_at::<true>(0);
}

/// MUTANT: tying the follow-up to `cost == 4` predicts a roll even when the
/// owner has no square (`destination < 0`), where the body returns early.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
#[kani::should_panic]
pub fn tomori_follow_up_mutant_cost_four() {
    let owner = 0i32;
    let mover = 1i32;
    let mut pos = [0i32; MAX_PLAYERS];
    pos[owner as usize] = -1;
    pos[mover as usize] = 4;
    tomori_world(tomori_follow_up2_dispatch, owner, mover, 3, pos);
    borrow_frame(mover, true);
    world::with_world_mut(|w| w.forced_roll = 1);
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    let cost = 4;
    crate::kassert!(
        !spec::skills_mygo::tomori_follow_up_mutant_cost_four(cost) || roll_calls() == 1,
        "mutant: cost 4 always rolls"
    );
}

/// The roll threshold: [停留] + owed latch + schedule exactly on 6 or less.
fn tomori_marks_the_mover_iff_roll_at(owner: i32) {
    let mover = 1 - owner;
    let destination = 6;
    let mut pos = [0i32; MAX_PLAYERS];
    pos[owner as usize] = destination;
    pos[mover as usize] = 4;
    tomori_world(tomori_follow_up2_dispatch, owner, mover, 3, pos);
    borrow_frame(mover, true);
    let face: i32 = kani::any();
    kani::assume(face >= 1 && face <= 20);
    world::with_world_mut(|w| w.forced_roll = face);
    let used_key = crate::intern::intern(USED_THIS_MOVE);
    let half_key = crate::intern::intern(HALF_PRICE);
    let owed_key = crate::intern::intern(OWED_SETTLE);

    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));

    let followed = spec::skills_mygo::tomori_follow_up(face);
    kani::cover!(followed, "face 6 or less");
    kani::cover!(!followed, "face above 6");

    let post_stay = world::with_world(|w| w.stay[mover as usize]);
    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    let effects = &logbuf[..log_n];
    if followed {
        crate::kassert!(
            spec::skills_mygo::tomori_follow_up_effects_ok(
                effects,
                mover,
                destination,
                used_key.0,
                half_key.0,
                owed_key.0
            ),
            "face <= 6: exactly one stay + owed latch"
        );
    } else {
        crate::kassert!(
            spec::skills_mygo::tomori_pull_effects_ok(
                effects,
                mover,
                destination,
                used_key.0,
                half_key.0
            ),
            "face > 6: no follow-up effects"
        );
    }
    crate::kassert!(
        post_stay == spec::skills_mygo::tomori_stay_after(0, followed),
        "stay iff the face is low"
    );
    crate::kassert!(
        key_get(mover, owed_key) == spec::skills_mygo::tomori_owed_after(followed),
        "owed latch iff the face is low"
    );
    crate::kassert!(
        schedules() == spec::skills_mygo::tomori_schedules_after(followed),
        "schedule iff the face is low"
    );
    crate::kassert!(roll_calls() == 1, "the roll happens on the (2) arm");
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_owed_settle_marks_the_mover_iff_the_roll_is_low() {
    tomori_marks_the_mover_iff_roll_at(0);
}

/// MUTANT: an off-by-one threshold (`< 6`) must disagree at face 6.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
#[kani::should_panic]
pub fn tomori_owed_settle_mutant_below_six() {
    let owner = 0i32;
    let mover = 1i32;
    let mut pos = [0i32; MAX_PLAYERS];
    pos[owner as usize] = 6;
    pos[mover as usize] = 4;
    tomori_world(tomori_follow_up2_dispatch, owner, mover, 3, pos);
    borrow_frame(mover, true);
    let face: i32 = kani::any();
    kani::assume(face >= 1 && face <= 20);
    world::with_world_mut(|w| w.forced_roll = face);
    let owed_key = crate::intern::intern(OWED_SETTLE);
    invoke::unwrap_done(skill_personas::tomori::before_settle_kani(owner));
    crate::kassert!(
        key_get(mover, owed_key)
            == spec::skills_mygo::tomori_follow_up_mutant_below_six(face) as i32,
        "mutant: owed latch on face < 6"
    );
}

/// The owed settle always clears the owed and borrowed latches; it asks for
/// exactly one settle on the player's own square only when there is one. The
/// half price armed by the forced settle rides that settle and the deferred
/// `SettleAfter` body disarms it; with no square it is dropped in place.
fn tomori_owed_settle_clears_iff_at(player: i32) {
    debug_assert!(player == 0 || player == 1);
    let mut pos = [0i32; MAX_PLAYERS];
    tomori_world(tomori_owed_dispatch, player, player, 3, pos);
    let at: i32 = kani::any();
    kani::assume(at >= -1 && at <= 8);
    world::with_world_mut(|w| {
        w.pos[player as usize] = at;
        // The settle is the card-forced one, so its `settleAfter` body runs.
        w.move_snap.settle_redirect = true;
        // Stale latches: the callback must clear them.
        w.key_set(player, crate::intern::intern(OWED_SETTLE), 1);
        w.key_set(player, crate::intern::intern(HALF_PRICE), 1);
        w.key_set(player, crate::intern::intern(BORROWED), 1);
    });
    let owed_key = crate::intern::intern(OWED_SETTLE);
    let half_key = crate::intern::intern(HALF_PRICE);
    let borrowed_key = crate::intern::intern(BORROWED);

    invoke::unwrap_done(skill_personas::tomori::owed_settle_kani(player));
    kani::cover!(at >= 0, "on the board (settles)");
    kani::cover!(at < 0, "off the board (no settle)");

    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    let settles_here = at >= 0;
    crate::kassert!(
        spec::skills_mygo::tomori_owed_effects_ok(
            &logbuf[..log_n],
            player,
            owed_key.0,
            borrowed_key.0,
            half_key.0,
            settles_here
        ),
        "owed settle: clear owed + borrowed; half only with a settle"
    );
    crate::kassert!(key_get(player, owed_key) == 0, "owed latch cleared");
    crate::kassert!(key_get(player, borrowed_key) == 0, "borrowed latch cleared");
    if settles_here {
        crate::kassert!(settles() == 1, "settles exactly once");
        crate::kassert!(
            world::with_world(|w| w.last_settle) == (player, at),
            "settles the player's own square"
        );
        crate::kassert!(
            key_get(player, half_key) == 1,
            "half price still armed for the settle paying now"
        );
        // The deferred `SettleAfter` body.
        invoke::unwrap_done(skill_personas::tomori::clear_owed_paying_kani(player));
        let (logbuf2, cn) = world::with_world(|w| (w.log, w.log_n));
        kani::cover!(cn == 4, "owed settle: four latch writes");
        crate::kassert!(
            spec::skills_mygo::tomori_owed_clear_effects_ok(
                &logbuf2[log_n..cn],
                player,
                borrowed_key.0,
                half_key.0
            ),
            "SettleAfter: disarm half, disarm borrowed"
        );
    } else {
        crate::kassert!(settles() == 0, "no settle with no square");
    }
    crate::kassert!(
        key_get(player, half_key) == 0,
        "half price disarmed afterwards"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
pub fn tomori_owed_settle_clears_its_latches_iff_it_settles() {
    tomori_owed_settle_clears_iff_at(0);
}

/// MUTANT: disarming the half price inside `owed_settle` itself (before the
/// deferred settle pays) must disagree.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(20)]
#[kani::should_panic]
pub fn tomori_owed_settle_mutant_half_disarmed_early() {
    tomori_world(tomori_owed_dispatch, 0, 0, 3, [0i32; MAX_PLAYERS]);
    world::with_world_mut(|w| {
        w.pos[0] = 2;
        w.move_snap.settle_redirect = true;
        w.key_set(0, crate::intern::intern(HALF_PRICE), 1);
    });
    let half_key = crate::intern::intern(HALF_PRICE);
    invoke::unwrap_done(skill_personas::tomori::owed_settle_kani(0));
    let (logbuf, log_n) = world::with_world(|w| (w.log, w.log_n));
    crate::kassert!(
        spec::skills_mygo::tomori_owed_effects_mutant_half_disarmed_early(
            &logbuf[..log_n],
            half_key.0
        ),
        "mutant: owed_settle itself disarms the half latch"
    );
}

// ===========================================================================
// Classification notes (no harness — documented in the unit table):
// C-MYGO-band-01 (开局 3d20 start tile) — PROTOCOL-LEVEL: game-start
//   scheduling (`On::Action(DeckBeforeGame)`); the body's 3d20 sum is
//   `roll(3,20)` (symbolic sum 3..=60) then `teleport_to`. Leaf body could be
//   proven but the start-of-game window is engine scheduling.
// C-MYGO-band-03 (move-1 instead of dice) — the press body spends a crystal
//   and shapes a 1-step move via `plan::*` (protocol stubs). PROTOCOL-LEVEL.
// Character skills (tomori/anon/rana/soyo/rikki) — see the unit table; most
//   windows are multi-turn / counteraction / settle pipelines (PROTOCOL-LEVEL)
//   or hit known black-box FAILS recorded in mygo.md §Findings.
// ===========================================================================
