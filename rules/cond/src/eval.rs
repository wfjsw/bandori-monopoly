//! Evaluation: one window scope per trigger/chain window, one child scope per
//! candidate. The window scope is where the savings of G1 live -- the CEL
//! context (and the custom lookup functions) are built once and reused across
//! every candidate probe in the window.

use std::collections::HashMap;
use std::sync::Arc;

use cel::common::value::{CowVal, Val};
use cel::objects::{Key, Map, Value};
use cel::{Context, ExecutionError, FunctionContext};

use crate::compile::{shared_env, Cond};
use crate::ctx::{CandidateCtx, WindowCtx};
use crate::kinds;

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
        bind_window(&mut root, win);
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
        bind_candidate(&mut child, cand, cond);
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
// bindings
// ---------------------------------------------------------------------------

fn bind_window(ctx: &mut Context<'static, 'static>, win: &WindowCtx) {
    // Kind constants (bare identifiers in conditions).
    for (name, val) in kinds::constants() {
        ctx.add_variable_from_value(*name, Value::Int(*val));
    }

    // Window scalars (§4.2). Absent optionals bind CEL `null`.
    ctx.add_variable_from_value("kind", Value::Int(win.kind));
    ctx.add_variable_from_value("actor", Value::Int(win.actor));
    ctx.add_variable_from_value("target", Value::Int(win.target));
    ctx.add_variable_from_value("value", Value::Int(win.value));
    ctx.add_variable_from_value("step", Value::Int(win.step));
    ctx.add_variable_from_value("by", Value::Int(win.by));
    ctx.add_variable_from_value("pay_is_rent", Value::Bool(win.pay_is_rent));
    ctx.add_variable_from_value("roll_source", Value::Int(win.roll_source));
    ctx.add_variable_from_value("abnormal", Value::Bool(win.abnormal));
    ctx.add_variable_from_value("turn_player", Value::Int(win.turn_player));
    ctx.add_variable_from_value("turn_key", Value::Int(win.turn_key));

    // move.* (flattened)
    ctx.add_variable_from_value(
        "move_roll",
        match win.mv.roll {
            Some(r) => Value::Int(r),
            None => Value::Null,
        },
    );
    ctx.add_variable_from_value(
        "move_kind",
        match win.mv.kind {
            Some(k) => Value::Int(k),
            None => Value::Null,
        },
    );
    ctx.add_variable_from_value("move_remaining", Value::Int(win.mv.remaining));
    ctx.add_variable_from_value("move_main", Value::Bool(win.mv.main));

    // tile.* (flattened)
    ctx.add_variable_from_value("tile_id", Value::Int(win.tile.id));
    ctx.add_variable_from_value("tile_owner", Value::Int(win.tile.owner));
    ctx.add_variable_from_value("tile_houses", Value::Int(win.tile.houses));
    ctx.add_variable_from_value("tile_mortgaged", Value::Int(win.tile.mortgaged));
    ctx.add_variable_from_value("tile_price", Value::Int(win.tile.price));

    // tile-kind sets for `is_circle(t)` / `is_ring(t)` / ...
    ctx.add_variable_from_value("_circle_tiles", int_list(win.circle_tiles.iter().copied()));
    ctx.add_variable_from_value("_ring_tiles", int_list(win.ring_tiles.iter().copied()));
    ctx.add_variable_from_value("_live_house_tiles", int_list(win.live_house_tiles.iter().copied()));
    ctx.add_variable_from_value("_buyable_tiles", int_list(win.buyable_tiles.iter().copied()));

    // effect/chain
    ctx.add_variable_from_value("effect_count", Value::Int(win.chain.len() as i64));

    // Lookup tables for the functions.
    ctx.add_variable_from_value("_seats", Value::Int(win.seat_count()));
    ctx.add_variable_from_value("_chain_kinds", int_list(win.chain.iter().map(|l| l.kind)));
    ctx.add_variable_from_value("_hit_seats", int_list(win.chain.iter().map(|l| l.hits)));
    ctx.add_variable_from_value("_chain_froms", int_list(win.chain.iter().map(|l| l.from)));

    // Player table for money(p) / character_is(p, …).
    ctx.add_variable_from_value("_money", player_field(win, |p| p.money));
    ctx.add_variable_from_value("_fire", player_field(win, |p| p.fire));
    ctx.add_variable_from_value("_crystals", player_field(win, |p| p.crystals));
    ctx.add_variable_from_value("_hand", player_field(win, |p| p.hand));
    ctx.add_variable_from_value("_pos", player_field(win, |p| p.pos));
    ctx.add_variable_from_value("_character", player_field(win, |p| p.character));
    ctx.add_variable_from_value("_band", player_field(win, |p| p.band));
    ctx.add_variable_from_value("_tiles", player_field(win, |p| p.tiles));

    // tile_named(name) -> id
    let mut names = HashMap::new();
    for (k, v) in &win.tile_ids {
        names.insert(Key::String(Arc::new(k.clone())), Value::Int(*v));
    }
    ctx.add_variable_from_value("_tile_ids", Value::Map(Map { map: Arc::new(names) }));
}

/// Candidate-side bindings, restricted to what `cond` actually reads. This is
/// the per-probe cost of the evaluator; binding the whole schema every time
/// measured ~5 µs/eval, binding only the used names gets the simple
/// predicates down toward the AST walk itself.
fn bind_candidate(ctx: &mut Context<'_, '_>, cand: &CandidateCtx, cond: &Cond) {
    let wants = |n: &str| cond.used_vars().iter().any(|v| v == n);
    let fn_wants = |n: &str| cond.used_fns().iter().any(|v| v == n);

    if wants("owner") {
        ctx.add_variable_from_value("owner", Value::Int(cand.owner));
    }
    macro_rules! maybe_int {
        ($name:literal, $field:ident) => {
            if wants($name) {
                ctx.add_variable_from_value($name, Value::Int(cand.$field));
            }
        };
    }
    maybe_int!("owner_money", owner_money);
    maybe_int!("owner_fire", owner_fire);
    maybe_int!("owner_crystals", owner_crystals);
    maybe_int!("owner_hand", owner_hand);
    maybe_int!("owner_pos", owner_pos);
    maybe_int!("owner_out", owner_out);
    maybe_int!("owner_stay", owner_stay);
    maybe_int!("owner_stun", owner_stun);
    maybe_int!("owner_exile", owner_exile);
    maybe_int!("owner_no_hand", owner_no_hand);
    maybe_int!("owner_character", owner_character);
    maybe_int!("owner_band", owner_band);
    maybe_int!("owner_tiles", owner_tiles);
    maybe_int!("card_id", card_id);
    maybe_int!("card_cp", card_cp);
    if wants("card_placed") {
        ctx.add_variable_from_value("card_placed", Value::Bool(cand.card_placed));
    }

    if fn_wants("slot") {
        let mut slots = HashMap::new();
        for (k, v) in &cand.slots {
            slots.insert(Key::String(Arc::new(k.clone())), Value::Int(*v));
        }
        ctx.add_variable_from_value("_slots", Value::Map(Map { map: Arc::new(slots) }));
    }
    if fn_wants("tok") {
        let mut toks = HashMap::new();
        for (k, v) in &cand.toks {
            toks.insert(Key::Int(*k), Value::Int(*v));
        }
        ctx.add_variable_from_value("_toks", Value::Map(Map { map: Arc::new(toks) }));
    }
    if fn_wants("blocked") {
        let blocked: Vec<Value> = cand.blocked_bands.iter().map(|b| Value::Int(*b)).collect();
        ctx.add_variable_from_value("_blocked", Value::List(Arc::new(blocked)));
    }
}

fn int_list(it: impl Iterator<Item = i64>) -> Value {
    Value::List(Arc::new(it.map(Value::Int).collect()))
}

fn player_field(win: &WindowCtx, f: impl Fn(&crate::ctx::PlayerSnap) -> i64) -> Value {
    let mut m = HashMap::new();
    for (i, p) in win.players.iter().enumerate() {
        m.insert(Key::Int(i as i64), Value::Int(f(p)));
    }
    Value::Map(Map { map: Arc::new(m) })
}

// ---------------------------------------------------------------------------
// lookup functions (registered per window root)
// ---------------------------------------------------------------------------

fn install_functions(ctx: &mut Context<'static, 'static>) {
    // Each closure reads hidden `_`-prefixed variables from the live context.
    // Inference picks the `WithFunctionContext` impls (first arg is
    // `&FunctionContext`).
    ctx.add_function("slot", |ftx: &FunctionContext, name: Arc<String>| -> Result<i64, ExecutionError> {
        let m = map_var(ftx, "_slots")?;
        Ok(int_at_str(&m, name.as_str()))
    })
    .expect("slot");

    ctx.add_function("tok", |ftx: &FunctionContext, kind: i64| -> Result<i64, ExecutionError> {
        let m = map_var(ftx, "_toks")?;
        Ok(int_at_int(&m, kind))
    })
    .expect("tok");

    ctx.add_function("money", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_money")
    })
    .expect("money");
    ctx.add_function("fire", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_fire")
    })
    .expect("fire");
    ctx.add_function("crystals", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_crystals")
    })
    .expect("crystals");
    ctx.add_function("hand", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_hand")
    })
    .expect("hand");
    ctx.add_function("pos", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_pos")
    })
    .expect("pos");
    ctx.add_function("character", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_character")
    })
    .expect("character");
    ctx.add_function("band", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_band")
    })
    .expect("band");
    ctx.add_function("tiles", |ftx: &FunctionContext, p: i64| -> Result<i64, ExecutionError> {
        player_int(ftx, p, "_tiles")
    })
    .expect("tiles");

    ctx.add_function(
        "character_is",
        |ftx: &FunctionContext, p: i64, id: i64| -> Result<bool, ExecutionError> {
            Ok(player_int(ftx, p, "_character")? == id)
        },
    )
    .expect("character_is");
    ctx.add_function(
        "band_is",
        |ftx: &FunctionContext, p: i64, id: i64| -> Result<bool, ExecutionError> {
            Ok(player_int(ftx, p, "_band")? == id)
        },
    )
    .expect("band_is");

    ctx.add_function(
        "blocked",
        |ftx: &FunctionContext, band: i64| -> Result<bool, ExecutionError> {
            let list = ftx
                .ptx
                .get_variable("_blocked")
                .ok_or_else(|| missing("_blocked"))?;
            let v = to_value(list.as_ref())?;
            Ok(match v {
                Value::List(items) => items.iter().any(|i| matches!(i, Value::Int(x) if *x == band)),
                _ => false,
            })
        },
    )
    .expect("blocked");

    ctx.add_function(
        "neighbor",
        |ftx: &FunctionContext, p: i64, delta: i64| -> Result<i64, ExecutionError> {
            let n = int_var(ftx, "_seats")?;
            if n <= 0 {
                return Ok(-1);
            }
            Ok(((p + delta) % n + n) % n)
        },
    )
    .expect("neighbor");

    ctx.add_function(
        "tile_named",
        |ftx: &FunctionContext, name: Arc<String>| -> Result<i64, ExecutionError> {
            let m = map_var(ftx, "_tile_ids")?;
            // -1 when unregistered, matching the guest `ctx::tile_named`
            // (`CardWorld::tile_named`): tile 0 is a real tile (CiRCLE), so a
            // 0 sentinel would alias it.
            Ok(match m.get(&Key::String(Arc::new(name.as_str().to_string()))) {
                Some(Value::Int(i)) => *i,
                _ => -1,
            })
        },
    )
    .expect("tile_named");

    for (fname, var) in [
        ("is_circle", "_circle_tiles"),
        ("is_ring", "_ring_tiles"),
        ("is_live_house", "_live_house_tiles"),
        ("is_buyable", "_buyable_tiles"),
    ] {
        let var = var.to_string();
        ctx.add_function(
            fname,
            move |ftx: &FunctionContext, tile: i64| -> Result<bool, ExecutionError> {
                Ok(list_contains(ftx, &var, tile))
            },
        )
        .expect(fname);
    }

    ctx.add_function(
        "chain_has",
        |ftx: &FunctionContext, kind: i64| -> Result<bool, ExecutionError> {
            Ok(list_contains(ftx, "_chain_kinds", kind))
        },
    )
    .expect("chain_has");
    ctx.add_function(
        "chain_hits",
        |ftx: &FunctionContext, seat: i64| -> Result<bool, ExecutionError> {
            Ok(list_contains(ftx, "_hit_seats", seat))
        },
    )
    .expect("chain_hits");
}

fn missing(name: &str) -> ExecutionError {
    ExecutionError::UndeclaredReference(name.to_string().into())
}

fn to_value(v: &dyn Val) -> Result<Value, ExecutionError> {
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

fn int_at_str(m: &HashMap<Key, Value>, key: &str) -> i64 {
    match m.get(&Key::String(Arc::new(key.to_string()))) {
        Some(Value::Int(i)) => *i,
        _ => 0,
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