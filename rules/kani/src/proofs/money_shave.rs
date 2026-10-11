//! Harness (a): pure money-shave hook, guard evaluated by the single-source
//! IR (no `assume` of the host condition).
//!
//! Clause `C-PP-aya-haruka-02` (`docs/rulebook/spec/cards/pp.md`):
//! > 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
//! >  将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」
//!
//! Trigger: `On::Hook(PayAdd)` with the single-source condition
//! `"actor == owner && value > 0"` — the SAME string the host compiles.
//! Under Kani we lower that string to the heap-free IR once (generated) and
//! evaluate it against a symbolic context. When the guard admits we invoke
//! the unmodified body and prove the clause; when it rejects we assert the
//! host would not have called the body (cover + no-op).
//!
//! Isolation: no `PP:[衍生]共鸣` in hand, so clause (2)'s stacked 1500
//! shave cannot fire (`try_resonance` returns false before any prompt).

use crate::host;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn symbolic_pay_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = kani::any();
        w.fans_pos[p] = kani::any();
        kani::assume(w.fans_pos[p] >= 0 && w.fans_pos[p] <= 20);
        w.fans_neg[p] = kani::any();
        kani::assume(w.fans_neg[p] >= 0 && w.fans_neg[p] <= 20);
        // clause-(2) isolation
        w.hand_resonance[p] = 0;
    }
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    kani::assume(!w.out[owner as usize]);
    w.trig_player = owner;
    // `value` is fully symbolic — including `<= 0`. The guard IR decides.
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= 0 && w.trig_value <= 10_000);
    w.pay_amount = w.trig_value;
    w
}

/// Post-state + effect log are exactly the clause **when the single-source
/// guard admits**. When it rejects, the body is not run (the host's
/// `admits()` gate) and the log is empty by construction.
#[kani::proof]
#[kani::unwind(24)]
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
        assert!(
            !(actor == owner && value > 0),
            "guard rejected but actor==owner && value>0 — translator bug"
        );
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::aya_longing_pay_add(owner));

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
        assert!(
            post.pay_amount == expected,
            "C-PP-aya-haruka-02: pay amount must be max(pre-X, 0)"
        );
        assert!(
            spec::aya_haruka_02_log_ok(effects, expected),
            "C-PP-aya-haruka-02: effect log must be exactly the pay-amount rewrite"
        );
    } else {
        assert!(
            log_n == 0,
            "C-PP-aya-haruka-02: non-poorest owner must produce no effects"
        );
        assert!(
            post.pay_amount == pre_amount,
            "C-PP-aya-haruka-02: non-poorest owner must not touch the amount"
        );
    }

    assert!(post.money == pre.money, "frame: money unchanged at rewrite time");
    assert!(post.pos == pre.pos, "frame: positions unchanged");
    assert!(post.fire == pre.fire, "frame: fire unchanged");
    assert!(post.stay == pre.stay, "frame: stay unchanged");
    assert!(post.hand == pre.hand, "frame: hand unchanged");
    assert!(post.discard == pre.discard, "frame: discard unchanged");
}

/// MUTANT: the reference wants X+1. The card does X. Kani must report FAIL.
#[kani::proof]
#[kani::unwind(24)]
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
    invoke::unwrap_done(invoke::aya_longing_pay_add(owner));
    let post = world::snapshot();
    let x = spec::aya_haruka_02_x_mutant(
        pre.fans_pos[owner as usize],
        pre.fans_neg[owner as usize],
    );
    let expected = spec::aya_haruka_02_amount(pre_amount, x);
    assert!(post.pay_amount == expected);
}

/// Companion (no `should_panic`): the mutant spec does NOT match the card.
/// If this FAILS, the off-by-one is live — the expected outcome.
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_02_mutant_is_detected() {
    let w = symbolic_pay_world();
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
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
    invoke::unwrap_done(invoke::aya_longing_pay_add(owner));
    let post = world::snapshot();
    let x = spec::aya_haruka_02_x_mutant(
        pre.fans_pos[owner as usize],
        pre.fans_neg[owner as usize],
    );
    let expected = spec::aya_haruka_02_amount(pre_amount, x);
    // The card does X, the mutant wants X+1: they must differ.
    assert!(post.pay_amount != expected);
}