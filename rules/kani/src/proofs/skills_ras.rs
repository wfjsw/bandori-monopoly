//! Unit `skills_ras`: clause proofs.
//!
//! Character skills (RAISE A SUILEN) + band skill UNSTOPPABLE.
//! Classification lives in `docs/rulebook/spec/kani/ras.md`.
//!
//! **ppcost pattern.** Concrete user seats. Direct body call via the
//! cfg(kani) `NAME_kani` wrapper. Per-harness host-op subset.
//!
//! Isolation: fire-pot pass gains assume `fire + 1 <= charge_max` so the
//! (unmodeled) cap does not bind. The printed cap itself is BLOCKED-HOST.

use crate::host;
use crate::invoke;
use crate::spec::skills_ras as spec;
use crate::world::{self, Effect, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Host-op subset: both `on_pass` bodies are one-liners into
// `rules_status::charge::gain` → `gain_charge`.
// ---------------------------------------------------------------------------
crate::host_subset!(fire_pass_dispatch {
    gain_charge => arm_gain_charge,
    log | effect => arm_log,
});

/// In-place 2-seat world: fire pots symbolic, default cap 3, sub-cap so the
/// unmodeled cap does not bind.
fn fire_world(user: i32) {
    host::install_backend(fire_pass_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        for p in 0..MAX_PLAYERS {
            w.out[p] = false;
            w.charge[p] = kani::any();
            kani::assume(w.charge[p] >= 0 && w.charge[p] <= 2);
            // default model cap; skill declare_cap is BLOCKED-HOST
            w.charge_max[p] = 3;
            kani::assume(w.charge[p] + 1 <= w.charge_max[p]);
        }
        w.trig_player = user;
    });
}

/// Shared leaf iff at a **concrete** user seat (seat-symmetric 「自己的」).
/// `call` is a *direct* body invocation already expanded at the call site
/// (do not pass `fn` pointers — that reintroduces the code* IF-chain).
macro_rules! fire_pass_leaf_iff_at {
    ($user:expr, $call:expr) => {{
        let user: i32 = $user;
        debug_assert!(user == 0 || user == 1);
        fire_world(user);
        let pre = world::snapshot();
        invoke::unwrap_done($call);
        kani::cover!(true, "body returned");
        let post = world::snapshot();
        let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
        let log_n = world::with_world(|ww| ww.log_n);
        let effects = &logbuf[..log_n];

        crate::kassert!(
            spec::fire_pass_log_ok(effects, user),
            "leaf is exactly one +1 GainFire"
        );
        crate::kassert!(
            post.charge[user as usize] == pre.charge[user as usize] + spec::fire_pass_gain(),
            "fire is pre+1"
        );
        crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
        crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: positions unchanged");
    }};
}

/// C-RAS-masking-01 leaf: `skill:佐藤益木` `on_pass` requests +1 [火罐].
/// The tile gate (`白雪学园` / `银河拉面馆`) is the hook's CEL pre and is not
/// re-checked here — this harness proves the leaf the body runs once admitted.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
pub fn masking_01_fire_gain_leaf() {
    fire_pass_leaf_iff_at!(0, skill_personas::sato_red::on_pass_kani(0));
}

/// Same leaf at seat 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
pub fn masking_01_user1_fire_gain_leaf() {
    fire_pass_leaf_iff_at!(1, skill_personas::sato_red::on_pass_kani(1));
}

/// C-RAS-rio-01 leaf: `skill:和奏瑞依` `on_pass` requests +1 [火罐].
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
pub fn rio_01_fire_gain_leaf() {
    fire_pass_leaf_iff_at!(0, skill_personas::raise_effort::on_pass_kani(0));
}

/// Mutant companion for the fire-pass leaf (+2 ≠ +1).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
pub fn fire_pass_gain_mutant_is_detected() {
    fire_world(0);
    let user = 0i32;
    invoke::unwrap_done(skill_personas::sato_red::on_pass_kani(user));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::fire_pass_log_ok_mutant(effects, user),
        "mutant: card must not match the +2 GainFire spec"
    );
}