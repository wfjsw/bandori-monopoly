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

/// Shared empty [`CandidateCtx`] for the window-half binds. A `default()` on
/// the caller's frame is ~240 B per probe; STACK-01's 512 KiB thread adds
/// those up along a deep settle chain.
fn empty_cand() -> &'static CandidateCtx {
    use std::sync::LazyLock;
    static EMPTY: LazyLock<CandidateCtx> = LazyLock::new(CandidateCtx::default);
    &EMPTY
}

/// A window's reusable CEL root context. Build once per `declare_one` /
/// `run_hook` window; then [`Cond::eval`] (or [`Cond::eval_with_scope`]) for
/// each candidate.
pub struct WindowScope {
    /// Heap-boxed: a `Context` plus the window snapshot is several KiB, and a
    /// scope is built per gate/condition probe. Keeping it off the caller's
    /// frame is what lets a deep settle chain (STACK-01) stay inside a small
    /// thread stack.
    root: Box<Context<'static, 'static>>,
    /// The window snapshot, kept so the function tables can be baked on
    /// demand (see [`Self::bind_window_tables`]). Heap-boxed with `root`.
    win: Box<WindowCtx>,
    /// Window-side function tables already baked for this window, by the
    /// function's flat name: `(hidden variable, table)`.
    tables: std::sync::Mutex<HashMap<String, Option<(&'static str, Value)>>>,
}

impl WindowScope {
    /// Build the window half of the context: every window variable, the kind
    /// constants, the effect-chain lookup tables, and the shared lookup
    /// functions. `O(schema)` once per window.
    pub fn new(win: &WindowCtx) -> Self {
        Self::from_owned(win.clone())
    }

    /// [`Self::new`] taking the snapshot by value (no copy).
    pub fn from_owned(win: WindowCtx) -> Self {
        let win = Box::new(win);
        let mut root = Context::with_env(shared_env());
        // The eager snapshot is the reference `CondView`; the host's live
        // view (`game-rules`'s `LiveSnap`) can fill the same tables directly.
        {
            let view = crate::SnapshotView {
                win: &win,
                cand: empty_cand(),
            };
            bind_window(&mut root, &view);
        }
        install_functions(&mut root);
        WindowScope {
            root: Box::new(root),
            win,
            tables: Default::default(),
        }
    }

    /// Bind the window-side function tables `cond` actually calls into the
    /// candidate scope. They used to be baked eagerly for every function on
    /// every window; most windows serve a handful of probes (or none) that read
    /// one or two of them, and baking the tile tables dominated a match. Each
    /// table is built once per window on first use.
    fn bind_window_tables(&self, scope: &mut Context<'_, '_>, cond: &Cond) {
        for flat in cond.used_fns() {
            let hit = {
                let mut cache = self.tables.lock().unwrap_or_else(|e| e.into_inner());
                cache
                    .entry(flat.clone())
                    .or_insert_with(|| {
                        let n = vocab::by_flat(flat)?;
                        if !matches!(n.scope, Scope::Func { cand: false, .. }) {
                            return None;
                        }
                        let view = crate::SnapshotView {
                            win: &self.win,
                            cand: empty_cand(),
                        };
                        vocab::fn_table(n, &view).map(|(var, t)| (var, table_value(&t)))
                    })
                    .clone()
            };
            if let Some((var, v)) = hit {
                // Several functions share a table (`money` bakes `_money`
                // once); re-binding the same var is idempotent.
                scope.add_variable_from_value(var, v);
            }
        }
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
        // on the root); borrow this scope's already-boxed snapshot so the
        // probe does not materialise a 400-odd-byte `WindowCtx` on the frame.
        let view = crate::SnapshotView {
            win: &self.win,
            cand,
        };
        bind_candidate(&mut child, cond, &view);
        self.bind_window_tables(&mut child, cond);
        let val = child.resolve(cond.expr()).map_err(EvalError::Exec)?;
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
        vocab::Val::Str(s) => Value::String(Arc::new(s.clone())),
        vocab::Val::Null => Value::Null,
    }
}

/// [`build_table_value`] through a per-thread memo. The big tables (tile
/// attrs / classes / kinds, pairwise distances) are a function of the board,
/// which is the same from one window to the next, yet rebuilding their CEL
/// maps dominated every window. A hit returns the shared (`Arc`) value; the
/// stored table is compared in full, so a hash collision cannot alias.
fn table_value(t: &Table) -> Value {
    use std::hash::{Hash, Hasher};
    // Small tables cost less to build than to hash.
    let size = match t {
        Table::IntMap(r) => r.len(),
        Table::StrMap(r) => r.len(),
        Table::IntList(r) => r.len(),
        Table::Seats(_) => 0,
        Table::BinMap(r) => r.len(),
        Table::StrIntList(r) => r.len() * 8,
        Table::IntStrMap(r) => r.len(),
        Table::TripleSet(r) => r.len(),
    };
    if size < 16 {
        return build_table_value(t);
    }
    thread_local! {
        static MEMO: std::cell::RefCell<HashMap<u64, Vec<(Table, Value)>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    let key = h.finish();
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(v) = m.get(&key).and_then(|b| b.iter().find(|(c, _)| c == t)) {
            return v.1.clone();
        }
        // Bounded: a match touches a handful of distinct boards; a runaway
        // stream of different tables just starts over.
        if m.len() >= 256 {
            m.clear();
        }
        let v = build_table_value(t);
        m.entry(key).or_default().push((t.clone(), v.clone()));
        v
    })
}

fn build_table_value(t: &Table) -> Value {
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
        Table::BinMap(rows) => {
            // `(a, b) -> v` as a map keyed by `a * 1024 + b`. Tile ids and
            // seat ids (and the small radii) stay well under 1024, so the
            // packing is injective over the domains the fills produce.
            let mut m = HashMap::new();
            for (a, b, v) in rows {
                m.insert(Key::Int(a * 1024 + b), Value::Int(*v));
            }
            Value::Map(Map { map: Arc::new(m) })
        }
        Table::StrIntList(rows) => {
            // kind -> tile list (`tile_is`). Nested map of kind -> list.
            let mut outer = HashMap::new();
            for (k, tiles) in rows {
                let list: Vec<Value> = tiles.iter().map(|t| Value::Int(*t)).collect();
                outer.insert(
                    Key::String(Arc::new(k.clone())),
                    Value::List(Arc::new(list)),
                );
            }
            Value::Map(Map {
                map: Arc::new(outer),
            })
        }
        Table::IntStrMap(rows) => {
            // `(tile, attr) -> v`, keyed by `"tile:attr"` (`tile_attr`).
            let mut m = HashMap::new();
            for (tile, attr, v) in rows {
                m.insert(
                    Key::String(Arc::new(format!("{tile}:{attr}"))),
                    Value::Int(*v),
                );
            }
            Value::Map(Map { map: Arc::new(m) })
        }
        Table::TripleSet(rows) => {
            // true `(player, tile, class)` triples (`tile_in`), keyed by
            // `"player:tile:class"`.
            let mut m = HashMap::new();
            for (p, t, c) in rows {
                m.insert(Key::String(Arc::new(format!("{p}:{t}:{c}"))), Value::Int(1));
            }
            Value::Map(Map { map: Arc::new(m) })
        }
    }
}

/// Window half: kind constants, every VOCAB window scalar, and the window-side
/// function tables. Built once per window (the amortised G1 saving).
fn bind_window(ctx: &mut Context<'static, 'static>, view: &dyn CondView) {
    // Kind-spelling constants bind as CEL strings (`kind == Exile`).
    for (name, spelling) in kinds::str_constants() {
        ctx.add_variable_from_value(*name, Value::String(Arc::new((*spelling).to_string())));
    }
    // Move-kind ints stay ints (`move.kind == Walk`).
    for (name, val) in kinds::int_constants() {
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

    // The window-side function tables (money(p), tile_named, is_*, chain_*, …)
    // are baked on demand per evaluation -- see `WindowScope::bind_window_tables`.
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
    let fname = n.flat;
    match n.fx {
        Fx::SeatIntIs { var, .. } => {
            let var = var.to_string();
            // Second arg is either an int id or a string name (the guest's
            // `persona_is(p, "名")` / `group_is(p, "Band")` spelling). A
            // string hashes through `id_of` -- the same name-stable id the
            // view's `character` / `band` accessors return.
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, p: i64, id: Value| -> Result<bool, ExecutionError> {
                    let want = match &id {
                        Value::Int(i) => *i,
                        Value::String(s) => crate::id_of(s.as_str()),
                        other => {
                            return Err(ExecutionError::FunctionError {
                                function: fname.into(),
                                message: format!("expected int or string id, got {other:?}"),
                            })
                        }
                    };
                    Ok(player_int(ftx, p, &var)? == want)
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
            // The arg is an int id or a string name: a string hashes through
            // `id_of` -- the same name-stable id `counter_is('cp')` /
            // `persona_is(p, "名")` compare against. Int callers
            // (`chain_has(roll)`, `is_start(t)`, `blocked(band)`) pass the
            // wire value directly and are unaffected.
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, x: Value| -> Result<bool, ExecutionError> {
                    let want = match &x {
                        Value::Int(i) => *i,
                        Value::String(s) => crate::id_of(s.as_str()),
                        other => {
                            return Err(ExecutionError::FunctionError {
                                function: fname.into(),
                                message: format!("expected int or string id, got {other:?}"),
                            })
                        }
                    };
                    Ok(list_contains(ftx, &var, want))
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
        Fx::IntIntConst { var, calc } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, a: i64, b: i64| -> Result<i64, ExecutionError> {
                    let n = int_var(ftx, &var)?;
                    Ok(calc(a, b, n))
                },
            )
            .expect(fname);
        }
        Fx::BinInt { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext, a: i64, b: i64| -> Result<i64, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(int_at_int(&m, a * 1024 + b))
                },
            )
            .expect(fname);
        }
        Fx::IntPred { calc } => {
            ctx.add_function(
                fname,
                move |_ftx: &FunctionContext, x: i64| -> Result<bool, ExecutionError> {
                    Ok(calc(x))
                },
            )
            .expect(fname);
        }
        Fx::TileIs { var, .. } => {
            let var = var.to_string();
            // `tile_is(t, kind)` -- kind is a string (or an id_of hash we
            // cannot reverse, so only string args resolve).
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext,
                      tile: i64,
                      kind: Arc<String>|
                      -> Result<bool, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    let want = format!("{tile}:{}", kind.as_str());
                    // Table is kind -> tile list; scan for the pair.
                    for (k, v) in m.iter() {
                        if let Key::String(ks) = k {
                            if ks.as_str() != kind.as_str() {
                                continue;
                            }
                            if let Value::List(items) = v {
                                return Ok(items
                                    .iter()
                                    .any(|i| matches!(i, Value::Int(x) if *x == tile)));
                            }
                        }
                    }
                    let _ = want;
                    Ok(false)
                },
            )
            .expect(fname);
        }
        Fx::TileAttr { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext,
                      tile: i64,
                      attr: Arc<String>|
                      -> Result<i64, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(int_at_str(
                        &m,
                        &format!("{tile}:{}", attr.as_str()),
                        default_attr(attr.as_str()),
                    ))
                },
            )
            .expect(fname);
        }
        Fx::TileIn { var, .. } => {
            let var = var.to_string();
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext,
                      player: i64,
                      tile: i64,
                      class: Arc<String>|
                      -> Result<bool, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(m.get(&Key::String(Arc::new(format!(
                        "{player}:{tile}:{}",
                        class.as_str()
                    ))))
                    .is_some())
                },
            )
            .expect(fname);
        }
        Fx::SeatStrInt { var, .. } => {
            let var = var.to_string();
            // `state(p, 'key')` -- the key is a string; missing = 0.
            ctx.add_function(
                fname,
                move |ftx: &FunctionContext,
                      seat: i64,
                      key: Arc<String>|
                      -> Result<i64, ExecutionError> {
                    let m = map_var(ftx, &var)?;
                    Ok(int_at_str(&m, &format!("{seat}:{}", key.as_str()), 0))
                },
            )
            .expect(fname);
        }
    }
}

/// Missing-value default for `tile_attr` (mirrors `tile_class::attr_of`).
fn default_attr(attr: &str) -> i64 {
    match attr {
        "group" | "tier" | "area" | "index" => -1,
        _ => 0,
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
        Ok(Value::List(items)) => items
            .iter()
            .any(|i| matches!(i, Value::Int(x) if *x == needle)),
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

    pub fn eval_checked(&self, win: &WindowCtx, cand: &CandidateCtx) -> Result<bool, EvalError> {
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
