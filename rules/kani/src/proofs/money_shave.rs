//! Harness (a): pure money-shave hook, guard evaluated by the single-source
//! IR (no `assume` of the host condition).
//!
//! Clause `C-PP-aya-haruka-02` (`docs/rulebook/spec/cards/pp.md`):
//! > 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
//! >  将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」
//!
//! Trigger: the `On::Filter(PaySplitAdd)` body (its registration guard is
//! `"card.placed"`). The body's own residual condition
//! `"actor == owner && value > 0"` is a single-source CEL string; under Kani
//! we lower it to the heap-free IR once (generated) and evaluate it against a
//! symbolic context. When it admits we invoke the unmodified filter body and
//! prove the clause; when it rejects the body no-ops, so the proof skips the
//! call (cover). `card.placed` itself is not evaluated -- a filter only runs
//! while its card is on the field.
//!
//! Isolation: no `PP:[衍生]共鸣` in hand, so clause (2)'s stacked 1500
//! shave cannot fire (`try_resonance` returns false before any prompt).

/// Run the clause-(1) `PaySplitAdd` filter and apply the host's
/// `drive_payload` write (`PayAddValue::write`): `set_pay_amount` +
/// `set_price`, only when the figure changed.
fn run_shave_filter(owner: i32, value: i32) -> i32 {
    let out = invoke::filter_pay_add_only_idx(
        crate::card_id!("PP:[丸山彩]憧憬的前方"),
        card_sdk::abi::FilterKind::PaySplitAdd,
        owner,
        card_sdk::PayAddValue { value, payee: None },
    );
    if out.value != value {
        card_sdk::ctx::trigger::set_pay_amount(out.value);
        card_sdk::ctx::trigger::set_price(out.value);
    }
    out.value
}

use crate::host;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

/// Symbolic PaySplitAdd world for C-PP-aya-haruka-02.
///
/// State restrictions (each is a documented reduction in the clause's terms,
/// never a silent cut of reachable behaviour):
///
/// * **owner's [P✽P粉丝] (either face) fully symbolic in `0..=20`** — that is
///   the X threshold (「至少10个」). Other seats' fans are **not read** by
///   `shave_x` / `poorest` / `set_pay_amount`, so they stay at `World::new`'s 0.
/// * **money of every living seat symbolic in `0..=10_000`** — `poorest` is a
///   pure comparison (「资金数是所有存活玩家中最少」); the shave amount does
///   not read money. The upper bound only keeps the host pay-clamp out of
///   play (same isolation as the other event harnesses).
/// * **`out[]` symbolic** — 「存活玩家」; `player_out` short-circuits seats
///   `>= n_players` under the §7 `player_count()==4` model, so this covers
///   2..=4-player worlds' out-sets.
/// * **`hand_resonance[] == 0`** — isolation of clause (2)'s stacked 1500
///   shave (`try_resonance` returns false before any prompt).
/// * **`trig_value` symbolic in `0..=10_000`** — the guard requires `value > 0`;
///   including `<= 0` lets the IR reject.
fn symbolic_pay_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = kani::any();
        // clause-(2) isolation
        w.hand_resonance[p] = 0;
    }
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    kani::assume(!w.out[owner as usize]);
    let o = owner as usize;
    w.fans_pos[o] = kani::any();
    kani::assume(w.fans_pos[o] >= 0 && w.fans_pos[o] <= 20);
    w.fans_neg[o] = kani::any();
    kani::assume(w.fans_neg[o] >= 0 && w.fans_neg[o] <= 20);
    w.trig_player = owner;
    // `value` is fully symbolic — including `<= 0`. The guard IR decides.
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= 0 && w.trig_value <= 10_000);
    w.pay_amount = w.trig_value;
    w
}

/// Split helper: assume the owner **is** the poorest living player
/// (「[拥有者]的资金数是所有存活玩家中最少」). Stated as a money comparison,
/// not as an engine flag.
fn assume_poorest(w: &World) {
    let owner = w.trig_player as usize;
    let me = w.money[owner];
    for p in 0..MAX_PLAYERS {
        // `player_out` is true for `p >= n_players` under Kani regardless of
        // the raw `out` bits; the phantom seats must be skipped here too.
        if p != owner && p < w.n_players as usize && !w.out[p] {
            kani::assume(w.money[p] >= me);
        }
    }
}

/// Split helper: assume some living other player is strictly poorer.
fn assume_not_poorest(w: &World) {
    let owner = w.trig_player as usize;
    let me = w.money[owner];
    let mut poorer = false;
    for p in 0..MAX_PLAYERS {
        if p != owner && p < w.n_players as usize && !w.out[p] && w.money[p] < me {
            poorer = true;
        }
    }
    kani::assume(poorer);
}

/// Post-state + effect log are exactly the clause **when the single-source
/// guard admits**. When it rejects, the body is not run (the host's
/// `admits()` gate) and the log is empty by construction.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_haruka_02_shave_iff() {
    let w = symbolic_pay_world();
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;

    // ---- single-source guard: the real CEL string, lowered once ----
    // Const id — never a 139-way string scan under CBMC.
    let gid = crate::guard_id!("actor == owner && value > 0");
    let ctx = ir_fill::fill_pay_add(&w, owner);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    kani::cover!(!admitted, "guard rejects");

    if !admitted {
        // Host would not call the body. Nothing to prove about the body;
        // assert the precondition we rely on: value <= 0 or actor != owner.
        use rules_cond_ir::vars::VarId;
        let actor = ctx.scalars[VarId::Actor as usize];
        let owner = ctx.scalars[VarId::Owner as usize];
        let value = ctx.scalars[VarId::Value as usize];
        crate::kassert!(
            !(actor == owner && value > 0),
            "guard rejected but actor==owner && value>0 — translator bug"
        );
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    run_shave_filter(owner, pre_amount);

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    let x = spec::aya_haruka_02_x(pre.fans_pos[owner as usize], pre.fans_neg[owner as usize]);
    let poorest = spec::is_poorest(&pre.money, &pre.out, pre.n_players, owner);
    kani::cover!(poorest, "owner is poorest (shave applies)");
    kani::cover!(!poorest, "owner is not poorest (no-op)");

    if poorest {
        let expected = spec::aya_haruka_02_amount(pre_amount, x);
        crate::kassert!(
            post.pay_amount == expected,
            "C-PP-aya-haruka-02: pay amount must be max(pre-X, 0)"
        );
        crate::kassert!(
            spec::aya_haruka_02_log_ok(effects, expected),
            "C-PP-aya-haruka-02: effect log must be exactly the pay-amount rewrite"
        );
    } else {
        crate::kassert!(
            log_n == 0,
            "C-PP-aya-haruka-02: non-poorest owner must produce no effects"
        );
        crate::kassert!(
            post.pay_amount == pre_amount,
            "C-PP-aya-haruka-02: non-poorest owner must not touch the amount"
        );
    }

    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money unchanged at rewrite time"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "frame: positions unchanged"
    );
    crate::kassert!(
        world::arr4_eq(post.charge, pre.charge),
        "frame: fire unchanged"
    );
    crate::kassert!(post.stay == pre.stay, "frame: stay unchanged");
    crate::kassert!(
        world::arr8b_eq(post.hand, pre.hand),
        "frame: hand unchanged"
    );
    crate::kassert!(post.discard == pre.discard, "frame: discard unchanged");
}

/// SPLIT of `aya_haruka_02_shave_iff` (poorest half). Same clause, narrower
/// assume — cheaper for the solver, no behaviour hidden: the iff's other half
/// is `aya_haruka_02_shave_not_poorest_noop`.
///
/// Together the two halves are the clause:
/// * **guard** (`actor == owner && value > 0`): proved in `shave_iff` (both
///   admit and reject) and `wrong_guard` (the single-source string). The
///   splits assume the residual condition holds (`On::Filter(PaySplitAdd)`
///   body invoked) so they do not re-evaluate the IR — that is a setup cut,
///   not a weaker claim.
/// * **poorest** (「资金数是所有存活玩家中最少」 ⇒ shave by X): this half.
/// * **not-poorest** ⇒ no-op: the other half.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_haruka_02_shave_poorest() {
    let w = symbolic_pay_world();
    assume_poorest(&w);
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
    // Host already invoked the body: guard terms hold (`shave_iff` proves
    // the IR agrees). No `fill_pay_add` / `guard!` here — that circuit is the
    // split's remaining cliff after the Msg/memcmp/by_idx cuts.
    kani::assume(w.trig_value > 0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    run_shave_filter(owner, pre_amount);
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let x = spec::aya_haruka_02_x(pre.fans_pos[owner as usize], pre.fans_neg[owner as usize]);
    let expected = spec::aya_haruka_02_amount(pre_amount, x);
    crate::kassert!(
        post.pay_amount == expected,
        "C-PP-aya-haruka-02: pay amount must be max(pre-X, 0)"
    );
    crate::kassert!(
        spec::aya_haruka_02_log_ok(effects, expected),
        "C-PP-aya-haruka-02: effect log must be exactly the pay-amount rewrite"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "frame: money unchanged at rewrite time"
    );
}

/// SPLIT of `aya_haruka_02_shave_iff` (not-poorest half): the body must
/// no-op (「如果[拥有者]的资金数是所有存活玩家中最少则…」 — the shave is
/// gated on the poorest reading).
///
/// Guard admission is assumed (host invoked the body); see `shave_poorest`
/// for how the two halves + `shave_iff` cover the full clause.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_haruka_02_shave_not_poorest_noop() {
    let w = symbolic_pay_world();
    assume_not_poorest(&w);
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
    kani::assume(w.trig_value > 0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    run_shave_filter(owner, pre_amount);
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    crate::kassert!(
        log_n == 0,
        "C-PP-aya-haruka-02: non-poorest owner must produce no effects"
    );
    crate::kassert!(
        post.pay_amount == pre_amount,
        "C-PP-aya-haruka-02: non-poorest owner must not touch the amount"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// MUTANT: the reference wants X+1. The card does X. Kani must report FAIL.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn aya_haruka_02_mutant_x_off_by_one() {
    let w = symbolic_pay_world();
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
    // Pin the admitted + poorest region with plain assumptions (the guard IR
    // itself is proved in `aya_haruka_02_shave_iff` and `wrong_guard.rs`).
    kani::assume(w.trig_player == owner);
    kani::assume(pre_amount >= 200);
    for p in 0..MAX_PLAYERS {
        if p != owner as usize {
            kani::assume(w.out[p] || w.money[p] >= w.money[owner as usize]);
        }
    }
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    run_shave_filter(owner, pre_amount);
    let post = world::snapshot();
    let x =
        spec::aya_haruka_02_x_mutant(pre.fans_pos[owner as usize], pre.fans_neg[owner as usize]);
    let expected = spec::aya_haruka_02_amount(pre_amount, x);
    crate::kassert!(post.pay_amount == expected);
}

/// Companion (no `should_panic`): the mutant spec does NOT match the card.
/// Restricted to amounts the shave does not floor (where X and X+1 differ).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn aya_haruka_02_mutant_is_detected() {
    let w = symbolic_pay_world();
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
    kani::assume(w.trig_player == owner);
    kani::assume(pre_amount >= 200);
    // Stay off the 0 floor: `max(pre - X, 0)` and `max(pre - (X+1), 0)`
    // coincide when the shave bottoms out, so the off-by-one is only
    // observable while some amount survives even the mutant's X+1.
    let xm = spec::aya_haruka_02_x_mutant(w.fans_pos[owner as usize], w.fans_neg[owner as usize]);
    kani::assume(pre_amount > xm);
    for p in 0..MAX_PLAYERS {
        if p != owner as usize {
            kani::assume(w.out[p] || w.money[p] >= w.money[owner as usize]);
        }
    }
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    run_shave_filter(owner, pre_amount);
    let post = world::snapshot();
    let expected = spec::aya_haruka_02_amount(pre_amount, xm);
    // The card does X, the mutant wants X+1: they must differ.
    crate::kassert!(post.pay_amount != expected);
}
