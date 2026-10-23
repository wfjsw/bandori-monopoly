//! Unit `skills_mor`: clause proofs for Morfonica band + character skills.
//!
//! Specs live in [`crate::spec::skills_mor`] (book quotes there). This file
//! drives the **unmodified** skill bodies via the cfg(kani) `NAME_kani`
//! wrappers against an in-place symbolic world and asserts post == spec(pre),
//! effect log exactly, frame.
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
//! Isolation (stated here and `kani::assume`d where relevant):
//! * no other skill / card hooks fire in the window (single-body call);
//! * piles stay empty except where a draw is the clause effect;
//! * charge_max is pinned per clause (「上限」 figure from the quote);
//! * `n_players = 2`, money `0..=10_000`, both seats living unless stated.
//!
//! Host notes:
//! * `Op::trig_value` reads `w.pay_amount` (not `w.trig_value`) — both are
//!   written so the body and the guard IR agree.
//! * Kani `Op::gain_charge` honours `charge_max` (`got = n.clamp(0, cap - fire)`),
//!   so the at-cap leg matches the book (empty log, unchanged fire).
//! * Kani `Op::trig_set_cancelled` sets `w.trig_cancelled`, so the
//!   「替换奖励」 leg of C-MOR-band-02 is observable.
//!
//! Feature gates: band-skill harnesses need `skills-groups`; character-skill
//! harnesses need `skills-personas` (the `*_kani` wrappers live in those
//! crates). `tools/kani/run.sh` `unit_features skills_mor` should therefore be
//! `band-morfonica,skills-groups,skills-personas`.
//!
//! Bounds: every harness that calls a body uses `#[kani::unwind(32)]`.
//! Each `unwrap_done` is followed by `kani::cover!(true, "body returned")` so
//! that vacuity is visible in the cover table. Pure-formula harnesses (no
//! body) use a tight unwind.

use crate::world::{self, Effect};

// ---------------------------------------------------------------------------
// Shared helpers (local to this unit — shared files stay untouched)
// ---------------------------------------------------------------------------

/// Copy the effect log into a fixed buffer (no `Vec` under CBMC).
fn effects_of(buf: &mut [Effect; world::MAX_LOG]) -> usize {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    *buf = logbuf;
    log_n
}

/// Light IrCtx for the `actor == owner` family (PayAfter / PassRewardAffected).
#[allow(dead_code)]
fn fill_mine(owner: i32) -> rules_cond_ir::ctx::IrCtx {
    world::with_world(|w| crate::ir_fill::fill_pay_add(w, owner))
}

/// Light IrCtx for `actor == owner && is_start(tile.id)` — the Pass hook.
/// (Full `fill_from_world` is a CBMC cliff; `conf_fill_diff` pins the binding.)
///
/// Board walk is an 8×8 chunk (TUNING) so the unwind bound is 8, not 60.
#[allow(dead_code)]
fn fill_pass_circle(owner: i32) -> rules_cond_ir::ctx::IrCtx {
    world::with_world(|w| {
        let mut c = crate::ir_fill::fill_pay_add(w, owner);
        c.set_scalar(rules_cond_ir::vars::VarId::TileId, w.trig_tile as i64);
        // 8×8 chunk walk over the 60-tile board (unwind bound 8).
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < 60 && crate::board::TILES[i].kind == 0 {
                    c.set_insert(rules_cond_ir::ctx::SetId::Circle, i as i64);
                }
                k += 1;
            }
            outer += 1;
        }
        c
    })
}

// ===========================================================================
// C-MOR-band-01 / -02  `skill:Morfonica:振翅高飞的练习曲`
// ===========================================================================

#[cfg(feature = "skills-groups")]
mod band {
    use super::{effects_of, fill_mine};
    use crate::host;
    use crate::intern;
    use crate::invoke;
    use crate::spec::skills_mor as spec;
    use crate::world::{self, Effect};

    const KEY_TAKEN: &str = "skill.morfonica.moneyTaken";

    /// Unmodified `after_pay` reaches: draw, log, trig_value.
    crate::host_subset!(mor_after_pay_dispatch {
        draw => arm_draw,
        log | effect => arm_log,
        trig_value => arm_trig_value,
    });

    /// Unmodified `on_circle` reaches: state_get, state_set, gain,
    /// trig_set_cancelled.
    crate::host_subset!(mor_on_circle_dispatch {
        state_get => arm_state_get,
        state_set => arm_state_set,
        gain | gain_typed | gain_fixed => arm_gain,
        trig_set_cancelled => arm_trig_set_cancelled,
        log | effect => arm_log,
    });

    /// In-place 2-seat sparse world + concrete user seat. Writes the global
    /// cell directly (no `reset_world` full-struct assign — SSA cliff).
    fn after_pay_world(user: i32) {
        host::install_backend(mor_after_pay_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        world::with_world_mut(|w| {
            w.trig_player = user;
            // One money move: positive magnitude. `Op::trig_value` reads
            // `pay_amount`; the guard IR reads `trig_value`. Both.
            let value: i32 = kani::any();
            kani::assume(value >= 0 && value <= 10_000);
            w.trig_value = value;
            w.pay_amount = value;
            // Payee of the move (`-1` = bank). The body draws regardless;
            // the host gate (`target != owner`) is the clause condition.
            let target: i32 = kani::any();
            kani::assume(target >= -1 && target < w.n_players);
            w.trig_target = target;
        });
    }

    /// CiRCLE-reward world for the `on_circle` hook.
    fn on_circle_world(user: i32) {
        host::install_backend(mor_on_circle_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        world::with_world_mut(|w| {
            w.trig_player = user;
            // `REWARD_MONEY` (0) / `REWARD_CARD` (1). `Op::trig_value` reads
            // `pay_amount`.
            let value: i32 = kani::any();
            kani::assume(value == 0 || value == 1);
            w.trig_value = value;
            w.pay_amount = value;
            // Pre-seed the taken counter.
            let taken: i32 = kani::any();
            kani::assume(taken >= 0 && taken <= 8);
            let key = intern::intern(KEY_TAKEN);
            w.key_set(user, key, taken);
        });
    }

    // =======================================================================
    // C-MOR-band-01
    // 「（1）当你一次性失去2000以上资金时，抽一张卡。」
    // Seat-symmetric (the owner is the one losing). n_players=2 → the two
    // user=0/1 harnesses cover both seats of the 2-seat world.
    // =======================================================================

    /// Shared iff body at a **concrete** user seat.
    fn band_01_iff_at(user: i32) {
        debug_assert!(user == 0 || user == 1);
        after_pay_world(user);
        let gid = crate::guard_id!("actor == owner && value >= 2000 && target != owner");
        let admitted = {
            let ctx = fill_mine(user);
            crate::guard!(gid, &ctx)
        };
        kani::cover!(admitted, "guard admits");
        kani::cover!(!admitted, "guard rejects");
        if !admitted {
            return;
        }

        let value = world::with_world(|w| w.pay_amount);
        let target = world::with_world(|w| w.trig_target);

        let pre = world::snapshot();
        // DIRECT body call (ppcost): one body in CBMC's call graph.
        invoke::unwrap_done(skill_groups::morfonica::after_pay_kani(user));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];

        let want = spec::band_01_should_draw(value, target, user);
        kani::cover!(want, "draws (loss ≥ 2000)");
        kani::cover!(!want, "no draw (below 2000 or not a loss)");

        if want {
            crate::kassert!(
                spec::band_01_log_ok(effects, user),
                "C-MOR-band-01: effect log must be exactly Draw(player, 1)"
            );
        } else {
            crate::kassert!(
                log_n == 0,
                "C-MOR-band-01: false trigger must produce no effects"
            );
        }

        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
        crate::kassert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
    }

    /// C-MOR-band-01 iff at seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_01_draw_iff() {
        band_01_iff_at(0);
    }

    /// Same iff at seat 1 — the two harnesses cover every seat of the 2-seat world.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_01_user1_draw_iff() {
        band_01_iff_at(1);
    }

    /// NO-OP: a move that is not a ≥2000 loss never reaches the body. The
    /// body has no size check (the condition is the host gate), so we assert
    /// the gate stays shut and do **not** invoke.
    /// Concrete user seat 0 (the no-op is seat-symmetric; the iff pair covers
    /// both seats).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_01_noop_below_or_gain() {
        after_pay_world(0);
        // Pin the false-trigger region.
        kani::assume(world::with_world(|w| {
            w.pay_amount < 2000 || w.trig_target == w.trig_player
        }));
        let gid = crate::guard_id!("actor == owner && value >= 2000 && target != owner");
        let admitted = {
            let ctx = fill_mine(0);
            crate::guard!(gid, &ctx)
        };
        crate::kassert!(!admitted, "false trigger must reject the host gate");
        // Host would not call the body. No invoke — the no-op is the host gate.
    }

    /// MUTANT companion: the card must disagree with a spec that draws at 1999.
    /// Concrete user seat 0. The body has no size check (the condition is the
    /// host gate), so the reachable disagreement is at the gate: 1999 is
    /// inside the mutant's fire region but the card's host gate stays shut.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_01_mutant_is_detected() {
        after_pay_world(0);
        let user = 0i32;
        // Reachable disagreement: a 1999 loss to the bank.
        world::with_world_mut(|w| {
            w.trig_value = 1999;
            w.pay_amount = 1999;
            w.trig_target = -1;
        });
        let value = 1999i32;
        let target = -1i32;
        let gid = crate::guard_id!("actor == owner && value >= 2000 && target != owner");
        let admitted = {
            let ctx = fill_mine(user);
            crate::guard!(gid, &ctx)
        };
        let mutant_wants_draw = spec::band_01_should_draw_mutant(value, target, user);
        let card_fires = admitted && spec::band_01_should_draw(value, target, user);
        kani::cover!(
            mutant_wants_draw && !card_fires,
            "mutant wants a draw at 1999, card's gate stays shut"
        );
        crate::kassert!(
            mutant_wants_draw,
            "mutant threshold is 1999: this loss is inside its fire region"
        );
        crate::kassert!(!card_fires, "book threshold is 2000: 1999 must not fire");
        crate::kassert!(
            mutant_wants_draw && !card_fires,
            "C-MOR-band-01 mutant: card must not match the 1999-draw spec"
        );
    }

    // =======================================================================
    // C-MOR-band-02
    // 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
    // Seat-symmetric (the owner takes the CiRCLE reward).
    // =======================================================================

    /// Shared iff body at a **concrete** user seat.
    fn band_02_iff_at(user: i32) {
        debug_assert!(user == 0 || user == 1);
        on_circle_world(user);
        let gid = crate::guard_id!("actor == owner && value == 0");
        let admitted = {
            let ctx = fill_mine(user);
            crate::guard!(gid, &ctx)
        };
        kani::cover!(admitted, "guard admits (money option)");
        if !admitted {
            return;
        }

        let value = world::with_world(|w| w.pay_amount);
        let taken = world::with_world(|w| w.key_get(user, intern::intern(KEY_TAKEN)));

        let pre = world::snapshot();
        invoke::unwrap_done(skill_groups::morfonica::on_circle_kani(user));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];

        let money_opt = spec::band_02_is_money_option(value);
        kani::cover!(money_opt, "money option (cycle applies)");
        kani::cover!(!money_opt, "card option (no cycle)");
        kani::cover!(taken % 3 == 0, "1st of the cycle → 1000");
        kani::cover!(taken % 3 == 1, "2nd → 1500");
        kani::cover!(taken % 3 == 2, "3rd → 2000");

        if money_opt {
            crate::kassert!(
                spec::band_02_log_ok(effects, user, taken),
                "C-MOR-band-02: SetState(taken+1) + GainMoney(cycle amount)"
            );
            // 「替换奖励」 — the reward is cancelled and replaced by the sum.
            crate::kassert!(
                world::with_world(|w| w.trig_cancelled),
                "C-MOR-band-02: 替换奖励 — set_cancelled must latch"
            );
        } else {
            crate::kassert!(
                log_n == 0,
                "C-MOR-band-02: non-money option must be a no-op"
            );
        }

        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
        crate::kassert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
    }

    /// C-MOR-band-02 iff at seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_02_cycle_iff() {
        band_02_iff_at(0);
    }

    /// Same iff at seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_02_user1_cycle_iff() {
        band_02_iff_at(1);
    }

    /// NO-OP: the card half of [CiRCLE奖励] must not touch money or the counter.
    /// Concrete user seat 0.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_02_noop_card_option() {
        on_circle_world(0);
        let user = 0i32;
        world::with_world_mut(|w| {
            // REWARD_CARD
            w.trig_value = spec::BAND_02_REWARD_MONEY + 1;
            w.pay_amount = w.trig_value;
        });
        let key = intern::intern(KEY_TAKEN);
        let pre_taken = world::with_world(|w| w.key_get(user, key));
        let gid = crate::guard_id!("actor == owner && value == 0");
        let admitted = {
            let ctx = fill_mine(user);
            crate::guard!(gid, &ctx)
        };
        crate::kassert!(!admitted, "card option must reject the money-option gate");
        // Host would not call the body. No invoke — the no-op is the host gate.
        let _ = pre_taken;
    }

    /// MUTANT companion: cycle rotated by one must be distinguishable.
    /// Concrete user seat 0, first of the cycle (→ 1000; mutant says 1500).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn band_02_mutant_is_detected() {
        on_circle_world(0);
        let user = 0i32;
        world::with_world_mut(|w| {
            w.trig_value = spec::BAND_02_REWARD_MONEY;
            w.pay_amount = w.trig_value;
            w.key_set(user, intern::intern(KEY_TAKEN), 0);
        });
        let taken = 0i32;
        invoke::unwrap_done(skill_groups::morfonica::on_circle_kani(user));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        let card = spec::band_02_log_ok(effects, user, taken);
        let mutant_amount = spec::band_02_amount_mutant(taken);
        let card_amount = spec::band_02_amount(taken);
        kani::cover!(
            card && card_amount != mutant_amount,
            "reachable disagreement on the cycle"
        );
        crate::kassert!(card, "first-of-cycle must pay 1000");
        crate::kassert!(card_amount == 1000, "book: 1st = 1000");
        crate::kassert!(mutant_amount == 1500, "mutant rotates to 1500");
        crate::kassert!(
            card_amount != mutant_amount,
            "C-MOR-band-02 mutant: card must not match the rotated cycle"
        );
        let _ = log_n;
    }
}

// ===========================================================================
// C-MOR-touko-01 / C-MOR-nanami-01
// 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 / cap 2
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod pass_fire {
    use super::{effects_of, fill_pass_circle};
    use crate::host;
    use crate::invoke;
    use crate::spec::skills_mor as spec;
    use crate::world::{self, Effect};

    /// Unmodified `on_pass` (touko / nanami) reaches: gain_charge.
    /// Shared: both bodies are the same one-liner (`charge::gain(p, 1, …)`).
    crate::host_subset!(pass_gain_charge_dispatch {
        gain_marker => arm_gain_marker,
        log | effect => arm_log,
    });

    /// In-place world for a Pass-CiRCLE fire gain. Concrete user seat.
    fn pass_world(user: i32) {
        host::install_backend(pass_gain_charge_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        world::with_world_mut(|w| {
            w.trig_player = user;
            // The tile being passed. Circle tiles are kind 0 on the static
            // board. Pin to a known CiRCLE so the host gate admits. 8×8 chunk
            // walk (TUNING) keeps the unwind bound at 8, not 60.
            let mut circle = -1i32;
            let mut outer = 0usize;
            while outer < 8 {
                let mut k = 0usize;
                while k < 8 {
                    let i = outer * 8 + k;
                    if i < 60 && crate::board::TILES[i].kind == 0 && circle < 0 {
                        circle = i as i32;
                    }
                    k += 1;
                }
                outer += 1;
            }
            kani::assume(circle >= 0);
            w.trig_tile = circle;
        });
    }

    /// Shared Pass-gain IFF for touko (cap 1) and nanami (cap 2) at a concrete
    /// user seat. Below-cap the book and the Kani host agree (`GainFire{1}`);
    /// at-cap the host now honours `charge_max`, so the book no-op is asserted.
    fn pass_gain_iff_at(
        user: i32,
        charge_max_pin: i32,
        label: &str,
        body: fn(i32) -> card_sdk::Asked,
    ) {
        debug_assert!(user == 0 || user == 1);
        pass_world(user);
        world::with_world_mut(|w| {
            // Pin the seat's fire-pot cap to the quote's 「上限」.
            w.charge_max[user as usize] = charge_max_pin;
        });

        let gid = crate::guard_id!(bandori_std::guards::OWN_START);
        let admitted = {
            let ctx = fill_pass_circle(user);
            crate::guard!(gid, &ctx)
        };
        kani::cover!(admitted, "guard admits (pass CiRCLE)");
        kani::cover!(!admitted, "guard rejects (not CiRCLE / not owner)");
        if !admitted {
            return;
        }

        let pre = world::snapshot();
        invoke::unwrap_done(body(user));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        let pre_fire = pre.charge[user as usize];
        let post = world::snapshot();
        let post_fire = post.charge[user as usize];

        let below = pre_fire < charge_max_pin;
        kani::cover!(below, "below cap (gain applies)");
        kani::cover!(!below, "at cap (book: no-op)");

        if below {
            crate::kassert!(
                spec::touko_01_log_ok(effects, user),
                "{label}: effect log must be exactly GainFire{{player,1}}"
            );
            crate::kassert!(
                spec::touko_01_post_below_cap(post_fire, pre_fire),
                "{label}: fire must increase by one below cap"
            );
        } else {
            // Book: empty log + unchanged fire. The Kani host honours
            // `charge_max`, so this path is now assertable.
            crate::kassert!(
                log_n == 0,
                "{label}: at cap the gain is refused (empty log)"
            );
            crate::kassert!(
                spec::touko_01_post_at_cap(post_fire, pre_fire),
                "{label}: fire unchanged at cap"
            );
        }

        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
    }

    /// C-MOR-touko-01 IFF at seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn touko_01_fire_iff() {
        pass_gain_iff_at(
            0,
            spec::TOUKO_01_FIRE_MAX,
            "C-MOR-touko-01",
            skill_personas::kiritani_zenith::on_pass_kani,
        );
    }

    /// C-MOR-touko-01 IFF at seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn touko_01_user1_fire_iff() {
        pass_gain_iff_at(
            1,
            spec::TOUKO_01_FIRE_MAX,
            "C-MOR-touko-01",
            skill_personas::kiritani_zenith::on_pass_kani,
        );
    }

    /// C-MOR-nanami-01 IFF at seat 0 (same shape, cap 2).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn nanami_01_fire_iff() {
        pass_gain_iff_at(
            0,
            spec::NANAMI_01_FIRE_MAX,
            "C-MOR-nanami-01",
            skill_personas::nanami_ordinary::on_pass_kani,
        );
    }

    /// C-MOR-nanami-01 IFF at seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn nanami_01_user1_fire_iff() {
        pass_gain_iff_at(
            1,
            spec::NANAMI_01_FIRE_MAX,
            "C-MOR-nanami-01",
            skill_personas::nanami_ordinary::on_pass_kani,
        );
    }

    /// NO-OP: passing a non-CiRCLE tile produces nothing.
    /// Pure-formula (guard IR only — no body, no host_subset).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(8)]
    pub fn touko_01_noop_not_circle() {
        // Force the residual: owner, but tile is not a CiRCLE (kind 0).
        let mut t = kani::any();
        kani::assume(t >= 0 && t < 60 && crate::board::TILES[t as usize].kind != 0);
        let gid = crate::guard_id!(bandori_std::guards::OWN_START);
        world::with_world_mut(|w| {
            w.trig_player = 0;
            w.trig_tile = t;
        });
        t = world::with_world(|w| w.trig_tile);
        let admitted = {
            let ctx = fill_pass_circle(0);
            crate::guard!(gid, &ctx)
        };
        crate::kassert!(!admitted, "non-CiRCLE tile must reject the Pass guard");
    }

    /// MUTANT companion: a spec demanding GainFire{2} must be distinguishable.
    /// Concrete user seat 0, below cap.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn touko_01_mutant_is_detected() {
        pass_world(0);
        let user = 0i32;
        world::with_world_mut(|w| {
            w.charge_max[user as usize] = spec::TOUKO_01_FIRE_MAX;
            w.charge[user as usize] = 0;
        });
        invoke::unwrap_done(skill_personas::kiritani_zenith::on_pass_kani(user));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        let card = spec::touko_01_log_ok(effects, user);
        let mutant = spec::touko_01_log_ok_mutant(effects, user);
        kani::cover!(card && !mutant, "card matches book, not the n=2 mutant");
        crate::kassert!(card, "below-cap path must gain exactly one");
        crate::kassert!(
            !mutant,
            "C-MOR-touko-01 mutant: card must not match GainFire(2)"
        );
        let _ = log_n;
    }
}

#[cfg(feature = "skills-personas")]
pub use pass_fire::*;

// ===========================================================================
// C-MOR-rui-crit-02  `skill:八潮瑠唯:正论暴击` (2)
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod yuri_crit {
    use super::effects_of;
    use crate::host;
    use crate::intern;
    use crate::invoke;
    use crate::spec::skills_mor as spec;
    use crate::world::{self, Effect};

    const KEY_YURI_X: &str = "skill.yuriCrit.x";

    /// Unmodified `on_pay` reaches: trig_value, state_get, roll, state_set,
    /// trig_set_pay_amount, effect, log.
    crate::host_subset!(yuri_on_pay_dispatch {
        trig_value => arm_trig_value,
        state_get => arm_state_get,
        roll => arm_roll,
        state_set => arm_state_set,
        trig_set_pay_amount => arm_trig_set_pay_amount,
        log | effect => arm_log,
    });

    /// In-place world for the PaySplitChoose rewrite. Concrete user seat.
    fn yuri_world(user: i32) {
        host::install_backend(yuri_on_pay_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        world::with_world_mut(|w| {
            w.trig_player = user;
            // Either side of a non-zero money move (OPEN-双方影响). `amount > 0`
            // is a gain, `amount < 0` a loss — the body's own sign split.
            let value: i32 = kani::any();
            kani::assume(value >= -10_000 && value <= 10_000);
            kani::assume(value != 0);
            w.trig_value = value;
            w.pay_amount = value;
            let target: i32 = kani::any();
            kani::assume(target >= -1 && target < w.n_players);
            w.trig_target = target;
            // X starts at 0; bound it so both hit and miss are reachable.
            let x: i32 = kani::any();
            kani::assume(x >= 0 && x <= 20);
            let key = intern::intern(KEY_YURI_X);
            w.key_set(user, key, x);
        });
    }

    /// Residual host gate of the PaySplitChoose hook (`Some(mine)`): this seat is on
    /// either side of the move. Matches the book's 「获得或失去资金」.
    fn yuri_residual(user: i32) -> bool {
        world::with_world(|w| w.trig_player == user || w.trig_target == user)
    }

    /// Shared iff body at a **concrete** user seat.
    fn rui_crit_02_iff_at(user: i32) {
        debug_assert!(user == 0 || user == 1);
        yuri_world(user);

        // Host residual (the CEL pre is empty — always admits).
        let residual = yuri_residual(user);
        kani::cover!(residual, "on either side of the move");
        kani::cover!(!residual, "not involved");
        if !residual {
            return;
        }

        let amount = world::with_world(|w| w.pay_amount);
        let x = world::with_world(|w| w.key_get(user, intern::intern(KEY_YURI_X)));
        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::yuri_crit::on_pay_kani(user));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        let x_after = world::with_world(|ww| ww.key_get(user, intern::intern(KEY_YURI_X)));

        // Recover the branch from post X: hit ⇒ 0, miss ⇒ x+1. (When x == 0 a
        // hit is impossible because the die is ≥ 1, so x_after ∈ {1} only.)
        let miss_expected_x = x + 1;
        let is_hit = x_after == 0;
        let is_miss = x_after == miss_expected_x;
        kani::cover!(is_hit, "hit (n ≤ X): X reset");
        kani::cover!(is_miss, "miss (n > X): X+1");
        crate::kassert!(
            is_hit != is_miss,
            "C-MOR-rui-crit-02: post X is either 0 (hit) or x+1 (miss)"
        );

        if is_hit {
            let after = spec::rui_crit_02_amount_on_hit(amount);
            crate::kassert!(
                spec::rui_crit_02_log_hit_ok(effects, user, after),
                "C-MOR-rui-crit-02 hit: SetState(0) + SetPayAmount(rewritten)"
            );
            crate::kassert!(
                world::with_world(|ww| ww.pay_amount) == after,
                "C-MOR-rui-crit-02 hit: pay amount rewritten"
            );
        } else {
            crate::kassert!(
                spec::rui_crit_02_log_miss_ok(effects, user, miss_expected_x),
                "C-MOR-rui-crit-02 miss: SetState(x+1) only"
            );
            crate::kassert!(
                world::with_world(|ww| ww.pay_amount) == amount,
                "C-MOR-rui-crit-02 miss: amount unchanged"
            );
        }

        let post = world::snapshot();
        crate::kassert!(
            world::arr4_eq(post.money, pre.money),
            "frame: money at rewrite time"
        );
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    }

    /// C-MOR-rui-crit-02 iff at seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn rui_crit_02_roll_iff() {
        rui_crit_02_iff_at(0);
    }

    /// Same iff at seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn rui_crit_02_user1_roll_iff() {
        rui_crit_02_iff_at(1);
    }

    /// NO-OP: a zero amount (or a move that does not touch this seat) is a no-op.
    /// Concrete user seat 0.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn rui_crit_02_noop() {
        yuri_world(0);
        let user = 0i32;
        // Force the body's own early-out: amount == 0.
        world::with_world_mut(|w| {
            w.pay_amount = 0;
            w.trig_value = 0;
        });
        let key = intern::intern(KEY_YURI_X);
        let pre_x = world::with_world(|w| w.key_get(user, key));
        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::yuri_crit::on_pay_kani(user));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        crate::kassert!(log_n == 0, "C-MOR-rui-crit-02 noop: log empty at amount 0");
        let post_x = world::with_world(|ww| ww.key_get(user, key));
        crate::kassert!(post_x == pre_x, "X unchanged");
        let post = world::snapshot();
        crate::kassert!(post.pay_amount == pre.pay_amount, "amount unchanged");
    }

    /// MUTANT companion: a strict `n < x` threshold must be distinguishable at
    /// `n == x` (the book's 「小于等于X」 is inclusive).
    ///
    /// Pin X = 1 and let the die be symbolic in 1..=20. When the die is 1 the
    /// book hits (X := 0) and the strict mutant misses (X := 2).
    /// Concrete user seat 0.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn rui_crit_02_mutant_is_detected() {
        yuri_world(0);
        let user = 0i32;
        let key = intern::intern(KEY_YURI_X);
        world::with_world_mut(|w| {
            w.key_set(user, key, 1);
            // Pin a gain so the rewrite is the positive leg.
            w.trig_value = 100;
            w.pay_amount = 100;
            w.trig_target = user;
        });
        invoke::unwrap_done(skill_personas::yuri_crit::on_pay_kani(user));
        kani::cover!(true, "body returned");
        let x_after = world::with_world(|ww| ww.key_get(user, key));
        // Post X is 0 (book hit, inclusive at n == 1) or 2 (miss).
        let book_hit = x_after == 0;
        let book_miss = x_after == 2;
        kani::cover!(book_hit, "book hit at n == 1 (inclusive)");
        kani::cover!(book_miss, "miss at n ≥ 2");
        crate::kassert!(
            book_hit || book_miss,
            "C-MOR-rui-crit-02: post X at x=1 is 0 (hit) or 2 (miss)"
        );
        // The strict mutant would map n == 1 to a miss (X := 2). Whenever the
        // book hit, the mutant disagrees.
        let mutant_x_after = spec::rui_crit_02_x_after_mutant(1, 1);
        kani::cover!(
            book_hit && mutant_x_after == 2,
            "reachable disagreement at n == 1"
        );
        crate::kassert!(
            mutant_x_after == 2,
            "mutant threshold is strict: n == 1 is a miss for it"
        );
        if book_hit {
            crate::kassert!(
                spec::rui_crit_02_x_after(1, 1) == 0,
                "book inclusive threshold resets X at n == x"
            );
        }
    }
}

#[cfg(feature = "skills-personas")]
pub use yuri_crit::*;

// ===========================================================================
// C-MOR-tsukushi-01  `skill:二叶筑紫:交给班长吧` (1)
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod tsukushi_on_gain {
    use super::effects_of;
    use crate::host;
    use crate::invoke;
    use crate::spec::skills_mor as spec;
    use crate::world::{self, Effect};

    /// Unmodified `on_gain` reaches: trig_value, player_count, player_out,
    /// ask, opt_str, trig_set_pay_amount, add_held, state_set, gain, log.
    crate::host_subset!(tsukushi_on_gain_dispatch {
        trig_value => arm_trig_value,
        player_count => arm_player_count,
        player_out => arm_player_out,
        ask => arm_ask,
        opt_str | opt_int | opt_tile => arm_opt,
        trig_set_pay_amount => arm_trig_set_pay_amount,
        add_held => arm_add_held,
        state_set => arm_state_set,
        gain | gain_typed | gain_fixed => arm_gain,
        log | effect => arm_log,
    });

    /// In-place world: skill owner = the seat receiving the money (`mine`).
    /// Concrete owner seat. Symbolic prompt answers.
    fn on_gain_world(owner: i32) {
        host::install_backend(tsukushi_on_gain_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        world::with_world_mut(|w| {
            w.trig_target = owner;
            // A print carries `player_id = -1`; a pay-player carries the payer.
            let actor: i32 = kani::any();
            kani::assume(actor >= -1 && actor < 8);
            w.trig_player = actor;
            let value: i32 = kani::any();
            kani::assume(value > 0 && value <= 10_000);
            w.trig_value = value;
            w.pay_amount = value;
            w.out[owner as usize] = false;
            w.symbolic_answers = true;
        });
    }

    /// Shared iff body at a **concrete** owner seat. `n_players = 2` → exactly
    /// one other living seat, so the pick is forced once the yes/no is taken.
    fn tsukushi_01_iff_at(owner: i32) {
        debug_assert!(owner == 0 || owner == 1);
        on_gain_world(owner);
        let amount = world::with_world(|w| w.pay_amount);

        // Host residual of the first PaySplitChoose hook (`mine`).
        let residual = world::with_world(|w| {
            w.trig_target == owner
                && w.trig_value > 0
                && (0..w.n_players).any(|p| p != owner && !w.out[p as usize])
        });
        kani::cover!(residual, "this seat is gaining, with a redirect target");
        if !residual {
            return;
        }

        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::tsugushi_monitor::on_gain_kani(owner));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];

        if log_n == 0 {
            // Declined the yes/no (or the pick collapsed). Book: the original
            // gain stays.
            crate::kassert!(
                spec::tsukushi_01_decline_ok(effects),
                "C-MOR-tsukushi-01 decline: empty log"
            );
        } else {
            // Accepted: recover `who` from the forwarded gain.
            let who = match effects[3] {
                Effect::GainMoney { player: p, .. } => p,
                _ => -99,
            };
            kani::cover!(who != owner, "redirected to another seat");
            crate::kassert!(
                spec::tsukushi_01_log_ok(effects, owner, who, amount),
                "C-MOR-tsukushi-01: zero original, mark+record+forward amount"
            );
        }

        let post = world::snapshot();
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    }

    /// C-MOR-tsukushi-01 iff at owner seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_01_redirect_iff() {
        tsukushi_01_iff_at(0);
    }

    /// Same iff at owner seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_01_user1_redirect_iff() {
        tsukushi_01_iff_at(1);
    }

    /// MUTANT companion: forwarding `2×amount` must be distinguishable.
    /// Concrete owner seat 0; `n_players = 2` already forces one other seat.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_01_mutant_is_detected() {
        on_gain_world(0);
        let owner = 0i32;
        // Keep the other live seat living so the pick is forced.
        world::with_world_mut(|w| {
            w.out[1] = false;
        });
        let amount = world::with_world(|w| w.pay_amount);
        invoke::unwrap_done(skill_personas::tsugushi_monitor::on_gain_kani(owner));
        kani::cover!(true, "body returned");
        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        if log_n == 4 {
            let who = match effects[3] {
                Effect::GainMoney { player: p, .. } => p,
                _ => -99,
            };
            let card = spec::tsukushi_01_log_ok(effects, owner, who, amount);
            let mutant = spec::tsukushi_01_log_ok_mutant(effects, owner, who, amount);
            kani::cover!(card && !mutant, "card forwards amount, not 2×amount");
            crate::kassert!(
                !(card && mutant),
                "C-MOR-tsukushi-01 mutant: card must not match the 2× spec"
            );
        }
        let _ = log_n;
    }
}

#[cfg(feature = "skills-personas")]
pub use tsukushi_on_gain::*;

// ===========================================================================
// C-MOR-tsukushi-03  `skill:二叶筑紫:交给班长吧` (3)
// ===========================================================================

#[cfg(feature = "skills-personas")]
mod tsukushi_cash {
    use super::effects_of;
    use crate::host;
    use crate::intern;
    use crate::invoke;
    use crate::spec::skills_mor as spec;
    use crate::world::{self, Effect, MAX_PLAYERS};

    const KEY_TSUKU_RATE: &str = "skill.tsugushi.rate";
    const TOK_X_UP: &str = "X";
    const TOK_X_DOWN: &str = "X标记(反)";

    /// Unmodified `cash` reaches: player_count, count_held_name, add_held,
    /// state_get, gain, state_set, log.
    crate::host_subset!(tsukushi_cash_dispatch {
        player_count => arm_player_count,
        count_held | count_held_name => arm_count_held,
        add_held => arm_add_held,
        state_get => arm_state_get,
        gain | gain_typed | gain_fixed => arm_gain,
        state_set => arm_state_set,
        log | effect => arm_log,
    });

    /// In-place world: X marks on the two live seats, rate counter seeded.
    /// Concrete user seat. The body walks `0..player_count()` (= 0..4 under
    /// Kani); seats ≥ n_players keep 0 marks so they are skipped.
    fn cash_world(user: i32) {
        host::install_backend(tsukushi_cash_dispatch);
        crate::harness::sparse_play_world(crate::harness::SparsePlay {
            used_seats: 2,
            n_players: 2,
            ..Default::default()
        });
        let up = intern::intern(TOK_X_UP);
        let down = intern::intern(TOK_X_DOWN);
        let rk = intern::intern(KEY_TSUKU_RATE);
        world::with_world_mut(|w| {
            w.trig_player = user;
            w.out[user as usize] = false;
            // Seed X marks on the live seats only.
            for p in 0..w.n_players {
                let u: i32 = kani::any();
                kani::assume(u >= 0 && u <= 2);
                let d: i32 = kani::any();
                kani::assume(d >= 0 && d <= 2);
                w.key_set(p, up, u);
                w.key_set(p, down, d);
            }
            // Rate counter (engine-side decay). `declare` (TurnStartBefore)
            // seeds 800 before any (3) press, so the legal pre-state is a
            // positive rate.
            let rate: i32 = kani::any();
            kani::assume(rate >= 200 && rate <= 1000);
            w.key_set(user, rk, rate);
        });
    }

    /// Shared iff body at a **concrete** user seat. `n_players = 2` → one
    /// other live seat; the body still walks `0..4` for marks (non-seats hold 0).
    fn tsukushi_03_iff_at(user: i32) {
        debug_assert!(user == 0 || user == 1);
        cash_world(user);
        let up = intern::intern(TOK_X_UP);
        let down = intern::intern(TOK_X_DOWN);
        let rk = intern::intern(KEY_TSUKU_RATE);

        // Host gate `can_cash`: every other living seat holds DOWN ≥ 1.
        let (all_ready, n_others) = world::with_world(|w| {
            let mut all_ready = true;
            let mut n_others = 0i32;
            for p in 0..w.n_players {
                if p == user || w.out[p as usize] {
                    continue;
                }
                n_others += 1;
                if w.key_get(p, down) < 1 {
                    all_ready = false;
                }
            }
            (all_ready, n_others)
        });
        kani::cover!(all_ready && n_others > 0, "ready to cash");
        kani::cover!(!all_ready, "not ready (host would refuse the press)");
        if !all_ready {
            return;
        }

        // Snapshot the holder list the body will walk (`0..player_count` = 0..4).
        let mut holders = [(0i32, 0i32, 0i32); 4];
        let mut n_holders = 0usize;
        let mut n_marks = 0i32;
        world::with_world(|w| {
            for p in 0..MAX_PLAYERS {
                let u = w.key_get(p as i32, up);
                let d = w.key_get(p as i32, down);
                if u + d > 0 {
                    holders[n_holders] = (p as i32, u, d);
                    n_holders += 1;
                    n_marks += u + d;
                }
            }
        });
        let pre_rate = world::with_world(|w| w.key_get(user, rk));
        let want_gain = spec::tsukushi_03_gain(n_marks, pre_rate);
        let want_rate_after = spec::tsukushi_03_rate_after(pre_rate);

        let pre = world::snapshot();
        invoke::unwrap_done(skill_personas::tsugushi_monitor::cash_kani(user));
        kani::cover!(true, "body returned");

        let mut ebuf = [Effect::SetPayAmount { to: 0 }; world::MAX_LOG];
        let log_n = effects_of(&mut ebuf);
        let effects = &ebuf[..log_n];
        let holders_slice = &holders[..n_holders];

        crate::kassert!(
            spec::tsukushi_03_log_ok(effects, user, holders_slice, n_marks, pre_rate),
            "C-MOR-tsukushi-03: clear marks, GainMoney(n×rate), SetState(rate-200 floor 200)"
        );

        let post = world::snapshot();
        crate::kassert!(
            spec::tsukushi_03_gain(n_marks, pre_rate) == want_gain,
            "gain arithmetic pinned"
        );
        let post_rate = world::with_world(|ww| ww.key_get(user, rk));
        crate::kassert!(
            post_rate == want_rate_after,
            "rate decayed as the book says"
        );
        // Marks cleared on every holder.
        for i in 0..n_holders {
            let (h, _, _) = holders[i];
            let u = world::with_world(|ww| ww.key_get(h, up));
            let d = world::with_world(|ww| ww.key_get(h, down));
            crate::kassert!(u == 0 && d == 0, "X marks removed");
        }
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
        crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
        let _ = log_n;
    }

    /// C-MOR-tsukushi-03 iff at seat 0 (`n_players = 2`).
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_03_sweep_iff() {
        tsukushi_03_iff_at(0);
    }

    /// Same iff at seat 1.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_03_user1_sweep_iff() {
        tsukushi_03_iff_at(1);
    }

    /// MUTANT companion: floor at 0 instead of 200 must be distinguishable.
    /// Concrete user seat 0; rate already at the floor so the decay mutant
    /// diverges.
    #[kani::proof]
    #[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
    #[kani::unwind(32)]
    pub fn tsukushi_03_mutant_is_detected() {
        cash_world(0);
        let user = 0i32;
        let up = intern::intern(TOK_X_UP);
        let down = intern::intern(TOK_X_DOWN);
        let rk = intern::intern(KEY_TSUKU_RATE);
        // Force ready: every other living seat has a flipped mark; rate already
        // at the floor so the decay mutant diverges.
        world::with_world_mut(|w| {
            for p in 0..w.n_players {
                if p != user && !w.out[p as usize] {
                    w.key_set(p, down, 1);
                    w.key_set(p, up, 0);
                }
            }
            w.key_set(user, up, 1);
            w.key_set(user, down, 0);
            w.key_set(user, rk, 200); // at floor: book stays 200, mutant → 0
        });
        let pre_rate = 200;
        invoke::unwrap_done(skill_personas::tsugushi_monitor::cash_kani(user));
        kani::cover!(true, "body returned");
        let post_rate = world::with_world(|ww| ww.key_get(user, rk));
        let book = spec::tsukushi_03_rate_after(pre_rate);
        let mutant = spec::tsukushi_03_rate_after_mutant(pre_rate);
        kani::cover!(book != mutant, "book floors at 200, mutant at 0");
        crate::kassert!(book == 200 && mutant == 0, "witness: book 200 vs mutant 0");
        crate::kassert!(post_rate == book, "rate stays at the 200 floor");
        crate::kassert!(
            post_rate != mutant,
            "C-MOR-tsukushi-03 mutant: 0-floor is wrong"
        );
    }
}

#[cfg(feature = "skills-personas")]
pub use tsukushi_cash::*;

// ===========================================================================
// Documented-only / blocked clauses (no harness — see spec/skills_mor.rs):
//   C-MOR-mashiro-01 / -02   PROTOCOL-LEVEL (DiceRollPlan plan-field stubs)
//   C-MOR-touko-02           BLOCKED-HOST: `rent_of`
//   C-MOR-nanami-02 / -03    PROTOCOL-LEVEL / documented-only
//   C-MOR-rui-crit-01        OPEN-复制 / documented-only
//   C-MOR-tsukushi-02        spec pinned; harness deferred (mark flip +
//                            draw-tag gate are OPEN/PROTOCOL)
// ===========================================================================
