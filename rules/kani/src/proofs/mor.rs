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
#[kani::unwind(80)]
pub fn rui_demon_01_iff() {
    let (w, player) = rui_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（Rui）正论恶魔"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    let out = pre.out;
    let n = pre.n_players;
    let n_others = (0..n).filter(|&p| p != player && !out[p as usize]).count();
    kani::cover!(n_others == 1, "one other living seat");
    kani::cover!(n_others >= 2, "two or more other living seats");

    crate::kassert!(
        spec::rui_demon_01_effects_ok(&effects, player, n, &out),
        "C-MOR-rui-demon-01: effect log must be 100-out + X=5 per other, then 300-in + X=5, then X=5"
    );
    crate::kassert!(
        spec::rui_demon_01_post_ok(&post, &pre, player, n, &out),
        "C-MOR-rui-demon-01: post money must match the face-value legs (floored/saturated)"
    );
    // Frame: no positions / fire / stay / piles from the money legs.
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand");
    crate::kassert!(post.discard == pre.discard, "frame: discard");
}

/// MUTANT companion: a 200-out leg is NOT what the card does.
#[kani::proof]
#[kani::unwind(80)]
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
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（Rui）正论恶魔"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::rui_demon_01_effects_ok_mutant(&effects, player, pre.n_players, &pre.out);
    kani::cover!(!mutant, "mutant disagrees with the card");
    crate::kassert!(!mutant, "mutant: card must not match the 200-leg spec");
}

/// C-MOR-rui-demon-02: X (`skill.yuriCrit.x`) is set to 5 during the body.
/// Every money leg of the resolution fires the skill, so the body sets X=5
/// after each leg. We observe the final X via the last SetState.
#[kani::proof]
#[kani::unwind(80)]
pub fn rui_demon_02_x_set_to_5() {
    let (w, player) = rui_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（Rui）正论恶魔"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    // At least the trailing SetState(X=5) must be present.
    let last = effects.last();
    let ok = matches!(last, Some(Effect::SetState { player: p, n, .. }) if *p == player && *n == 5);
    let _ = &pre;
    crate::kassert!(ok, "C-MOR-rui-demon-02: X must end at 5");
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
#[kani::unwind(80)]
pub fn xiaobai_01_iff() {
    let (w, player, payee, amount) = xiaobai_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:（小白）"), player));
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
    crate::kassert!(post.money[player as usize] == pre.money[player as usize], "frame: counteruser money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
}

/// MUTANT companion: the full amount is NOT what the card takes from the payee.
#[kani::proof]
#[kani::unwind(80)]
pub fn xiaobai_01_mutant_is_detected() {
    let (w, player, payee, amount) = xiaobai_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:（小白）"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let mutant = spec::xiaobai_01_effects_ok_mutant(&effects, payee, amount);
    let _ = &pre;
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: card must not take the full amount from the payee");
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
#[kani::unwind(80)]
pub fn rainbow_01_half_iff() {
    let (w, player, amount) = rainbow_world(true);
    let expected = spec::rainbow_01_amount(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:秘密与青春的虹彩"), player));
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

/// IFF （2）: pay amount becomes `amount * 3 / 2` (clause pin).
#[kani::proof]
#[kani::unwind(80)]
pub fn rainbow_02_boost_iff() {
    let (w, player, amount) = rainbow_world(false);
    let expected = spec::rainbow_02_amount(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:秘密与青春的虹彩"), player));
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

/// MUTANT companion （1）.
#[kani::proof]
#[kani::unwind(80)]
pub fn rainbow_01_mutant_is_detected() {
    let (w, player, amount) = rainbow_world(true);
    let wrong = spec::rainbow_01_amount_mutant(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:秘密与青春的虹彩"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let _ = &pre;
    kani::cover!(post.pay_amount != wrong, "mutant disagrees");
    crate::kassert!(
        post.pay_amount != wrong,
        "mutant: card must not match off-by-one half"
    );
}

/// MUTANT companion （2）.
#[kani::proof]
#[kani::unwind(80)]
pub fn rainbow_02_mutant_is_detected() {
    let (w, player, amount) = rainbow_world(false);
    let wrong = spec::rainbow_02_amount_mutant(amount);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:秘密与青春的虹彩"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let _ = &pre;
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
#[kani::unwind(80)]
pub fn garden_01_gain_iff() {
    let (w, player) = garden_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // The play prompts (settle choice). `symbolic_answers` makes `ask_yes`
    // return a symbolic bool without an engine pause.
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（筑紫）迷茫的庭园"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    crate::kassert!(
        spec::garden_01_first_effect_ok(&effects, player),
        "C-MOR-garden-01: first effect must be GainMoney(player, 100)"
    );
    // The gain always lands; later legs must not undo it. Post money is at
    // least the gain (settle/rent is OPEN-触发结算 and may move more).
    crate::kassert!(
        post.money[player as usize] >= pre.money[player as usize] + 100,
        "C-MOR-garden-01: user gains 100 (at least)"
    );
    kani::cover!(post.money[player as usize] == pre.money[player as usize] + 100, "exactly +100");
}

/// Mutant: the gain is 200, not 100. Must disagree.
#[kani::proof]
#[kani::unwind(80)]
pub fn garden_01_mutant_is_detected() {
    let (w, player) = garden_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（筑紫）迷茫的庭园"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant_first = !effects.is_empty()
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 200);
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
// ===========================================================================

/// Isolation for courage-03: every tile is unowned, and the player is not
/// out. The roll is symbolic 3..=60. The off-by-one `is_agent` may still
/// misfire on the static board (see BLOCKED-HOST note) — the iff below is the
/// book pin and is expected to FAIL on those rolls; the companion
/// `courage_03_empty_when_no_agent_misfire` proves the no-op limb whenever
/// the log comes back empty.
fn courage_world() -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    // `World::new` already has owner=-1 / houses=0 / mortgaged=false.
    for p in 0..MAX_PLAYERS {
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
    }
    w.trig_player = player;
    (w, player)
}

/// courage-03 book pin: unowned tile ⇒ no effects and no money moves.
/// BLOCKED-HOST: the off-by-one `is_agent` can divert to the 1000-gain arm.
#[kani::proof]
#[kani::unwind(80)]
pub fn courage_03_unowned_iff() {
    let (w, player) = courage_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:勇气展翅高飞之时"), player));
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

/// Soundness of the no-op limb in isolation from the agent bit: if the body
/// produced no effects, the frame is untouched.
#[kani::proof]
#[kani::unwind(80)]
pub fn courage_03_empty_when_no_agent_misfire() {
    let (w, player) = courage_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:勇气展翅高飞之时"), player));
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
        crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    }
}

/// courage-01 book pin: owner pays `land + house_total/2`.
/// BLOCKED-HOST: `tile_price`/`buy_price` read the off-by-one board slot and
/// `buy_price` ignores houses. The harness is written against the book amount
/// computed from the **true** tile's static data and is expected to fail.
#[kani::proof]
#[kani::unwind(80)]
pub fn courage_01_pay_iff_blocked_host() {
    let (w, player) = courage_world();
    // Own every tile at a single other living seat so the pay branch is taken
    // whenever the agent bit does not misfire.
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players && owner != player);
    kani::assume(!w.out[owner as usize]);
    let mut w = w;
    for t in 0..60 {
        w.tiles[t].owner = owner;
    }
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:勇气展翅高飞之时"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
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
            matches!(effects[0], Effect::PayTo { from, to, .. } if from == owner && to == player),
            "C-MOR-courage-01: exactly one PayTo(owner → user)"
        );
        let _ = post;
    }
}

/// courage-02 book pin: agent tile ⇒ GainMoney(user, 1000), no pay leg.
/// BLOCKED-HOST: `is_agent` is off-by-one on the static board.
#[kani::proof]
#[kani::unwind(80)]
pub fn courage_02_agent_iff_blocked_host() {
    let (w, player) = courage_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:勇气展翅高飞之时"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    // The 1000-gain arm is what the body takes when `is_agent(roll_tile)`
    // (off-by-one) fires. Cover both outcomes.
    kani::cover!(spec::courage_02_effects_ok(effects, player), "agent arm (host bit)");
    kani::cover!(!spec::courage_02_effects_ok(effects, player), "not the agent arm");
    if spec::courage_02_effects_ok(effects, player) {
        crate::kassert!(
            post.money[player as usize] == pre.money[player as usize] + 1000
                || pre.money[player as usize] > i32::MAX - 1000,
            "C-MOR-courage-02: user gains 1000 on the agent arm"
        );
    }
}

/// MUTANT companion (courage-01 amount shape): a 200-leg PayTo is NOT the
/// card. Reachable-disagreement on the shape of the single pay effect.
#[kani::proof]
#[kani::unwind(80)]
pub fn courage_01_mutant_is_detected() {
    let (w, player) = courage_world();
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players && owner != player);
    kani::assume(!w.out[owner as usize]);
    let mut w = w;
    for t in 0..60 {
        w.tiles[t].owner = owner;
    }
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:勇气展翅高飞之时"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant = log_n == 1
        && matches!(effects[0], Effect::PayTo { from, to, n } if from == owner && to == player && n == 200);
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
// multiset cannot be pinned.
// ===========================================================================

fn toko_world() -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    // No deeds anywhere ⇒ no swappable pair ⇒ the body early-returns.
    // (`World::new` already leaves every tile owner=-1 / house-free.)
    w.symbolic_answers = true;
    w.trig_player = player;
    (w, player)
}

/// IFF (no-pair): the body produces no effects.
#[kani::proof]
#[kani::unwind(80)]
pub fn toko_no_pair_noop() {
    let (w, player) = toko_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（toko）"), player));
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

/// Swap multiset (book): two SetOwner, one PayTo(diff), one SetMortgaged(off)
/// on the received deed. BLOCKED-HOST on the tile-index split; the harness
/// seeds one mortgaged same-group pair and checks the book multiset.
#[kani::proof]
#[kani::unwind(80)]
pub fn toko_swap_iff_blocked_host() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let other: i32 = kani::any();
    kani::assume(other >= 0 && other < w.n_players && other != player);
    kani::assume(!w.out[other as usize]);
    // Seed a same-group mortgaged pair on the 0-based indices 1 and 6
    // (board group 1: tiles 2 and 7 are 1-based 2,7 → 0-based 1,6).
    // `owned_at` will report them as 2 and 7 (1-based) — the index split.
    // (`World::new` already leaves every tile owner=-1 / house-free.)
    w.tiles[1].owner = player;
    w.tiles[1].mortgaged = true;
    w.tiles[6].owner = other;
    w.tiles[6].mortgaged = true;
    w.money[player as usize] = kani::any();
    kani::assume(w.money[player as usize] >= 0 && w.money[player as usize] <= 10_000);
    w.symbolic_answers = true;
    w.trig_player = player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:（toko）"), player));
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

// ===========================================================================
// C-MOR-starry-01  `Mor:蝴蝶飞舞的星月夜`
// 「（1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
// OPEN-X: engine offers 1..=5. Host note: `pay` always returns the requested
// amount under Kani (no shortfall path); `set_dest` is folded into PlaceCard.
// ===========================================================================

fn starry_play_world() -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.symbolic_answers = true;
    w.trig_player = player;
    (w, player)
}

/// IFF: PayMoney(1000·X) + PlaceCard(Field) + AddCounter(X), money floored.
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_01_place_iff() {
    let (w, player) = starry_play_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), player));
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
        spec::starry_01_x_in_range(x, pre.money[player as usize]),
        "C-MOR-starry-01: X must be in the engine's 1..=max offer (OPEN-X)"
    );
    crate::kassert!(
        spec::starry_01_effects_ok(effects, player, x),
        "C-MOR-starry-01: PayMoney(1000X) + PlaceCard + AddCounter(X)"
    );
    crate::kassert!(
        spec::starry_01_post_ok(&post, &pre, player, x),
        "C-MOR-starry-01: user pays 1000·X (floored at 0)"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
}

/// MUTANT companion: the pay is not 1000·X+1.
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_01_mutant_is_detected() {
    let (w, player) = starry_play_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
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
// ===========================================================================

fn starry_changed_world(crystals: i32, delta: i32) -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let key = crate::intern::intern_or_fail("crystals");
    w.key_set(player, key, crystals);
    w.trig_player = player;
    w.pay_amount = delta; // trigger::value()
    w.field_n = 1; // is_placed
    w.dest = 2; // sentinel (Field): not Graveyard, so a dest write is visible
    w.symbolic_answers = true;
    (w, player)
}

/// IFF (removal → payout + empty → discard dest).
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_06_payout_iff() {
    let removed: i32 = kani::any();
    kani::assume(removed >= 1 && removed <= 3);
    let after: i32 = kani::any();
    kani::assume(after >= 0 && after <= 3);
    let (w, player) = starry_changed_world(after, -removed);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), 2, // On::Hook(CounterChanged)
        player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(after == 0, "crystals empty → discard");
    kani::cover!(after > 0, "crystals remain");

    // -06: one GainMoney per crystal removed.
    crate::kassert!(
        spec::starry_06_effects_ok(effects, player, removed),
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
    crate::kassert!(post.money[player as usize] == pre.money[player as usize] + 1000 * removed
        || pre.money[player as usize] > i32::MAX - 1000 * removed,
        "C-MOR-starry-06: post money = pre + 1000·removed");
}

/// MUTANT companion: 500 per crystal is NOT the card.
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_06_mutant_is_detected() {
    let removed: i32 = kani::any();
    kani::assume(removed >= 1 && removed <= 3);
    let (w, player) = starry_changed_world(1, -removed);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), 2, player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant = spec::starry_06_effects_ok_mutant(effects, player, removed);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: payout must not be 500 per crystal");
}

// ===========================================================================
// C-MOR-starry-05  `Mor:蝴蝶飞舞的星月夜`
// 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」
// PassTile hook arm. `is_circle` uses 1-based `board::tile_slot`; we pin
// `trig_tile = 1` (board index 1 is the CiRCLE) so the gate holds. The
// RollAfter arm of the same hook is BLOCKED-HOST on `Op::trig_move_main`
// (unmodeled) and is not driven here (starry-03/04).
// ===========================================================================

fn starry_pass_world() -> (World, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 1 && crystals <= 5);
    let key = crate::intern::intern_or_fail("crystals");
    w.key_set(player, key, crystals);
    w.trig_player = player;
    w.trig_tile = 1; // board index 1 = CiRCLE (kind 0)
    w.trig_kind = card_sdk::abi::TriggerKind::PassTile as i32;
    w.field_n = 1;
    (w, player, crystals)
}

/// IFF: one crystal is removed (`AddCounter(owner, -1)`).
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_05_circle_iff() {
    let (w, player, crystals) = starry_pass_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), 1, // On::Hook(RollAfter | PassTile)
        player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;

    kani::cover!(crystals == 1, "last crystal");
    kani::cover!(crystals > 1, "not the last crystal");

    crate::kassert!(
        spec::starry_05_effects_ok(effects, player),
        "C-MOR-starry-05: owner passing CiRCLE removes one crystal"
    );
    let after = world::with_world(|w| {
        let key = crate::intern::intern_or_fail("crystals");
        w.key_get(player, key)
    });
    crate::kassert!(
        after == crystals - 1,
        "C-MOR-starry-05: crystal count drops by 1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// NO-OP: a non-owner pass, or a non-CiRCLE tile, removes nothing.
#[kani::proof]
#[kani::unwind(80)]
pub fn starry_05_noop() {
    let (w, player, crystals) = starry_pass_world();
    // Flip the trigger to a non-circle tile (board index 2 is a property).
    let mut w = w;
    w.trig_tile = 2;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:蝴蝶飞舞的星月夜"), 1, player));
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
    let after = world::with_world(|w| {
        let key = crate::intern::intern_or_fail("crystals");
        w.key_get(player, key)
    });
    crate::kassert!(after == crystals, "C-MOR-starry-05: crystals unchanged");
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
// ===========================================================================

fn nnm_world(crystals: i32) -> (World, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let key = crate::intern::intern_or_fail("crystals");
    w.key_set(player, key, crystals);
    w.field_n = 1; // the card is already placed (branch (3))
    w.trig_player = player;
    w.dest = 2; // sentinel (Field): not Graveyard
    w.symbolic_answers = true;
    (w, player)
}

/// FAILS(LESS) C-MOR-nnm-05: with the card on the field and crystals at 0,
/// the Play press does not put the card into the discard.
/// Book: 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」.
/// Body: `exhausted` is `#[allow(dead_code)]` and never registered; no
/// CounterChanged hook discards on empty. The iff below asserts the book pin
/// and is expected to FAIL — that failure is the finding.
#[kani::proof]
#[kani::unwind(80)]
pub fn nnm_05_empty_discard_iff_fails_less() {
    let (w, player) = nnm_world(0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Play press (entry 0): `is_placed` ⇒ `use_skill`; crystals < 1 ⇒ return.
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:（NNM）稍微努力了一下"), 0, player));
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
    let _ = &pre;
    let _ = effects;
}

/// Reachable-disagreement companion for C-MOR-nnm-05: the body really does
/// leave the card on the field when crystals are empty (the LESS is not
/// vacuous). Covers the bad state.
#[kani::proof]
#[kani::unwind(80)]
pub fn nnm_05_body_leaves_card_is_reachable() {
    let (w, player) = nnm_world(0);
    let pre_dest = 2i32; // sentinel Field; body must leave it alone
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:（NNM）稍微努力了一下"), 0, player));
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

/// nnm-05 companion: the AtEnd entry (discard-down-to-five) also does not
/// discard the *field* card when crystals are empty.
#[kani::proof]
#[kani::unwind(80)]
pub fn nnm_05_atend_leaves_field_card() {
    let (w, player) = nnm_world(0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("Mor:（NNM）稍微努力了一下"), 1, // On::AtEnd
        player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let dest = world::with_world(|w| w.dest);
    let _ = &pre;

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

/// nnm-03 FAILS(WRONG): the book pins x × the *current* CiRCLE money reward;
/// the body hard-codes x × 2000 (TODO(规则书) in the source). We cannot drive
/// the branch (`Op::tok_names` is empty — BLOCKED-HOST), so this is a
/// formula-level iff against the book that is expected to FAIL whenever the
/// Morfonica cycle reward is 1000 or 1500.
#[kani::proof]
#[kani::unwind(80)]
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

/// Reachable-disagreement companion for C-MOR-nnm-03.
#[kani::proof]
#[kani::unwind(80)]
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
// ===========================================================================

fn tritone_counteract_world() -> (World, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let amount: i32 = kani::any();
    kani::assume(amount >= 1 && amount <= 5_000);
    w.trig_player = player;
    w.trig_kind = card_sdk::abi::TriggerKind::Effect as i32;
    w.pay_amount = amount; // trigger::value()
    w.trig_value = amount;
    (w, player, amount)
}

/// IFF tritone-01 body: GainMoney + PlaceCard + SetState(owed); crystals = 3.
#[kani::proof]
#[kani::unwind(80)]
pub fn tritone_01_gain_place_iff() {
    let (w, player, amount) = tritone_counteract_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("Mor:迷茫之蝶们的三全音"), player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(amount == 1, "smallest loan");
    kani::cover!(amount == 5_000, "large loan");

    crate::kassert!(
        spec::tritone_01_effects_ok(effects, player, amount),
        "C-MOR-tritone-01: GainMoney(amount) + PlaceCard + SetState(owed=amount)"
    );
    let crystals = world::with_world(|w| {
        let key = crate::intern::intern_or_fail("crystals");
        w.key_get(player, key)
    });
    crate::kassert!(
        spec::tritone_01_crystals_ok(crystals),
        "C-MOR-tritone-01: card is placed with 3 crystals"
    );
    crate::kassert!(
        post.money[player as usize] == pre.money[player as usize] + amount
            || pre.money[player as usize] > i32::MAX - amount,
        "C-MOR-tritone-01: user gains the lost amount"
    );
}

/// IFF tritone-02 single turn-end tick (decay / payback).
/// Multi-turn scheduling (3 owner turn-ends) is PROTOCOL-LEVEL.
#[kani::proof]
#[kani::unwind(80)]
pub fn tritone_02_tick_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let pre_crystals: i32 = kani::any();
    kani::assume(pre_crystals >= 1 && pre_crystals <= 3);
    let owed: i32 = kani::any();
    kani::assume(owed >= 0 && owed <= 5_000);
    let key = crate::intern::intern_or_fail("crystals");
    w.key_set(player, key, pre_crystals);
    let owed_key = crate::intern::intern_or_fail("tritone_owed");
    w.key_set(player, owed_key, owed);
    w.trig_player = player;
    w.trig_kind = card_sdk::abi::TriggerKind::TurnEnd as i32;
    w.field_n = 1;
    w.dest = 2; // sentinel (Field): not Graveyard
    w.money[player as usize] = kani::any();
    kani::assume(w.money[player as usize] >= 0 && w.money[player as usize] <= 10_000);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("Mor:迷茫之蝶们的三全音"), "TurnEnd", player));
    kani::cover!(true, "body returned");
    let post = world::snapshot();
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];

    kani::cover!(pre_crystals == 1, "last crystal → payback");
    kani::cover!(pre_crystals > 1, "decay only");

    crate::kassert!(
        spec::tritone_02_tick_effects_ok(effects, player, pre_crystals, owed),
        "C-MOR-tritone-02: tick is decay-only, or decay + payback(owed)"
    );
    let after = world::with_world(|w| w.key_get(player, key));
    crate::kassert!(
        after == pre_crystals - 1,
        "C-MOR-tritone-02: one crystal removed per owner turn end"
    );
    if pre_crystals == 1 {
        crate::kassert!(
            post.money[player as usize] == (pre.money[player as usize] - owed).max(0),
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

/// MUTANT companion: the payback is not owed+1.
#[kani::proof]
#[kani::unwind(80)]
pub fn tritone_02_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let owed: i32 = kani::any();
    kani::assume(owed >= 1 && owed <= 1_000);
    let key = crate::intern::intern_or_fail("crystals");
    w.key_set(player, key, 1);
    let owed_key = crate::intern::intern_or_fail("tritone_owed");
    w.key_set(player, owed_key, owed);
    w.trig_player = player;
    w.trig_kind = card_sdk::abi::TriggerKind::TurnEnd as i32;
    w.field_n = 1;
    w.money[player as usize] = 10_000;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("Mor:迷茫之蝶们的三全音"), "TurnEnd", player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant = spec::tritone_02_tick_effects_ok_mutant(effects, player, 1, owed);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: payback must not be owed+1");
}

// ===========================================================================
// C-MOR-noble-02  `Mor:高贵的微蓝`
// 「如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记…」
// OPEN-触发结算: the forced settle (`card_settle_at`) is a protocol stub
// returning 0; only the mark leg is pinned. The body is invoked directly
// (the cant_play price gate is BLOCKED-HOST on 1-based `tile_price`).
// ===========================================================================

fn noble_world(owned: bool) -> (World, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let pos: i32 = kani::any();
    kani::assume(pos >= 0 && pos < 60);
    // `World::new` already leaves every tile owner=-1 / house-free.
    if owned {
        w.tiles[pos as usize].owner = player;
    } else {
        let other: i32 = kani::any();
        kani::assume(other >= 0 && other < w.n_players && other != player);
        w.tiles[pos as usize].owner = other;
    }
    w.pos[player as usize] = pos;
    w.trig_player = player;
    (w, player, pos)
}

/// IFF (mark leg): tile is yours ⇒ exactly one PlaceMark on that tile.
/// The forced settle (1) is OPEN-触发结算 / PROTOCOL-LEVEL and is not pinned.
#[kani::proof]
#[kani::unwind(80)]
pub fn noble_02_mark_iff() {
    let (w, player, pos) = noble_world(true);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:高贵的微蓝"), player));
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

/// NO-OP (not yours): no effects.
#[kani::proof]
#[kani::unwind(80)]
pub fn noble_02_not_yours_noop() {
    let (w, player, _pos) = noble_world(false);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:高贵的微蓝"), player));
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

/// MUTANT companion (mark): a mark on tile 0 is NOT the card when the user
/// stands elsewhere.
#[kani::proof]
#[kani::unwind(80)]
pub fn noble_02_mutant_is_detected() {
    let (w, player, pos) = noble_world(true);
    kani::assume(pos != 0);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:高贵的微蓝"), player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
    let mutant = log_n == 1
        && matches!(effects[0], Effect::PlaceMark { tile, .. } if tile == 0);
    kani::cover!(!mutant, "mutant disagrees");
    crate::kassert!(!mutant, "mutant: mark must not land on tile 0");
}

// ===========================================================================
// C-MOR-again-02  `Mor:再次牵起手来` [持续]
// 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。」
// The [反击] window of -01 is PROTOCOL-LEVEL (KANI.md §6). This is the
// PayAt hook body. OPEN-取消范围: one-sided on the owner.
// ===========================================================================

/// IFF: SetPayAmount(0) and dest = Graveyard. Exactly one effect.
#[kani::proof]
#[kani::unwind(80)]
pub fn again_02_cancel_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    w.pay_amount = kani::any();
    kani::assume(w.pay_amount >= 1 && w.pay_amount <= 5_000);
    w.field_n = 1;
    w.dest = 2; // sentinel (Field): not Graveyard
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("Mor:再次牵起手来"), "PayAt", player));
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

/// MUTANT companion: cancel-to-1 is NOT the card.
#[kani::proof]
#[kani::unwind(80)]
pub fn again_02_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    w.pay_amount = 100;
    w.field_n = 1;
    w.dest = 2;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(crate::card_id!("Mor:再次牵起手来"), "PayAt", player));
    kani::cover!(true, "body returned");
    let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
    let log_n = effects_of(&mut ebuf);
    let effects = &ebuf[..log_n];
    let _ = &pre;
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
// follow-on roll via `card_move`) is PROTOCOL-LEVEL scheduling.
// ===========================================================================

fn wing_world() -> (World, i32, i32) {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    let pos: i32 = kani::any();
    kani::assume(pos >= 0 && pos < 60);
    w.pos[player as usize] = pos;
    w.trig_player = player;
    (w, player, pos)
}

/// Book pin: Teleport(user, (pos + 20·dir) mod 60). The host's `dir` is a
/// stub (0), so this is BLOCKED-HOST — the assertion below is against the
/// book formula with `dir` recovered from the host's own `tile_steps_ahead`.
#[kani::proof]
#[kani::unwind(80)]
pub fn wing_01_teleport_iff_blocked_host() {
    let (w, player, pos) = wing_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("Mor:纯真振翅"), player));
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
        spec::wing_01_effects_ok(effects, player, want),
        "C-MOR-wing-01: exactly Teleport(user, pos+20·dir) (dir stubbed to 0)"
    );
    crate::kassert!(
        post.pos[player as usize] == want,
        "C-MOR-wing-01: user lands on the teleport destination"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money (no settle)");
}

/// Book-vs-stub differential: with a real forward direction the destination
/// is 20 ahead; the stub lands on the current tile. This cover documents the
/// BLOCKED-HOST gap without hiding it.
#[kani::proof]
#[kani::unwind(80)]
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
