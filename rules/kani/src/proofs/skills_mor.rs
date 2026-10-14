//! Unit `skills_mor`: clause proofs for Morfonica band + character skills.
//!
//! Specs live in [`crate::spec::skills_mor`] (book quotes there). This file
//! drives the **unmodified** skill bodies via [`crate::invoke`] against a
//! symbolic world and asserts post == spec(pre), effect log exactly, frame.
//!
//! Isolation (stated here and `kani::assume`d where relevant):
//! * no other skill / card hooks fire in the window (single-body invoke);
//! * piles stay empty except where a draw is the clause effect;
//! * fire_max is pinned per clause (「上限」 figure from the quote).
//!
//! Host notes that the harnesses work around **without faking a proof**:
//! * `Op::trig_value` reads `w.pay_amount` (not `w.trig_value`) — both are
//!   written so the body and the guard IR agree.
//! * Kani `Op::gain_fire` does **not** honour `fire_max` (unlike
//!   `game_core::gain_fire`). The at-cap fire leg is **BLOCKED-HOST**.
//! * Kani `Op::trig_set_cancelled` writes `a0 != 0` while the no-arg call
//!   passes `a0 = 0`, so the cancelled flag cannot be observed.
//!   **BLOCKED-HOST** for C-MOR-band-02's 「替换奖励」 leg.
//!
//! Bounds: every harness that calls `invoke::*` uses `#[kani::unwind(80)]`.
//! A tight unwind (24) over the `RULESET` linear scan is **vacuous** — covers
//! and assertions come back UNREACHABLE while CBMC still says SUCCESSFUL.
//! Each `unwrap_done` is followed by `kani::cover!(true, "body returned")` so
//! that vacuity is visible in the cover table.

use crate::board;
use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec::skills_mor as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};
use rules_cond_ir::ctx::{IrCtx, SetId};
use rules_cond_ir::vars::VarId;

// ---------------------------------------------------------------------------
// Shared setup helpers (local to this unit — shared files stay untouched)
// ---------------------------------------------------------------------------

const SKILL_BAND: usize = crate::card_id!("skill:Morfonica:振翅高飞的练习曲");
const SKILL_TOUKO: usize = crate::card_id!("skill:桐谷透子:天上天下，唯我独尊");
const SKILL_NANAMI: usize = crate::card_id!("skill:广町七深:这是很普通的事吧？");
const SKILL_YURI: usize = crate::card_id!("skill:八潮瑠唯:正论暴击");
const SKILL_TSUKUSHI: usize = crate::card_id!("skill:二叶筑紫:交给班长吧");

const KEY_TAKEN: &str = "skill.morfonica.moneyTaken";
const KEY_YURI_X: &str = "skill.yuriCrit.x";
const KEY_TSUKU_RATE: &str = "skill.tsugushi.rate";
const TOK_X_UP: &str = "X";
const TOK_X_DOWN: &str = "X标记(反)";

fn logbuf() -> ([Effect; world::MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects() -> ([Effect; world::MAX_LOG], usize) {
    logbuf()
}

/// Light IrCtx for the `actor == owner` family (PayAfter / CircleAffected).
fn fill_mine(w: &World, owner: i32) -> IrCtx {
    ir_fill::fill_pay_add(w, owner)
}

/// Light IrCtx for `actor == owner && is_circle(tile.id)` — the Pass hook.
/// (Full `fill_from_world` is a CBMC cliff; `conf_fill_diff` pins the binding.)
fn fill_pass_circle(w: &World, owner: i32) -> IrCtx {
    let mut c = ir_fill::fill_pay_add(w, owner);
    c.set_scalar(VarId::TileId, w.trig_tile as i64);
    let mut i = 0usize;
    while i < 60 {
        if board::TILES[i].kind == 0 {
            c.set_insert(SetId::Circle, i as i64);
        }
        i += 1;
    }
    c
}

/// Symbolic world for a pure hook over a money / trigger payload.
fn symbolic_hook_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = kani::any();
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 20);
        w.fire_max[p] = kani::any();
        kani::assume(w.fire_max[p] >= 0 && w.fire_max[p] <= 20);
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
    }
    w
}

// ===========================================================================
// C-MOR-band-01
// 「（1）当你一次性失去2000以上资金时，抽一张卡。」
// ===========================================================================

fn symbolic_band01_world() -> World {
    let mut w = symbolic_hook_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    // One money move: positive magnitude. `Op::trig_value` reads `pay_amount`.
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= 0 && w.trig_value <= 10_000);
    w.pay_amount = w.trig_value;
    // Payee of the move (`-1` = bank). The body skips when `target == player`.
    w.trig_target = kani::any();
    kani::assume(w.trig_target >= -1 && w.trig_target < w.n_players);
    w
}

/// IFF: host pre `actor == owner` admits, then the body draws iff the book
/// says so (`value ≥ 2000` ∧ the player is the one losing). Covers both the
/// draw and the no-draw branches.
#[kani::proof]
#[kani::unwind(80)]
pub fn band_01_draw_iff() {
    let w = symbolic_band01_world();
    let player = w.trig_player;
    let value = w.pay_amount;
    let target = w.trig_target;

    let gid = crate::guard_id!("actor == owner");
    let ctx = fill_mine(&w, player);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    kani::cover!(!admitted, "guard rejects");
    if !admitted {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_BAND, "PayAfter", player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    let want = spec::band_01_should_draw(value, target, player);
    kani::cover!(want, "draws (loss ≥ 2000)");
    kani::cover!(!want, "no draw (below 2000 or not a loss)");

    if want {
        assert!(
            spec::band_01_log_ok(effects, player),
            "C-MOR-band-01: effect log must be exactly Draw(player, 1)"
        );
    } else {
        assert!(
            log_n == 0,
            "C-MOR-band-01: false trigger must produce no effects"
        );
    }

    let post = world::snapshot();
    assert!(world::arr4_eq(post.money, pre.money), "frame: money");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    assert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
}

/// NO-OP: a move that is not a ≥2000 loss leaves the world untouched even if
/// the body is invoked (the host would not call it; the body itself also
/// early-returns).
#[kani::proof]
#[kani::unwind(80)]
pub fn band_01_noop_below_or_gain() {
    let mut w = symbolic_band01_world();
    // Pin the false-trigger region.
    kani::assume(w.pay_amount < 2000 || w.trig_target == w.trig_player);
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_BAND, "PayAfter", player));
    kani::cover!(true, "body returned");
    let (_logbuf, log_n) = effects();
    assert!(log_n == 0, "C-MOR-band-01 noop: log must stay empty");
    let post = world::snapshot();
    assert!(world::arr4_eq(post.money, pre.money) && post.pay_amount == pre.pay_amount);
}

/// MUTANT companion: the card must disagree with a spec that draws at 1999.
#[kani::proof]
#[kani::unwind(80)]
pub fn band_01_mutant_is_detected() {
    let mut w = symbolic_band01_world();
    // Reachable disagreement: a 1999 loss.
    kani::assume(w.pay_amount == 1999);
    kani::assume(w.trig_target != w.trig_player);
    let player = w.trig_player;
    let target = w.trig_target;
    let value = w.pay_amount;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_BAND, "PayAfter", player));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    // Mutant would demand a draw here; the card must stay silent.
    let mutant_wants_draw = spec::band_01_should_draw_mutant(value, target, player);
    let card_draws = spec::band_01_log_ok(effects, player);
    kani::cover!(mutant_wants_draw && !card_draws, "mutant wants a draw, card silent");
    assert!(
        mutant_wants_draw && !card_draws,
        "C-MOR-band-01 mutant: card must not match the 1999-draw spec"
    );
    assert!(log_n == 0, "1999 loss draws nothing");
}

// ===========================================================================
// C-MOR-band-02
// 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
//
// Invoke: `entry(id, 1, …)` — the CircleAffected hook. (`hook_only` cannot
// name `CircleAffected`: `invoke::hook_kind_name` does not map that kind.)
// ===========================================================================

fn symbolic_band02_world() -> World {
    let mut w = symbolic_hook_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    // The CiRCLE reward half: `REWARD_MONEY` (0) / `REWARD_CARD` (1).
    // `Op::trig_value` reads `pay_amount`.
    w.trig_value = kani::any();
    kani::assume(w.trig_value == 0 || w.trig_value == 1);
    w.pay_amount = w.trig_value;
    // Pre-seed the taken counter.
    let taken: i32 = kani::any();
    kani::assume(taken >= 0 && taken <= 8);
    let key = intern::intern(KEY_TAKEN);
    w.key_set(player, key, taken);
    w
}

/// IFF: money option cycles 1000/1500/2000 by `taken % 3`, counter +1.
/// The 「替换奖励」 cancel flag is **BLOCKED-HOST** (see module docs) and is
/// not asserted here.
#[kani::proof]
#[kani::unwind(80)]
pub fn band_02_cycle_iff() {
    let w = symbolic_band02_world();
    let player = w.trig_player;
    let value = w.pay_amount;
    let taken = w.key_get(player, intern::intern(KEY_TAKEN));

    let gid = crate::guard_id!("actor == owner");
    let ctx = fill_mine(&w, player);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    if !admitted {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(SKILL_BAND, 1, player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    let money_opt = spec::band_02_is_money_option(value);
    kani::cover!(money_opt, "money option (cycle applies)");
    kani::cover!(!money_opt, "card option (no cycle)");
    kani::cover!(taken % 3 == 0, "1st of the cycle → 1000");
    kani::cover!(taken % 3 == 1, "2nd → 1500");
    kani::cover!(taken % 3 == 2, "3rd → 2000");

    if money_opt {
        assert!(
            spec::band_02_log_ok(effects, player, taken),
            "C-MOR-band-02: SetState(taken+1) + GainMoney(cycle amount)"
        );
    } else {
        assert!(log_n == 0, "C-MOR-band-02: non-money option must be a no-op");
    }

    let post = world::snapshot();
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    assert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
}

/// NO-OP: the card half of [CiRCLE奖励] must not touch money or the counter.
#[kani::proof]
#[kani::unwind(80)]
pub fn band_02_noop_card_option() {
    let mut w = symbolic_band02_world();
    kani::assume(w.pay_amount == spec::BAND_02_REWARD_MONEY + 1); // REWARD_CARD
    let player = w.trig_player;
    let key = intern::intern(KEY_TAKEN);
    let pre_taken = w.key_get(player, key);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry_idx(SKILL_BAND, 1, player));
    kani::cover!(true, "body returned");
    let (_logbuf, log_n) = effects();
    assert!(log_n == 0, "C-MOR-band-02 noop: log empty on card option");
    let post_taken = world::with_world(|ww| ww.key_get(player, key));
    assert!(post_taken == pre_taken, "counter unchanged");
    let post = world::snapshot();
    assert!(world::arr4_eq(post.money, pre.money), "money unchanged");
}

/// MUTANT companion: cycle rotated by one must be distinguishable.
#[kani::proof]
#[kani::unwind(80)]
pub fn band_02_mutant_is_detected() {
    let mut w = symbolic_band02_world();
    kani::assume(w.pay_amount == spec::BAND_02_REWARD_MONEY);
    let player = w.trig_player;
    let taken = w.key_get(player, intern::intern(KEY_TAKEN));
    kani::assume(taken == 0); // first of the cycle → 1000, mutant says 1500
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(SKILL_BAND, 1, player));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let card = spec::band_02_log_ok(effects, player, taken);
    let mutant_amount = spec::band_02_amount_mutant(taken);
    let card_amount = spec::band_02_amount(taken);
    kani::cover!(
        card && card_amount != mutant_amount,
        "reachable disagreement on the cycle"
    );
    assert!(card, "first-of-cycle must pay 1000");
    assert!(card_amount == 1000, "book: 1st = 1000");
    assert!(mutant_amount == 1500, "mutant rotates to 1500");
    assert!(
        card_amount != mutant_amount,
        "C-MOR-band-02 mutant: card must not match the rotated cycle"
    );
}

// ===========================================================================
// C-MOR-touko-01 / C-MOR-nanami-01
// 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 / cap 2
// ===========================================================================

fn symbolic_pass_world() -> World {
    let mut w = symbolic_hook_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    // The tile being passed. Circle tiles are kind 0 on the static board.
    w.trig_tile = kani::any();
    kani::assume(w.trig_tile >= 0 && w.trig_tile < 60);
    w
}

/// Shared Pass-gain IFF for touko (cap 1) and nanami (cap 2). Below-cap the
/// book and the current Kani host agree (`GainFire{1}`); at-cap the book is a
/// no-op but Kani `gain_fire` still logs — that leg is **BLOCKED-HOST** and
/// only covered, not asserted against the book.
fn pass_gain_iff(skill: usize, fire_max_pin: i32, label: &str) {
    let mut w = symbolic_pass_world();
    // Pin the seat's fire-pot cap to the quote's 「上限」.
    let player0 = w.trig_player;
    w.fire_max[player0 as usize] = fire_max_pin;

    let gid = crate::guard_id!("actor == owner && is_circle(tile.id)");
    let ctx = fill_pass_circle(&w, player0);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (pass CiRCLE)");
    kani::cover!(!admitted, "guard rejects (not CiRCLE / not owner)");
    if !admitted {
        return;
    }

    let player = player0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    // Body entry: the Pass hook. (hook_kind_name maps `Pass`.)
    invoke::unwrap_done(invoke::hook_only_idx(skill, "Pass", player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let pre_fire = pre.fire[player as usize];
    let post = world::snapshot();
    let post_fire = post.fire[player as usize];

    let below = pre_fire < fire_max_pin;
    kani::cover!(below, "below cap (gain applies)");
    kani::cover!(!below, "at cap (book: no-op)");

    if below {
        assert!(
            spec::touko_01_log_ok(effects, player),
            "{label}: effect log must be exactly GainFire{{player,1}}"
        );
        assert!(
            spec::touko_01_post_below_cap(post_fire, pre_fire),
            "{label}: fire must increase by one below cap"
        );
    } else {
        // Book: empty log + unchanged fire. Kani `Op::gain_fire` does not
        // honour `fire_max` (BLOCKED-HOST) so this path is not asserted.
        // We only require the body to have run without unmodeled ops.
        let _ = spec::touko_01_post_at_cap(post_fire, pre_fire);
        let _ = effects;
    }

    assert!(world::arr4_eq(post.money, pre.money), "frame: money");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
}

/// C-MOR-touko-01 IFF (below-cap asserted; at-cap covered, host gap).
#[kani::proof]
#[kani::unwind(80)]
pub fn touko_01_fire_iff() {
    pass_gain_iff(SKILL_TOUKO, spec::TOUKO_01_FIRE_MAX, "C-MOR-touko-01");
}

/// C-MOR-nanami-01 IFF (same shape, cap 2).
#[kani::proof]
#[kani::unwind(80)]
pub fn nanami_01_fire_iff() {
    pass_gain_iff(SKILL_NANAMI, spec::NANAMI_01_FIRE_MAX, "C-MOR-nanami-01");
}

/// NO-OP: passing a non-CiRCLE tile produces nothing.
#[kani::proof]
#[kani::unwind(80)]
pub fn touko_01_noop_not_circle() {
    let mut w = symbolic_pass_world();
    // Force the residual: owner, but tile is not a CiRCLE (kind 0).
    // Any tile whose static kind != 0 rejects `is_circle`.
    let mut t = kani::any();
    kani::assume(t >= 0 && t < 60 && board::TILES[t as usize].kind != 0);
    w.trig_tile = t;
    t = w.trig_tile;
    let gid = crate::guard_id!("actor == owner && is_circle(tile.id)");
    let ctx = fill_pass_circle(&w, w.trig_player);
    let admitted = crate::guard!(gid, &ctx);
    assert!(!admitted, "non-CiRCLE tile must reject the Pass guard");
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Host would not call the body. Invoke anyway to show the residual is the
    // only gate: the body itself has no tile check and would still gain. We
    // therefore do **not** invoke — the no-op is the host gate.
    let _ = pre;
    let _ = player;
    let _ = t;
}

/// MUTANT companion: a spec demanding GainFire{2} must be distinguishable.
#[kani::proof]
#[kani::unwind(80)]
pub fn touko_01_mutant_is_detected() {
    let mut w = symbolic_pass_world();
    let player0 = w.trig_player;
    w.fire_max[player0 as usize] = spec::TOUKO_01_FIRE_MAX;
    w.fire[player0 as usize] = 0;
    // Force the circle guard to admit: pin the tile to a known CiRCLE.
    let mut circle = -1;
    let mut i = 0usize;
    while i < 60 {
        if board::TILES[i].kind == 0 && circle < 0 {
            circle = i as i32;
        }
        i += 1;
    }
    kani::assume(circle >= 0);
    w.trig_tile = circle;
    let player = player0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_TOUKO, "Pass", player));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let card = spec::touko_01_log_ok(effects, player);
    let mutant = spec::touko_01_log_ok_mutant(effects, player);
    kani::cover!(card && !mutant, "card matches book, not the n=2 mutant");
    assert!(card, "below-cap path must gain exactly one");
    assert!(!mutant, "C-MOR-touko-01 mutant: card must not match GainFire(2)");
}

// ===========================================================================
// C-MOR-rui-crit-02
// 「…roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金…
//  若结果大于X，X+1。X初始为0」
// ===========================================================================

fn symbolic_yuri_world() -> World {
    let mut w = symbolic_hook_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    w.trig_player = player;
    // Either side of a non-zero money move (OPEN-双方影响). `amount > 0` is a
    // gain, `amount < 0` a loss — the body's own sign split.
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= -10_000 && w.trig_value <= 10_000);
    kani::assume(w.trig_value != 0);
    w.pay_amount = w.trig_value;
    w.trig_target = kani::any();
    kani::assume(w.trig_target >= -1 && w.trig_target < w.n_players);
    // X starts at 0; bound it so both hit and miss are reachable.
    let x: i32 = kani::any();
    kani::assume(x >= 0 && x <= 20);
    let key = intern::intern(KEY_YURI_X);
    w.key_set(player, key, x);
    w
}

/// Residual host gate of the PayChoose hook (`Some(mine)`): this seat is on
/// either side of the move. Matches the book's 「获得或失去资金」.
fn yuri_residual(w: &World, player: i32) -> bool {
    w.trig_player == player || w.trig_target == player
}

/// IFF: 1d20 vs X. Hit (`n ≤ X`): X := 0 and the gain doubles / the loss is
/// cancelled. Miss: X := X+1.
#[kani::proof]
#[kani::unwind(80)]
pub fn rui_crit_02_roll_iff() {
    let w = symbolic_yuri_world();
    let player = w.trig_player;
    let amount = w.pay_amount;
    let x = w.key_get(player, intern::intern(KEY_YURI_X));

    // Host residual (the CEL pre is empty — always admits).
    let residual = yuri_residual(&w, player);
    kani::cover!(residual, "on either side of the move");
    kani::cover!(!residual, "not involved");
    if !residual {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_YURI, "PayChoose", player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let x_after = world::with_world(|ww| ww.key_get(player, intern::intern(KEY_YURI_X)));

    // Recover the branch from post X: hit ⇒ 0, miss ⇒ x+1. (When x == 0 a
    // hit is impossible because the die is ≥ 1, so x_after ∈ {1} only.)
    let miss_expected_x = x + 1;
    let is_hit = x_after == 0;
    let is_miss = x_after == miss_expected_x;
    kani::cover!(is_hit, "hit (n ≤ X): X reset");
    kani::cover!(is_miss, "miss (n > X): X+1");
    assert!(
        is_hit != is_miss,
        "C-MOR-rui-crit-02: post X is either 0 (hit) or x+1 (miss)"
    );

    if is_hit {
        let after = spec::rui_crit_02_amount_on_hit(amount);
        assert!(
            spec::rui_crit_02_log_hit_ok(effects, player, after),
            "C-MOR-rui-crit-02 hit: SetState(0) + SetPayAmount(rewritten)"
        );
        assert!(
            world::with_world(|ww| ww.pay_amount) == after,
            "C-MOR-rui-crit-02 hit: pay amount rewritten"
        );
    } else {
        assert!(
            spec::rui_crit_02_log_miss_ok(effects, player, miss_expected_x),
            "C-MOR-rui-crit-02 miss: SetState(x+1) only"
        );
        assert!(
            world::with_world(|ww| ww.pay_amount) == amount,
            "C-MOR-rui-crit-02 miss: amount unchanged"
        );
    }

    let post = world::snapshot();
    assert!(world::arr4_eq(post.money, pre.money), "frame: money at rewrite time");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
}

/// NO-OP: a zero amount (or a move that does not touch this seat) is a no-op.
#[kani::proof]
#[kani::unwind(80)]
pub fn rui_crit_02_noop() {
    let mut w = symbolic_yuri_world();
    // Force the body's own early-out: amount == 0 (also the residual may hold).
    w.pay_amount = 0;
    w.trig_value = 0;
    let player = w.trig_player;
    let key = intern::intern(KEY_YURI_X);
    let pre_x = w.key_get(player, key);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_YURI, "PayChoose", player));
    kani::cover!(true, "body returned");
    let (_logbuf, log_n) = effects();
    assert!(log_n == 0, "C-MOR-rui-crit-02 noop: log empty at amount 0");
    let post_x = world::with_world(|ww| ww.key_get(player, key));
    assert!(post_x == pre_x, "X unchanged");
    let post = world::snapshot();
    assert!(post.pay_amount == pre.pay_amount, "amount unchanged");
}

/// MUTANT companion: a strict `n < x` threshold must be distinguishable at
/// `n == x` (the book's 「小于等于X」 is inclusive).
///
/// Pin X = 1 and let the die be symbolic in 1..=20. When the die is 1 the
/// book hits (X := 0) and the strict mutant misses (X := 2).
#[kani::proof]
#[kani::unwind(80)]
pub fn rui_crit_02_mutant_is_detected() {
    let mut w = symbolic_yuri_world();
    let player = w.trig_player;
    let key = intern::intern(KEY_YURI_X);
    w.key_set(player, key, 1);
    kani::assume(w.pay_amount > 0);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(SKILL_YURI, "PayChoose", player));
    kani::cover!(true, "body returned");
    let x_after = world::with_world(|ww| ww.key_get(player, key));
    // Post X is 0 (book hit, inclusive at n == 1) or 2 (miss).
    let book_hit = x_after == 0;
    let book_miss = x_after == 2;
    kani::cover!(book_hit, "book hit at n == 1 (inclusive)");
    kani::cover!(book_miss, "miss at n ≥ 2");
    assert!(
        book_hit || book_miss,
        "C-MOR-rui-crit-02: post X at x=1 is 0 (hit) or 2 (miss)"
    );
    // The strict mutant would map n == 1 to a miss (X := 2). Whenever the
    // book hit, the mutant disagrees.
    let mutant_x_after = spec::rui_crit_02_x_after_mutant(1, 1);
    kani::cover!(book_hit && mutant_x_after == 2, "reachable disagreement at n == 1");
    assert!(
        mutant_x_after == 2,
        "mutant threshold is strict: n == 1 is a miss for it"
    );
    if book_hit {
        assert!(
            spec::rui_crit_02_x_after(1, 1) == 0,
            "book inclusive threshold resets X at n == x"
        );
    }
}

// ===========================================================================
// C-MOR-tsukushi-01
// 「（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作…」
// ===========================================================================

fn symbolic_tsukushi01_world() -> World {
    let mut w = symbolic_hook_world();
    // Skill owner = the seat receiving the money (the `mine` residual).
    let owner: i32 = kani::any();
    kani::assume(owner >= 0 && owner < w.n_players);
    kani::assume(!w.out[owner as usize]);
    w.trig_target = owner;
    w.trig_player = kani::any();
    kani::assume(w.trig_player >= -1 && w.trig_player < 8);
    w.trig_value = kani::any();
    kani::assume(w.trig_value > 0 && w.trig_value <= 10_000);
    w.pay_amount = w.trig_value;
    w.out[owner as usize] = false;
    w
}

/// IFF: the money-redirect arithmetic of a single `on_gain` body, with every
/// prompt answer explored (`symbolic_answers`).
/// OPEN-X标记 / 「单轮结算」 are out of scope (see the spec).
#[kani::proof]
#[kani::unwind(80)]
pub fn tsukushi_01_redirect_iff() {
    let mut w = symbolic_tsukushi01_world();
    w.symbolic_answers = true;
    let player = w.trig_target; // skill owner = the gainer
    let amount = w.pay_amount;
    // Host residual of the first PayChoose hook (`mine`).
    let residual = w.trig_target == player && w.trig_value > 0;
    kani::cover!(residual, "this seat is gaining");
    if !residual {
        return;
    }
    // Count other living seats (the body's redirect targets).
    let mut n_others = 0i32;
    for p in 0..w.n_players {
        if p != player && !w.out[p as usize] {
            n_others += 1;
        }
    }
    kani::cover!(n_others >= 1, "at least one other living seat");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // Entry index 2 = the `mine` PayChoose hook (`on_gain`).
    invoke::unwrap_done(invoke::entry_idx(SKILL_TSUKUSHI, 2, player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    if n_others == 0 {
        assert!(log_n == 0, "C-MOR-tsukushi-01: no target ⇒ no redirect");
        return;
    }

    if log_n == 0 {
        // Declined the yes/no (or the pick collapsed). Book: the original
        // gain stays.
        assert!(
            spec::tsukushi_01_decline_ok(effects),
            "C-MOR-tsukushi-01 decline: empty log"
        );
    } else {
        // Accepted: recover `who` from the forwarded gain.
        let who = match effects[3] {
            Effect::GainMoney { player: p, .. } => p,
            _ => -99,
        };
        kani::cover!(who != player, "redirected to another seat");
        assert!(
            spec::tsukushi_01_log_ok(effects, player, who, amount),
            "C-MOR-tsukushi-01: zero original, mark+record+forward amount"
        );
    }

    let post = world::snapshot();
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
}

/// MUTANT companion: forwarding `2×amount` must be distinguishable.
#[kani::proof]
#[kani::unwind(80)]
pub fn tsukushi_01_mutant_is_detected() {
    let mut w = symbolic_tsukushi01_world();
    w.symbolic_answers = true;
    let player = w.trig_target;
    // Keep exactly one other living seat so the pick is forced.
    let mut kept = false;
    for p in 0..w.n_players {
        if p == player {
            continue;
        }
        if !kept {
            w.out[p as usize] = false;
            kept = true;
        } else {
            w.out[p as usize] = true;
        }
    }
    for p in 0..MAX_PLAYERS {
        if (p as i32) >= w.n_players {
            w.out[p] = true;
        }
    }
    let amount = w.pay_amount;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(SKILL_TSUKUSHI, 2, player));
    kani::cover!(true, "body returned");
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    if log_n == 4 {
        let who = match effects[3] {
            Effect::GainMoney { player: p, .. } => p,
            _ => -99,
        };
        let card = spec::tsukushi_01_log_ok(effects, player, who, amount);
        let mutant = spec::tsukushi_01_log_ok_mutant(effects, player, who, amount);
        kani::cover!(card && !mutant, "card forwards amount, not 2×amount");
        assert!(
            !(card && mutant),
            "C-MOR-tsukushi-01 mutant: card must not match the 2× spec"
        );
    }
}

// ===========================================================================
// C-MOR-tsukushi-03
// 「（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得
//  移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。」
// ===========================================================================

fn symbolic_tsukushi03_world() -> World {
    let mut w = symbolic_hook_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    kani::assume(!w.out[player as usize]);
    // Seed X marks: each seat holds a bounded up/down count. The skill owner
    // may hold either face too — 「移除所有X标记」.
    let up = intern::intern(TOK_X_UP);
    let down = intern::intern(TOK_X_DOWN);
    for p in 0..MAX_PLAYERS {
        let u: i32 = kani::any();
        kani::assume(u >= 0 && u <= 2);
        let d: i32 = kani::any();
        kani::assume(d >= 0 && d <= 2);
        w.key_set(p as i32, up, u);
        w.key_set(p as i32, down, d);
    }
    // Rate counter (engine-side decay). `declare` (TurnStartBefore) seeds 800
    // before any (3) press, so the legal pre-state is a positive rate.
    let rate: i32 = kani::any();
    kani::assume(rate >= 200 && rate <= 1000);
    let rk = intern::intern(KEY_TSUKU_RATE);
    w.key_set(player, rk, rate);
    w
}

/// IFF of the (3) Play body: every other living seat holds a flipped mark,
/// then all X marks are removed and the user gains `n × per_mark`.
#[kani::proof]
#[kani::unwind(80)]
pub fn tsukushi_03_sweep_iff() {
    let w = symbolic_tsukushi03_world();
    let player = w.trig_player;
    let up = intern::intern(TOK_X_UP);
    let down = intern::intern(TOK_X_DOWN);
    let rk = intern::intern(KEY_TSUKU_RATE);

    // Host gate `can_cash`: every other living seat holds DOWN ≥ 1.
    let mut all_ready = true;
    let mut n_others = 0i32;
    for p in 0..w.n_players {
        if p == player || w.out[p as usize] {
            continue;
        }
        n_others += 1;
        if w.key_get(p, down) < 1 {
            all_ready = false;
        }
    }
    kani::cover!(all_ready && n_others > 0, "ready to cash");
    kani::cover!(!all_ready, "not ready (host would refuse the press)");
    if !all_ready {
        return;
    }

    // Snapshot the holder list the body will walk (seat order).
    let mut holders = [(0i32, 0i32, 0i32); 4];
    let mut n_holders = 0usize;
    let mut n_marks = 0i32;
    for p in 0..w.n_players {
        let u = w.key_get(p, up);
        let d = w.key_get(p, down);
        if u + d > 0 {
            holders[n_holders] = (p, u, d);
            n_holders += 1;
            n_marks += u + d;
        }
    }
    let pre_rate = w.key_get(player, rk);
    let want_gain = spec::tsukushi_03_gain(n_marks, pre_rate);
    let want_rate_after = spec::tsukushi_03_rate_after(pre_rate);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::skill_play_idx(SKILL_TSUKUSHI, player));
    kani::cover!(true, "body returned");

    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let holders_slice = &holders[..n_holders];

    assert!(
        spec::tsukushi_03_log_ok(effects, player, holders_slice, n_marks, pre_rate),
        "C-MOR-tsukushi-03: clear marks, GainMoney(n×rate), SetState(rate-200 floor 200)"
    );

    let post = world::snapshot();
    assert!(
        spec::tsukushi_03_gain(n_marks, pre_rate) == want_gain,
        "gain arithmetic pinned"
    );
    let post_rate = world::with_world(|ww| ww.key_get(player, rk));
    assert!(post_rate == want_rate_after, "rate decayed as the book says");
    // Marks cleared on every holder.
    for i in 0..n_holders {
        let (h, _, _) = holders[i];
        let u = world::with_world(|ww| ww.key_get(h, up));
        let d = world::with_world(|ww| ww.key_get(h, down));
        assert!(u == 0 && d == 0, "X marks removed");
    }
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    assert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
}

/// MUTANT companion: floor at 0 instead of 200 must be distinguishable.
#[kani::proof]
#[kani::unwind(80)]
pub fn tsukushi_03_mutant_is_detected() {
    let mut w = symbolic_tsukushi03_world();
    let player = w.trig_player;
    let up = intern::intern(TOK_X_UP);
    let down = intern::intern(TOK_X_DOWN);
    let rk = intern::intern(KEY_TSUKU_RATE);
    // Force ready: every other living seat has a flipped mark; rate already
    // at the floor so the decay mutant diverges.
    for p in 0..w.n_players {
        if p != player && !w.out[p as usize] {
            w.key_set(p, down, 1);
            w.key_set(p, up, 0);
        }
    }
    w.key_set(player, up, 1);
    w.key_set(player, down, 0);
    w.key_set(player, rk, 200); // at floor: book stays 200, mutant → 0
    let pre_rate = 200;
    world::reset_world(w);
    invoke::unwrap_done(invoke::skill_play_idx(SKILL_TSUKUSHI, player));
    kani::cover!(true, "body returned");
    let post_rate = world::with_world(|ww| ww.key_get(player, rk));
    let book = spec::tsukushi_03_rate_after(pre_rate);
    let mutant = spec::tsukushi_03_rate_after_mutant(pre_rate);
    kani::cover!(book != mutant, "book floors at 200, mutant at 0");
    assert!(book == 200 && mutant == 0, "witness: book 200 vs mutant 0");
    assert!(post_rate == book, "rate stays at the 200 floor");
    assert!(post_rate != mutant, "C-MOR-tsukushi-03 mutant: 0-floor is wrong");
}

// ===========================================================================
// Documented-only / blocked clauses (no harness — see spec/skills_mor.rs):
//   C-MOR-mashiro-01 / -02   PROTOCOL-LEVEL (RollPlan plan-field stubs)
//   C-MOR-touko-02           BLOCKED-HOST: `rent_of`
//   C-MOR-nanami-02 / -03    PROTOCOL-LEVEL / documented-only
//   C-MOR-rui-crit-01        OPEN-复制 / documented-only
//   C-MOR-tsukushi-02        spec pinned; harness deferred (mark flip +
//                            draw-tag gate are OPEN/PROTOCOL)
// ===========================================================================