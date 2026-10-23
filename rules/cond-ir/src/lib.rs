//! # rules-cond-ir — single-source guard conditions for Kani
//!
//! The production host evaluates a guard's CEL string through `rules-cond`
//! (vendored `cel` interpreter). Kani cannot: the `cel` crate pulls `String` /
//! `Vec` / `BTreeMap` and hits unwinding cliffs. This crate is the **Rust
//! evaluator of the same linted/rewritten AST**: a heap-free arena IR, a total
//! interpreter over [`Val`], and an independently written reference semantics
//! ([`spec`]) that the Kani proofs pin the interpreter to.
//!
//! Single source of truth: the CEL `&'static str` on each `On::*` entry.
//! `rules-cond`'s `lower` feature turns that string's rewritten AST into a
//! [`Program`]; a generator embeds the program as `const` data for Kani; a CI
//! check re-lowers every inventory string and asserts equality. Nothing is
//! authored twice.
//!
//! Design + proof table: `docs/rulebook/spec/COND-SINGLE-SOURCE.md`.
//!
//! # Semantics preserved (from the vendored `cel` crate)
//!
//! * int64 arithmetic uses `checked_*`; overflow is an error
//! * division / remainder by zero is an error
//! * `null` is its own value: `null == null` is true, `null == 1` is false,
//!   ordering involving `null` is an error
//! * `&&` / `||` absorb an error on one side when the other side decides
//!   (`false && e` = false, `e && false` = false, `true || e` = true,
//!   `e || true` = true); every other error pairing is an error
//! * ternary is lazy and only the taken branch is evaluated
//! * **any** evaluation error makes the whole condition false
//!   (`Cond::eval = eval_checked.unwrap_or(false)`)

pub mod ctx;
pub mod eval;
pub mod nodes;
pub mod spec;
pub mod vars;

pub use ctx::{FnId, IrCtx, SeatField, SetId};
pub use eval::{eval_node, ListBuf};
pub use nodes::{Kind, Node, Program};
pub use vars::{VarId, VarTy, SCALAR_COUNT};

/// The value lattice of the closed operator set. `Err` is the absorbing
/// "evaluation failed" result; the host maps it to `false` at the root.
/// `List` is only produced by list literals and consumed by `in` / `size`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Int(i64),
    Bool(bool),
    /// A kind-spelling scalar: the payload is `id_of(spelling)`. Only `Eq` /
    /// `Ne` compare it (against another `Str` hash or an `Int` with the same
    /// payload -- the hash-baked string-literal form). Arithmetic and ordering
    /// are `Err`, matching CEL's type error on `kind + 1`.
    Str(i64),
    Null,
    /// Fixed-capacity int list (max 8) — no heap.
    List(ListBuf),
    Err,
}

impl Val {
    pub fn is_err(self) -> bool {
        matches!(self, Val::Err)
    }

    /// CEL `try_bool`: only a real bool is truthy/falsy; anything else is an
    /// error (including `null`, `List`, and `Err`).
    pub fn try_bool(self) -> Result<bool, ()> {
        match self {
            Val::Bool(b) => Ok(b),
            _ => Err(()),
        }
    }

    /// Host root rule: any error (or a non-bool root) is `false`.
    pub fn as_cond_bool(self) -> bool {
        matches!(self, Val::Bool(true))
    }
}

/// A compiled program view over borrowed nodes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ProgramRef<'a> {
    pub nodes: &'a [Node],
    pub root: u16,
}

impl<'a> ProgramRef<'a> {
    pub fn eval(&self, ctx: &IrCtx) -> Val {
        eval_node(self.nodes, self.root, ctx)
    }

    /// Host root rule — matches `rules_cond::Cond::eval`.
    pub fn eval_bool(&self, ctx: &IrCtx) -> bool {
        self.eval(ctx).as_cond_bool()
    }
}
