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
use crate::Val;

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
pub fn eval_binop(kind: Kind, a: Val, b: Val) -> Val {
    match kind {
        Kind::Add => spec_arith_local(a, b, true, false),
        Kind::Sub => spec_arith_local(a, b, false, false),
        Kind::Mul => spec_arith_local(a, b, false, true),
        Kind::Div => divrem(a, b, true),
        Kind::Rem => divrem(a, b, false),
        Kind::Eq => match (a, b) {
            (Val::Err, _) | (_, Val::Err) => Val::Err,
            (x, y) => Val::Bool(values_eq(x, y)),
        },
        Kind::Ne => match (a, b) {
            (Val::Err, _) | (_, Val::Err) => Val::Err,
            (x, y) => Val::Bool(!values_eq(x, y)),
        },
        Kind::Lt => cmp(a, b, |o| o == Ordering::Less),
        Kind::Le => cmp(a, b, |o| o != Ordering::Greater),
        Kind::Gt => cmp(a, b, |o| o == Ordering::Greater),
        Kind::Ge => cmp(a, b, |o| o != Ordering::Less),
        Kind::In => match (a, b) {
            (Val::Err, _) | (_, Val::Err) => Val::Err,
            (x, Val::List(items)) => {
                Val::Bool(items.as_slice().iter().any(|i| values_eq(x, Val::Int(*i))))
            }
            _ => Val::Err,
        },
        _ => Val::Err,
    }
}

/// Unary operator step.
pub fn eval_unop(kind: Kind, a: Val) -> Val {
    match kind {
        Kind::Neg => match a {
            Val::Int(v) => match v.checked_neg() {
                Some(w) => Val::Int(w),
                None => Val::Err,
            },
            _ => Val::Err,
        },
        Kind::Not => match a.try_bool() {
            Ok(b) => Val::Bool(!b),
            Err(()) => Val::Err,
        },
        Kind::Size => match a {
            Val::List(items) => Val::Int(items.n as i64),
            _ => Val::Err,
        },
        _ => Val::Err,
    }
}

/// Logical connectives with CEL error absorption. `right_evaluated` = false
/// means the left side short-circuited and `r` is a placeholder.
pub fn eval_and(l: Val, r: Val) -> Val {
    let lb = l.try_bool();
    if lb == Ok(false) {
        return Val::Bool(false);
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(true), Ok(b)) => Val::Bool(b),
        (Err(()), Ok(false)) => Val::Bool(false),
        _ => Val::Err,
    }
}

pub fn eval_or(l: Val, r: Val) -> Val {
    let lb = l.try_bool();
    if lb == Ok(true) {
        return Val::Bool(true);
    }
    let rb = r.try_bool();
    match (lb, rb) {
        (Ok(false), Ok(b)) => Val::Bool(b),
        (Err(()), Ok(true)) => Val::Bool(true),
        _ => Val::Err,
    }
}

fn spec_arith_local(a: Val, b: Val, is_add: bool, is_mul: bool) -> Val {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => {
            let r = if is_add {
                x.checked_add(y)
            } else if is_mul {
                x.checked_mul(y)
            } else {
                x.checked_sub(y)
            };
            match r {
                Some(z) => Val::Int(z),
                None => Val::Err,
            }
        }
        _ => Val::Err,
    }
}

fn divrem(a: Val, b: Val, is_div: bool) -> Val {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => {
            if y == 0 {
                return Val::Err;
            }
            let r = if is_div {
                x.checked_div(y)
            } else {
                x.checked_rem(y)
            };
            match r {
                Some(z) => Val::Int(z),
                None => Val::Err,
            }
        }
        _ => Val::Err,
    }
}

fn cmp(a: Val, b: Val, pred: impl Fn(Ordering) -> bool) -> Val {
    match values_cmp(a, b) {
        Some(o) => Val::Bool(pred(o)),
        None => Val::Err,
    }
}

/// Evaluate `nodes[root]`. Children are earlier indices; short-circuiting
/// skips the untaken arm entirely.
pub fn eval_node(nodes: &[Node], root: u16, ctx: &IrCtx) -> Val {
    let Some(n) = nodes.get(root as usize) else {
        return Val::Err;
    };
    match n.kind {
        Kind::ConstInt => Val::Int(n.payload),
        Kind::ConstBool => Val::Bool(n.payload != 0),
        Kind::ConstNull => Val::Null,
        Kind::Var => ctx.scalar_val(n.payload as u16),
        Kind::Neg | Kind::Not | Kind::Size => eval_unop(n.kind, eval_node(nodes, n.a, ctx)),
        Kind::And => {
            let l = eval_node(nodes, n.a, ctx);
            if l.try_bool() == Ok(false) {
                Val::Bool(false)
            } else {
                eval_and(l, eval_node(nodes, n.b, ctx))
            }
        }
        Kind::Or => {
            let l = eval_node(nodes, n.a, ctx);
            if l.try_bool() == Ok(true) {
                Val::Bool(true)
            } else {
                eval_or(l, eval_node(nodes, n.b, ctx))
            }
        }
        Kind::Tern => match eval_node(nodes, n.a, ctx).try_bool() {
            Ok(true) => eval_node(nodes, n.b, ctx),
            Ok(false) => eval_node(nodes, n.c, ctx),
            Err(()) => Val::Err,
        },
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
        Kind::Call => eval_call(nodes, n, ctx),
        // Binary ops — operands first, then the pure step.
        Kind::Add | Kind::Sub | Kind::Mul | Kind::Div | Kind::Rem | Kind::Eq | Kind::Ne
        | Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge | Kind::In => {
            let a = eval_node(nodes, n.a, ctx);
            let b = eval_node(nodes, n.b, ctx);
            eval_binop(n.kind, a, b)
        }
    }
}

/// Equality is total over the lattice: cross-type is false, `null == null` is
/// true. Lists compare elementwise (only used by tests).
pub fn values_eq(a: Val, b: Val) -> bool {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => x == y,
        (Val::Bool(x), Val::Bool(y)) => x == y,
        (Val::Null, Val::Null) => true,
        (Val::List(x), Val::List(y)) => x == y,
        (Val::Err, _) | (_, Val::Err) => false,
        _ => false,
    }
}

/// Ordering: only like-with-like scalar. **Any** `null` operand is an
/// incomparable error — including `null` vs `null`. The vendored `cel` crate's
/// `CelNull` does not implement `Comparer`, so `compare_values` fails for
/// every null pairing. Confirmed by the differential tests.
pub fn values_cmp(a: Val, b: Val) -> Option<Ordering> {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => Some(x.cmp(&y)),
        (Val::Bool(x), Val::Bool(y)) => Some(x.cmp(&y)),
        (Val::Err, _) | (_, Val::Err) => None,
        _ => None,
    }
}

fn eval_call(nodes: &[Node], n: &Node, ctx: &IrCtx) -> Val {
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
    call_fn(ctx, fid, &args[..nargs])
}

/// Narrow function surface. Every name goes through here so the Kani host has
/// one place to model.
pub fn call_fn(ctx: &IrCtx, fid: FnId, args: &[Val]) -> Val {
    // Helper: require int arg.
    let int = |i: usize| -> Result<i64, ()> {
        match args.get(i) {
            Some(Val::Int(v)) => Ok(*v),
            Some(Val::Err) => Err(()),
            _ => Err(()),
        }
    };
    match fid {
        FnId::Slot | FnId::Tok | FnId::CardCounter | FnId::TileNamed | FnId::MoveTag => {
            let name = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let missing = match fid {
                FnId::TileNamed => -1,
                _ => 0,
            };
            Val::Int(ctx.named_get(name).unwrap_or(missing))
        }
        FnId::CounterIs => {
            let want = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            // `counter_is('name')` ≡ `counter_name == id_of(name)`.
            Val::Bool(ctx.scalar_val(vars_counter_name_idx()) == Val::Int(want))
        }
        FnId::IsCircle => set_has(ctx, SetId::Circle, int(0)),
        FnId::IsRing => set_has(ctx, SetId::Ring, int(0)),
        FnId::IsLiveHouse => set_has(ctx, SetId::LiveHouse, int(0)),
        FnId::IsBuyable => set_has(ctx, SetId::Buyable, int(0)),
        // ABI v56 generic tile queries. String args lower to `id_of` hashes.
        // Membership is modelled through the existing sets + named table
        // (keep the IR integer-only; fills that need more should bake
        // `named` rows -- see `fill_board_sets`).
        FnId::TileIs => {
            let tile = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let kind_id = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            // Known wire kinds reuse the membership sets; the rest read
            // `named[tile]` as a bitset of the 8 `TileKind::as_str` indices.
            if kind_id == ctx::id_of("circle") {
                return set_has(ctx, SetId::Circle, Ok(tile));
            }
            if kind_id == ctx::id_of("ring") {
                return set_has(ctx, SetId::Ring, Ok(tile));
            }
            let bits = ctx.named_get(tile).unwrap_or(0);
            match wire_kind_bit(kind_id) {
                Some(b) => Val::Bool(bits & (1i64 << b) != 0),
                None => Val::Bool(false),
            }
        }
        FnId::TileAttr => {
            let tile = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let attr_id = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let idx = wire_attr_index(attr_id);
            if idx < 0 {
                return Val::Int(0);
            }
            let key = tile * 16 + idx;
            let missing = match idx {
                // group / tier / area / index
                0 | 1 | 4 | 6 => -1,
                _ => 0,
            };
            Val::Int(ctx.named_get(key).unwrap_or(missing))
        }
        FnId::TileIn => {
            let player = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let tile = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let class_id = match int(2) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            // Sugar classes reuse the sets; `color:<g>` / generic `class*`
            // need a filled `named` row (key = packed player/tile, value =
            // class id). Missing -> false.
            if class_id == ctx::id_of("buyable") {
                return set_has(ctx, SetId::Buyable, Ok(tile));
            }
            if class_id == ctx::id_of("livehouse") {
                return set_has(ctx, SetId::LiveHouse, Ok(tile));
            }
            if class_id == ctx::id_of("shop") {
                // shop = buyable && group 10; not a set today -> named row.
                let key = ((player + 2) << 32) ^ tile;
                return Val::Bool(ctx.named_get(key) == Some(class_id));
            }
            let key = ((player + 2) << 32) ^ tile;
            Val::Bool(ctx.named_get(key) == Some(class_id))
        }
        FnId::Money => seat_i64(ctx, args, SeatField::Money),
        FnId::Fire => seat_i64(ctx, args, SeatField::Fire),
        FnId::Crystals => seat_i64(ctx, args, SeatField::Crystals),
        FnId::Hand => seat_i64(ctx, args, SeatField::Hand),
        FnId::Pos => seat_i64(ctx, args, SeatField::Pos),
        FnId::Character => seat_i64(ctx, args, SeatField::Character),
        FnId::Band => seat_i64(ctx, args, SeatField::Band),
        FnId::Tiles => seat_i64(ctx, args, SeatField::Tiles),
        FnId::GainsThisTurn => seat_i64(ctx, args, SeatField::Gains),
        FnId::TargetedCount => seat_i64(ctx, args, SeatField::Targeted),
        FnId::Between => seat_i64(ctx, args, SeatField::Between),
        FnId::CharacterIs | FnId::BandIs => {
            let p = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let want = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let f = if fid == FnId::CharacterIs {
                SeatField::Character
            } else {
                SeatField::Band
            };
            Val::Bool(ctx.seat_field(p, f) == want)
        }
        FnId::Blocked => {
            let band = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Bool(ctx.set_contains(SetId::BlockedBands, band))
        }
        FnId::Neighbor => {
            let p = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let d = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Int(ctx::neighbor(p, d, ctx.n_players))
        }
        FnId::ChainHas => {
            let k = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Bool(ctx.set_contains(SetId::ChainKinds, k))
        }
        FnId::ChainHits => {
            let seat = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Bool(ctx.set_contains(SetId::ChainHits, seat))
        }
        FnId::RepeatedDigits => {
            let x = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Bool(ctx::repeated_digits(x))
        }
        FnId::Dist => {
            let a = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let b = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let n = ctx.tile_count;
            if n <= 0 {
                return Val::Int(0);
            }
            let d = ((b - a) % n + n) % n;
            Val::Int(d.min(n - d))
        }
        FnId::PlayersOn | FnId::NextDist | FnId::OthersWithin | FnId::OwnedWithin
        | FnId::OnPath => {
            let a = match int(0) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            let b = match int(1) {
                Ok(v) => v,
                Err(()) => return Val::Err,
            };
            Val::Int(ctx.bin_get(a, b))
        }
    }
}

fn vars_counter_name_idx() -> u16 {
    // Const index — never a VARS string scan on the eval path.
    crate::vars::VarId::CounterName as u16
}

fn set_has(ctx: &IrCtx, set: SetId, r: Result<i64, ()>) -> Val {
    match r {
        Ok(id) => Val::Bool(ctx.set_contains(set, id)),
        Err(()) => Val::Err,
    }
}

/// `TileKind::as_str` wire spelling -> bit index in `named[tile]`'s kind bitset
/// (circle=0 … event=7). `None` for anything else.
fn wire_kind_bit(kind_id: i64) -> Option<u32> {
    const KINDS: [&str; 8] = [
        "circle",
        "cafe",
        "edogawa",
        "ryuseido",
        "property",
        "ring",
        "agent",
        "event",
    ];
    KINDS
        .iter()
        .position(|k| kind_id == ctx::id_of(k))
        .map(|i| i as u32)
}

/// `tile_attr` field name -> slot in the `named` packing (`tile * 16 + slot`).
/// `-1` for unknown attrs.
fn wire_attr_index(attr_id: i64) -> i64 {
    const ATTRS: [&str; 7] = ["group", "tier", "price", "house", "area", "rent", "index"];
    ATTRS
        .iter()
        .position(|a| attr_id == ctx::id_of(a))
        .map(|i| i as i64)
        .unwrap_or(-1)
}

fn seat_i64(ctx: &IrCtx, args: &[Val], f: SeatField) -> Val {
    match args.first() {
        Some(Val::Int(p)) => Val::Int(ctx.seat_field(*p, f)),
        Some(Val::Err) => Val::Err,
        _ => Val::Err,
    }
}