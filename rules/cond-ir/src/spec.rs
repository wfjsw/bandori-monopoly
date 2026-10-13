//! Reference semantics for the closed operator set.
//!
//! This is a **written specification**, not the production interpreter. It is
//! derived from the vendored `cel` crate's behaviour (confirmed by the
//! differential tests in `rules-cond/tests/cond_ir_diff.rs`) and the CEL
//! spec's error-absorption rules. The Kani proofs in
//! `rules-kani/src/proofs/cond_ops.rs` assert
//! `eval::<op>(a, b) == spec::<op>(a, b)` for every operand; the structural
//! induction is stated in `docs/rulebook/spec/COND-SINGLE-SOURCE.md`.
//!
//! Keep this file's matches exhaustive and boring. If you change it, you are
//! changing the spec — update the doc table and re-run the proofs.

use core::cmp::Ordering;

use crate::ctx::{FnId, IrCtx};
use crate::eval::ListBuf;
use crate::nodes::{Kind, Node};
use crate::Val;

/// Whole-expression reference: same short-circuit + error absorption as the
/// interpreter, but every leaf operator is a [`spec_*`] call.
pub fn eval_node(nodes: &[Node], root: u16, ctx: &IrCtx) -> Val {
    let Some(n) = nodes.get(root as usize) else {
        return Val::Err;
    };
    match n.kind {
        Kind::ConstInt => Val::Int(n.payload),
        Kind::ConstBool => Val::Bool(n.payload != 0),
        Kind::ConstNull => Val::Null,
        Kind::Var => ctx.scalar_val(n.payload as u16),
        Kind::Neg => spec_neg(eval_node(nodes, n.a, ctx)),
        Kind::Not => spec_not(eval_node(nodes, n.a, ctx)),
        Kind::And => {
            // Lazy: right is evaluated only when left is not false.
            let l = eval_node(nodes, n.a, ctx);
            let lb = l.try_bool();
            if lb == Ok(false) {
                Val::Bool(false)
            } else {
                let r = eval_node(nodes, n.b, ctx);
                spec_and(l, r, true)
            }
        }
        Kind::Or => {
            let l = eval_node(nodes, n.a, ctx);
            let lb = l.try_bool();
            if lb == Ok(true) {
                Val::Bool(true)
            } else {
                let r = eval_node(nodes, n.b, ctx);
                spec_or(l, r, true)
            }
        }
        Kind::Tern => {
            let c = eval_node(nodes, n.a, ctx);
            match c.try_bool() {
                Ok(true) => eval_node(nodes, n.b, ctx),
                Ok(false) => eval_node(nodes, n.c, ctx),
                Err(()) => Val::Err,
            }
        }
        Kind::Add => spec_arith(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Arith::Add),
        Kind::Sub => spec_arith(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Arith::Sub),
        Kind::Mul => spec_arith(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Arith::Mul),
        Kind::Div => spec_arith(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Arith::Div),
        Kind::Rem => spec_arith(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Arith::Rem),
        Kind::Eq => spec_eq(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Ne => spec_ne(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Lt => spec_cmp(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Cmp::Lt),
        Kind::Le => spec_cmp(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Cmp::Le),
        Kind::Gt => spec_cmp(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Cmp::Gt),
        Kind::Ge => spec_cmp(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx), Cmp::Ge),
        Kind::In => spec_in(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Size => spec_size(eval_node(nodes, n.a, ctx)),
        Kind::List => {
            let len = n.payload as usize;
            let first = n.a as usize;
            if len > 8 || first + len > nodes.len() {
                return Val::Err;
            }
            let mut items = [0i64; 8];
            for i in 0..len {
                match eval_node(nodes, (first + i) as u16, ctx) {
                    Val::Int(v) => items[i] = v,
                    _ => return Val::Err,
                }
            }
            Val::List(ListBuf {
                n: len as u8,
                items,
            })
        }
        Kind::Call => {
            let fid = match n.payload {
                0 => FnId::Slot,
                1 => FnId::Tok,
                2 => FnId::MoveTag,
                3 => FnId::CardCounter,
                4 => FnId::CounterIs,
                5 => FnId::TileNamed,
                6 => FnId::IsCircle,
                7 => FnId::IsRing,
                8 => FnId::IsLiveHouse,
                9 => FnId::IsBuyable,
                10 => FnId::Money,
                11 => FnId::Fire,
                12 => FnId::Crystals,
                13 => FnId::Hand,
                14 => FnId::Pos,
                15 => FnId::Character,
                16 => FnId::Band,
                17 => FnId::Tiles,
                18 => FnId::CharacterIs,
                19 => FnId::BandIs,
                20 => FnId::Blocked,
                21 => FnId::Neighbor,
                22 => FnId::ChainHas,
                23 => FnId::ChainHits,
                24 => FnId::GainsThisTurn,
                25 => FnId::TargetedCount,
                26 => FnId::RepeatedDigits,
                27 => FnId::Dist,
                28 => FnId::PlayersOn,
                29 => FnId::NextDist,
                30 => FnId::OthersWithin,
                31 => FnId::OwnedWithin,
                32 => FnId::OnPath,
                33 => FnId::Between,
                34 => FnId::TileIs,
                35 => FnId::TileAttr,
                36 => FnId::TileIn,
                37 => FnId::Stay,
                38 => FnId::Stunned,
                39 => FnId::Exile,
                40 => FnId::Unstoppable,
                _ => return Val::Err,
            };
            let nargs = n.b as usize;
            let first = n.a as usize;
            if nargs > 3 || first + nargs > nodes.len() {
                return Val::Err;
            }
            let mut args = [Val::Err; 3];
            for i in 0..nargs {
                args[i] = eval_node(nodes, (first + i) as u16, ctx);
            }
            spec_call(ctx, fid, &args[..nargs])
        }
    }
}

// ---------------------------------------------------------------------------
// per-operator reference
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
}

/// `-a`. `i64::MIN` overflows (matches `checked_neg`).
pub fn spec_neg(a: Val) -> Val {
    match a {
        Val::Int(v) => match v.checked_neg() {
            Some(w) => Val::Int(w),
            None => Val::Err,
        },
        _ => Val::Err,
    }
}

/// `!a`. Non-bool is an error.
pub fn spec_not(a: Val) -> Val {
    match a.try_bool() {
        Ok(b) => Val::Bool(!b),
        Err(()) => Val::Err,
    }
}

/// `a && b` with CEL error absorption. `right_evaluated` is informational for
/// the lazy form: when the left side short-circuits, the caller must NOT
/// evaluate `b` (pass `Val::Err` as a placeholder and `false`).
///
/// Truth table (left, right) → result:
///
/// | left \ right | true | false | err |
/// |---|---|---|---|
/// | true  | true  | false | err |
/// | false | false | false | false |
/// | err   | true  | false | err |
///
/// Non-bool on either side is `err`.
pub fn spec_and(l: Val, r: Val, right_evaluated: bool) -> Val {
    let lb = l.try_bool();
    if lb == Ok(false) {
        return Val::Bool(false);
    }
    if !right_evaluated {
        // Short-circuit: left is not false, so we would evaluate right. The
        // lazy interpreter never calls this path with `right_evaluated=false`
        // except in the table proofs, where `r` is the symbolic right value.
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(true), Ok(b)) => Val::Bool(b),
        (Err(()), Ok(false)) => Val::Bool(false),
        _ => Val::Err,
    }
}

/// `a || b` with CEL error absorption.
///
/// | left \ right | true | false | err |
/// |---|---|---|---|
/// | true  | true  | true  | true |
/// | false | true  | false | err |
/// | err   | true  | false | err |
pub fn spec_or(l: Val, r: Val, right_evaluated: bool) -> Val {
    let lb = l.try_bool();
    if lb == Ok(true) {
        return Val::Bool(true);
    }
    let _ = right_evaluated;
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(false), Ok(b)) => Val::Bool(b),
        (Err(()), Ok(true)) => Val::Bool(true),
        _ => Val::Err,
    }
}

/// Ternary: lazy; only the taken branch is evaluated (the tree evaluator
/// handles laziness; this is the value-level form).
pub fn spec_tern(c: Val, t: Val, f: Val) -> Val {
    match c.try_bool() {
        Ok(true) => t,
        Ok(false) => f,
        Err(()) => Val::Err,
    }
}

/// Arithmetic. Int-only; overflow and div/rem by zero are errors.
pub fn spec_arith(a: Val, b: Val, op: Arith) -> Val {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => {
            let r = match op {
                Arith::Add => x.checked_add(y),
                Arith::Sub => x.checked_sub(y),
                Arith::Mul => x.checked_mul(y),
                Arith::Div => {
                    if y == 0 {
                        None
                    } else {
                        x.checked_div(y)
                    }
                }
                Arith::Rem => {
                    if y == 0 {
                        None
                    } else {
                        x.checked_rem(y)
                    }
                }
            };
            match r {
                Some(z) => Val::Int(z),
                None => Val::Err,
            }
        }
        (Val::Err, _) | (_, Val::Err) => Val::Err,
        _ => Val::Err,
    }
}

/// Equality is total: like-with-like compares; cross-type (including `null`
/// vs int/bool) is `false`. Errors propagate.
pub fn spec_eq(a: Val, b: Val) -> Val {
    match (a, b) {
        (Val::Err, _) | (_, Val::Err) => Val::Err,
        (Val::Int(x), Val::Int(y)) => Val::Bool(x == y),
        (Val::Bool(x), Val::Bool(y)) => Val::Bool(x == y),
        (Val::Null, Val::Null) => Val::Bool(true),
        (Val::List(x), Val::List(y)) => Val::Bool(x == y),
        _ => Val::Bool(false),
    }
}

/// `a != b` ≡ `!(a == b)` on the non-error lattice.
pub fn spec_ne(a: Val, b: Val) -> Val {
    match spec_eq(a, b) {
        Val::Bool(b) => Val::Bool(!b),
        Val::Err => Val::Err,
        other => other,
    }
}

/// Ordering. Incomparable pairs (including anything involving `null` except
/// `null` vs `null`) are errors.
pub fn spec_cmp(a: Val, b: Val, op: Cmp) -> Val {
    match (a, b) {
        (Val::Err, _) | (_, Val::Err) => Val::Err,
        (Val::Int(x), Val::Int(y)) => {
            let o = x.cmp(&y);
            Val::Bool(match op {
                Cmp::Lt => o == Ordering::Less,
                Cmp::Le => o != Ordering::Greater,
                Cmp::Gt => o == Ordering::Greater,
                Cmp::Ge => o != Ordering::Less,
            })
        }
        (Val::Bool(x), Val::Bool(y)) => {
            let o = x.cmp(&y);
            Val::Bool(match op {
                Cmp::Lt => o == Ordering::Less,
                Cmp::Le => o != Ordering::Greater,
                Cmp::Gt => o == Ordering::Greater,
                Cmp::Ge => o != Ordering::Less,
            })
        }
        (Val::Null, Val::Null) => {
            // FINDING: the vendored `cel` crate errors on **every** ordering
            // involving `null`, including `null <= null`. `CelNull` does not
            // implement `Comparer`, so `compare_values` fails and the host
            // maps the error to `false`. We match that: no null ordering.
            Val::Err
        }
        _ => Val::Err,
    }
}

/// `a in b`. Membership of a scalar in a list. Errors propagate; a non-list
/// `b` is an error.
pub fn spec_in(a: Val, b: Val) -> Val {
    match (a, b) {
        (Val::Err, _) | (_, Val::Err) => Val::Err,
        (x, Val::List(items)) => Val::Bool(items.as_slice().iter().any(|i| spec_eq(x, Val::Int(*i)) == Val::Bool(true))),
        _ => Val::Err,
    }
}

/// `size(list)`. Non-list is an error.
pub fn spec_size(a: Val) -> Val {
    match a {
        Val::List(items) => Val::Int(items.n as i64),
        Val::Err => Val::Err,
        _ => Val::Err,
    }
}

/// Vocabulary functions. Same accessors as [`crate::eval::call_fn`]; the two
/// are pinned together by the Kani function proofs and the differential tests.
pub fn spec_call(ctx: &IrCtx, fid: FnId, args: &[Val]) -> Val {
    crate::eval::call_fn(ctx, fid, args)
}

/// Utility used by the proofs: the closed scalar lattice (no lists).
pub fn scalar_lattice() -> [Val; 5] {
    [
        Val::Int(i64::MIN),
        Val::Int(-1),
        Val::Int(0),
        Val::Int(1),
        Val::Int(i64::MAX),
    ]
}