//! The IR interpreter — what Kani runs. Integer-only, no heap, fixed recursion
//! depth = expression depth (every real guard is < 16).
//!
//! This file is the *implementation*. [`crate::spec`] is the independently
//! written reference; the Kani proofs assert the two agree on every operator
//! for every operand, and the differential tests assert both agree with the
//! vendored `cel` evaluator.

use core::cmp::Ordering;

use crate::ctx::{self, FnId, IrCtx, SeatField, SetId};
use crate::nodes::{Kind, Node};
use crate::Value;

/// Fixed-capacity int list (max 8). Only used by list literals / `in` / `size`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ListBuf {
    pub n: u8,
    pub items: [i64; 8],
}

impl ListBuf {
    pub fn as_slice(&self) -> &[i64] {
        &self.items[..self.n as usize]
    }
}

/// Binary / unary operator step over already-evaluated operands. Split out
/// of [`eval_node`] so the Kani operator proofs do not pull the `Call` /
/// `repeated_digits` paths into the unwinding graph.
pub fn eval_binop(kind: Kind, a: Value, b: Value) -> Value {
    match kind {
        Kind::Add => spec_arith_local(a, b, true, false),
        Kind::Sub => spec_arith_local(a, b, false, false),
        Kind::Mul => spec_arith_local(a, b, false, true),
        Kind::Div => divrem(a, b, true),
        Kind::Rem => divrem(a, b, false),
        Kind::Eq => match (a, b) {
            (Value::Err, _) | (_, Value::Err) => Value::Err,
            (x, y) => Value::Bool(values_eq(x, y)),
        },
        Kind::Ne => match (a, b) {
            (Value::Err, _) | (_, Value::Err) => Value::Err,
            (x, y) => Value::Bool(!values_eq(x, y)),
        },
        Kind::Lt => cmp(a, b, |o| o == Ordering::Less),
        Kind::Le => cmp(a, b, |o| o != Ordering::Greater),
        Kind::Gt => cmp(a, b, |o| o == Ordering::Greater),
        Kind::Ge => cmp(a, b, |o| o != Ordering::Less),
        Kind::In => match (a, b) {
            (Value::Err, _) | (_, Value::Err) => Value::Err,
            (x, Value::List(items)) => Value::Bool(
                items
                    .as_slice()
                    .iter()
                    .any(|i| values_eq(x, Value::Int(*i))),
            ),
            _ => Value::Err,
        },
        _ => Value::Err,
    }
}

/// Unary operator step.
pub fn eval_unop(kind: Kind, a: Value) -> Value {
    match kind {
        Kind::Neg => match a {
            Value::Int(v) => match v.checked_neg() {
                Some(w) => Value::Int(w),
                None => Value::Err,
            },
            _ => Value::Err,
        },
        Kind::Not => match a.try_bool() {
            Ok(b) => Value::Bool(!b),
            Err(()) => Value::Err,
        },
        Kind::Size => match a {
            Value::List(items) => Value::Int(items.n as i64),
            _ => Value::Err,
        },
        _ => Value::Err,
    }
}

/// Logical connectives with CEL error absorption. `right_evaluated` = false
/// means the left side short-circuited and `r` is a placeholder.
pub fn eval_and(l: Value, r: Value) -> Value {
    let lb = l.try_bool();
    if lb == Ok(false) {
        return Value::Bool(false);
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(true), Ok(b)) => Value::Bool(b),
        (Err(()), Ok(false)) => Value::Bool(false),
        _ => Value::Err,
    }
}

pub fn eval_or(l: Value, r: Value) -> Value {
    let lb = l.try_bool();
    if lb == Ok(true) {
        return Value::Bool(true);
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(false), Ok(b)) => Value::Bool(b),
        (Err(()), Ok(true)) => Value::Bool(true),
        _ => Value::Err,
    }
}

fn spec_arith_local(a: Value, b: Value, is_add: bool, is_mul: bool) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            let r = if is_add {
                x.checked_add(y)
            } else if is_mul {
                x.checked_mul(y)
            } else {
                x.checked_sub(y)
            };
            match r {
                Some(z) => Value::Int(z),
                None => Value::Err,
            }
        }
        _ => Value::Err,
    }
}

fn divrem(a: Value, b: Value, is_div: bool) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            if y == 0 {
                return Value::Err;
            }
            let r = if is_div {
                x.checked_div(y)
            } else {
                x.checked_rem(y)
            };
            match r {
                Some(z) => Value::Int(z),
                None => Value::Err,
            }
        }
        _ => Value::Err,
    }
}

fn cmp(a: Value, b: Value, pred: impl Fn(Ordering) -> bool) -> Value {
    match values_cmp(a, b) {
        Some(o) => Value::Bool(pred(o)),
        None => Value::Err,
    }
}

/// Evaluate `nodes[root]`. Children are earlier indices; short-circuiting
/// skips the untaken arm entirely.
pub fn eval_node(nodes: &[Node], root: u16, ctx: &IrCtx) -> Value {
    let Some(n) = nodes.get(root as usize) else {
        return Value::Err;
    };
    match n.kind {
        Kind::ConstInt => Value::Int(n.payload),
        Kind::ConstBool => Value::Bool(n.payload != 0),
        Kind::ConstNull => Value::Null,
        Kind::Var => ctx.scalar_val(n.payload as u16),
        Kind::Neg | Kind::Not | Kind::Size => eval_unop(n.kind, eval_node(nodes, n.a, ctx)),
        Kind::And => {
            let l = eval_node(nodes, n.a, ctx);
            if l.try_bool() == Ok(false) {
                Value::Bool(false)
            } else {
                eval_and(l, eval_node(nodes, n.b, ctx))
            }
        }
        Kind::Or => {
            let l = eval_node(nodes, n.a, ctx);
            if l.try_bool() == Ok(true) {
                Value::Bool(true)
            } else {
                eval_or(l, eval_node(nodes, n.b, ctx))
            }
        }
        Kind::Tern => match eval_node(nodes, n.a, ctx).try_bool() {
            Ok(true) => eval_node(nodes, n.b, ctx),
            Ok(false) => eval_node(nodes, n.c, ctx),
            Err(()) => Value::Err,
        },
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
        Kind::Call => eval_call(nodes, n, ctx),
        // Binary ops — operands first, then the pure step.
        Kind::Add
        | Kind::Sub
        | Kind::Mul
        | Kind::Div
        | Kind::Rem
        | Kind::Eq
        | Kind::Ne
        | Kind::Lt
        | Kind::Le
        | Kind::Gt
        | Kind::Ge
        | Kind::In => {
            let a = eval_node(nodes, n.a, ctx);
            let b = eval_node(nodes, n.b, ctx);
            eval_binop(n.kind, a, b)
        }
    }
}

/// Equality is total over the lattice: cross-type is false, `null == null` is
/// true. Lists compare elementwise (only used by tests).
pub fn values_eq(a: Value, b: Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        // Str is `id_of(spelling)`; string-literal nodes bake to `ConstInt` of
        // the same hash, so `Str(h) == Int(h)` is how a bare ident and a
        // string-literal kind spelling agree.
        (Value::Str(x), Value::Str(y))
        | (Value::Str(x), Value::Int(y))
        | (Value::Int(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::List(x), Value::List(y)) => x == y,
        (Value::Err, _) | (_, Value::Err) => false,
        _ => false,
    }
}

/// Ordering: only like-with-like scalar. **Any** `null` operand is an
/// incomparable error — including `null` vs `null`. The vendored `cel` crate's
/// `CelNull` does not implement `Comparer`, so `compare_values` fails for
/// every null pairing. Confirmed by the differential tests.
pub fn values_cmp(a: Value, b: Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(&y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(&y)),
        (Value::Err, _) | (_, Value::Err) => None,
        _ => None,
    }
}

fn eval_call(nodes: &[Node], n: &Node, ctx: &IrCtx) -> Value {
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
        36 => FnId::Resolving,
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
    call_fn(ctx, fid, &args[..nargs])
}

/// Narrow function surface. Every name goes through here so the Kani host has
/// one place to model.
pub fn call_fn(ctx: &IrCtx, fid: FnId, args: &[Value]) -> Value {
    // Helper: require int arg.
    let int = |i: usize| -> Result<i64, ()> {
        match args.get(i) {
            Some(Value::Int(v)) => Ok(*v),
            Some(Value::Err) => Err(()),
            _ => Err(()),
        }
    };
    match fid {
        FnId::Slot | FnId::Tok | FnId::CardCounter | FnId::TileNamed | FnId::MoveTag => {
            let name = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let missing = match fid {
                FnId::TileNamed => -1,
                _ => 0,
            };
            Value::Int(ctx.named_get(name).unwrap_or(missing))
        }
        FnId::CounterIs => {
            let want = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            // `counter_is('name')` ≡ `counter_name == id_of(name)`.
            Value::Bool(ctx.scalar_val(vars_counter_name_idx()) == Value::Int(want))
        }
        FnId::CardIs => {
            let want = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            // `card_is('id')` ≡ `trigger_card == id_of(id)`.
            Value::Bool(ctx.scalar_val(vars_trigger_card_idx()) == Value::Int(want))
        }
        FnId::Resolving => {
            let want = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            // `resolving('id')` -- is that card's body on the resolving stack?
            // The host registers each resolving name in the named table.
            Value::Bool(ctx.named_get(want).is_some())
        }
        // ABI v56 generic tile queries. String args lower to `id_of` hashes.
        FnId::TileIs => {
            let tile = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let kind_id = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            if tile < 0 || tile as usize >= crate::ctx::MAX_TILES {
                return Value::Bool(false);
            }
            Value::Bool(ctx.tile_kind_ids[tile as usize] == kind_id)
        }
        FnId::TileAttr => {
            let tile = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let attr_id = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let idx = wire_attr_index(attr_id);
            if idx < 0 {
                return Value::Int(0);
            }
            let key = tile * 16 + idx;
            let missing = match idx {
                // group / tier / area / index
                0 | 1 | 4 | 6 => -1,
                _ => 0,
            };
            Value::Int(ctx.named_get(key).unwrap_or(missing))
        }
        FnId::TileIn => {
            let player = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let tile = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let class_id = match int(2) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            // Generic named row: key = packed player/tile, value = class id.
            // Missing -> false.
            let key = ((player + 2) << 32) ^ tile;
            Value::Bool(ctx.named_get(key) == Some(class_id))
        }
        FnId::Money => seat_i64(ctx, args, SeatField::Money),
        FnId::Hand => seat_i64(ctx, args, SeatField::Hand),
        FnId::Pos => seat_i64(ctx, args, SeatField::Pos),
        FnId::Tiles => seat_i64(ctx, args, SeatField::Tiles),
        FnId::GainsThisTurn => seat_i64(ctx, args, SeatField::Gains),
        FnId::TargetedCount => seat_i64(ctx, args, SeatField::Targeted),
        FnId::Between => seat_i64(ctx, args, SeatField::Between),
        // `state(p, 'key')`: seat + `id_of(key)` (baked at lower time).
        FnId::State => {
            let p = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let key_id = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Int(ctx.state_get(p, key_id))
        }
        FnId::CandHas => {
            // `cand_has(key, member)`. The sugar `blocked(b)` lowers to
            // `Call(CandHas, [id_of(key), b])` (bake key leads).
            let key_id = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let member = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Bool(ctx.cand_has(key_id, member))
        }
        FnId::Neighbor => {
            let p = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let d = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Int(ctx::neighbor(p, d, ctx.n_players))
        }
        FnId::ChainHas => {
            let k = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Bool(ctx.set_contains(SetId::ChainKinds, k))
        }
        FnId::ChainHits => {
            let seat = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Bool(ctx.set_contains(SetId::ChainHits, seat))
        }
        FnId::RepeatedDigits => {
            let x = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Bool(ctx::repeated_digits(x))
        }
        FnId::Dist => {
            let a = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let b = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let n = ctx.tile_count;
            if n <= 0 {
                return Value::Int(0);
            }
            let d = ((b - a) % n + n) % n;
            Value::Int(d.min(n - d))
        }
        FnId::PlayersOn
        | FnId::NextDist
        | FnId::OthersWithin
        | FnId::OwnedWithin
        | FnId::OnPath => {
            let a = match int(0) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            let b = match int(1) {
                Ok(v) => v,
                Err(()) => return Value::Err,
            };
            Value::Int(ctx.bin_get(a, b))
        }
    }
}

fn vars_counter_name_idx() -> u16 {
    // Const index — never a VARS string scan on the eval path.
    crate::vars::VarId::CounterName as u16
}

fn vars_trigger_card_idx() -> u16 {
    // Const index — never a VARS string scan on the eval path.
    crate::vars::VarId::TriggerCard as u16
}

/// `tile_attr` field name -> slot in the `named` packing (`tile * 16 + slot`).
/// `-1` for unknown attrs. Engine monopoly attrs.
fn wire_attr_index(attr_id: i64) -> i64 {
    const ATTRS: [&str; 7] = ["group", "tier", "price", "house", "area", "rent", "index"];
    ATTRS
        .iter()
        .position(|a| attr_id == ctx::id_of(a))
        .map(|i| i as i64)
        .unwrap_or(-1)
}

fn seat_i64(ctx: &IrCtx, args: &[Value], f: SeatField) -> Value {
    match args.first() {
        Some(Value::Int(p)) => Value::Int(ctx.seat_field(*p, f)),
        Some(Value::Err) => Value::Err,
        _ => Value::Err,
    }
}
