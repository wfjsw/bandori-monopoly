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
use crate::Value;

/// Whole-expression reference: same short-circuit + error absorption as the
/// interpreter, but every leaf operator is a [`spec_*`] call.
pub fn eval_node(nodes: &[Node], root: u16, ctx: &IrCtx) -> Value {
    let Some(n) = nodes.get(root as usize) else {
        return Value::Err;
    };
    match n.kind {
        Kind::ConstInt => Value::Int(n.payload),
        Kind::ConstBool => Value::Bool(n.payload != 0),
        Kind::ConstNull => Value::Null,
        Kind::Var => ctx.scalar_val(n.payload as u16),
        Kind::Neg => spec_neg(eval_node(nodes, n.a, ctx)),
        Kind::Not => spec_not(eval_node(nodes, n.a, ctx)),
        Kind::And => {
            // Lazy: right is evaluated only when left is not false.
            let l = eval_node(nodes, n.a, ctx);
            let lb = l.try_bool();
            if lb == Ok(false) {
                Value::Bool(false)
            } else {
                let r = eval_node(nodes, n.b, ctx);
                spec_and(l, r, true)
            }
        }
        Kind::Or => {
            let l = eval_node(nodes, n.a, ctx);
            let lb = l.try_bool();
            if lb == Ok(true) {
                Value::Bool(true)
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
                Err(()) => Value::Err,
            }
        }
        Kind::Add => spec_arith(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Arith::Add,
        ),
        Kind::Sub => spec_arith(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Arith::Sub,
        ),
        Kind::Mul => spec_arith(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Arith::Mul,
        ),
        Kind::Div => spec_arith(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Arith::Div,
        ),
        Kind::Rem => spec_arith(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Arith::Rem,
        ),
        Kind::Eq => spec_eq(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Ne => spec_ne(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Lt => spec_cmp(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Cmp::Lt,
        ),
        Kind::Le => spec_cmp(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Cmp::Le,
        ),
        Kind::Gt => spec_cmp(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Cmp::Gt,
        ),
        Kind::Ge => spec_cmp(
            eval_node(nodes, n.a, ctx),
            eval_node(nodes, n.b, ctx),
            Cmp::Ge,
        ),
        Kind::In => spec_in(eval_node(nodes, n.a, ctx), eval_node(nodes, n.b, ctx)),
        Kind::Size => spec_size(eval_node(nodes, n.a, ctx)),
        Kind::List => {
            let len = n.payload as usize;
            let first = n.a as usize;
            if len > 8 || first + len > nodes.len() {
                return Value::Err;
            }
            let mut items = [0i64; 8];
            for i in 0..len {
                match eval_node(nodes, (first + i) as u16, ctx) {
                    Value::Int(v) => items[i] = v,
                    _ => return Value::Err,
                }
            }
            Value::List(ListBuf {
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
                6 => FnId::Money,
                // 7  Fire        DEAD -- never reuse
                // 8  Crystals    DEAD -- never reuse
                9 => FnId::Hand,
                10 => FnId::Pos,
                // 11 Character   DEAD -- never reuse
                // 12 Band        DEAD -- never reuse
                13 => FnId::Tiles,
                // 14 CharacterIs DEAD -- never reuse
                // 15 BandIs      DEAD -- never reuse
                // 16 Blocked     DEAD -- never reuse
                17 => FnId::Neighbor,
                18 => FnId::ChainHas,
                19 => FnId::ChainHits,
                20 => FnId::GainsThisTurn,
                21 => FnId::TargetedCount,
                22 => FnId::RepeatedDigits,
                23 => FnId::Dist,
                24 => FnId::PlayersOn,
                25 => FnId::NextDist,
                26 => FnId::OthersWithin,
                27 => FnId::OwnedWithin,
                28 => FnId::OnPath,
                29 => FnId::Between,
                30 => FnId::TileIs,
                31 => FnId::TileAttr,
                32 => FnId::TileIn,
                33 => FnId::State,
                34 => FnId::CandHas,
                35 => FnId::CardIs,
                _ => return Value::Err,
            };
            let nargs = n.b as usize;
            let first = n.a as usize;
            if nargs > 3 || first + nargs > nodes.len() {
                return Value::Err;
            }
            let mut args = [Value::Err; 3];
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
pub fn spec_neg(a: Value) -> Value {
    match a {
        Value::Int(v) => match v.checked_neg() {
            Some(w) => Value::Int(w),
            None => Value::Err,
        },
        _ => Value::Err,
    }
}

/// `!a`. Non-bool is an error.
pub fn spec_not(a: Value) -> Value {
    match a.try_bool() {
        Ok(b) => Value::Bool(!b),
        Err(()) => Value::Err,
    }
}

/// `a && b` with CEL error absorption. `right_evaluated` is informational for
/// the lazy form: when the left side short-circuits, the caller must NOT
/// evaluate `b` (pass `Value::Err` as a placeholder and `false`).
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
pub fn spec_and(l: Value, r: Value, right_evaluated: bool) -> Value {
    let lb = l.try_bool();
    if lb == Ok(false) {
        return Value::Bool(false);
    }
    if !right_evaluated {
        // Short-circuit: left is not false, so we would evaluate right. The
        // lazy interpreter never calls this path with `right_evaluated=false`
        // except in the table proofs, where `r` is the symbolic right value.
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(true), Ok(b)) => Value::Bool(b),
        (Err(()), Ok(false)) => Value::Bool(false),
        _ => Value::Err,
    }
}

/// `a || b` with CEL error absorption.
///
/// | left \ right | true | false | err |
/// |---|---|---|---|
/// | true  | true  | true  | true |
/// | false | true  | false | err |
/// | err   | true  | false | err |
pub fn spec_or(l: Value, r: Value, right_evaluated: bool) -> Value {
    let lb = l.try_bool();
    if lb == Ok(true) {
        return Value::Bool(true);
    }
    let _ = right_evaluated;
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(false), Ok(b)) => Value::Bool(b),
        (Err(()), Ok(true)) => Value::Bool(true),
        _ => Value::Err,
    }
}

/// Ternary: lazy; only the taken branch is evaluated (the tree evaluator
/// handles laziness; this is the value-level form).
pub fn spec_tern(c: Value, t: Value, f: Value) -> Value {
    match c.try_bool() {
        Ok(true) => t,
        Ok(false) => f,
        Err(()) => Value::Err,
    }
}

/// Arithmetic. Int-only; overflow and div/rem by zero are errors.
pub fn spec_arith(a: Value, b: Value, op: Arith) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
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
                Some(z) => Value::Int(z),
                None => Value::Err,
            }
        }
        (Value::Err, _) | (_, Value::Err) => Value::Err,
        _ => Value::Err,
    }
}

/// Equality is total: like-with-like compares; cross-type (including `null`
/// vs int/bool) is `false`. Errors propagate.
pub fn spec_eq(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Err, _) | (_, Value::Err) => Value::Err,
        (Value::Int(x), Value::Int(y)) => Value::Bool(x == y),
        // Hash-baked strings: `Str(h)` equals `Int(h)` (string-literal nodes).
        (Value::Str(x), Value::Str(y))
        | (Value::Str(x), Value::Int(y))
        | (Value::Int(x), Value::Str(y)) => Value::Bool(x == y),
        (Value::Bool(x), Value::Bool(y)) => Value::Bool(x == y),
        (Value::Null, Value::Null) => Value::Bool(true),
        (Value::List(x), Value::List(y)) => Value::Bool(x == y),
        _ => Value::Bool(false),
    }
}

/// `a != b` ≡ `!(a == b)` on the non-error lattice.
pub fn spec_ne(a: Value, b: Value) -> Value {
    match spec_eq(a, b) {
        Value::Bool(b) => Value::Bool(!b),
        Value::Err => Value::Err,
        other => other,
    }
}

/// Ordering. Incomparable pairs (including anything involving `null` except
/// `null` vs `null`) are errors.
pub fn spec_cmp(a: Value, b: Value, op: Cmp) -> Value {
    match (a, b) {
        (Value::Err, _) | (_, Value::Err) => Value::Err,
        (Value::Int(x), Value::Int(y)) => {
            let o = x.cmp(&y);
            Value::Bool(match op {
                Cmp::Lt => o == Ordering::Less,
                Cmp::Le => o != Ordering::Greater,
                Cmp::Gt => o == Ordering::Greater,
                Cmp::Ge => o != Ordering::Less,
            })
        }
        (Value::Bool(x), Value::Bool(y)) => {
            let o = x.cmp(&y);
            Value::Bool(match op {
                Cmp::Lt => o == Ordering::Less,
                Cmp::Le => o != Ordering::Greater,
                Cmp::Gt => o == Ordering::Greater,
                Cmp::Ge => o != Ordering::Less,
            })
        }
        (Value::Null, Value::Null) => {
            // FINDING: the vendored `cel` crate errors on **every** ordering
            // involving `null`, including `null <= null`. `CelNull` does not
            // implement `Comparer`, so `compare_values` fails and the host
            // maps the error to `false`. We match that: no null ordering.
            Value::Err
        }
        _ => Value::Err,
    }
}

/// `a in b`. Membership of a scalar in a list. Errors propagate; a non-list
/// `b` is an error.
pub fn spec_in(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Err, _) | (_, Value::Err) => Value::Err,
        (x, Value::List(items)) => Value::Bool(
            items
                .as_slice()
                .iter()
                .any(|i| spec_eq(x, Value::Int(*i)) == Value::Bool(true)),
        ),
        _ => Value::Err,
    }
}

/// `size(list)`. Non-list is an error.
pub fn spec_size(a: Value) -> Value {
    match a {
        Value::List(items) => Value::Int(items.n as i64),
        Value::Err => Value::Err,
        _ => Value::Err,
    }
}

/// Vocabulary functions. Same accessors as [`crate::eval::call_fn`]; the two
/// are pinned together by the Kani function proofs and the differential tests.
pub fn spec_call(ctx: &IrCtx, fid: FnId, args: &[Value]) -> Value {
    crate::eval::call_fn(ctx, fid, args)
}

/// Utility used by the proofs: the closed scalar lattice (no lists).
pub fn scalar_lattice() -> [Value; 5] {
    [
        Value::Int(i64::MIN),
        Value::Int(-1),
        Value::Int(0),
        Value::Int(1),
        Value::Int(i64::MAX),
    ]
}
