//! Unit `g14`: clause proofs.
//!
//! Per clause: iff proof (effect log exactly, frame), a reachable mutant, and
//! `kani::cover!` per branch.
//!
//! The full NNM Play body is BLOCKED-HOST under Kani (`arm_tok_names` is empty),
//! so the draw half is driven through the cfg(kani) `effect_draw_kani` wrapper
//! (one body in the call graph, no trampoline). The second half of the clause
//! (「回合结束后将手牌弃置到五张」) already has `nnm_discard_dispatch`.
//!
//! `arm_draw` reads `a[2]`'s [`card_sdk::abi::draw_flag::OVER_HAND_LIMIT`] bit
//! and logs `Effect::DrawOver` -- no new `Op`.

use crate::host;
use crate::invoke;
use crate::spec::g14_ai_and_hand_limit as spec;
use crate::world::{self, Effect};

// ---------------------------------------------------------------------------
// Host-op subsets. An undeclared op FAILS loudly.
// ---------------------------------------------------------------------------

/// `effect_draw`: the exempt import and the turn-end schedule.
crate::host_subset!(g14_draw_over {
    draw => arm_draw,
    schedule_turn_end => arm_stub_zero,
});

/// The plain contrast: 10次招募 pays, latches, then draws.
crate::host_subset!(g14_plain_draw {
    pay => arm_pay,
    pay_command => arm_pay_command,
    state_get => arm_state_get,
    state_set => arm_state_set,
    log => arm_log,
    draw => arm_draw,
});

fn effects_of() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|w| w.log);
    let log_n = world::with_world(|w| w.log_n);
    (logbuf, log_n)
}

fn two_seat_world(dispatch: card_sdk::kani_host::Backend, user: i32) {
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

// ===========================================================================
// C-MOR-nnm-02 (1) -- 「抽x张卡（可超过上限）」
// ===========================================================================
//
// The import count is CONCRETE per harness: a symbolic `n` across the `draw1!`
// guard chain pushed the CBMC run past the 900 s timeout. `x = 1` and `x = 2`
// bracket the clause's single / repeated shape.

fn nnm_02_draw_over_iff_at(user: i32, x: i32) {
    debug_assert!(user == 0 || user == 1);
    two_seat_world(g14_draw_over, user);
    let condition = world::snapshot();
    let pre_hand = world::with_world(|w| w.hand_n);
    let pre_dest = world::with_world(|w| w.dest);

    invoke::unwrap_done(card_morfonica::nanami_effort::effect_draw_kani(user, x));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    kani::cover!(log_n == x as usize, "exactly x imports");
    kani::cover!(
        effects.iter().all(|e| matches!(e, Effect::DrawOver { .. })),
        "no plain import"
    );
    crate::kassert!(
        spec::nnm_02_draw_over_iff(effects, user, x),
        "C-MOR-nnm-02 (1): effect log must be exactly x DrawOver{player,1}"
    );

    // Post: the hand rises by x.
    let post_hand = world::with_world(|w| w.hand_n);
    crate::kassert!(
        post_hand as i32 == pre_hand as i32 + x,
        "C-MOR-nnm-02 (1): the hand rises by x"
    );

    // Frame: money / dest unchanged (the schedule is a stub with no effect).
    let post = world::snapshot();
    crate::kassert!(world::arr4_eq(post.money, condition.money), "frame: money");
    crate::kassert!(world::with_world(|w| w.dest) == pre_dest, "frame: dest");
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn nnm_02_draw_over_x1_iff() {
    nnm_02_draw_over_iff_at(0, 1);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn nnm_02_draw_over_x2_iff() {
    nnm_02_draw_over_iff_at(0, 2);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn nnm_02_draw_over_user1_iff() {
    nnm_02_draw_over_iff_at(1, 1);
}

/// MUTANT: the same shape read as a plain draw must fail.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn nnm_02_draw_over_mutant_plain() {
    let user = 0;
    two_seat_world(g14_draw_over, user);
    let x = 2;
    invoke::unwrap_done(card_morfonica::nanami_effort::effect_draw_kani(user, x));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::nnm_02_draw_over_iff_mutant_plain(effects, user, x),
        "mutant: an exempt import is not a plain one"
    );
}

// ===========================================================================
// control -- a plain `ctx::draw` card keeps `Effect::Draw`
// ===========================================================================

/// 规则书 `通用:10次招募（1回限定）`: 「[消耗]1500资金，抽1张卡」. The import in a
/// non-exempt body stays plain.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn plain_draw_stays_plain() {
    let user = 0;
    two_seat_world(g14_plain_draw, user);
    world::with_world_mut(|w| {
        w.money[user as usize] = kani::any();
        kani::assume(w.money[user as usize] >= 1500 && w.money[user as usize] <= 10_000);
    });
    invoke::unwrap_done(card_general::gacha10::gacha10_kani(user));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects_of();
    let draws: Vec<Effect> = logbuf[..log_n]
        .iter()
        .filter(|e| matches!(e, Effect::Draw { .. } | Effect::DrawOver { .. }))
        .cloned()
        .collect();
    crate::kassert!(
        spec::plain_draw_stays_plain(&draws, user, 1),
        "a plain ctx::draw logs one unexempt Draw"
    );
}

// ===========================================================================
// C-MOR-夏日合宿 -- the static `aiNever` prop
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn summer_ai_never_declared() {
    crate::kassert!(
        spec::summer_ai_never_declared(card_morfonica::summer_camp::SUMMER_CAMP.props),
        "C-MOR-summer: the def declares aiNever = 1"
    );
}

/// MUTANT: a copy without the prop fails the spec.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
#[kani::should_panic]
pub fn summer_ai_never_mutant() {
    crate::kassert!(
        spec::summer_ai_never_declared_mutant(card_morfonica::summer_camp::SUMMER_CAMP.props),
        "mutant: the shipped def does declare it"
    );
}
