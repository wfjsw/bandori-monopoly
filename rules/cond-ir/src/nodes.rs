//! Arena IR: a post-order array of [`Node`]s. No heap at evaluation time.

/// Node kind. Children are indices into the program's node slice and must be
/// strictly less than this node's index (post-order).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `payload` = the int literal.
    ConstInt,
    /// `payload` = 0 or 1.
    ConstBool,
    /// CEL `null`.
    ConstNull,
    /// `payload` = variable index ([`crate::vars::VarId`] as i64).
    Var,
    /// `-a`. Overflow is an error.
    Neg,
    /// `!a`. Non-bool is an error.
    Not,
    /// `a && b` with CEL error absorption (lazy in `b`).
    And,
    /// `a || b` with CEL error absorption (lazy in `b`).
    Or,
    /// `a ? b : c` (lazy in `b`/`c`; non-bool `a` is an error).
    Tern,
    /// Arithmetic. Overflow / div-by-zero / rem-by-zero is an error.
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    /// Equality. Cross-type is false (never an error), including `null`.
    Eq,
    Ne,
    /// Ordering. Incomparable (e.g. `null` vs int) is an error.
    Lt,
    Le,
    Gt,
    Ge,
    /// `a in b` where `b` is a list node ([`Kind::List`]).
    In,
    /// `size(a)` over a list.
    Size,
    /// List literal: `payload` = length; `a` = index of the first of `payload`
    /// consecutive element nodes.
    List,
    /// Vocabulary function call: `payload` = [`crate::ctx::FnId`] as i64,
    /// `a` = index of the first of `b` consecutive argument nodes.
    Call,
}

/// One arena node. `payload` meaning depends on [`Kind`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: Kind,
    pub payload: i64,
    pub a: u16,
    pub b: u16,
    pub c: u16,
}

impl Node {
    pub const fn new(kind: Kind, payload: i64, a: u16, b: u16, c: u16) -> Self {
        Node {
            kind,
            payload,
            a,
            b,
            c,
        }
    }

    pub const fn leaf_const_int(v: i64) -> Self {
        Node::new(Kind::ConstInt, v, 0, 0, 0)
    }

    pub const fn leaf_const_bool(v: bool) -> Self {
        Node::new(Kind::ConstBool, v as i64, 0, 0, 0)
    }

    pub const fn leaf_null() -> Self {
        Node::new(Kind::ConstNull, 0, 0, 0, 0)
    }

    pub const fn leaf_var(idx: u16) -> Self {
        Node::new(Kind::Var, idx as i64, 0, 0, 0)
    }

    pub const fn unary(kind: Kind, a: u16) -> Self {
        Node::new(kind, 0, a, 0, 0)
    }

    pub const fn binary(kind: Kind, a: u16, b: u16) -> Self {
        Node::new(kind, 0, a, b, 0)
    }

    pub const fn ternary(a: u16, b: u16, c: u16) -> Self {
        Node::new(Kind::Tern, 0, a, b, c)
    }
}

/// A finished program: post-order nodes + root. `eval` walks only what short
/// circuiting demands.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub nodes: &'static [Node],
    pub root: u16,
}

impl Program {
    pub fn eval(&self, ctx: &crate::ctx::IrCtx) -> crate::Val {
        crate::eval::eval_node(self.nodes, self.root, ctx)
    }

    /// Host root rule — matches `rules_cond::Cond::eval`.
    pub fn eval_bool(&self, ctx: &crate::ctx::IrCtx) -> bool {
        self.eval(ctx).as_cond_bool()
    }
}