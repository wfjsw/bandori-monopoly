//! Lower a linted/rewritten CEL AST (`Cond::expr`) to a heap-free
//! `rules-cond-ir` [`Program`].
//!
//! This is the **translator** whose correctness is the "check 2" deliverable:
//! `cel_eval(s, ctx) == ir_eval(lower(compile(s)), ctx)` for every guard string
//! `s` and every context `ctx`. Established by
//! (a) the written reference semantics + differential tests,
//! (b) Kani proofs per operator,
//! (c) per-guard consistency (`lower(compile(s)) == embedded IR`),
//! (d) property tests over every real guard.
//!
//! The lowerer runs on the host only (feature `lower`); production wasm and
//! the live host path never see it.

use cel::common::ast::operators;
use cel::common::ast::{Expr, IdedExpr, LiteralValue};
use rules_cond_ir::nodes::{Kind, Node};
use rules_cond_ir::vars::by_flat;
use rules_cond_ir::{ctx::id_of, ctx::FnId};

use crate::ext::{self, LowerAs};
use crate::kinds;
use crate::vocab::{self, Scope};
use crate::CondError;

/// Lower a rewritten AST to a [`Program`]. `nodes` is post-order; the root is
/// the last node.
pub fn lower(expr: &IdedExpr) -> Result<OwnedProgram, CondError> {
    lower_with_expr(expr, &ext::vocab())
}

/// Hermetic lower against an explicit [`ext::Vocab`].
pub fn lower_with(cond: &crate::Cond, v: &ext::Vocab) -> Result<OwnedProgram, CondError> {
    lower_with_expr(cond.expr(), v)
}

fn lower_with_expr(expr: &IdedExpr, v: &ext::Vocab) -> Result<OwnedProgram, CondError> {
    let mut nodes: Vec<Node> = Vec::new();
    let root = lower_expr(expr, &mut nodes, v)?;
    Ok(OwnedProgram { nodes, root })
}

/// Owned program (generation-time). The Kani side freezes this into `&'static`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedProgram {
    pub nodes: Vec<Node>,
    pub root: u16,
}

impl OwnedProgram {
    pub fn eval(&self, ctx: &rules_cond_ir::IrCtx) -> rules_cond_ir::Val {
        rules_cond_ir::eval_node(&self.nodes, self.root, ctx)
    }

    pub fn eval_bool(&self, ctx: &rules_cond_ir::IrCtx) -> bool {
        self.eval(ctx).as_cond_bool()
    }
}

fn push(nodes: &mut Vec<Node>, n: Node) -> Result<u16, CondError> {
    if nodes.len() >= u16::MAX as usize {
        return Err(CondError::Parse("condition IR too large".into()));
    }
    nodes.push(n);
    Ok((nodes.len() - 1) as u16)
}

fn lower_expr(e: &IdedExpr, nodes: &mut Vec<Node>, v: &ext::Vocab) -> Result<u16, CondError> {
    match &e.expr {
        Expr::Unspecified => Err(CondError::Parse("unspecified expression".into())),
        Expr::Literal(lit) => lower_lit(lit, nodes),
        Expr::Ident(name) => lower_ident(name, nodes, v),
        Expr::Call(call) => lower_call(&call.func_name, &call.args, nodes, v),
        Expr::List(list) => {
            if list.elements.len() > 8 {
                return Err(CondError::Parse("list literal longer than 8".into()));
            }
            let first = nodes.len() as u16;
            for el in &list.elements {
                lower_expr(el, nodes, v)?;
            }
            push(
                nodes,
                Node::new(Kind::List, list.elements.len() as i64, first, 0, 0),
            )
        }
        Expr::Select(sel) => {
            // After `lint_and_rewrite` a schema select is already flattened to
            // `Ident`. Anything left is unsupported.
            Err(CondError::UnknownVar(format!(
                "unsupported select after rewrite: {}",
                sel.field
            )))
        }
        Expr::Map(_) | Expr::Struct(_) | Expr::Comprehension(_) => Err(CondError::UnknownVar(
            "map/struct/comprehension nodes are not in the IR closed set".into(),
        )),
    }
}

fn lower_lit(lit: &LiteralValue, nodes: &mut Vec<Node>) -> Result<u16, CondError> {
    match lit {
        LiteralValue::Boolean(b) => push(nodes, Node::leaf_const_bool(*b.inner())),
        LiteralValue::Int(i) => push(nodes, Node::leaf_const_int(*i.inner())),
        LiteralValue::Null => push(nodes, Node::leaf_null()),
        // String literals only appear as function arguments and are baked to
        // `id_of` at lower time (the same hash the view's accessors return).
        LiteralValue::String(s) => push(nodes, Node::leaf_const_int(id_of(s.inner()))),
        LiteralValue::UInt(u) => push(nodes, Node::leaf_const_int(*u.inner() as i64)),
        LiteralValue::Double(_) | LiteralValue::Bytes(_) => Err(CondError::FloatLiteral(
            "float/bytes literal in IR lower".into(),
        )),
    }
}

fn lower_ident(name: &str, nodes: &mut Vec<Node>, v: &ext::Vocab) -> Result<u16, CondError> {
    // OwnerState scalar sugar: `owner.f` → `Call(State, [Var(Owner),
    // Const(id_of(key))])` -- the same node shape `state(owner, 'key')`
    // produces.
    if let Some(n) = v.by_flat(name) {
        if let LowerAs::OwnerState { fid, key } = n.lower_as {
            let owner_idx = by_flat("owner").map(|(idx, _)| idx).unwrap_or(0);
            let first = nodes.len() as u16;
            push(nodes, Node::leaf_var(owner_idx))?;
            push(nodes, Node::leaf_const_int(id_of(key)))?;
            return push(nodes, Node::new(Kind::Call, fid as i64, first, 2, 0));
        }
    }
    if let Some((idx, _)) = by_flat(name) {
        return push(nodes, Node::leaf_var(idx));
    }
    // Kind spellings bake to `id_of(spelling)` -- the same hash a `VarTy::Str`
    // scalar holds and the same hash a string literal bakes to. Eq compares
    // hashes; no string pool.
    if let Some((_, s)) = v.str_constants().find(|(n, _)| *n == name) {
        return push(nodes, Node::leaf_const_int(id_of(s)));
    }
    // Move-kind ints stay raw i64.
    if let Some((_, val)) = kinds::int_constants().iter().find(|(n, _)| *n == name) {
        return push(nodes, Node::leaf_const_int(*val));
    }
    Err(CondError::UnknownVar(name.to_string()))
}

fn lower_call(name: &str, args: &[IdedExpr], nodes: &mut Vec<Node>, v: &ext::Vocab) -> Result<u16, CondError> {
    // Operator names (post-parse).
    match name {
        operators::ADD => return bin(Kind::Add, args, nodes, v),
        operators::SUBSTRACT => return bin(Kind::Sub, args, nodes, v),
        operators::MULTIPLY => return bin(Kind::Mul, args, nodes, v),
        operators::DIVIDE => return bin(Kind::Div, args, nodes, v),
        operators::MODULO => return bin(Kind::Rem, args, nodes, v),
        operators::EQUALS => return bin(Kind::Eq, args, nodes, v),
        operators::NOT_EQUALS => return bin(Kind::Ne, args, nodes, v),
        operators::LESS => return bin(Kind::Lt, args, nodes, v),
        operators::LESS_EQUALS => return bin(Kind::Le, args, nodes, v),
        operators::GREATER => return bin(Kind::Gt, args, nodes, v),
        operators::GREATER_EQUALS => return bin(Kind::Ge, args, nodes, v),
        operators::LOGICAL_AND => return bin(Kind::And, args, nodes, v),
        operators::LOGICAL_OR => return bin(Kind::Or, args, nodes, v),
        operators::NEGATE => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes, v)?;
            return push(nodes, Node::unary(Kind::Neg, a));
        }
        operators::LOGICAL_NOT => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes, v)?;
            return push(nodes, Node::unary(Kind::Not, a));
        }
        operators::CONDITIONAL => {
            require_arity(name, args, 3)?;
            let a = lower_expr(&args[0], nodes, v)?;
            let b = lower_expr(&args[1], nodes, v)?;
            let c = lower_expr(&args[2], nodes, v)?;
            return push(nodes, Node::ternary(a, b, c));
        }
        operators::IN => return bin(Kind::In, args, nodes, v),
        // `@not_strictly_false(x)` is the parser's booleanization wrapper; it
        // is the identity on our bool-only lattice.
        "@not_strictly_false" => {
            require_arity(name, args, 1)?;
            return lower_expr(&args[0], nodes, v);
        }
        "size" => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes, v)?;
            return push(nodes, Node::unary(Kind::Size, a));
        }
        _ => {}
    }

    // Vocabulary functions (flat names after rewrite; dotted already lowered).
    let n = v
        .by_cel(name)
        .or_else(|| v.by_flat(name))
        .filter(|n| matches!(n.scope, Scope::Func { .. }))
        .ok_or_else(|| CondError::UnknownFn(name.to_string()))?;
    let user_arity = match n.scope {
        Scope::Func { arity, .. } => arity,
        _ => unreachable!(),
    };
    if args.len() != user_arity {
        return Err(CondError::UnknownVar(format!(
            "{name} takes exactly {user_arity} argument(s)"
        )));
    }
    match n.lower_as {
        LowerAs::Direct => {
            let fid = direct_fn_id(n.flat)
                .ok_or_else(|| CondError::UnknownFn(name.to_string()))?;
            let first = nodes.len() as u16;
            for a in args {
                lower_expr(a, nodes, v)?;
            }
            push(
                nodes,
                Node::new(Kind::Call, fid as i64, first, user_arity as u16, 0),
            )
        }
        LowerAs::Call { fid, bake } => {
            // `Call(fid, [user_args…, Const(id_of(bake))…])` — bake consts
            // occupy the trailing key slots of the target (`state(p, key)`).
            // `cand_has(key, member)` has the key in slot 0, so its bake
            // leads: `Call(CandHas, [id_of(key), member…])`.
            let first = nodes.len() as u16;
            if fid == FnId::CandHas {
                for b in bake {
                    push(nodes, Node::leaf_const_int(id_of(b)))?;
                }
                for a in args {
                    lower_expr(a, nodes, v)?;
                }
            } else {
                for a in args {
                    lower_expr(a, nodes, v)?;
                }
                for b in bake {
                    push(nodes, Node::leaf_const_int(id_of(b)))?;
                }
            }
            let arity = user_arity + bake.len();
            push(nodes, Node::new(Kind::Call, fid as i64, first, arity as u16, 0))
        }
        LowerAs::CallEq { fid, bake } => {
            // `Eq(Call(fid, [user_args[..n-1]…, Const(id_of(bake))…]),
            //    user_args[n-1])`.
            let (last, head) = args
                .split_last()
                .ok_or_else(|| CondError::UnknownVar(format!("{name} takes arguments")))?;
            let call_first = nodes.len() as u16;
            for a in head {
                lower_expr(a, nodes, v)?;
            }
            for b in bake {
                push(nodes, Node::leaf_const_int(id_of(b)))?;
            }
            let call_arity = head.len() + bake.len();
            let call = push(
                nodes,
                Node::new(Kind::Call, fid as i64, call_first, call_arity as u16, 0),
            )?;
            let rhs = lower_expr(last, nodes, v)?;
            push(nodes, Node::binary(Kind::Eq, call, rhs))
        }
        LowerAs::OwnerState { .. } => Err(CondError::UnknownFn(name.to_string())),
    }
}

fn bin(kind: Kind, args: &[IdedExpr], nodes: &mut Vec<Node>, v: &ext::Vocab) -> Result<u16, CondError> {
    require_arity("binop", args, 2)?;
    let a = lower_expr(&args[0], nodes, v)?;
    let b = lower_expr(&args[1], nodes, v)?;
    push(nodes, Node::binary(kind, a, b))
}

fn require_arity(name: &str, args: &[IdedExpr], n: usize) -> Result<(), CondError> {
    if args.len() != n {
        return Err(CondError::UnknownVar(format!(
            "{name} takes exactly {n} argument(s)"
        )));
    }
    Ok(())
}

/// Kernel `Direct` function ids. Sugar names resolve through `LowerAs`'s `fid`.
fn direct_fn_id(flat: &str) -> Option<FnId> {
    Some(match flat {
        "slot" => FnId::Slot,
        "tok" => FnId::Tok,
        "move_tag" => FnId::MoveTag,
        "card_counter" => FnId::CardCounter,
        "counter_is" => FnId::CounterIs,
        "tile_named" => FnId::TileNamed,
        "tile_is" => FnId::TileIs,
        "tile_attr" => FnId::TileAttr,
        "tile_in" => FnId::TileIn,
        "money" => FnId::Money,
        "hand" => FnId::Hand,
        "pos" => FnId::Pos,
        "tiles" => FnId::Tiles,
        "neighbor" => FnId::Neighbor,
        "chain_has" => FnId::ChainHas,
        "chain_hits" => FnId::ChainHits,
        "gains_this_turn" => FnId::GainsThisTurn,
        "targeted_count" => FnId::TargetedCount,
        "repeated_digits" => FnId::RepeatedDigits,
        "dist" => FnId::Dist,
        "players_on" => FnId::PlayersOn,
        "next_dist" => FnId::NextDist,
        "others_within" => FnId::OthersWithin,
        "owned_within" => FnId::OwnedWithin,
        "on_path" => FnId::OnPath,
        "between" => FnId::Between,
        "state" => FnId::State,
        "cand_has" => FnId::CandHas,
        _ => return None,
    })
}

/// Compile a source string all the way to an IR program (host-only convenience).
pub fn lower_src(src: &str) -> Result<OwnedProgram, CondError> {
    lower_with_src(src, &ext::vocab())
}

/// Hermetic source → IR against an explicit [`ext::Vocab`].
pub fn lower_with_src(src: &str, v: &ext::Vocab) -> Result<OwnedProgram, CondError> {
    let cond = crate::compile::compile_with(src, v)?;
    lower_with(&cond, v)
}

/// Collect every `On::*` condition string under `root` (a `rules/` subdirectory).
/// Used by the generator and the consistency / differential tests so all three
/// see the same inventory.
pub fn inventory_guards(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_dir(root, &mut out);
    out.push("actor == owner".to_string()); // `pre::MINE`
    out.sort();
    out.dedup();
    out
}

/// Blank out `//` line comments and `/* */` block comments outside of string
/// literals. Comment bytes become spaces; every other byte is kept verbatim
/// (so UTF-8 spellings like `Pastel✽Palettes` survive).
fn strip_rust_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                // Keep string literals (and their escapes) verbatim.
                out.push(b'"');
                i += 1;
                while i < b.len() {
                    out.push(b[i]);
                    if b[i] == b'\\' && i + 1 < b.len() {
                        out.push(b[i + 1]);
                        i += 2;
                        continue;
                    }
                    if b[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'r' if i + 1 < b.len() && (b[i + 1] == b'"' || b[i + 1] == b'#') => {
                // Raw string: r"..." / r#"..."#.
                let mut h = 0;
                let mut j = i + 1;
                while j < b.len() && b[j] == b'#' {
                    h += 1;
                    j += 1;
                }
                if j < b.len() && b[j] == b'"' {
                    out.extend_from_slice(&b[i..=j]);
                    let mut k = j + 1;
                    while k < b.len() {
                        out.push(b[k]);
                        if b[k] == b'"' {
                            let mut ok = true;
                            for w in 1..=h {
                                if k + w >= b.len() || b[k + w] != b'#' {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok {
                                for _ in 0..h {
                                    out.push(b'#');
                                }
                                k += h + 1;
                                break;
                            }
                        }
                        k += 1;
                    }
                    i = k;
                    continue;
                }
                out.push(b'r');
                i += 1;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    out.push(b' ');
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                out.push(b' ');
                out.push(b' ');
                i += 2;
                while i < b.len() {
                    if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                        out.push(b' ');
                        out.push(b' ');
                        i += 2;
                        break;
                    }
                    out.push(if b[i] == b'\n' { b'\n' } else { b' ' });
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    // Safe: every non-comment byte is copied verbatim from a UTF-8 source, and
    // comments are replaced by ASCII spaces (also valid UTF-8).
    String::from_utf8(out).expect("strip_rust_comments keeps UTF-8")
}

fn collect_dir(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_dir(&p, out);
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        // Comments are not registrations. A `// ... On::Message("place")` note
        // otherwise lands its quoted word in the inventory as a fake guard.
        let text = strip_rust_comments(&text);
        // Phase 3 typed DSL: `On::Filter(Filter::Name(pre, …))` /
        // `On::Action(Action::Name(pre, …))` (and ruleset `Filters::` /
        // `Actions::` namespaces). Retired `On::FilterFlag` / `On::FilterValue`
        // are not recognized.
        for kind in [
            "Play",
            "Action",
            "Filter",
            "Counteract",
            "Settle",
            "Mortgage",
            "Redeem",
            "Buy",
            "Build",
            "Message",
        ] {
            let pat = format!("On::{kind}(");
            let mut rest = text.as_str();
            while let Some(i) = rest.find(&pat) {
                rest = &rest[i + pat.len()..];
                if let Some(s) = on_pre_string(rest, kind == "Filter" || kind == "Action") {
                    if !s.is_empty() {
                        out.push(s);
                    }
                }
            }
        }
    }
}

/// After `On::Kind(`, the `pre` argument.
///
/// `On::Filter` / `On::Action` take one nested ctor
/// (`Ns::Name(pre, …)` / `Ns::Name(pre, guard, body)`); `pre` is ctor
/// arg 1 inside the inner parentheses. The namespace is whatever the ruleset
/// named it (`Filter`/`Action` engine, `Filters`/`Actions`/… downstream).
/// Everything else keeps the flat first-arg shape (no `&[kinds]` slice since
/// the multi-kind lists are gone).
fn on_pre_string(after: &str, nested: bool) -> Option<String> {
    if nested {
        let (open, _end) = find_ctor_call(after)?;
        let inner = extract_parens(after, open)?;
        let first = first_top_arg(inner)?;
        return pre_string(first);
    }
    // Flat: skip an optional `&[...]` leftover, then first string / `pre::MINE`.
    pre_string(after)
}

/// `Path::Ident(` at the head of `s`. Returns the index of `(`.
fn find_ctor_call(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut name_end = 0;
    loop {
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        if i == start {
            return None;
        }
        name_end = i;
        let save = i;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == b':' && b[i + 1] == b':' {
            i += 2;
            continue;
        }
        i = save;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'(' {
            return Some((i, name_end));
        }
        return None;
    }
}

/// Text inside the balanced parens starting at `open`, excluding the parens.
fn extract_parens(s: &str, open: usize) -> Option<&str> {
    let b = s.as_bytes();
    if open >= b.len() || b[open] != b'(' {
        return None;
    }
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        if b[j] == b'(' {
            depth += 1;
        } else if b[j] == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(&s[open + 1..j]);
            }
        }
        j += 1;
    }
    None
}

/// First top-level comma-separated argument (nested `()`, `[]`, `{}`).
fn first_top_arg(inner: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (k, c) in inner.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let first = inner[..k].trim();
                if !first.is_empty() {
                    return Some(first);
                }
                // Leading comma / empty first arg: keep scanning.
            }
            _ => {}
        }
    }
    let last = inner.trim();
    if last.is_empty() {
        None
    } else {
        Some(last)
    }
}

/// The `pre` argument: a string literal, `pre::MINE`, or a known guard const.
/// Only the first argument is read — a later `key!("…")` / `name` is not a
/// guard.
fn pre_string(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    // Optional `&[ ... ]` kind / name slice (legacy shape).
    if i < b.len() && b[i] == b'&' {
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'[' {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'[' {
                    depth += 1;
                } else if b[i] == b']' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
        }
    }
    let rest = &s[i..];
    let rest = rest
        .trim_start()
        .strip_prefix(',')
        .map(|r| r.trim_start())
        .unwrap_or(rest.trim_start());
    // `pre::MINE` sugar (any path prefix: `pre::MINE`, `card_sdk::pre::MINE`).
    // Known guard-const path references (content `guards::OWN_START` paths)
    // — resolve through the installed extension.
    let path_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == ':' || c == '_'))
        .unwrap_or(rest.len());
    let path = &rest[..path_end];
    if path == "pre::MINE" || path.ends_with("::pre::MINE") {
        return Some("actor == owner".to_string());
    }
    if let Some(lit) = ext::ext().guard_consts.iter().find(|(p, _)| *p == path).map(|(_, lit)| *lit)
    {
        return Some(lit.to_string());
    }
    // A string literal in first-argument position. Stop at a top-level `,`
    // so a later `key!("…")` / message name is never mistaken for the guard.
    if !rest.starts_with('"') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut j = 1;
    while j < bytes.len() && bytes[j] != b'"' {
        if bytes[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    let raw = &rest[1..j];
    Some(unescape_cel_src(raw))
}

/// Rust string-literal body → the CEL source the parser sees. Handles `\"`,
/// `\\`, and `\` + newline line-continuations (which Rust eats along with the
/// next line's leading whitespace). Non-ASCII bytes pass through verbatim.
fn unescape_cel_src(raw: &str) -> String {
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            match b[i + 1] {
                b'\n' => {
                    i += 2;
                    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
                        i += 1;
                    }
                }
                b'"' => {
                    out.push(b'"');
                    i += 2;
                }
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                }
                _ => {
                    out.push(b'\\');
                    i += 1;
                }
            }
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("unescape_cel_src keeps UTF-8")
}

/// Every kernel `Direct` FnId, for the vocab-sync test. Sugar names carry
/// their target in `LowerAs::fid` instead.
pub fn all_fn_ids() -> &'static [(&'static str, FnId)] {
    use FnId::*;
    &[
        ("slot", Slot),
        ("tok", Tok),
        ("move_tag", MoveTag),
        ("card_counter", CardCounter),
        ("counter_is", CounterIs),
        ("tile_named", TileNamed),
        ("tile_is", TileIs),
        ("tile_attr", TileAttr),
        ("tile_in", TileIn),
        ("money", Money),
        ("hand", Hand),
        ("pos", Pos),
        ("tiles", Tiles),
        ("neighbor", Neighbor),
        ("chain_has", ChainHas),
        ("chain_hits", ChainHits),
        ("gains_this_turn", GainsThisTurn),
        ("targeted_count", TargetedCount),
        ("repeated_digits", RepeatedDigits),
        ("dist", Dist),
        ("players_on", PlayersOn),
        ("next_dist", NextDist),
        ("others_within", OthersWithin),
        ("owned_within", OwnedWithin),
        ("on_path", OnPath),
        ("between", Between),
        ("state", State),
        ("cand_has", CandHas),
    ]
}

/// Re-export for the vocab-sync test.
pub fn vocab_scalars() -> impl Iterator<Item = (&'static str, Scope)> {
    ext::vocab()
        .names()
        .filter(|n| !matches!(n.scope, Scope::Func { .. }))
        .map(|n| (n.flat, n.scope))
}
