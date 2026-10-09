//! Evaluation: one window scope per trigger/chain window, one child scope per
//! candidate. The window scope is where the savings of G1 live -- the CEL
//! context (and the custom lookup functions) are built once and reused across
//! every candidate probe in the window.
//!
//! Every variable and function binding is **driven from [`crate::vocab::VOCAB`]
//! through [`crate::view::CondView`]**: adding a name never edits this file's
//! tables. The evaluator only knows the CEL `Value` plumbing and the few
//! shapes of [`crate::vocab::Fx`] a function closure can take.

use std::collections::HashMap;
use std::sync::Arc;

use cel::common::value::{CowVal, Val as CelVal};
use cel::objects::{Key, Map, Value};
use cel::{Context, ExecutionError, FunctionContext};

use crate::compile::{shared_env, Cond};
use crate::ctx::{CandidateCtx, WindowCtx};
use crate::kinds;
use crate::view::CondView;
use crate::vocab::{self, Fx, Name, Scope, Table, VOCAB};

/// A window's reusable CEL root context. Build once per `declare_one` /
/// `run_hook` window; then [`Cond::eval`] (or [`Cond::eval_with_scope`]) for
/// each candidate.
pub struct WindowScope {
    root: Context<'static, 'static>,
}

impl WindowScope {
    /// Build the window half of the context: every window variable, the kind
    /// constants, the effect-chain lookup tables, and the shared lookup
    /// functions. `O(schema)` once per window.
    pub fn new(win: &WindowCtx) -> Self {
        let mut root = Context::with_env(shared_env());
        // The eager snapshot is the reference `CondView`; the host's live
        // view (`game-rules`'s `LiveSnap`) can fill the same tables directly.
        let empty_cand = CandidateCtx::default();
        let view = crate::SnapshotView {
            win,
            cand: &empty_cand,
        };
        bind_window(&mut root, &view);
        install_functions(&mut root);
        WindowScope { root }
    }

    /// Evaluate `cond` against a candidate. The child scope borrows the window
    /// root and only adds the owner/candidate overlay the condition reads.
    pub fn eval(&self, cond: &Cond, cand: &CandidateCtx) -> bool {
        self.eval_checked(cond, cand).unwrap_or(false)
    }

    /// Like [`eval`](Self::eval) but surfaces the underlying CEL error. A
    /// well-typed condition over well-filled contexts cannot fail here; this
    /// exists for the G3 audit and the tests.
    pub fn eval_checked(&self, cond: &Cond, cand: &CandidateCtx) -> Result<bool, EvalError> {
        let mut child = self.root.new_inner_scope();
        // Candidate overlay through the same `CondView` contract. The window
        // half of the view is not needed here (those names are already bound
        // on the root); a default window keeps `SnapshotView` total.
        let empty_win = WindowCtx::default();
        let view = crate::SnapshotView {
            win: &empty_win,
            cand,
        };
        bind_candidate(&mut child, cond, &view);
        let val = child
            .resolve(cond.expr())
            .map_err(EvalError::Exec)?;
        match val {
            Value::Bool(b) => Ok(b),
            other => Err(EvalError::NotBool(format!("{other:?}"))),
        }
    }
}

/// Error from evaluating a compiled condition.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    Exec(ExecutionError),
    NotBool(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Exec(e) => write!(f, "eval error: {e}"),
            EvalError::NotBool(s) => write!(f, "condition did not return bool: {s}"),
        }
    }
}

impl std::error::Error for EvalError {}

// ---------------------------------------------------------------------------
// bindings (driven from VOCAB through CondView)
// ---------------------------------------------------------------------------

fn emit_var(ctx: &mut Context<'_, '_>, name: &str, v: vocab::Val) {
    ctx.add_variable_from_value(name, cel_value(&v));
}

fn cel_value(v: &vocab::Val) -> Value {
    match v {
        vocab::Val::Int(i) => Value::Int(*i),
        vocab::Val::Bool(b) => Value::Bool(*b),
        vocab::Val::Null => Value::Null,
    }
}

fn table_value(t: &Table) -> Value {
    match t {
        Table::IntMap(rows) => {
            let mut m = HashMap::new();
            for (k, v) in rows {
                m.insert(Key::Int(*k), Value::Int(*v));
            }
            Value::Map(Map { map: Arc::new(m) })
        }
        Table::StrMap(rows) => {
            let mut m = HashMap::new();
            for (k, v) in rows {
                m.insert(Key::String(Arc::new(k.clone())), Value::Int(*v));
            }
            Value::Map(Map { map: Arc::new(m) })
        }
        Table::IntList(items) => {
            Value::List(Arc::new(items.iter().map(|i| Value::Int(*i)).collect()))
        }
        Table::Seats(n) => Value::Int(*n),
    }
}

/// Window half: kind constants, every VOCAB window scalar, and the window-side
/// function tables. Built once per window (the amortised G1 saving).
fn bind_window(ctx: &mut Context<'static, 'static>, view: &dyn CondView) {
    // Kind constants (bare identifiers in conditions).
    for (name, val) in kinds::constants() {
        ctx.add_variable_from_value(*name, Value::Int(*val));
    }

    // Window scalars: every VOCAB Window name, fetched through the view.
    for n in VOCAB {
        if n.scope != Scope::Window {
            continue;
        }
        let v = vocab::fetch_name(n, view).unwrap_or(vocab::Val::Null);
        emit_var(ctx, n.flat, v);
    }

    // Window-side function tables (money(p), tile_named, is_*, chain_*, …).
    for n in VOCAB {
        if !matches!(n.scope, Scope::Func { cand: false, .. }) {
            continue;
        }
        if let Some((var, t)) = vocab::fn_table(n, view) {
            // Several functions share a table (`money` bakes `_money` once);
            // re-baking the same var is idempotent, so just write it.
            ctx.add_variable_from_value(var, table_value(&t));
        }
    }
}

/// Candidate-side bindings, restricted to what `cond` actually reads. This is
/// the per-probe cost of the evaluator; binding the whole schema every time
/// measured ~5 µs/eval, binding only the used names gets the simple
/// predicates down toward the AST walk itself.
fn bind_candidate(ctx: &mut Context<'_, '_>, cond: &Cond, view: &dyn CondView) {
    // Walk the condition's own used names (small) and look each up in VOCAB,
    // rather than scanning the whole vocabulary per probe.
    for flat in cond.used_vars() {
        let Some(n) = vocab::by_flat(flat) else {
            continue;
        };
        if n.scope != Scope::Candidate {
            continue;
        }
        let v = vocab::fetch_name(n, view).unwrap_or(vocab::Val::Null);
        emit_var(ctx, n.flat, v);
    }

    // Candidate-side function tables (`slot` / `tok` / `blocked`). The
    // window-side tables (`_seats`, `_money`, …) are already on the root.
    for flat in cond.used_fns() {
        let Some(n) = vocab::by_flat(flat) else {
            continue;
        };
        if !matches!(n.scope, Scope::Func { cand: true, .. }) {
            continue;
        }
        if let Some((var, t)) = vocab::fn_table(n, view) {
            ctx.add_variable_from_value(var, table_value(&t));
        }
    }
}

// ---------------------------------------------------------------------------
// lookup functions (registered per window root, shaped by VOCAB's Fx)
// ---------------------------------------------------------------------------

fn install_functions(ctx: &mut Context<'static, 'static>) {
    // Each closure reads the hidden table `vocab::fn_table` baked from the
    // view. Inference picks the `WithFunctionContext` impls (first arg is
    // `&FunctionContext`).
    for n in VOCAB {
        install_one(ctx, n);
    }
}

fn install_one(ctx: &mut Context<'static, 'static>, n: &Name) {
    if !matches!(n.scope, Scope::Func { .. }) {
        return;
    }
    let fname = n.cel;
    match n.fx {
        Fx::SeatIntIs { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, p: i64, id: i64| -> Result<bool, ExecutionError> {
                    Ok(player_int(ftx, p, &var)? == id)
                },
            )
            .expect(fname);
        }
        Fx::SeatInt { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
                    player_int(ftx, p, &var)
                },
            )
            .expect(fname);
        }
        Fx::StrInt { var, missing, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, name: Arc<String>| -> Result<i64, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(int_at_str(&m, name.as_str(), missing))
                },
            )
            .expect(fname);
        }
        Fx::IntInt { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, kind: i64| -> Result<i64, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(int_at_int(&m, kind))
                },
            )
            .expect(fname);
        }
        Fx::IntHas { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, x: i64| -> Result<bool, ExecutionError> {
                    Ok(list_contains(ftx, &var, x))
                },
            )
            .expect(fname);
        }
        Fx::Neighbor => {
            ctx.add_function(
                fname,
                |ftx: &FunctionContext, p: i64, delta: i64| -> Result<i64, ExecutionError> {
                    let n = int_var(ftx, "_seats")?;
                    if n <= 0 {
                        return Ok(-1);
                    }
                    Ok(((p + delta) % n + n) % n)
                },
            )
            .expect(fname);
        }
    }
}

fn missing(name: &str) -> ExecutionError {
    ExecutionError::UndeclaredReference(name.to_string().into())
}

fn to_value(v: &dyn CelVal) -> Result<Value, ExecutionError> {
    Value::try_from(v).map_err(|_| ExecutionError::InternalError("value conversion".into()))
}

fn map_var(ftx: &FunctionContext, name: &str) -> Result<Arc<HashMap<Key, Value>>, ExecutionError> {
    let v = ftx.ptx.get_variable(name).ok_or_else(|| missing(name))?;
    match to_value(v.as_ref())? {
        Value::Map(m) => Ok(m.map.clone()),
        other => Err(ExecutionError::FunctionError {
            function: name.into(),
            message: format!("expected map, got {other:?}"),
        }),
    }
}

fn int_var(ftx: &FunctionContext, name: &str) -> Result<i64, ExecutionError> {
    let v = ftx.ptx.get_variable(name).ok_or_else(|| missing(name))?;
    match to_value(v.as_ref())? {
        Value::Int(i) => Ok(i),
        other => Err(ExecutionError::FunctionError {
            function: name.into(),
            message: format!("expected int, got {other:?}"),
        }),
    }
}

fn int_at_str(m: &HashMap<Key, Value>, key: &str, missing: i64) -> i64 {
    match m.get(&Key::String(Arc::new(key.to_string()))) {
        Some(Value::Int(i)) => *i,
        _ => missing,
    }
}

fn int_at_int(m: &HashMap<Key, Value>, key: i64) -> i64 {
    match m.get(&Key::Int(key)) {
        Some(Value::Int(i)) => *i,
        _ => 0,
    }
}

fn player_int(ftx: &FunctionContext, p: i64, var: &str) -> Result<i64, ExecutionError> {
    let m = map_var(ftx, var)?;
    Ok(int_at_int(&m, p))
}

fn list_contains(ftx: &FunctionContext, name: &str, needle: i64) -> bool {
    let Ok(v) = ftx.ptx.get_variable(name).ok_or_else(|| missing(name)) else {
        return false;
    };
    match to_value(v.as_ref()) {
        Ok(Value::List(items)) => items.iter().any(|i| matches!(i, Value::Int(x) if *x == needle)),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// one-shot eval (no scope reuse) -- convenience for tests and low-frequency
// call sites; the hot path should build a [`WindowScope`].
// ---------------------------------------------------------------------------

impl Cond {
    /// Build a throwaway window scope and evaluate. Equivalent to
    /// `WindowScope::new(w).eval(self, c)`; prefer the two-step form when
    /// probing many candidates in one window.
    pub fn eval(&self, win: &WindowCtx, cand: &CandidateCtx) -> bool {
        WindowScope::new(win).eval(self, cand)
    }

    pub fn eval_checked(
        &self,
        win: &WindowCtx,
        cand: &CandidateCtx,
    ) -> Result<bool, EvalError> {
        WindowScope::new(win).eval_checked(self, cand)
    }
}

// Silence an unused-import warning if CowVal is not referenced in some cfgs.
#[allow(dead_code)]
fn _cow_ty(_: CowVal<'_, '_>) {}

// ---------------------------------------------------------------------------
// lazy probe (measurement only) -- see docs/GUARDS.md §4.2b
// ---------------------------------------------------------------------------
//
// A CEL `Context` wants owned `'static` values, so the production path bakes
// the used names / function tables into the context once per window. The
// alternative -- have every function closure fetch through a `&dyn CondView`
// at call time -- needs a side channel. A thread-local is the least unsafe
// one; this module exposes it so `examples/bench_cond.rs` can measure whether
// that indirection beats the eager copy. Spoiler: it does not (GUARDS.md).

thread_local! {
    static LIVE_VIEW: std::cell::Cell<Option<&'static dyn CondView>> =
        const { std::cell::Cell::new(None) };
}

/// Run `f` with `view` installed as the thread-local live [`CondView`].
///
/// # Safety of the lifetime cast
/// The `'static` is a lie for the duration of `f` only: the pointer is
/// cleared (and the previous value restored) before this returns, and `f`
/// cannot stash it. Measurement-only -- the production path bakes values.
pub fn with_live_view<R>(view: &dyn CondView, f: impl FnOnce() -> R) -> R {
    LIVE_VIEW.with(|c| {
        let prev = c.get();
        let static_view: &'static dyn CondView = unsafe { std::mem::transmute(view) };
        c.set(Some(static_view));
        let r = f();
        c.set(prev);
        r
    })
}

/// The live view installed by [`with_live_view`], if any.
pub fn live_view() -> Option<&'static dyn CondView> {
    LIVE_VIEW.with(|c| c.get())
}

/// Fetch one function argument through the live view (lazy path). Used by the
/// measurement closures in `bench_cond`; the production closures read the
/// baked tables instead.
pub fn lazy_money(seat: i64) -> i64 {
    match live_view() {
        Some(v) => v.money(seat),
        None => 0,
    }
}