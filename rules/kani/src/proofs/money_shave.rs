//! Harness (a): pure money-shave hook.
//!
//! Clause `C-PP-aya-haruka-02` (`docs/rulebook/spec/cards/pp.md`):
//! > 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
//! >  将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」
//!
//! Trigger: `On::Hook(PayAdd)` with the host-side condition
//! `actor == owner && value > 0` (so we assume `value > 0`).
//! Isolation: no `PP:[衍生]共鸣` in hand, so clause (2)'s stacked 1500
//! shave cannot fire (`try_resonance` returns false before any prompt).

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn symbolic_pay_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume((0..=10_000).contains(&w.money[p]));
        w.out[p] = kani::any();
        w.fans_pos[p] = kani::any();
        kani::assume((0..=20).contains(&w.fans_pos[p]));
        w.fans_neg[p] = kani::any();
        kani::assume((0..=20).contains(&w.fans_neg[p]));
        // clause-(2) isolation
        w.hand_resonance[p] = 0;
    }
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    kani::assume(!w.out[owner as usize]);
    w.trig_player = owner;
    w.trig_value = kani::any();
    kani::assume(w.trig_value > 0);
    w.pay_amount = w.trig_value;
    w
}

/// Post-state + effect log are exactly the clause: reduce by X, floor 0,
/// only while the owner is poorest; no-op otherwise.
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_02_shave_iff() {
    let w = symbolic_pay_world();
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;
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
    kani::assume(spec::is_poorest(&w.money, &w.out, w.n_players, owner));
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
    assert!(
        post.pay_amount == expected,
        "mutant: card must not match the off-by-one spec"
    );
}
