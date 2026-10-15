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

use cel::common::ast::{Expr, IdedExpr, LiteralValue};
use cel::common::ast::operators;
use rules_cond_ir::nodes::{Kind, Node};
use rules_cond_ir::vars::by_flat;
use rules_cond_ir::{ctx::FnId, ctx::id_of};

use crate::kinds;
use crate::vocab::{self, Scope};
use crate::CondError;

/// Lower a rewritten AST to a [`Program`]. `nodes` is post-order; the root is
/// the last node.
pub fn lower(expr: &IdedExpr) -> Result<OwnedProgram, CondError> {
    let mut nodes: Vec<Node> = Vec::new();
    let root = lower_expr(expr, &mut nodes)?;
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

fn lower_expr(e: &IdedExpr, nodes: &mut Vec<Node>) -> Result<u16, CondError> {
    match &e.expr {
        Expr::Unspecified => Err(CondError::Parse("unspecified expression".into())),
        Expr::Literal(lit) => lower_lit(lit, nodes),
        Expr::Ident(name) => lower_ident(name, nodes),
        Expr::Call(call) => lower_call(&call.func_name, &call.args, nodes),
        Expr::List(list) => {
            if list.elements.len() > 8 {
                return Err(CondError::Parse("list literal longer than 8".into()));
            }
            let first = nodes.len() as u16;
            for el in &list.elements {
                lower_expr(el, nodes)?;
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
        LiteralValue::Boolean(b) => {
            push(nodes, Node::leaf_const_bool(*b.inner()))
        }
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

fn lower_ident(name: &str, nodes: &mut Vec<Node>) -> Result<u16, CondError> {
    if let Some((idx, _)) = by_flat(name) {
        return push(nodes, Node::leaf_var(idx));
    }
    if let Some((_, v)) = kinds::constants().iter().find(|(n, _)| *n == name) {
        return push(nodes, Node::leaf_const_int(*v));
    }
    Err(CondError::UnknownVar(name.to_string()))
}

fn lower_call(name: &str, args: &[IdedExpr], nodes: &mut Vec<Node>) -> Result<u16, CondError> {
    // Operator names (post-parse).
    match name {
        operators::ADD => return bin(Kind::Add, args, nodes),
        operators::SUBSTRACT => return bin(Kind::Sub, args, nodes),
        operators::MULTIPLY => return bin(Kind::Mul, args, nodes),
        operators::DIVIDE => return bin(Kind::Div, args, nodes),
        operators::MODULO => return bin(Kind::Rem, args, nodes),
        operators::EQUALS => return bin(Kind::Eq, args, nodes),
        operators::NOT_EQUALS => return bin(Kind::Ne, args, nodes),
        operators::LESS => return bin(Kind::Lt, args, nodes),
        operators::LESS_EQUALS => return bin(Kind::Le, args, nodes),
        operators::GREATER => return bin(Kind::Gt, args, nodes),
        operators::GREATER_EQUALS => return bin(Kind::Ge, args, nodes),
        operators::LOGICAL_AND => return bin(Kind::And, args, nodes),
        operators::LOGICAL_OR => return bin(Kind::Or, args, nodes),
        operators::NEGATE => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes)?;
            return push(nodes, Node::unary(Kind::Neg, a));
        }
        operators::LOGICAL_NOT => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes)?;
            return push(nodes, Node::unary(Kind::Not, a));
        }
        operators::CONDITIONAL => {
            require_arity(name, args, 3)?;
            let a = lower_expr(&args[0], nodes)?;
            let b = lower_expr(&args[1], nodes)?;
            let c = lower_expr(&args[2], nodes)?;
            return push(nodes, Node::ternary(a, b, c));
        }
        operators::IN => return bin(Kind::In, args, nodes),
        // `@not_strictly_false(x)` is the parser's booleanization wrapper; it
        // is the identity on our bool-only lattice.
        "@not_strictly_false" => {
            require_arity(name, args, 1)?;
            return lower_expr(&args[0], nodes);
        }
        "size" => {
            require_arity(name, args, 1)?;
            let a = lower_expr(&args[0], nodes)?;
            return push(nodes, Node::unary(Kind::Size, a));
        }
        _ => {}
    }

    // Vocabulary functions (flat names after rewrite; dotted already lowered).
    let fid = fn_id_of(name).ok_or_else(|| CondError::UnknownFn(name.to_string()))?;
    let arity = fn_arity(fid);
    if args.len() != arity {
        return Err(CondError::UnknownVar(format!(
            "{name} takes exactly {arity} argument(s)"
        )));
    }
    let first = nodes.len() as u16;
    for a in args {
        lower_expr(a, nodes)?;
    }
    push(
        nodes,
        Node::new(Kind::Call, fid as i64, first, arity as u16, 0),
    )
}

fn bin(kind: Kind, args: &[IdedExpr], nodes: &mut Vec<Node>) -> Result<u16, CondError> {
    require_arity("binop", args, 2)?;
    let a = lower_expr(&args[0], nodes)?;
    let b = lower_expr(&args[1], nodes)?;
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

fn fn_id_of(name: &str) -> Option<FnId> {
    // Accept both the CEL spelling and the flat id (post-rewrite).
    let flat = vocab::by_cel(name).map(|n| n.flat).unwrap_or(name);
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
        "charge" => FnId::Fire,
        "crystals" => FnId::Crystals,
        "hand" => FnId::Hand,
        "pos" => FnId::Pos,
        "persona" => FnId::Character,
        "group" => FnId::Band,
        "tiles" => FnId::Tiles,
        "persona_is" => FnId::CharacterIs,
        "group_is" => FnId::BandIs,
        "blocked" => FnId::Blocked,
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
        _ => return None,
    })
}

fn fn_arity(fid: FnId) -> usize {
    use FnId::*;
    match fid {
        Slot | Tok | MoveTag | CardCounter | CounterIs | TileNamed | Money | Fire | Crystals | Hand | Pos | Character | Band
        | Tiles | Blocked | ChainHas | ChainHits | GainsThisTurn | TargetedCount
        | RepeatedDigits | Between => 1,
        CharacterIs | BandIs | Neighbor | Dist | PlayersOn | NextDist | OthersWithin
        | OwnedWithin | OnPath | TileIs | TileAttr | State => 2,
        TileIn => 3,
    }
}

/// Compile a source string all the way to an IR program (host-only convenience).
pub fn lower_src(src: &str) -> Result<OwnedProgram, CondError> {
    let cond = crate::compile(src)?;
    lower(cond.expr())
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
        for kind in [
            "Play",
            "Hook",
            "Counteract",
            "Gate",
            "RollPlan",
            "AtEnd",
            "Settle",
            "Message",
        ] {
            let pat = format!("On::{kind}(");
            let mut rest = text.as_str();
            while let Some(i) = rest.find(&pat) {
                rest = &rest[i + pat.len()..];
                if let Some(s) = pre_string(rest) {
                    if !s.is_empty() {
                        out.push(s);
                    }
                }
            }
        }
    }
}

/// After `On::Kind(`, the `pre` argument: skip an optional `&[...]` kind/name
/// slice, then the first string literal (or `pre::MINE`).
fn pre_string(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    // Optional `&[ ... ]` kind / name slice.
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
    // `pre::MINE` sugar.
    let rest = &s[i..];
    if rest.trim_start().starts_with("pre::MINE") {
        return Some("actor == owner".to_string());
    }
    // First string literal (handles `\"` escapes).
    let bytes = rest.as_bytes();
    let mut j = 0;
    while j < bytes.len() && bytes[j] != b'"' {
        if bytes[j] == b';' {
            return None;
        }
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    j += 1;
    let start = j;
    while j < bytes.len() && bytes[j] != b'"' {
        if bytes[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    if j > bytes.len() {
        return None;
    }
    // Unescape `\"` and `\\` for the source text we compile.
    let raw = &rest[start..j];
    Some(raw.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Every FnId, for the vocab-sync test.
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
        ("charge", Fire),
        ("crystals", Crystals),
        ("hand", Hand),
        ("pos", Pos),
        ("persona", Character),
        ("group", Band),
        ("tiles", Tiles),
        ("persona_is", CharacterIs),
        ("group_is", BandIs),
        ("blocked", Blocked),
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
    ]
}

/// Re-export for the vocab-sync test.
pub fn vocab_scalars() -> impl Iterator<Item = (&'static str, Scope)> {
    vocab::VOCAB
        .iter()
        .filter(|n| !matches!(n.scope, Scope::Func { .. }))
        .map(|n| (n.flat, n.scope))
}