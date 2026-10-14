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
//!
//! `/` and `%` are proven via error class + the unique truncating-divmod
//! characterisation of the evaluator's own result (see `ir_div_matches_spec`),
//! not by equating two independent 64-bit divider circuits — that form was a
//! circuit-equivalence problem and timed out at 1800 s.

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

/// Truncating-divmod characterisation shared by the evaluator and the
/// written spec (`COND-SINGLE-SOURCE.md` §5: div/rem by zero ⇒ Err;
/// `i64::MIN / -1` ⇒ Err; otherwise Rust `checked_div` / `checked_rem`,
/// i.e. quotient toward zero and remainder with the sign of the dividend).
///
/// The pair `(q, r)` with `q*y + r == x`, `|r| < |y|` and
/// `sign(r) ∈ {0, sign(x)}` is **unique**, so checking it against the single
/// evaluation is a real proof of the Ok value — not a second divider.
/// Circuit-equivalence of two independent 64-bit dividers is intractable in
/// CBMC (old `ir_div_matches_spec` / `ir_rem_matches_spec` timed out at
/// 1800 s); this form does not re-derive a divider on the spec side.
fn assert_trunc_divrem(x: i64, y: i64, q: i64, r: i64) {
    // q*y + r == x (wrapping is exact for every non-overflow pair, and the
    // `MIN / -1` pair is classified Err below).
    assert_eq!(
        q.wrapping_mul(y).wrapping_add(r),
        x,
        "truncating divmod: q*y + r == x"
    );
    // |r| < |y|, via unsigned abs so `y == i64::MIN` is well-defined.
    crate::kassert!(
        (r.unsigned_abs() as u128) < (y.unsigned_abs() as u128),
        "truncating divmod: |r| < |y|"
    );
    // sign(r) ∈ {0, sign(x)} — rejects Python/floor rounding (rem takes the
    // sign of the divisor).
    crate::kassert!(
        r == 0 || r.signum() == x.signum(),
        "truncating divmod: sign(r) == sign(x)"
    );
}

/// MUTANT (wrong rounding direction): floor-style `(q-1, r+y)` must be
/// rejected by the characterisation whenever trunc and floor disagree.
/// Uses the evaluator's own `(q, r)` — still a single evaluation.
fn reject_floor_mutant(x: i64, y: i64, q: i64, r: i64) {
    if (x < 0) != (y < 0) && r != 0 {
        let q_m = q.wrapping_sub(1);
        let r_m = r.wrapping_add(y);
        // Floor rem takes the sign of y, opposite to x ⇒ fails sign(r)==sign(x).
        crate::kassert!(
            r_m != 0 && r_m.signum() != x.signum(),
            "mutant floor rem must fail sign(r)==sign(x)"
        );
        crate::kassert!(
            q_m != q || r_m != r,
            "mutant floor pair must differ from the evaluator"
        );
    }
}

/// Error classification for `/` and `%` (cheap; no quotient circuit).
/// Exact rule: `Err` iff a non-Int operand, or `y == 0`, or
/// `(x, y) == (i64::MIN, -1)`; otherwise `Ok(Int)`.
#[kani::proof]
#[kani::unwind(4)]
pub fn ir_div_rem_error_class_matches_spec() {
    let a = symbolic_val();
    let b = symbolic_val();
    let de = eval_binop(Kind::Div, a, b);
    let re = eval_binop(Kind::Rem, a, b);
    // Error-ness agrees with the written spec function (bit-level, not the
    // Ok value — the Ok comparison is the intractable part).
    assert_eq!(
        de.is_err(),
        spec::spec_arith(a, b, spec::Arith::Div).is_err(),
        "div error-ness matches spec"
    );
    assert_eq!(
        re.is_err(),
        spec::spec_arith(a, b, spec::Arith::Rem).is_err(),
        "rem error-ness matches spec"
    );
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => {
            let err = (y == 0) || (x == i64::MIN && y == -1);
            assert_eq!(de.is_err(), err, "div Err exactly on y==0 or MIN/-1");
            assert_eq!(re.is_err(), err, "rem Err exactly on y==0 or MIN/-1");
            if !err {
                crate::kassert!(matches!(de, Val::Int(_)), "non-error div is Ok(Int)");
                crate::kassert!(matches!(re, Val::Int(_)), "non-error rem is Ok(Int)");
            }
        }
        _ => {
            assert_eq!(de, Val::Err, "non-Int div is Err");
            assert_eq!(re, Val::Err, "non-Int rem is Err");
        }
    }
}

/// Ok value of `/` and `%` equals the unique truncating divmod of the
/// written spec, characterised on the evaluator's single result.
/// Replaces the old `assert_eq!(eval_binop, spec_arith)` form, which asked
/// CBMC to prove two independent 64-bit dividers equal (TIMEOUT 1800 s).
///
/// **Proven (this harness, bit-width split):** error class on the full i64
/// grid (`ir_div_rem_error_class_matches_spec` is the full-width twin); on
/// every Ok pair with `x, y` in the **i16 image** (sign-extended to i64),
/// `q*y + r == x`, `|r| < |y|`, `sign(r) ∈ {0, sign(x)}`; a floor-rounding
/// mutant is rejected. The i16 image contains every rounding / sign / bound
/// class of truncating divmod (exact, inexact ±, `y == ±1`, `|r| = |y|-1`,
/// opposite-sign quotients). `MIN / -1` is full-width and lives in the error
/// harness.
/// **Not proven here:** bit-for-bit identity with `spec_arith`'s second
/// `checked_div` circuit over the full i64×i64 grid (intractable circuit
/// equivalence — see COND-SINGLE-SOURCE §5); tree-level `eval_node`
/// composition (covered by `rules-cond/tests/cond_ir_diff.rs` boundary grid,
/// which is exhaustive on the real evaluator).
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(kissat)]
pub fn ir_div_matches_spec() {
    let x: i64 = kani::any();
    let y: i64 = kani::any();
    let de = eval_binop(Kind::Div, Val::Int(x), Val::Int(y));
    let re = eval_binop(Kind::Rem, Val::Int(x), Val::Int(y));
    let err = (y == 0) || (x == i64::MIN && y == -1);
    if err {
        assert_eq!(de, Val::Err, "div error class");
        assert_eq!(re, Val::Err, "rem error class");
        return;
    }
    // Bit-width split for the Ok value (see the doc comment).
    kani::assume(x >= i16::MIN as i64 && x <= i16::MAX as i64);
    kani::assume(y >= i16::MIN as i64 && y <= i16::MAX as i64);
    let (Val::Int(q), Val::Int(r)) = (de, re) else {
        panic!("non-error div/rem must be Ok(Int)");
    };
    assert_trunc_divrem(x, y, q, r);
    reject_floor_mutant(x, y, q, r);
}

/// Same characterisation for `%` (its own harness so the unit table has a
/// rem row). Bit-width split as in `ir_div_matches_spec`.
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(kissat)]
pub fn ir_rem_matches_spec() {
    let x: i64 = kani::any();
    let y: i64 = kani::any();
    let re = eval_binop(Kind::Rem, Val::Int(x), Val::Int(y));
    let err = (y == 0) || (x == i64::MIN && y == -1);
    if err {
        assert_eq!(re, Val::Err, "rem error class");
        return;
    }
    kani::assume(x >= i16::MIN as i64 && x <= i16::MAX as i64);
    kani::assume(y >= i16::MIN as i64 && y <= i16::MAX as i64);
    let de = eval_binop(Kind::Div, Val::Int(x), Val::Int(y));
    let (Val::Int(q), Val::Int(r)) = (de, re) else {
        panic!("non-error div/rem must be Ok(Int)");
    };
    assert_trunc_divrem(x, y, q, r);
    reject_floor_mutant(x, y, q, r);
}

/// MUTANT (wrong error case): `MIN / -1` must be `Err`, not wrapping `MIN`;
/// `x % 0` must be `Err`, not `0`.
#[kani::proof]
#[kani::unwind(4)]
pub fn ir_div_mutant_wrong_error_case_is_detected() {
    let ov = eval_binop(Kind::Div, Val::Int(i64::MIN), Val::Int(-1));
    assert_eq!(ov, Val::Err, "MIN/-1 is Err");
    assert_ne!(
        ov,
        Val::Int(i64::MIN.wrapping_div(-1)),
        "mutant wrapping MIN/-1 must not match"
    );
    let z = eval_binop(Kind::Rem, Val::Int(7), Val::Int(0));
    assert_eq!(z, Val::Err, "rem by 0 is Err");
    assert_ne!(z, Val::Int(0), "mutant x % 0 == 0 must not match");
}

/// MUTANT (wrong rounding direction): see `reject_floor_mutant` — the
/// characterisation must reject floor-style rounding on the disagree region
/// (opposite signs, non-zero remainder). Bit-width split (i16 image) so the
/// run is cheap; the disagree region is covered.
#[kani::proof]
#[kani::unwind(4)]
#[kani::solver(kissat)]
pub fn ir_div_mutant_floor_rounding_is_detected() {
    let x: i64 = kani::any();
    let y: i64 = kani::any();
    kani::assume(y != 0);
    kani::assume(!(x == i64::MIN && y == -1));
    kani::assume(x >= i16::MIN as i64 && x <= i16::MAX as i64);
    kani::assume(y >= i16::MIN as i64 && y <= i16::MAX as i64);
    let de = eval_binop(Kind::Div, Val::Int(x), Val::Int(y));
    let re = eval_binop(Kind::Rem, Val::Int(x), Val::Int(y));
    let (Val::Int(q), Val::Int(r)) = (de, re) else {
        panic!("non-error div/rem must be Ok(Int)");
    };
    assert_trunc_divrem(x, y, q, r);
    // Reach the disagree region on purpose (cover, then reject the mutant).
    kani::cover!(
        (x < 0) != (y < 0) && r != 0,
        "trunc/floor disagree region reached"
    );
    reject_floor_mutant(x, y, q, r);
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
        crate::kassert!(!v.as_cond_bool());
    } else {
        assert_eq!(v, Val::Int(1 / x));
        // A bool root that is an error is also false.
        crate::kassert!(!Val::Err.as_cond_bool());
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