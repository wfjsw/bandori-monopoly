//! A deliberately WRONG guard is caught by the same proof machinery.
//!
//! The host condition of `C-PP-aya-haruka-02` is `"actor == owner && value > 0"`.
//! A too-loose mutant (`value >= 0`) admits `value == 0`, where the real guard
//! rejects. The harness below asserts the **iff** between the guard under test
//! and the clause's stated trigger under a symbolic context. The wrong guard
//! must FAIL that iff.
//!
//! Both IRs are written as tiny node arrays here so CBMC does not walk the
//! 139-entry generated table (`guard_by_src` string-compares every entry).
//! That the real array is the lower of the CEL string is pinned by
//! `rules/cond/tests/cond_ir_consistency.rs` and by
//! `aya_haruka_02_shave_iff`, which evaluates the generated program.

use rules_cond_ir::ctx::IrCtx;
use rules_cond_ir::nodes::{Kind, Node};
use rules_cond_ir::vars::VarId;

/// Lower of `"actor == owner && value > 0"` (the real guard). See
/// `generated/cond_irs.rs` for the checked-in copy of the same nodes.
fn real_guard_nodes() -> [Node; 6] {
    [
        Node::leaf_var(VarId::Actor as u16),
        Node::leaf_var(VarId::Owner as u16),
        Node::binary(Kind::Eq, 0, 1),
        Node::leaf_var(VarId::Value as u16),
        Node::leaf_const_int(0),
        Node::binary(Kind::Gt, 3, 4),
        // And is node 6 — see eval below (root = 6, so we need 7 slots).
    ]
}

fn real_guard_nodes7() -> [Node; 7] {
    [
        Node::leaf_var(VarId::Actor as u16),
        Node::leaf_var(VarId::Owner as u16),
        Node::binary(Kind::Eq, 0, 1),
        Node::leaf_var(VarId::Value as u16),
        Node::leaf_const_int(0),
        Node::binary(Kind::Gt, 3, 4),
        Node::binary(Kind::And, 2, 5),
    ]
}

/// `value >= 0` — the WRONG guard (too loose at `value == 0`).
fn wrong_guard_nodes() -> [Node; 3] {
    [
        Node::leaf_var(VarId::Value as u16),
        Node::leaf_const_int(0),
        Node::binary(Kind::Ge, 0, 1),
    ]
}

/// The clause's stated condition from the 规则书 quote (the proof obligation):
/// the hook fires on the owner's own pay/consume with a positive amount.
fn clause_condition(actor: i64, owner: i64, value: i64) -> bool {
    actor == owner && value > 0
}

fn fill(actor: i64, owner: i64, value: i64) -> IrCtx {
    let mut ctx = IrCtx::default();
    ctx.n_players = 4;
    ctx.set_scalar(VarId::Actor, actor);
    ctx.set_scalar(VarId::Owner, owner);
    ctx.set_scalar(VarId::Value, value);
    ctx
}

/// `#[kani::should_panic]`: the wrong guard is NOT equivalent to the clause.
/// Kani must find the `value == 0` counterexample where `value >= 0` admits
/// but `value > 0` rejects.
#[kani::proof]
#[kani::unwind(6)]
#[kani::should_panic]
pub fn wrong_guard_too_loose_is_caught() {
    let actor: i64 = kani::any();
    let owner: i64 = kani::any();
    let value: i64 = kani::any();
    kani::assume(actor >= -4 && actor <= 4);
    kani::assume(owner >= -4 && owner <= 4);
    kani::assume(value >= -2 && value <= 2);

    let ctx = fill(actor, owner, value);
    let wn = wrong_guard_nodes();
    let wrong = rules_cond_ir::eval_node(&wn, 2, &ctx).as_cond_bool();
    let rn = real_guard_nodes7();
    let real = rules_cond_ir::eval_node(&rn, 6, &ctx).as_cond_bool();
    let book = clause_condition(actor, owner, value);

    // The same proof obligation the real harness uses:
    // guard ⇔ clause condition. The wrong guard fails it at `value == 0`.
    assert!(wrong == book);
    assert!(real == book);
}

/// The real guard satisfies the iff — non-vacuous companion.
#[kani::proof]
#[kani::unwind(6)]
pub fn real_guard_matches_clause_iff() {
    let actor: i64 = kani::any();
    let owner: i64 = kani::any();
    let value: i64 = kani::any();
    kani::assume(actor >= -4 && actor <= 4);
    kani::assume(owner >= -4 && owner <= 4);
    kani::assume(value >= -2 && value <= 2);

    let ctx = fill(actor, owner, value);
    let rn = real_guard_nodes7();
    let real = rules_cond_ir::eval_node(&rn, 6, &ctx).as_cond_bool();
    let book = clause_condition(actor, owner, value);
    assert!(real == book);
    kani::cover!(real);
    kani::cover!(!real);
}

/// Companion of `wrong_guard_too_loose_is_caught` (no `should_panic`): the
/// wrong guard is **distinguishable** from the clause — reachable disagreement.
/// If this FAILS, the mutant is equivalent (vacuous iff).
#[kani::proof]
#[kani::unwind(6)]
pub fn wrong_guard_disagreement_is_reachable() {
    let actor: i64 = kani::any();
    let owner: i64 = kani::any();
    let value: i64 = kani::any();
    kani::assume(actor >= -4 && actor <= 4);
    kani::assume(owner >= -4 && owner <= 4);
    kani::assume(value >= -2 && value <= 2);

    let ctx = fill(actor, owner, value);
    let wn = wrong_guard_nodes();
    let wrong = rules_cond_ir::eval_node(&wn, 2, &ctx).as_cond_bool();
    let book = clause_condition(actor, owner, value);
    // The disagreement is live at `value == 0` (wrong admits, clause rejects).
    kani::cover!(wrong && !book, "value == 0: wrong admits, clause rejects");
    kani::cover!(!wrong && book, "value > 0 && actor==owner: wrong rejects? no — cover other side");
    kani::cover!(wrong == book, "they agree");
    assert!(
        !(wrong == book && value == 0 && actor == owner),
        "at value==0 the wrong guard must disagree with the clause"
    );
    // Stronger: there EXISTS a reachable input where they disagree. Prove by
    // showing the specific witness is inside the symbolic range.
    let witness_ctx = fill(1, 1, 0);
    let w_witness = rules_cond_ir::eval_node(&wn, 2, &witness_ctx).as_cond_bool();
    let book_witness = clause_condition(1, 1, 0);
    assert!(w_witness != book_witness, "witness value==0 must disagree");
    kani::cover!(w_witness != book_witness, "reachable disagreement");
}

/// Sanity: `#[kani::should_panic]` plumbing works at all.
#[kani::proof]
#[kani::unwind(2)]
#[kani::should_panic]
pub fn sanity_should_panic() {
    let x: i64 = kani::any();
    assert!(x != x);
}

/// Sanity 2: the range assume actually keeps `value == 0`.
#[kani::proof]
#[kani::unwind(2)]
#[kani::should_panic]
pub fn sanity_value_zero_reachable() {
    let value: i64 = kani::any();
    kani::assume(value >= -2 && value <= 2);
    assert!(value != 0);
}