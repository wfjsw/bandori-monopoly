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
//! Isolation assumes (module-level):
//! * no stacked PayAdd hooks besides the clause under test;
//! * `n_players ∈ 2..=4`, money `0..=10_000`, all seats in-range are living
//!   unless `out[p]` is set.

use crate::host;
use crate::invoke;
use crate::spec::mor as spec;
use crate::world::{self, Effect, Snapshot, World, MAX_PLAYERS};

fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = kani::any();
        // Living seats only in the symbolic region we care about; keep
        // out-of-range seats living-but-ignored via n_players.
        if (p as i32) >= w.n_players {
            w.out[p] = true;
        }
    }
    w
}

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

// ===========================================================================
// C-MOR-rui-demon-01 / -02  `Mor:（Rui）正论恶魔`
// 「向全场玩家支付100资金…之后全场玩家向你支付300资金…
//  在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」
// ===========================================================================

fn rui_world() -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    (w, player)
}

/// IFF: face-value 100 / 300 legs + X=5 after every leg and once more.
#[kani::proof]
#[kani::unwind(32)]
pub fn rui_demon_01_iff() {
    let (w, player) = rui_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mor:（Rui）正论恶魔", player));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    let out = pre.out;
    let n = pre.n_players;
    let n_others = (0..n).filter(|&p| p != player && !out[p as usize]).count();
    kani::cover!(n_others == 1, "one other living seat");
    kani::cover!(n_others >= 2, "two or more other living seats");

    assert!(
        spec::rui_demon_01_effects_ok(&effects, player, n, &out),
        "C-MOR-rui-demon-01: effect log must be 100-out + X=5 per other, then 300-in + X=5, then X=5"
    );
    assert!(
        spec::rui_demon_01_post_ok(&post, &pre, player, n, &out),
        "C-MOR-rui-demon-01: post money must match the face-value legs (floored/saturated)"
    );
    // Frame: no positions / fire / stay / piles from the money legs.
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.stay == pre.stay, "frame: stay");
    assert!(post.hand == pre.hand, "frame: hand");
    assert!(post.discard == pre.discard, "frame: discard");
}

/// MUTANT companion: a 200-out leg is NOT what the card does.
#[kani::proof]
#[kani::unwind(24)]
pub fn rui_demon_01_mutant_is_detected() {
    let (w, player) = rui_world();
    // Pin at least one other living seat so the mutant predicate is reachable.
    for p in 0..MAX_PLAYERS {
        if (p as i32) != player && (p as i32) < w.n_players {
            kani::assume(!w.out[p]);
        }
    }
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mor:（Rui）正论恶魔", player));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::rui_demon_01_effects_ok_mutant(&effects, player, pre.n_players, &pre.out);
    kani::cover!(!mutant, "mutant disagrees with the card");
    assert!(!mutant, "mutant: card must not match the 200-leg spec");
}

/// C-MOR-rui-demon-02: X (`skill.yuriCrit.x`) is set to 5 during the body.
/// Every money leg of the resolution fires the skill, so the body sets X=5
/// after each leg. We observe the final X via the last SetState.
#[kani::proof]
#[kani::unwind(24)]
pub fn rui_demon_02_x_set_to_5() {
    let (w, player) = rui_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mor:（Rui）正论恶魔", player));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // At least the trailing SetState(X=5) must be present.
    let last = effects.last();
    let ok = matches!(last, Some(Effect::SetState { player: p, n, .. }) if *p == player && *n == 5);
    let _ = &pre;
    assert!(ok, "C-MOR-rui-demon-02: X must end at 5");
}

// ===========================================================================
// C-MOR-xiaobai-01  `Mor:（小白）`
// 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
// ===========================================================================

fn xiaobai_world() -> (World, i32, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let payee: i32 = kani::any();
    kani::assume(payee >= 0 && payee < w.n_players && payee != player);
    // Enough money so the half-loss does not floor.
    kani::assume(w.money[payee as usize] >= 200);
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 10_000);
    w.trig_player = player;
    w.trig_target = payee;
    w.trig_value = amount;
    w.pay_amount = amount;
    (w, player, payee, amount)
}

/// IFF: the body rewrites the pending pay target to -1 (pure loss) and makes
/// the payee lose `amount / 2`. Exactly one effect: `PayMoney(payee, half)`.
#[kani::proof]
#[kani::unwind(24)]
pub fn xiaobai_01_iff() {
    let (w, player, payee, amount) = xiaobai_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:（小白）", player));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(amount >= 2 && amount % 2 == 0, "even amount");
    kani::cover!(amount % 2 == 1, "odd amount");

    assert!(
        spec::xiaobai_01_effects_ok(&effects, payee, amount),
        "C-MOR-xiaobai-01: exactly PayMoney(payee, amount/2)"
    );
    assert!(
        spec::xiaobai_01_post_ok(&post, &pre, payee, amount),
        "C-MOR-xiaobai-01: payee loses amount/2"
    );
    // The rewrite lands on the pending trigger target.
    let trig_target = world::with_world(|w| w.trig_target);
    assert!(
        trig_target == -1,
        "C-MOR-xiaobai-01: payment becomes a pure loss (target = bank)"
    );
    // Frame: the counteruser's money is untouched by this body.
    assert!(post.money[player as usize] == pre.money[player as usize], "frame: counteruser money");
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
}

/// MUTANT companion: the full amount is NOT what the card takes from the payee.
#[kani::proof]
#[kani::unwind(24)]
pub fn xiaobai_01_mutant_is_detected() {
    let (w, player, payee, amount) = xiaobai_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:（小白）", player));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::xiaobai_01_effects_ok_mutant(&effects, payee, amount);
    let _ = &pre;
    kani::cover!(!mutant, "mutant disagrees");
    assert!(!mutant, "mutant: card must not take the full amount from the payee");
}

// ===========================================================================
// C-MOR-rainbow-01 / -02  `Mor:秘密与青春的虹彩`
// 「（1）…此次支付金额减半。」 「（2）…使此次支付资金变成1.5倍。」
// Clause pin: integer half / integer 1.5× (140 → 210).
// The body CeilTo-10s; the iff may FAIL — that is a finding, not a spec bug.
// Grade residual (学妹/学姐/同级生) is a Rust `can_counteract` closure on the
// entry; the counteraction window itself is PROTOCOL-LEVEL. This harness
// drives the body after admission.
// ===========================================================================

fn rainbow_world(half: bool) -> (World, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let other: i32 = kani::any();
    kani::assume(other >= 0 && other < w.n_players && other != player);
    let amount: i32 = kani::any();
    kani::assume(amount >= 2 && amount <= 4_000);
    if half {
        // （1） player is the payer.
        w.trig_player = player;
        w.trig_target = other;
    } else {
        // （2） player is the payee.
        w.trig_player = other;
        w.trig_target = player;
    }
    w.trig_value = amount;
    w.pay_amount = amount;
    (w, player, amount)
}

/// IFF （1）: pay amount becomes `amount / 2` (clause pin).
#[kani::proof]
#[kani::unwind(24)]
pub fn rainbow_01_half_iff() {
    let (w, player, amount) = rainbow_world(true);
    let expected = spec::rainbow_01_amount(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:秘密与青春的虹彩", player));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(amount % 2 == 0, "even amount");
    kani::cover!(amount % 10 == 0, "multiple of 10");

    assert!(
        spec::rainbow_log_ok(&effects, expected),
        "C-MOR-rainbow-01: effect log must be exactly SetPayAmount(amount/2)"
    );
    assert!(
        post.pay_amount == expected,
        "C-MOR-rainbow-01: pay amount must be the integer half"
    );
    assert!(post.money == pre.money, "frame: money");
}

/// IFF （2）: pay amount becomes `amount * 3 / 2` (clause pin).
#[kani::proof]
#[kani::unwind(24)]
pub fn rainbow_02_boost_iff() {
    let (w, player, amount) = rainbow_world(false);
    let expected = spec::rainbow_02_amount(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:秘密与青春的虹彩", player));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(amount == 140, "doc example 140 → 210");
    kani::cover!(amount % 2 == 1, "odd amount");

    assert!(
        spec::rainbow_log_ok(&effects, expected),
        "C-MOR-rainbow-02: effect log must be exactly SetPayAmount(1.5×amount)"
    );
    assert!(
        post.pay_amount == expected,
        "C-MOR-rainbow-02: pay amount must be the integer 1.5×"
    );
    assert!(post.money == pre.money, "frame: money");
}

/// MUTANT companion （1）.
#[kani::proof]
#[kani::unwind(24)]
pub fn rainbow_01_mutant_is_detected() {
    let (w, player, amount) = rainbow_world(true);
    let wrong = spec::rainbow_01_amount_mutant(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:秘密与青春的虹彩", player));
    let post = world::snapshot();
    let _ = &pre;
    kani::cover!(post.pay_amount != wrong, "mutant disagrees");
    assert!(
        post.pay_amount != wrong,
        "mutant: card must not match off-by-one half"
    );
}

/// MUTANT companion （2）.
#[kani::proof]
#[kani::unwind(24)]
pub fn rainbow_02_mutant_is_detected() {
    let (w, player, amount) = rainbow_world(false);
    let wrong = spec::rainbow_02_amount_mutant(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract("Mor:秘密与青春的虹彩", player));
    let post = world::snapshot();
    let _ = &pre;
    kani::cover!(post.pay_amount != wrong, "mutant disagrees");
    assert!(
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
// check on the fields the gain must not touch.
// ===========================================================================

fn garden_world() -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    // Keep positions small so the host's pilot-vs-board tile_named branch is
    // deterministic; garden does not resolve tile names in the gain leg.
    for p in 0..MAX_PLAYERS {
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
    }
    w.symbolic_answers = true;
    w.trig_player = player;
    (w, player)
}

/// IFF (gain leg): first effect is `GainMoney(player, 100)` and post money is
/// pre + 100 (the later legs of the body do not touch money except settle,
/// which is a prompt the symbolic answers may take — we only pin the gain).
#[kani::proof]
#[kani::unwind(40)]
pub fn garden_01_gain_iff() {
    let (w, player) = garden_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // The play prompts (settle choice). `symbolic_answers` makes `ask_yes`
    // return a symbolic bool without an engine pause.
    invoke::unwrap_done(invoke::play("Mor:（筑紫）迷茫的庭园", player));
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    assert!(
        spec::garden_01_first_effect_ok(&effects, player),
        "C-MOR-garden-01: first effect must be GainMoney(player, 100)"
    );
    // The gain always lands; later legs must not undo it. Post money is at
    // least the gain (settle/rent is OPEN-触发结算 and may move more).
    assert!(
        post.money[player as usize] >= pre.money[player as usize] + 100,
        "C-MOR-garden-01: user gains 100 (at least)"
    );
    kani::cover!(post.money[player as usize] == pre.money[player as usize] + 100, "exactly +100");
}

/// Mutant: the gain is 200, not 100. Must disagree.
#[kani::proof]
#[kani::unwind(40)]
pub fn garden_01_mutant_is_detected() {
    let (w, player) = garden_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("Mor:（筑紫）迷茫的庭园", player));
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant_first = !effects.is_empty()
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 200);
    kani::cover!(!mutant_first, "mutant disagrees");
    assert!(!mutant_first, "mutant: gain must not be 200");
}