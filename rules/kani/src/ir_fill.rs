//! Fill a [`rules_cond_ir::IrCtx`] from the symbolic [`world::World`].
//!
//! This is the remaining trusted base of the single-source design: the
//! lowering from engine world → condition context fields. It is small,
//! total, and integer-only so Kani can walk it. Every field is also filled
//! by the host's `fill_window` / `fill_candidate` in production; the
//! differential tests pin the *evaluator*, and this pin is the *binding*.

use rules_cond_ir::ctx::{FnId, IrCtx, SeatField, SetId};
use rules_cond_ir::vars::{VarId, VarTy, VARS};
use rules_cond_ir::Val;

use crate::world::{self, World};

/// The three pilot guards (single source: the CEL string on the `On::*` entry).
pub mod src {
    /// PP:[丸山彩]憧憬的前方 `On::Hook(PayAdd)`.
    pub const AYA_PAY_ADD: &str = "actor == owner && value > 0";
    /// A function-carrying guard (`is_circle`).
    pub const IS_CIRCLE: &str = "actor == owner && is_circle(tile.id)";
    /// A chain/kind-carrying guard (`effect.has(Pay)`).
    pub const CHAIN_PAY: &str =
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0";
}

/// Look up a generated guard by its CEL source.
pub fn guard_by_src(src: &str) -> &'static rules_cond_ir::Program {
    for g in generated::ALL {
        if g.src == src {
            return &g.program;
        }
    }
    panic!("guard not in generated table; re-run gen_cond_ir");
}

pub mod generated {
    pub use crate::generated::cond_irs::{GuardIr, ALL};
}

/// Fill the window/candidate scalars a PayAdd hook sees.
///
/// * `actor` = the triggering seat (`trig_player`)
/// * `owner` = the card's owner (`player_id`)
/// * `value` = the pay amount (`trig_value`)
pub fn fill_pay_add(w: &World, owner: i32) -> IrCtx {
    let mut c = IrCtx::default();
    c.n_players = w.n_players as i64;
    // Window
    // Trigger kind: Pay = 9 (`kinds::trig::Pay`).
    c.set_scalar(VarId::Kind, 9);
    c.set_scalar(VarId::Actor, w.trig_player as i64);
    c.set_scalar(VarId::Value, w.trig_value as i64);
    c.set_scalar(VarId::Target, w.trig_player as i64);
    c.set_scalar(VarId::TurnPlayer, w.trig_player as i64);
    c.set_scalar(VarId::By, -1);
    c.set_scalar(VarId::Step, 0);
    c.set_scalar(VarId::RollSource, -1);
    c.set_scalar(VarId::TurnKey, 0);
    c.set_scalar(VarId::TriggerCard, 0);
    c.set_scalar(VarId::CounterName, 0);
    c.set_scalar(VarId::TileId, -1);
    c.set_scalar(VarId::TileOwner, -1);
    for (id, _, ty) in VARS {
        if *ty == VarTy::OptInt {
            c.set_scalar(*id, -1);
        }
    }
    // Candidate
    c.set_scalar(VarId::Owner, owner as i64);
    c.set_scalar(VarId::OwnerId, owner as i64);
    c.set_scalar(VarId::OwnerMoney, w.money[owner as usize] as i64);
    c.set_scalar(VarId::OwnerFire, w.fire[owner as usize] as i64);
    c.set_scalar(VarId::OwnerHand, w.hand_resonance[owner as usize] as i64);
    c.set_scalar(VarId::OwnerPos, w.pos[owner as usize] as i64);
    c.set_scalar(VarId::OwnerOut, w.out[owner as usize] as i64);
    c.set_scalar(VarId::OwnerStay, w.stay[owner as usize] as i64);
    c.set_scalar(VarId::CardPlaced, 1);
    c.set_scalar(VarId::CardId, 0);
    // Per-seat money (for `money(p)` / poorest scans if a guard used them).
    for p in 0..world::MAX_PLAYERS {
        c.seat[p][SeatField::Money as usize] = w.money[p] as i64;
        c.seat[p][SeatField::Fire as usize] = w.fire[p] as i64;
        c.seat[p][SeatField::Out as usize] = w.out[p] as i64;
        c.seat[p][SeatField::Pos as usize] = w.pos[p] as i64;
    }
    // Fan tokens (`tok('P✽P粉丝(正)')` etc.) — the shave clause reads them.
    let _ = (FnId::Tok, SetId::Circle);
    for p in 0..world::MAX_PLAYERS {
        // Named lookups are per-candidate; fill the owner's.
        let _ = p;
    }
    c.named_set(
        rules_cond_ir::ctx::id_of(world::TOK_PP_POS),
        w.fans_pos[owner as usize] as i64,
    );
    c.named_set(
        rules_cond_ir::ctx::id_of(world::TOK_PP_NEG),
        w.fans_neg[owner as usize] as i64,
    );
    c
}

/// Fill a context for the `is_circle` pilot: a placed card on `tile`.
pub fn fill_is_circle(w: &World, owner: i32, tile: i64) -> IrCtx {
    let mut c = fill_pay_add(w, owner);
    c.set_scalar(VarId::TileId, tile);
    // The pilot board: tile 1 is a circle.
    c.set_insert(SetId::Circle, 1);
    c
}

/// Fill a context for the `effect.has(Pay)` pilot.
pub fn fill_chain_pay(w: &World, owner: i32) -> IrCtx {
    let mut c = fill_pay_add(w, owner);
    // Chain link: kind = Pay (9), hits = owner.
    c.set_insert(SetId::ChainKinds, 9);
    c.set_insert(SetId::ChainHits, owner as i64);
    c.set_scalar(VarId::EffectCount, 1);
    c
}

/// Evaluate a program as a condition (host root rule).
pub fn eval_bool(p: &rules_cond_ir::Program, c: &IrCtx) -> bool {
    p.eval_bool(c)
}

/// Value-level helper for the proofs.
pub fn eval_val(p: &rules_cond_ir::Program, c: &IrCtx) -> Val {
    p.eval(c)
}