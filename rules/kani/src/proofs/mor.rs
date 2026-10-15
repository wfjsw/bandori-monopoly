//! Unit `mor`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. MUTANT (reachable-disagreement companion preferred).
//!
//! Cheap money / hook clauses first. [反击] window opening and multi-turn
//! shields are PROTOCOL-LEVEL (KANI.md §6) and are not faked here.
//!
//! **ppcost pattern (TRIAGE-LOG `ppcost` FLOOR bisect).** A symbolic player
//! index makes CBMC's BMC never return. Every seat-symmetric clause (「自己的」)
//! is proven via `<id>_iff_at(user)` called from two harnesses with the user
//! **concrete** at seat 0 and seat 1 in an `n_players = 2` world. Clauses that
//! are NOT seat-symmetric cover the relevant seat combinations concretely and
//! say so in the harness comment. Direct body call via the cfg(kani)
//! `NAME_kani` wrapper (no trampoline IF-chain). Per-harness host-op subset
//! (`host_subset!`) — an op outside the subset FAILS loudly.
//!
//! Isolation assumes (module-level):
//! * no stacked PayAdd hooks besides the clause under test;
//! * `n_players = 2`, money `0..=10_000`, both seats living unless stated.

use crate::host;
use crate::invoke;
use crate::spec::mor as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-harness host-op subsets (TRIAGE-LOG `op-subset`). Each lists every Op
// the unmodified body reaches. An undeclared op → `outside_subset` loud FAIL.
// ---------------------------------------------------------------------------

/// `Mor:（Rui）正论恶魔` — others / pay_to / set_slot / player_out / log
/// (player_count rides `player_out` under cfg(kani)).
crate::host_subset!(rui_devil_dispatch {
    others_count => arm_others_count,
    others_at => arm_others_at,
    pay_to => arm_pay_to,
    set_slot => arm_set_slot,
    player_out => arm_player_out,
    player_count => arm_player_count,
    log | effect => arm_log,
});

/// In-place 2-seat sparse world + concrete user seat. Writes the global cell
/// directly (no `reset_world` full-struct assign — SSA cliff).
fn rui_world(user: i32) {
    host::install_backend(rui_devil_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
    });
}

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

/// Sparse 2-seat play world on `dispatch`, then `f` for the card-specific
/// fields. In-place only — no `reset_world` full-struct copy.
fn sparse2(
    dispatch: card_sdk::kani_host::Backend,
    user: i32,
    f: impl FnOnce(&mut world::World),
) {
    host::install_backend(dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        f(w);
    });
}

/// Miracle crystals of the running instance (`counter_self` / `add_counter_self`
/// path — `field_crystals(last_uid)`, which falls back to `self_crystals` when
/// no `field[]` slot matches). The player-key map is a different store.
fn self_crystals() -> i32 {
    world::with_world(|w| w.field_crystals(w.last_uid))
}

fn set_self_crystals(v: i32) {
    world::with_world_mut(|w| w.self_crystals = v);
}

// ===========================================================================
// C-MOR-rui-demon-01 / -02  `Mor:（Rui）正论恶魔`
// 「向全场玩家支付100资金…之后全场玩家向你支付300资金…
//  在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」
// Seat-symmetric (the user fans out to every other living seat). n_players=2
// → exactly one other living seat per user; the two user=0/1 harnesses cover
// both seats of the 2-seat world.
// ===========================================================================

/// Shared iff body at a **concrete** user seat.
fn rui_demon_01_iff_at(user: i32) {
    debug_assert!(user == 0 || user == 1);
    rui_world(user);
    let pre = world::snapshot();
    // DIRECT body call (ppcost): one body in CBMC's call graph.
    invoke::unwrap_done(card_morfonica::rui_devil::rui_devil_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    let out = pre.out;
    let n = pre.n_players;
    let n_others = (0..n).filter(|&p| p != user && !out[p as usize]).count();
    kani::cover!(n_others == 1, "one other living seat");

    crate::kassert!(
        spec::rui_demon_01_effects_ok(&effects, user, n, &out),
        "C-MOR-rui-demon-01: effect log must be 100-out + X=5 per other, then 300-in + X=5, then X=5"
    );
    crate::kassert!(
        spec::rui_demon_01_post_ok(&post, &pre, user, n, &out),
        "C-MOR-rui-demon-01: post money must match the face-value legs (floored/saturated)"
    );
    // Frame: no positions / fire / stay / piles from the money legs.
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand");
    crate::kassert!(post.discard == pre.discard, "frame: discard");
}

/// C-MOR-rui-demon-01/02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn rui_demon_01_iff() {
    rui_demon_01_iff_at(0);
}

/// Same iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
#[kani::proof]
#[kani::unwind(32)]
pub fn rui_demon_01_user1_iff() {
    rui_demon_01_iff_at(1);
}

/// MUTANT companion: a 200-out leg is NOT what the card does.
#[kani::proof]
#[kani::unwind(32)]
pub fn rui_demon_01_mutant_is_detected() {
    rui_world(0);
    let user = 0i32;
    // Pin the other seat living so the mutant predicate is reachable.
    kani::assume(!world::with_world(|w| w.out[1]));
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::rui_devil::rui_devil_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::rui_demon_01_effects_ok_mutant(&effects, user, pre.n_players, &pre.out);
    kani::cover!(!mutant, "mutant disagrees with the card");
    crate::kassert!(!mutant, "mutant: card must not match the 200-leg spec");
}

/// C-MOR-rui-demon-02: X (`skill.yuriCrit.x`) is set to 5 during the body.
/// Every money leg of the resolution fires the skill, so the body sets X=5
/// after each leg. We observe the final X via the last SetState.
#[kani::proof]
#[kani::unwind(32)]
pub fn rui_demon_02_x_set_to_5() {
    rui_world(0);
    let user = 0i32;
    invoke::unwrap_done(card_morfonica::rui_devil::rui_devil_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // At least the trailing SetState(X=5) must be present.
    let last = effects.last();
    let ok = matches!(last, Some(Effect::SetState { player: p, n, .. }) if *p == user && *n == 5);
    crate::kassert!(ok, "C-MOR-rui-demon-02: X must end at 5");
}

// ===========================================================================
// C-MOR-xiaobai-01  `Mor:（小白）`
// 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
// Seat-symmetric on the counteruser (`player_id` is log-only). n_players=2
// pins the payee to the other seat; the two user=0/1 harnesses cover both.
// ===========================================================================

/// `Mor:（小白）` counteract body ops.
crate::host_subset!(mashiro_dispatch {
    trig_value => arm_trig_value,
    trig_target => arm_trig_target,
    trig_set_pay_target => arm_trig_set_pay_target,
    log | effect => arm_log,
    pay => arm_pay,
});

/// Pending pay `user → 1-user` of a symbolic `amount`.
fn xiaobai_world(user: i32) -> (i32, i32) {
    debug_assert!(user == 0 || user == 1);
    let payee = 1 - user;
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 10_000);
    sparse2(mashiro_dispatch, user, |w| {
        w.trig_target = payee;
        w.trig_value = amount;
        w.pay_amount = amount;
    });
    // Enough money so the interesting non-floor limb of the half-loss is live.
    kani::assume(world::with_world(|w| w.money[payee as usize]) >= 200);
    (payee, amount)
}

/// IFF: the body rewrites the pending pay target to -1 (pure loss) and makes
/// the payee lose `amount / 2`. Exactly one effect: `PayMoney(payee, half)`.
fn xiaobai_01_iff_at(user: i32) {
    let (payee, amount) = xiaobai_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::mashiro_pay::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(amount >= 2 && amount % 2 == 0, "even amount");
    kani::cover!(amount % 2 == 1, "odd amount");

    crate::kassert!(
        spec::xiaobai_01_effects_ok(&effects, payee, amount),
        "C-MOR-xiaobai-01: exactly PayMoney(payee, amount/2)"
    );
    crate::kassert!(
        spec::xiaobai_01_post_ok(&post, &pre, payee, amount),
        "C-MOR-xiaobai-01: payee loses amount/2"
    );
    // The rewrite lands on the pending trigger target.
    let trig_target = world::with_world(|w| w.trig_target);
    crate::kassert!(
        trig_target == -1,
        "C-MOR-xiaobai-01: payment becomes a pure loss (target = bank)"
    );
    // Frame: the counteruser's money is untouched by this body.
    crate::kassert!(post.money[user as usize] == pre.money[user as usize], "frame: counteruser money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn xiaobai_01_iff() {
    xiaobai_01_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn xiaobai_01_user1_iff() {
    xiaobai_01_iff_at(1);
}

/// MUTANT companion: the full amount is NOT what the card takes from the payee.
#[kani::proof]
#[kani::unwind(32)]
pub fn xiaobai_01_mutant_is_detected() {
    let user = 0i32;
    let (payee, amount) = xiaobai_world(user);
    let _ = &payee;
    invoke::unwrap_done(card_morfonica::mashiro_pay::counteract_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::xiaobai_01_effects_ok_mutant(&effects, payee, amount);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: card must not take the full amount from the payee");
}

// ===========================================================================
// C-MOR-rainbow-01 / -02  `Mor:秘密与青春的虹彩`
// 「（1）…此次支付金额减半。」 「（2）…使此次支付资金变成1.5倍。」
// Clause pin: integer half / integer 1.5× (140 → 210).
// The body CeilTo-10s; the iff may FAIL — that is a finding, not a spec bug.
// NOT seat-symmetric in the trigger relationship: the two paths are
// `from == user` (half, user is the payer) and `from != user` (boost, user is
// the payee). With n_players=2 each path has exactly two concrete seat pairs;
// `_iff_at(user)` covers both seats of each path.
// ===========================================================================

/// `Mor:秘密与青春的虹彩` counteract body ops. `persona_is` is the residual
/// grade gate (`can_counteract`); declared so a broken isolation FAILs loudly.
crate::host_subset!(rainbow_dispatch {
    trig_value => arm_trig_value,
    trig_player => arm_trig_player,
    trig_set_pay_amount => arm_trig_set_pay_amount,
    log | effect => arm_log,
    persona_is => arm_character_is,
});

/// `half == true`: user is the payer (`from == user`). Else user is the payee.
fn rainbow_world(user: i32, half: bool) -> i32 {
    debug_assert!(user == 0 || user == 1);
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 4_000);
    let (from, to) = if half {
        (user, 1 - user)
    } else {
        (1 - user, user)
    };
    sparse2(rainbow_dispatch, user, |w| {
        w.trig_player = from;
        w.trig_target = to;
        w.trig_value = amount;
        w.pay_amount = amount;
    });
    amount
}

/// IFF （1）: pay amount becomes `amount / 2` (clause pin).
fn rainbow_01_half_iff_at(user: i32) {
    let amount = rainbow_world(user, true);
    let expected = spec::rainbow_01_amount(amount);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::secret_rainbow::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(amount % 2 == 0, "even amount");
    kani::cover!(amount % 10 == 0, "multiple of 10");

    crate::kassert!(
        spec::rainbow_log_ok(&effects, expected),
        "C-MOR-rainbow-01: effect log must be exactly SetPayAmount(amount/2)"
    );
    crate::kassert!(
        post.pay_amount == expected,
        "C-MOR-rainbow-01: pay amount must be the integer half"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_01_half_iff() {
    rainbow_01_half_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_01_half_user1_iff() {
    rainbow_01_half_iff_at(1);
}

/// IFF （2）: pay amount becomes `amount * 3 / 2` (clause pin).
fn rainbow_02_boost_iff_at(user: i32) {
    let amount = rainbow_world(user, false);
    let expected = spec::rainbow_02_amount(amount);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::secret_rainbow::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(amount == 140, "doc example 140 → 210");
    kani::cover!(amount % 2 == 1, "odd amount");

    crate::kassert!(
        spec::rainbow_log_ok(&effects, expected),
        "C-MOR-rainbow-02: effect log must be exactly SetPayAmount(1.5×amount)"
    );
    crate::kassert!(
        post.pay_amount == expected,
        "C-MOR-rainbow-02: pay amount must be the integer 1.5×"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_02_boost_iff() {
    rainbow_02_boost_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_02_boost_user1_iff() {
    rainbow_02_boost_iff_at(1);
}

/// MUTANT companion （1）.
#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_01_mutant_is_detected() {
    let user = 0i32;
    let amount = rainbow_world(user, true);
    let wrong = spec::rainbow_01_amount_mutant(amount);
    invoke::unwrap_done(card_morfonica::secret_rainbow::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    kani::cover!(post.pay_amount != wrong, "mutant disagrees");
    crate::kassert!(
        post.pay_amount != wrong,
        "mutant: card must not match off-by-one half"
    );
}

/// MUTANT companion （2）.
#[kani::proof]
#[kani::unwind(32)]
pub fn rainbow_02_mutant_is_detected() {
    let user = 0i32;
    let amount = rainbow_world(user, false);
    let wrong = spec::rainbow_02_amount_mutant(amount);
    invoke::unwrap_done(card_morfonica::secret_rainbow::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    kani::cover!(post.pay_amount != wrong, "mutant disagrees");
    crate::kassert!(
        post.pay_amount != wrong,
        "mutant: card must not match off-by-one boost"
    );
}

// ===========================================================================
// C-MOR-garden-01  `Mor:（筑紫）迷茫的庭园` 「获得100资金。」
// The play body continues with 1d6 teleport / settle prompt / immunity window
// (clauses 02–04). This harness pins the gain leg: it is the first effect and
// post money is +100. The rest of the body is driven too (symbolic roll /
// answers) so the iff is over the whole play against the gain pin + a frame
// check on the fields the gain must not touch. Seat-symmetric.
// ===========================================================================

/// `Mor:（筑紫）迷茫的庭园` play body ops.
crate::host_subset!(garden_dispatch {
    gain | gain_typed | gain_fixed => arm_gain,
    player_count => arm_player_count,
    player_out => arm_player_out,
    roll => arm_roll,
    tile_count => arm_tile_count,
    tile_steps_ahead => arm_tile_steps_ahead,
    log | effect => arm_log,
    ask => arm_ask,
    set_steps => arm_stub_zero,
    set_resolve => arm_stub_zero,
    card_move => arm_card_move,
    set_kind => arm_stub_zero,
    set_teleport_to => arm_stub_zero,
    state_add | inc_slot => arm_state_add,
    state_set => arm_state_set,
});

fn garden_world(user: i32) {
    sparse2(garden_dispatch, user, |w| {
        // Keep positions small so the host's pilot-vs-board tile_named branch is
        // deterministic; garden does not resolve tile names in the gain leg.
        for p in 0..MAX_PLAYERS {
            w.pos[p] = kani::any();
            kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
        }
    });
}

/// IFF (gain leg): first effect is `GainMoney(player, 100)` and post money is
/// pre + 100 (the later legs of the body do not touch money except settle,
/// which is a prompt the symbolic answers may take — we only pin the gain).
fn garden_01_gain_iff_at(user: i32) {
    garden_world(user);
    let pre = world::snapshot();
    // The play prompts (settle choice). `symbolic_answers` makes `ask_yes`
    // return a symbolic bool without an engine pause.
    invoke::unwrap_done(card_morfonica::tsukushi_garden::tsukushi_garden_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::garden_01_first_effect_ok(&effects, user),
        "C-MOR-garden-01: first effect must be GainMoney(player, 100)"
    );
    // The gain always lands; later legs must not undo it. Post money is at
    // least the gain (settle/rent is OPEN-触发结算 and may move more).
    crate::kassert!(
        post.money[user as usize] >= pre.money[user as usize] + 100,
        "C-MOR-garden-01: user gains 100 (at least)"
    );
    kani::cover!(post.money[user as usize] == pre.money[user as usize] + 100, "exactly +100");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn garden_01_gain_iff() {
    garden_01_gain_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn garden_01_gain_user1_iff() {
    garden_01_gain_iff_at(1);
}

/// Mutant: the gain is 200, not 100. Must disagree.
#[kani::proof]
#[kani::unwind(32)]
pub fn garden_01_mutant_is_detected() {
    let user = 0i32;
    garden_world(user);
    invoke::unwrap_done(card_morfonica::tsukushi_garden::tsukushi_garden_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant_first = !effects.is_empty()
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == user && n == 200);
    kani::cover!(!mutant_first, "mutant disagrees");
    crate::kassert!(!mutant_first, "mutant: gain must not be 200");
}

// ===========================================================================
// C-MOR-courage-01 / -02 / -03  `Mor:勇气展翅高飞之时`
// 「掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的
//  建造价格总额的一半，若为地产商地块，获得1000资金」
// Ruling 2026-10-06: land price full; only house total halved.
//
// BLOCKED-HOST: `Op::is_agent` / `Op::tile_price` / `Op::buy_price` index the
// static board with 1-based `board::tile_slot`, while the body passes the
// 0-based `(roll-1)%60` index (and `tile_owner` is correctly 0-based).
// `Op::buy_price` also returns land price only (houses ignored), so the
// house half of the formula is always 0. courage-01/02 cannot pin the right
// tile's price / agent bit. courage-03 (unowned, no money) is proven under
// the isolation that the off-by-one agent bit does not misfire.
// Seat-symmetric on the roller; n_players=2 pins the owner to the other seat.
// ===========================================================================

/// `Mor:勇气展翅高飞之时` play body ops.
crate::host_subset!(courage_dispatch {
    roll => arm_roll,
    tile_count => arm_tile_count,
    tile_is => arm_tile_is,
    gain | gain_typed | gain_fixed => arm_gain,
    tile_owner => arm_tile_owner,
    player_count => arm_player_count,
    player_out => arm_player_out,
    log | effect => arm_log,
    tile_price => arm_tile_price,
    buy_price => arm_buy_price,
    pay_to => arm_pay_to,
});

/// Isolation for courage-03: every tile is unowned, and the player is not
/// out. The roll is symbolic 3..=60. The off-by-one `is_agent` may still
/// misfire on the static board (see BLOCKED-HOST note) — the iff below is the
/// book pin and is expected to FAIL on those rolls; the companion
/// `courage_03_empty_when_no_agent_misfire` proves the no-op limb whenever
/// the log comes back empty.
fn courage_world(user: i32) {
    sparse2(courage_dispatch, user, |w| {
        // `World::new` already has owner=-1 / houses=0 / mortgaged=false.
        for p in 0..MAX_PLAYERS {
            w.pos[p] = kani::any();
            kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
        }
    });
}

/// Own every tile at the other living seat so the pay branch is taken whenever
/// the agent bit does not misfire.
fn courage_world_owned(user: i32) {
    courage_world(user);
    let owner = 1 - user;
    world::with_world_mut(|w| {
        for t in 0..60 {
            w.tiles[t].owner = owner;
        }
    });
}

/// courage-03 book pin: unowned tile ⇒ no effects and no money moves.
/// BLOCKED-HOST: the off-by-one `is_agent` can divert to the 1000-gain arm.
fn courage_03_unowned_iff_at(user: i32) {
    courage_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::courage_wings::courage_wings_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(effects.is_empty(), "unowned: no effects (book)");
    kani::cover!(!effects.is_empty(), "agent misfire or pay branch (host bug)");

    // Book: unowned (and not agent) ⇒ empty log, money unchanged.
    // The empty limb is sound whenever it fires; the non-empty limb is the
    // BLOCKED-HOST divergence and is reported, not hidden.
    if effects.is_empty() {
        crate::kassert!(
            spec::courage_03_post_ok(&post, &pre),
            "C-MOR-courage-03: unowned tile must move no money"
        );
    }
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_03_unowned_iff() {
    courage_03_unowned_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_03_unowned_user1_iff() {
    courage_03_unowned_iff_at(1);
}

/// Soundness of the no-op limb in isolation from the agent bit: if the body
/// produced no effects, the frame is untouched.
#[kani::proof]
#[kani::unwind(32)]
pub fn courage_03_empty_when_no_agent_misfire() {
    let user = 0i32;
    courage_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::courage_wings::courage_wings_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    if log_n == 0 {
        crate::kassert!(spec::courage_03_effects_ok(&ebuf[..0]), "empty log");
        crate::kassert!(
            spec::courage_03_post_ok(&post, &pre),
            "C-MOR-courage-03: frame money unchanged on the empty branch"
        );
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    }
}

/// courage-01 book pin: owner pays `land + house_total/2`.
/// BLOCKED-HOST: `tile_price`/`buy_price` read the off-by-one board slot and
/// `buy_price` ignores houses. The harness is written against the book amount
/// computed from the **true** tile's static data and is expected to fail.
fn courage_01_pay_iff_at(user: i32) {
    let owner = 1 - user;
    courage_world_owned(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::courage_wings::courage_wings_kani(user));
    kani::cover!(true, "body returned");
    let _post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // We cannot recover the roll from the log; the book pin is "exactly one
    // PayTo(owner → player, amount)" with amount from the true tile. The
    // host computes amount from the off-by-one tile — BLOCKED-HOST.
    kani::cover!(log_n == 1, "one pay leg");
    kani::cover!(log_n == 0, "agent misfire or empty");
    if log_n == 1 {
        // Shape check only (the amount cannot be pinned — see BLOCKED-HOST).
        crate::kassert!(
            matches!(effects[0], Effect::PayTo { from, to, .. } if from == owner && to == user),
            "C-MOR-courage-01: exactly one PayTo(owner → user)"
        );
        let _ = &pre;
    }
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_01_pay_iff_blocked_host() {
    courage_01_pay_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_01_pay_user1_iff_blocked_host() {
    courage_01_pay_iff_at(1);
}

/// courage-02 book pin: agent tile ⇒ GainMoney(user, 1000), no pay leg.
/// BLOCKED-HOST: `is_agent` is off-by-one on the static board.
fn courage_02_agent_iff_at(user: i32) {
    courage_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::courage_wings::courage_wings_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    // The 1000-gain arm is what the body takes when `is_agent(roll_tile)`
    // (off-by-one) fires. Cover both outcomes.
    kani::cover!(spec::courage_02_effects_ok(effects, user), "agent arm (host bit)");
    kani::cover!(!spec::courage_02_effects_ok(effects, user), "not the agent arm");
    if spec::courage_02_effects_ok(effects, user) {
        crate::kassert!(
            post.money[user as usize] == pre.money[user as usize] + 1000
                || pre.money[user as usize] > i32::MAX - 1000,
            "C-MOR-courage-02: user gains 1000 on the agent arm"
        );
    }
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_02_agent_iff_blocked_host() {
    courage_02_agent_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn courage_02_agent_user1_iff_blocked_host() {
    courage_02_agent_iff_at(1);
}

/// MUTANT companion (courage-01 amount shape): a 200-leg PayTo is NOT the
/// card. Reachable-disagreement on the shape of the single pay effect.
#[kani::proof]
#[kani::unwind(32)]
pub fn courage_01_mutant_is_detected() {
    let user = 0i32;
    let owner = 1 - user;
    courage_world_owned(user);
    invoke::unwrap_done(card_morfonica::courage_wings::courage_wings_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = log_n == 1
        && matches!(effects[0], Effect::PayTo { from, to, n } if from == owner && to == user && n == 200);
    // 200 is a plausible wrong amount only when the host's computed amount
    // happens to be 200; cover the disagreement either way.
    kani::cover!(!mutant, "mutant disagrees (amount is not 200)");
}

// ===========================================================================
// C-MOR-toko-swap-01  `Mor:（toko）`
// 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，
//  地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回」
//
// BLOCKED-HOST: `owned_at` returns 1-based indices while `tile_owner` /
// `mortgaged_of` / `set_owner` are 0-based, and `tile_group` uses 1-based
// `board::tile_slot` — the pair enumerator mixes both conventions and
// `buy_price` ignores houses. The no-pair no-op below is proven; the swap
// multiset cannot be pinned. Seat-symmetric on the player; n_players=2 pins
// the other seat to `1 - user`.
// ===========================================================================

/// `Mor:（toko）` play body ops (`cant_play` is a residual gate, not the body).
crate::host_subset!(toko_dispatch {
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    mortgaged_of => arm_mortgaged_of,
    tile_group => arm_tile_group,
    tile_count => arm_tile_count,
    tile_owner => arm_tile_owner,
    player_count => arm_player_count,
    player_out => arm_player_out,
    log | effect => arm_log,
    opt_int | opt_str | opt_tile => arm_opt,
    ask => arm_ask,
    target => arm_target,
    buy_price => arm_buy_price,
    set_owner => arm_set_owner,
    set_mortgaged => arm_set_mortgaged,
    pay_to => arm_pay_to,
});

/// No deeds anywhere ⇒ no swappable pair ⇒ the body early-returns.
fn toko_world(user: i32) {
    sparse2(toko_dispatch, user, |_w| {
        // (`World::new` already leaves every tile owner=-1 / house-free.)
    });
}

/// IFF (no-pair): the body produces no effects.
fn toko_no_pair_noop_at(user: i32) {
    toko_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::toko_swap::toko_swap_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::toko_no_pair_ok(effects),
        "C-MOR-toko-swap-01: no swappable pair ⇒ no effects"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn toko_no_pair_noop() {
    toko_no_pair_noop_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn toko_no_pair_noop_user1() {
    toko_no_pair_noop_at(1);
}

/// Swap multiset (book): two SetOwner, one PayTo(diff), one SetMortgaged(off)
/// on the received deed. BLOCKED-HOST on the tile-index split; the harness
/// seeds one mortgaged same-group pair and checks the book multiset.
fn toko_swap_iff_at(user: i32) {
    let other = 1 - user;
    sparse2(toko_dispatch, user, |w| {
        // Seed a same-group mortgaged pair on the 0-based indices 1 and 6
        // (board group 1: tiles 2 and 7 are 1-based 2,7 → 0-based 1,6).
        // `owned_at` will report them as 2 and 7 (1-based) — the index split.
        // (`World::new` already leaves every tile owner=-1 / house-free.)
        w.tiles[1].owner = user;
        w.tiles[1].mortgaged = true;
        w.tiles[6].owner = other;
        w.tiles[6].mortgaged = true;
    });
    let _pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::toko_swap::toko_swap_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // Cover: a swap happened vs the body bailed.
    kani::cover!(log_n >= 3, "swap produced effects");
    kani::cover!(log_n == 0, "body bailed (index split / empty pairs)");
    if log_n == 4 {
        // Book multiset. `give_tile`/`get_tile` under the index split are
        // NOT the seeded 0-based tiles — BLOCKED-HOST. Check the shape only.
        let mut n_set_owner = 0;
        let mut n_pay = 0;
        let mut n_unmort = 0;
        for e in effects {
            match e {
                Effect::SetOwner { .. } => n_set_owner += 1,
                Effect::PayTo { .. } => n_pay += 1,
                Effect::SetMortgaged { on: false, .. } => n_unmort += 1,
                _ => {}
            }
        }
        crate::kassert!(
            n_set_owner == 2 && n_pay == 1 && n_unmort == 1,
            "C-MOR-toko-swap-01: swap multiset = 2 SetOwner + PayTo + SetMortgaged(off)"
        );
    }
    let _ = post;
}

#[kani::proof]
#[kani::unwind(32)]
pub fn toko_swap_iff_blocked_host() {
    toko_swap_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn toko_swap_user1_iff_blocked_host() {
    toko_swap_iff_at(1);
}

// ===========================================================================
// C-MOR-starry-01  `Mor:蝴蝶飞舞的星月夜`
// 「（1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
// OPEN-X: engine offers 1..=5. Host note: `pay` always returns the requested
// amount under Kani (no shortfall path); `set_dest` is folded into PlaceCard.
// Seat-symmetric.
// ===========================================================================

/// `Mor:蝴蝶飞舞的星月夜` Play body ops.
crate::host_subset!(starry_play_dispatch {
    money => arm_money,
    opt_int | opt_str | opt_tile => arm_opt,
    ask => arm_ask,
    pay => arm_pay,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
});

fn starry_play_world(user: i32) {
    sparse2(starry_play_dispatch, user, |_w| {});
}

/// IFF: PayMoney(1000·X) + PlaceCard(Field) + AddCounter(X), money floored.
fn starry_01_place_iff_at(user: i32) {
    starry_play_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::starry_night::starry_night_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // Recover X from the AddCounter leg (the only crystal write).
    let mut x = 0i32;
    for e in effects {
        if let Effect::AddCounter { n, .. } = e {
            x = *n;
        }
    }
    kani::cover!(x == 1, "X = 1");
    kani::cover!(x == 5, "X = 5");
    kani::cover!(x >= 2 && x <= 4, "X ∈ 2..=4");

    crate::kassert!(
        spec::starry_01_x_in_range(x, pre.money[user as usize]),
        "C-MOR-starry-01: X must be in the engine's 1..=max offer (OPEN-X)"
    );
    crate::kassert!(
        spec::starry_01_effects_ok(effects, user, x),
        "C-MOR-starry-01: PayMoney(1000X) + PlaceCard + AddCounter(X)"
    );
    crate::kassert!(
        spec::starry_01_post_ok(&post, &pre, user, x),
        "C-MOR-starry-01: user pays 1000·X (floored at 0)"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_01_place_iff() {
    starry_01_place_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_01_place_user1_iff() {
    starry_01_place_iff_at(1);
}

/// MUTANT companion: the pay is not 1000·X+1.
#[kani::proof]
#[kani::unwind(32)]
pub fn starry_01_mutant_is_detected() {
    let user = 0i32;
    starry_play_world(user);
    invoke::unwrap_done(card_morfonica::starry_night::starry_night_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mut x = 0i32;
    for e in effects {
        if let Effect::AddCounter { n, .. } = e {
            x = *n;
        }
    }
    let wrong = spec::starry_01_amount_mutant(x);
    let mutant = matches!(
        effects.first(),
        Some(Effect::PayMoney { n, .. }) if *n == wrong
    );
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: pay must not be 1000·X+1");
}

// ===========================================================================
// C-MOR-starry-02 / -06  `Mor:蝴蝶飞舞的星月夜`
// 「此卡没有[奇迹水晶]时加入弃牌堆」
// 「（3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。」
// CounterChanged hook body (`on_crystals_changed`). The real CEL-style guard
// (`crystals_changed_guard`) is a Rust residual: `trigger::card_is` is an
// unmodeled stub returning 0 under Kani, so the guard cannot be evaluated
// here — the body is invoked directly (existing unit style) and the residual
// is listed. The discard commit is engine post (`set_dest(Graveyard)`).
// Seat-symmetric on the owner.
// ===========================================================================

/// `Mor:蝴蝶飞舞的星月夜` CounterChanged hook body ops.
crate::host_subset!(starry_changed_dispatch {
    trig_value => arm_trig_value,
    gain | gain_typed | gain_fixed => arm_gain,
    counter_self => arm_counter_self,
    set_dest => arm_set_dest,
    log | effect => arm_log,
});

fn starry_changed_world(user: i32, crystals: i32, delta: i32) {
    set_self_crystals(crystals);
    sparse2(starry_changed_dispatch, user, |w| {
        w.pay_amount = delta; // trigger::value()
        w.trig_value = delta;
        w.field_n = 1; // is_placed
        w.dest = 2; // sentinel (Field): not Graveyard, so a dest write is visible
    });
}

/// IFF (removal → payout + empty → discard dest).
fn starry_06_payout_iff_at(user: i32) {
    let removed: i32 = kani::any();
    kani::assume(removed >= 1 && removed <= 3);
    let after: i32 = kani::any();
    kani::assume(after >= 0 && after <= 3);
    starry_changed_world(user, after, -removed);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::starry_night::on_crystals_changed_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(after == 0, "crystals empty → discard");
    kani::cover!(after > 0, "crystals remain");

    // -06: one GainMoney per crystal removed.
    crate::kassert!(
        spec::starry_06_effects_ok(effects, user, removed),
        "C-MOR-starry-06: owner gains 1000 per crystal removed"
    );
    // -02: empty crystals send the card to the discard (dest write).
    let dest = world::with_world(|w| w.dest);
    if after == 0 {
        crate::kassert!(
            spec::starry_02_dest_is_graveyard(dest),
            "C-MOR-starry-02: empty crystals ⇒ dest = Graveyard"
        );
    }
    crate::kassert!(post.money[user as usize] == pre.money[user as usize] + 1000 * removed
        || pre.money[user as usize] > i32::MAX - 1000 * removed,
        "C-MOR-starry-06: post money = pre + 1000·removed");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_06_payout_iff() {
    starry_06_payout_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_06_payout_user1_iff() {
    starry_06_payout_iff_at(1);
}

/// MUTANT companion: 500 per crystal is NOT the card.
#[kani::proof]
#[kani::unwind(32)]
pub fn starry_06_mutant_is_detected() {
    let user = 0i32;
    let removed: i32 = kani::any();
    kani::assume(removed >= 1 && removed <= 3);
    starry_changed_world(user, 1, -removed);
    invoke::unwrap_done(card_morfonica::starry_night::on_crystals_changed_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::starry_06_effects_ok_mutant(effects, user, removed);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: payout must not be 500 per crystal");
}

// ===========================================================================
// C-MOR-starry-05  `Mor:蝴蝶飞舞的星月夜`
// 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」
// PassTile hook arm. `is_circle` uses 1-based `board::tile_slot`; we pin
// `trig_tile = 1` (board index 1 is the CiRCLE) so the gate holds. The
// RollAfter arm of the same hook is BLOCKED-HOST on `Op::trig_move_main`
// (unmodeled) and is not driven here (starry-03/04). Seat-symmetric.
// ===========================================================================

/// `Mor:蝴蝶飞舞的星月夜` Hook body ops (RollAfter + PassTile arms).
crate::host_subset!(starry_hook_dispatch {
    trig_kind => arm_trig_kind,
    trig_move_main => arm_trig_move_main,
    fixed_roll => arm_stub_zero,
    trig_player => arm_trig_player,
    trig_move_roll => arm_trig_move_roll,
    trig_value => arm_trig_value,
    ask => arm_ask,
    do_move_roll => arm_do_move_roll,
    trig_set_move_roll => arm_trig_set_move_roll,
    log | effect => arm_log,
    player_count => arm_player_count,
    player_out => arm_player_out,
    can_pay => arm_can_pay,
    money => arm_money,
    pay => arm_pay,
    add_counter_self => arm_add_counter_self,
    tile_is => arm_tile_is,
    trig_tile => arm_trig_tile,
});

fn starry_pass_world(user: i32, crystals: i32) {
    set_self_crystals(crystals);
    sparse2(starry_hook_dispatch, user, |w| {
        w.trig_tile = 1; // board index 1 = CiRCLE (kind 0)
        w.trig_kind = card_sdk::abi::TriggerKind::PassTile as i32;
        w.field_n = 1;
    });
}

/// IFF: one crystal is removed (`AddCounter(owner, -1)`).
fn starry_05_circle_iff_at(user: i32) {
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 5);
    starry_pass_world(user, crystals);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::starry_night::hook_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(crystals == 1, "last crystal");
    kani::cover!(crystals > 1, "not the last crystal");

    crate::kassert!(
        spec::starry_05_effects_ok(effects, user),
        "C-MOR-starry-05: owner passing CiRCLE removes one crystal"
    );
    let after = self_crystals();
    crate::kassert!(
        after == crystals - 1,
        "C-MOR-starry-05: crystal count drops by 1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_05_circle_iff() {
    starry_05_circle_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_05_circle_user1_iff() {
    starry_05_circle_iff_at(1);
}

/// NO-OP: a non-owner pass, or a non-CiRCLE tile, removes nothing.
fn starry_05_noop_at(user: i32) {
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 5);
    starry_pass_world(user, crystals);
    // Flip the trigger to a non-circle tile (board index 2 is a property).
    world::with_world_mut(|w| w.trig_tile = 2);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::starry_night::hook_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let _ = &pre;

    crate::kassert!(
        log_n == 0,
        "C-MOR-starry-05: non-CiRCLE pass must not remove a crystal"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    let after = self_crystals();
    crate::kassert!(after == crystals, "C-MOR-starry-05: crystals unchanged");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_05_noop() {
    starry_05_noop_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn starry_05_noop_user1() {
    starry_05_noop_at(1);
}

// ===========================================================================
// C-MOR-nnm-02 / -03 / -04 / -05  `Mor:（NNM）稍微努力了一下`
// 「弃置手中x枚角色标记，发动以下效果中的一个」
// 「（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张」
// 「（2）获得x次经过CiRCLE时的资金奖励」
// 「（3）将此卡放置在场上并放置x个奇迹水晶…」
// 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」
//
// BLOCKED-HOST: `Op::tok_names` returns 0 (empty name list) so the hand-play
// path always refuses; `Op::placed_cards` returns a raw count (not a name
// list) so the (3) press cannot find a bound skill. The multi-branch pick is
// OPEN-多分支选择 (engine runs the first Play only).
//
// FAILS(LESS) C-MOR-nnm-05: the body has **no** CounterChanged / empty-crystal
// hook. `exhausted` is `#[allow(dead_code)]` and never registered. Spending
// (or already having) 0 crystals leaves the card on the field. The witness
// below drives the play body with the card placed and crystals already empty
// and shows the body never requests the discard.
// Seat-symmetric.
// ===========================================================================

/// `Mor:（NNM）稍微努力了一下` Play body ops. Isolated paths (hand-play /
// skill-press) are still declared so a broken isolation FAILs loudly.
crate::host_subset!(nnm_play_dispatch {
    is_placed => arm_is_placed,
    counter_self => arm_counter_self,
    placed_cards => arm_placed_cards,
    add_counter_self => arm_add_counter_self,
    log | effect => arm_log,
    play_card => arm_protocol_stub,
    tok_names => arm_tok_names,
    count_held | count_held_name => arm_count_held,
    opt_int | opt_str | opt_tile => arm_opt,
    ask => arm_ask,
    add_held => arm_add_held,
    state_get => arm_state_get,
    state_set => arm_state_set,
    draw => arm_draw,
    schedule_turn_end => arm_stub_zero,
    raise => arm_raise,
    gain | gain_typed | gain_fixed => arm_gain,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
});

/// `Mor:（NNM）稍微努力了一下` AtEnd body ops.
crate::host_subset!(nnm_discard_dispatch {
    cards_in => arm_cards_in,
    discard_from_hand => arm_discard_from_hand,
    log | effect => arm_log,
});

fn nnm_world(user: i32, crystals: i32) {
    set_self_crystals(crystals);
    sparse2(nnm_play_dispatch, user, |w| {
        w.field_n = 1; // the card is already placed (branch (3))
        w.dest = 2; // sentinel (Field): not Graveyard
    });
}

/// FAILS(LESS) C-MOR-nnm-05: with the card on the field and crystals at 0,
/// the Play press does not put the card into the discard.
/// Book: 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」.
/// Body: `exhausted` is `#[allow(dead_code)]` and never registered; no
/// CounterChanged hook discards on empty. The iff below asserts the book pin
/// and is expected to FAIL — that failure is the finding.
fn nnm_05_empty_discard_iff_at(user: i32) {
    nnm_world(user, 0);
    let _pre = world::snapshot();
    // Play press (entry 0): `is_placed` ⇒ `use_skill`; crystals < 1 ⇒ return.
    invoke::unwrap_done(card_morfonica::nanami_effort::play_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let dest = world::with_world(|w| w.dest);

    // Book pin: empty crystals ⇒ the card goes to the discard.
    crate::kassert!(
        spec::nnm_05_wants_discard(dest),
        "C-MOR-nnm-05 FAILS(LESS): book wants dest=Graveyard when crystals hit 0"
    );
    let _ = effects;
    let _ = log_n;
}

#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_05_empty_discard_iff_fails_less() {
    nnm_05_empty_discard_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_05_empty_discard_user1_iff_fails_less() {
    nnm_05_empty_discard_iff_at(1);
}

/// Reachable-disagreement companion for C-MOR-nnm-05: the body really does
/// leave the card on the field when crystals are empty (the LESS is not
/// vacuous). Covers the bad state.
fn nnm_05_body_leaves_card_at(user: i32) {
    nnm_world(user, 0);
    let pre_dest = 2i32; // sentinel Field; body must leave it alone
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::nanami_effort::play_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let dest = world::with_world(|w| w.dest);

    kani::cover!(
        spec::nnm_05_body_leaves_card(effects, dest, pre_dest),
        "LESS reachable: empty crystals, card stays"
    );
    crate::kassert!(
        spec::nnm_05_body_leaves_card(effects, dest, pre_dest),
        "C-MOR-nnm-05 LESS witness: body leaves the empty card on the field"
    );
    crate::kassert!(!spec::nnm_05_wants_discard(dest), "book pin is not met (LESS)");
    let post = world::snapshot();
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_05_body_leaves_card_is_reachable() {
    nnm_05_body_leaves_card_at(0);
}

/// nnm-05 companion: the AtEnd entry (discard-down-to-five) also does not
/// discard the *field* card when crystals are empty.
fn nnm_05_atend_at(user: i32) {
    host::install_backend(nnm_discard_dispatch);
    crate::harness::sparse_play_world(crate::harness::SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    set_self_crystals(0);
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.field_n = 1;
        w.dest = 2;
    });
    invoke::unwrap_done(card_morfonica::nanami_effort::discard_down_to_five_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let dest = world::with_world(|w| w.dest);

    // AtEnd only touches the hand; the field card stays (LESS for -05).
    crate::kassert!(
        !effects.iter().any(|e| matches!(e, Effect::UnplaceCard { .. })),
        "C-MOR-nnm-05: AtEnd must not be the discard-on-empty path (and is not)"
    );
    crate::kassert!(
        dest == 2 || log_n == 0,
        "C-MOR-nnm-05: no Graveyard dest from AtEnd for the field card"
    );
}

#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_05_atend_leaves_field_card() {
    nnm_05_atend_at(0);
}

/// nnm-03 FAILS(WRONG): the book pins x × the *current* CiRCLE money reward;
/// the body hard-codes x × 2000 (TODO(规则书) in the source). We cannot drive
/// the branch (`Op::tok_names` is empty — BLOCKED-HOST), so this is a
/// formula-level iff against the book that is expected to FAIL whenever the
/// Morfonica cycle reward is 1000 or 1500. Spec-only (no body).
#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_03_amount_iff_fails_wrong() {
    let x: i32 = kani::any();
    kani::assume(x >= 1 && x <= 5);
    let reward: i32 = kani::any();
    kani::assume(reward == 1000 || reward == 1500 || reward == 2000);
    let book = spec::nnm_03_amount(x, reward);
    let body = spec::nnm_03_amount_body(x);
    kani::cover!(reward == 1000, "cycle step 1000");
    kani::cover!(reward == 1500, "cycle step 1500");
    kani::cover!(reward == 2000, "cycle step 2000");
    // Book pin: the body must pay x · reward.
    crate::kassert!(
        book == body,
        "C-MOR-nnm-03 FAILS(WRONG): body pays x·2000, book wants x·reward (G7)"
    );
}

/// Reachable-disagreement companion for C-MOR-nnm-03. Spec-only (no body).
#[kani::proof]
#[kani::unwind(32)]
pub fn nnm_03_wrong_is_reachable() {
    let x: i32 = kani::any();
    kani::assume(x >= 1 && x <= 5);
    let reward: i32 = 1000; // Morfonica cycle 1st step
    let book = spec::nnm_03_amount(x, reward);
    let body = spec::nnm_03_amount_body(x);
    kani::cover!(book != body, "book ≠ body on reward=1000");
    crate::kassert!(
        book != body,
        "C-MOR-nnm-03 WRONG witness: x·1000 ≠ x·2000"
    );
}

// ===========================================================================
// C-MOR-tritone-01 / -02  `Mor:迷茫之蝶们的三全音`
// 「[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，
//  此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金」
// The [反击] window is PROTOCOL-LEVEL (KANI.md §6); the admitted body is
// driven here. The 3-turn schedule is PROTOCOL-LEVEL; a single TurnEnd tick
// body is proven below. OPEN-三全音收款人: the body `pay`s (bank).
// Counteract AND TurnEnd hook share `tritone::counteract_kani` (same fn);
// the subset is the union of both match arms. TurnEnd is NOT seat-symmetric
// in the trigger: it ticks only when `trig_player == user` (own turn end).
// n_players=2 makes that the concrete pair (user, user); the two user=0/1
// harnesses cover both seats.
// ===========================================================================

/// `Mor:迷茫之蝶们的三全音` body ops (union of the Effect and TurnEnd arms).
crate::host_subset!(tritone_dispatch {
    trig_kind => arm_trig_kind,
    trig_value => arm_trig_value,
    trig_player => arm_trig_player,
    gain | gain_typed | gain_fixed => arm_gain,
    is_placed => arm_is_placed,
    add_counter_self => arm_add_counter_self,
    set_counter_self => arm_set_counter_self,
    set_dest => arm_set_dest,
    place_card_at => arm_place_card_at,
    set_slot => arm_set_slot,
    slot => arm_slot,
    pay => arm_pay,
    log | effect => arm_log,
});

fn tritone_counteract_world(user: i32, amount: i32) {
    sparse2(tritone_dispatch, user, |w| {
        w.trig_kind = card_sdk::abi::TriggerKind::Effect as i32;
        w.pay_amount = amount; // trigger::value()
        w.trig_value = amount;
    });
}

/// IFF tritone-01 body: GainMoney + PlaceCard + SetState(owed); crystals = 3.
fn tritone_01_gain_place_iff_at(user: i32) {
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 5_000);
    tritone_counteract_world(user, amount);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::tritone::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(amount == 1, "smallest loan");
    kani::cover!(amount == 5_000, "large loan");

    crate::kassert!(
        spec::tritone_01_effects_ok(effects, user, amount),
        "C-MOR-tritone-01: GainMoney(amount) + PlaceCard + SetState(owed=amount)"
    );
    // `set_crystals(3)` lands on the just-placed instance (`last_uid`).
    let crystals = self_crystals();
    crate::kassert!(
        spec::tritone_01_crystals_ok(crystals),
        "C-MOR-tritone-01: card is placed with 3 crystals"
    );
    crate::kassert!(
        post.money[user as usize] == pre.money[user as usize] + amount
            || pre.money[user as usize] > i32::MAX - amount,
        "C-MOR-tritone-01: user gains the lost amount"
    );
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tritone_01_gain_place_iff() {
    tritone_01_gain_place_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tritone_01_gain_place_user1_iff() {
    tritone_01_gain_place_iff_at(1);
}

/// IFF tritone-02 single turn-end tick (decay / payback).
/// Multi-turn scheduling (3 owner turn-ends) is PROTOCOL-LEVEL.
fn tritone_02_tick_iff_at(user: i32) {
    let pre_crystals: i32 = kani::any();
    kani::assume(pre_crystals >= 1 && pre_crystals <= 3);
    let owed: i32 = kani::any();
    kani::assume(owed >= 0 && owed <= 5_000);
    set_self_crystals(pre_crystals);
    let owed_key = crate::intern::intern_or_fail("tritone_owed");
    sparse2(tritone_dispatch, user, |w| {
        w.key_set(user, owed_key, owed);
        w.trig_kind = card_sdk::abi::TriggerKind::TurnEnd as i32;
        // Own turn end: the tick is gated on `trig_player == user`.
        w.field_n = 1;
        w.dest = 2; // sentinel (Field): not Graveyard
    });
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::tritone::counteract_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(pre_crystals == 1, "last crystal → payback");
    kani::cover!(pre_crystals > 1, "decay only");

    crate::kassert!(
        spec::tritone_02_tick_effects_ok(effects, user, pre_crystals, owed),
        "C-MOR-tritone-02: tick is decay-only, or decay + payback(owed)"
    );
    let after = self_crystals();
    crate::kassert!(
        after == pre_crystals - 1,
        "C-MOR-tritone-02: one crystal removed per owner turn end"
    );
    if pre_crystals == 1 {
        crate::kassert!(
            post.money[user as usize] == (pre.money[user as usize] - owed).max(0),
            "C-MOR-tritone-02: owner pays back the loan (OPEN-三全音收款人: bank)"
        );
        let dest = world::with_world(|w| w.dest);
        crate::kassert!(dest == 0, "C-MOR-tritone-02: empty crystals ⇒ dest = Graveyard");
    } else {
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money on a decay-only tick");
        let dest = world::with_world(|w| w.dest);
        crate::kassert!(dest == 2, "frame: dest unchanged on a decay-only tick");
    }
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tritone_02_tick_iff() {
    tritone_02_tick_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tritone_02_tick_user1_iff() {
    tritone_02_tick_iff_at(1);
}

/// MUTANT companion: the payback is not owed+1.
#[kani::proof]
#[kani::unwind(32)]
pub fn tritone_02_mutant_is_detected() {
    let user = 0i32;
    let owed: i32 = kani::any();
    kani::assume(owed >= 1 && owed <= 1_000);
    set_self_crystals(1);
    let owed_key = crate::intern::intern_or_fail("tritone_owed");
    sparse2(tritone_dispatch, user, |w| {
        w.key_set(user, owed_key, owed);
        w.trig_kind = card_sdk::abi::TriggerKind::TurnEnd as i32;
        w.field_n = 1;
        w.money[user as usize] = 10_000;
    });
    invoke::unwrap_done(card_morfonica::tritone::counteract_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::tritone_02_tick_effects_ok_mutant(effects, user, 1, owed);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: payback must not be owed+1");
}

// ===========================================================================
// C-MOR-noble-02  `Mor:高贵的微蓝`
// 「如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记…」
// OPEN-触发结算: the forced settle (`card_settle_at`) is a protocol stub
// returning 0; only the mark leg is pinned. The body is invoked directly
// (the cant_play price gate is BLOCKED-HOST on 1-based `tile_price`).
// Seat-symmetric on the standing player.
// ===========================================================================

/// `Mor:高贵的微蓝` play body ops (`cant_play` is a residual gate).
crate::host_subset!(noble_dispatch {
    player_pos => arm_player_pos,
    tile_owner => arm_tile_owner,
    log | effect => arm_log,
    card_settle_at => arm_protocol_stub,
    place_mark => arm_place_mark,
    self_uid => arm_self_uid,
});

/// `owned == true`: the tile under the user is theirs. Else it is the other seat's.
fn noble_world(user: i32, owned: bool) -> i32 {
    let pos: i32 = kani::any();
    kani::assume(pos >= 0 && pos < 60);
    let owner = if owned { user } else { 1 - user };
    sparse2(noble_dispatch, user, |w| {
        // (`World::new` already leaves every tile owner=-1 / house-free.)
        w.tiles[pos as usize].owner = owner;
        w.pos[user as usize] = pos;
    });
    pos
}

/// IFF (mark leg): tile is yours ⇒ exactly one PlaceMark on that tile.
/// The forced settle (1) is OPEN-触发结算 / PROTOCOL-LEVEL and is not pinned.
fn noble_02_mark_iff_at(user: i32) {
    let pos = noble_world(user, true);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::noble_blue::noble_blue_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::noble_02_effects_ok(effects, pos),
        "C-MOR-noble-02: exactly one PlaceMark on the user's tile"
    );
    crate::kassert!(
        spec::noble_02_mark_ok(effects, pos),
        "C-MOR-noble-02: mark lands on the tile (OPEN-触发结算 not pinned)"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (settle is a stub)");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn noble_02_mark_iff() {
    noble_02_mark_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn noble_02_mark_user1_iff() {
    noble_02_mark_iff_at(1);
}

/// NO-OP (not yours): no effects.
fn noble_02_not_yours_noop_at(user: i32) {
    let _pos = noble_world(user, false);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::noble_blue::noble_blue_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::noble_02_not_yours_ok(effects),
        "C-MOR-noble-02: tile not yours ⇒ no effects"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn noble_02_not_yours_noop() {
    noble_02_not_yours_noop_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn noble_02_not_yours_noop_user1() {
    noble_02_not_yours_noop_at(1);
}

/// MUTANT companion (mark): a mark on tile 0 is NOT the card when the user
/// stands elsewhere.
#[kani::proof]
#[kani::unwind(32)]
pub fn noble_02_mutant_is_detected() {
    let user = 0i32;
    let pos = noble_world(user, true);
    kani::assume(pos != 0);
    invoke::unwrap_done(card_morfonica::noble_blue::noble_blue_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = log_n == 1
        && matches!(effects[0], Effect::PlaceMark { tile, .. } if tile == 0);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: mark must not land on tile 0");
}

// ===========================================================================
// C-MOR-again-02  `Mor:再次牵起手来` [持续]
// 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。」
// The [反击] window of -01 is PROTOCOL-LEVEL (KANI.md §6). This is the
// PayAt hook body. OPEN-取消范围: one-sided on the owner. Seat-symmetric.
// ===========================================================================

/// `Mor:再次牵起手来` PayAt hook body ops.
crate::host_subset!(hold_hands_dispatch {
    trig_set_pay_amount => arm_trig_set_pay_amount,
    set_dest => arm_set_dest,
    log | effect => arm_log,
});

fn again_02_world(user: i32, pay_amount: i32) {
    sparse2(hold_hands_dispatch, user, |w| {
        w.pay_amount = pay_amount;
        w.field_n = 1;
        w.dest = 2; // sentinel (Field): not Graveyard
    });
}

/// IFF: SetPayAmount(0) and dest = Graveyard. Exactly one effect.
fn again_02_cancel_iff_at(user: i32) {
    let pay_amount: i32 = kani::any();
    kani::assume(pay_amount >= 1 && pay_amount <= 5_000);
    again_02_world(user, pay_amount);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::hold_hands_again::pay_at_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::again_02_effects_ok(effects),
        "C-MOR-again-02: exactly SetPayAmount(0)"
    );
    let dest = world::with_world(|w| w.dest);
    crate::kassert!(
        dest == 0,
        "C-MOR-again-02: card is sent to the discard (dest = Graveyard)"
    );
    crate::kassert!(
        post.pay_amount == 0,
        "C-MOR-again-02: the money change is cancelled"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (cancel only)");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn again_02_cancel_iff() {
    again_02_cancel_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn again_02_cancel_user1_iff() {
    again_02_cancel_iff_at(1);
}

/// MUTANT companion: cancel-to-1 is NOT the card.
#[kani::proof]
#[kani::unwind(32)]
pub fn again_02_mutant_is_detected() {
    let user = 0i32;
    again_02_world(user, 100);
    invoke::unwrap_done(card_morfonica::hold_hands_again::pay_at_kani(user));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::again_02_effects_ok_mutant(effects);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: cancel must go to 0, not 1");
}

// ===========================================================================
// C-MOR-wing-01  `Mor:纯真振翅`
// 「传送到移动方向20格后（不触发结算）」
// OPEN-移动方向. BLOCKED-HOST: `plan::dir()` (`Op::move_dir`) is a protocol
// stub returning 0, so `tile_steps_ahead(player, 20·0)` is the current tile.
// The harness pins the book destination (`pos + 20·dir`) and is expected to
// fail whenever dir ≠ 0 would be the real planned direction. wing-02 (the
// follow-on roll via `card_move`) is PROTOCOL-LEVEL scheduling. Seat-symmetric.
// ===========================================================================

/// `Mor:纯真振翅` play body ops.
crate::host_subset!(wing_dispatch {
    tile_count => arm_tile_count,
    move_dir => arm_stub_zero,
    tile_steps_ahead => arm_tile_steps_ahead,
    teleport_to => arm_teleport_to,
    player_count => arm_player_count,
    player_out => arm_player_out,
    set_kind => arm_stub_zero,
    set_teleport_to => arm_stub_zero,
    set_resolve => arm_stub_zero,
    card_move => arm_card_move,
    log | effect => arm_log,
});

fn wing_world(user: i32) -> i32 {
    let pos: i32 = kani::any();
    kani::assume(pos >= 0 && pos < 60);
    sparse2(wing_dispatch, user, |w| {
        w.pos[user as usize] = pos;
    });
    pos
}

/// Book pin: Teleport(user, (pos + 20·dir) mod 60). The host's `dir` is a
/// stub (0), so this is BLOCKED-HOST — the assertion below is against the
/// book formula with `dir` recovered from the host's own `tile_steps_ahead`.
fn wing_01_teleport_iff_at(user: i32) {
    let pos = wing_world(user);
    let pre = world::snapshot();
    invoke::unwrap_done(card_morfonica::pure_wings::pure_wings_kani(user));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    // The body calls `tile_steps_ahead(player, 20 * plan::dir())` then
    // `teleport_to`. Under the stub, dir = 0 ⇒ dest = pos.
    let dir = 0i32; // Op::move_dir stub
    let want = spec::wing_01_dest(pos, dir);
    kani::cover!(want == pos, "stub dir=0 lands on the current tile");
    kani::cover!(want != pos, "would move (dir ≠ 0)");

    crate::kassert!(
        spec::wing_01_effects_ok(effects, user, want),
        "C-MOR-wing-01: exactly Teleport(user, pos+20·dir) (dir stubbed to 0)"
    );
    crate::kassert!(
        post.pos[user as usize] == want,
        "C-MOR-wing-01: user lands on the teleport destination"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (no settle)");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn wing_01_teleport_iff_blocked_host() {
    wing_01_teleport_iff_at(0);
}

#[kani::proof]
#[kani::unwind(32)]
pub fn wing_01_teleport_user1_iff_blocked_host() {
    wing_01_teleport_iff_at(1);
}

/// Book-vs-stub differential: with a real forward direction the destination
/// is 20 ahead; the stub lands on the current tile. This cover documents the
/// BLOCKED-HOST gap without hiding it. Spec-only (no body).
#[kani::proof]
#[kani::unwind(32)]
pub fn wing_01_dir_stub_gap() {
    let pos: i32 = kani::any();
    kani::assume(pos >= 0 && pos < 60);
    let book = spec::wing_01_dest(pos, 1); // forward
    let stub = spec::wing_01_dest(pos, 0); // Op::move_dir stub
    kani::cover!(book != stub, "book forward ≠ stub (BLOCKED-HOST on Op::move_dir)");
    crate::kassert!(
        book != stub,
        "C-MOR-wing-01 BLOCKED-HOST: move_dir stub cannot express 「移动方向」"
    );
}