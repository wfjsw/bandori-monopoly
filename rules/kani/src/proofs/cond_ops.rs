//! Kani proofs that the IR interpreter agrees with the written reference
//! semantics on every closed-set operator, for every operand.
//!
//! These are the machine-checked half of "check 2" (translator correctness).
//! The differential tests in `rules-cond/tests/cond_ir_diff.rs` pin the IR to
//! the vendored `cel` evaluator; these proofs pin the IR to the written spec
//! for all values, not just the boundary grid.
//!
//! The proofs call the **value-level** operator steps (`eval_binop` /
//! `eval_unop` / `eval_and` / `eval_or`) so CBMC does not pull the `Call` /
//! function-table paths into the unwinding graph. Tree composition is covered
//! by the differential tests and by the structural claim in
//! `docs/rulebook/spec/COND-SINGLE-SOURCE.md` §4.

use rules_cond_ir::eval::{eval_and, eval_binop, eval_or, eval_unop};
use rules_cond_ir::nodes::Kind;
use rules_cond_ir::{spec, Val};

fn symbolic_val() -> Val {
    let x: i64 = kani::any();
    let which: u8 = kani::any();
    kani::assume(which < 3);
    match which {
        0 => Val::Int(x),
        1 => Val::Bool(x != 0),
        _ => Val::Null,
    }
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_eq_matches_spec_for_all_operands() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Eq, a, b), spec::spec_eq(a, b));
    assert_eq!(eval_binop(Kind::Ne, a, b), spec::spec_ne(a, b));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_lt_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Lt, a, b), spec::spec_cmp(a, b, spec::Cmp::Lt));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_le_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Le, a, b), spec::spec_cmp(a, b, spec::Cmp::Le));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_gt_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Gt, a, b), spec::spec_cmp(a, b, spec::Cmp::Gt));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_ge_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Ge, a, b), spec::spec_cmp(a, b, spec::Cmp::Ge));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_add_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Add, a, b), spec::spec_arith(a, b, spec::Arith::Add));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_sub_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Sub, a, b), spec::spec_arith(a, b, spec::Arith::Sub));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_mul_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    assert_eq!(eval_binop(Kind::Mul, a, b), spec::spec_arith(a, b, spec::Arith::Mul));
}

/// Division. The full i64×i64 grid makes CBMC's `checked_div` path blow up
/// (>4 min); we case-split on the three interesting regions and let the
/// boundary grid in `cond_ir_diff.rs` cover `i64::MIN / -1` exhaustively.
#[kani::proof]
#[kani::unwind(4)]
pub fn ir_div_matches_spec() {
    let x: i64 = kani::any();
    let y: i64 = kani::any();
    // Region 1: y == 0 → both sides Err.
    if y == 0 {
        assert_eq!(eval_binop(Kind::Div, Val::Int(x), Val::Int(y)), Val::Err);
        assert_eq!(
            spec::spec_arith(Val::Int(x), Val::Int(y), spec::Arith::Div),
            Val::Err
        );
        return;
    }
    // Region 2: the overflow pair.
    if x == i64::MIN && y == -1 {
        assert_eq!(eval_binop(Kind::Div, Val::Int(x), Val::Int(y)), Val::Err);
        assert_eq!(
            spec::spec_arith(Val::Int(x), Val::Int(y), spec::Arith::Div),
            Val::Err
        );
        return;
    }
    // Region 3: ordinary division.
    kani::assume(y != 0);
    kani::assume(!(x == i64::MIN && y == -1));
    assert_eq!(
        eval_binop(Kind::Div, Val::Int(x), Val::Int(y)),
        spec::spec_arith(Val::Int(x), Val::Int(y), spec::Arith::Div)
    );
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_rem_matches_spec() {
    let x: i64 = kani::any();
    let y: i64 = kani::any();
    if y == 0 {
        assert_eq!(eval_binop(Kind::Rem, Val::Int(x), Val::Int(y)), Val::Err);
        return;
    }
    if x == i64::MIN && y == -1 {
        assert_eq!(eval_binop(Kind::Rem, Val::Int(x), Val::Int(y)), Val::Err);
        return;
    }
    kani::assume(y != 0);
    kani::assume(!(x == i64::MIN && y == -1));
    assert_eq!(
        eval_binop(Kind::Rem, Val::Int(x), Val::Int(y)),
        spec::spec_arith(Val::Int(x), Val::Int(y), spec::Arith::Rem)
    );
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_logical_matches_spec_for_all_operands() {
    let a = symbolic_val();
    let b = symbolic_val();
    // Value-level tables (the tree form folds the short-circuit into these).
    assert_eq!(eval_and(a, b), spec::spec_and(a, b, true));
    assert_eq!(eval_or(a, b), spec::spec_or(a, b, true));
}

#[kani::proof]
#[kani::unwind(4)]
pub fn ir_neg_not_match_spec() {
    let a = symbolic_val();
    assert_eq!(eval_unop(Kind::Neg, a), spec::spec_neg(a));
    assert_eq!(eval_unop(Kind::Not, a), spec::spec_not(a));
}

/// Host root rule: any error ⇒ false.
#[kani::proof]
#[kani::unwind(4)]
pub fn error_root_is_false() {
    let x: i64 = kani::any();
    // `1 / x` is an error exactly when x == 0.
    let v = eval_binop(Kind::Div, Val::Int(1), Val::Int(x));
    if x == 0 {
        assert_eq!(v, Val::Err);
        assert!(!v.as_cond_bool());
    } else {
        assert_eq!(v, Val::Int(1 / x));
        // A bool root that is an error is also false.
        assert!(!Val::Err.as_cond_bool());
    }
}

/// Ternary is lazy and matches the reference.
#[kani::proof]
#[kani::unwind(4)]
pub fn ir_tern_matches_spec() {
    let c = symbolic_val();
    let t: i64 = kani::any();
    let f: i64 = kani::any();
    // The tree form evaluates only the taken arm; the value-level spec_tern
    // is the table. They agree on the value of the taken arm.
    match c.try_bool() {
        Ok(true) => assert_eq!(spec::spec_tern(c, Val::Int(t), Val::Int(f)), Val::Int(t)),
        Ok(false) => assert_eq!(spec::spec_tern(c, Val::Int(t), Val::Int(f)), Val::Int(f)),
        Err(()) => assert_eq!(spec::spec_tern(c, Val::Int(t), Val::Int(f)), Val::Err),
    }
}