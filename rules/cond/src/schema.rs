//! Variable schema (GUARDS.md §4.2) and the compile-time AST lint/rewrite.
//!
//! Every name list is **derived from [`crate::vocab::VOCAB`]** -- one entry
//! there is all that is needed to add a variable or function name to the lint.
//! This module only knows the CEL operators/stdlib that ride along.
//!
//! The walker:
//!
//! * rejects float literals and float-producing calls (int-only mandate),
//! * allows `kind` (alias `trigger.kind`) like any other window variable --
//!   a multi-kind entry may need per-kind clauses; restating the declared
//!   category is a style matter for review / precheck, not a compile error,
//! * rejects unknown variables, fields and functions,
//! * rewrites `owner.money` -> `owner_money`, `effect.has(x)` -> `chain_has(x)`
//!   so eval binds flat ints and two small lookup functions.

use cel::common::ast::{Expr, IdedExpr, LiteralValue};

use crate::vocab::{self, Scope, VOCAB};
use crate::CondError;

/// CEL stdlib the lint keeps (int-only: `double` and friends are denied).
/// Not condition vocabulary -- these arrive as operators or stdlib calls.
pub const CEL_STDLIB: &[&str] = &[
    "size",
    "contains",
    "startsWith",
    "endsWith",
    "has",
    "int",
    "uint",
    "string",
    "bool",
    "type",
    // macros that the parser may leave as calls when the argument is not a
    // select (`has(x)`); harmless to allow.
    "all",
    "exists",
    "exists_one",
];

/// Calls that produce or consume floats. Rejected: the int-only mandate
/// (docs/P0-FINDINGS.md:76-85, CEL does not mix Int/Float).
const FLOAT_FNS: &[&str] = &["double", "dyn"];

/// Dotted roots that expand to flat `root_field` bindings. Derived from VOCAB
/// (every dotted CEL spelling and alias).
pub fn struct_roots() -> Vec<(&'static str, Vec<&'static str>)> {
    vocab::struct_roots()
}

/// Bare window-level identifiers (not dotted). Derived from VOCAB.
pub fn window_vars() -> Vec<&'static str> {
    VOCAB
        .iter()
        .filter(|n| n.scope == Scope::Window && !n.cel.contains('.'))
        .map(|n| n.flat)
        .collect()
}

/// Bare candidate-level identifiers. Derived from VOCAB.
pub fn candidate_vars() -> Vec<&'static str> {
    VOCAB
        .iter()
        .filter(|n| n.scope == Scope::Candidate && !n.cel.contains('.'))
        .map(|n| n.flat)
        .collect()
}

/// Functions this language exposes (plus CEL operators/stdlib). Derived from
/// VOCAB plus [`CEL_STDLIB`].
pub fn functions() -> Vec<&'static str> {
    let mut fns = vocab::func_names();
    for s in CEL_STDLIB {
        if !fns.contains(s) {
            fns.push(s);
        }
    }
    fns
}

fn field_ok(root: &str, field: &str) -> bool {
    vocab::struct_roots()
        .iter()
        .any(|(r, fields)| *r == root && fields.contains(&field))
}

fn is_operator(name: &str) -> bool {
    // CEL internal operator names: `_==_`, `_+_`, `!_`, `-_`, `_?_:_`, …
    name.starts_with('_') || name.ends_with('_')
}

fn is_macro(name: &str) -> bool {
    matches!(name, "has" | "all" | "exists" | "exists_one" | "map" | "filter")
}

/// True when `name` is a known constant, a VOCAB flat identifier (including
/// the flattened `root_field` forms), or a CEL stdlib name used as an ident.
pub fn ident_known(name: &str) -> bool {
    if crate::kinds::constants().iter().any(|(n, _)| *n == name) {
        return true;
    }
    if VOCAB
        .iter()
        .any(|n| n.flat == name && !matches!(n.scope, Scope::Func { .. }))
    {
        return true;
    }
    // flattened forms written literally: owner_money, tile_owner, …
    for (root, fields) in vocab::struct_roots() {
        for f in fields {
            if name == format!("{root}_{f}") {
                return true;
            }
        }
    }
    false
}

/// Walk `expr`, enforcing the lint rules and rewriting dotted roots to flat
/// identifiers. Returns the rewritten tree.
pub fn lint_and_rewrite(expr: &IdedExpr) -> Result<IdedExpr, CondError> {
    rewrite(expr)
}

fn rewrite(e: &IdedExpr) -> Result<IdedExpr, CondError> {
    Ok(IdedExpr {
        id: e.id,
        expr: match &e.expr {
            Expr::Unspecified => Expr::Unspecified,
            Expr::Literal(lit) => {
                if matches!(lit, LiteralValue::Double(_)) {
                    return Err(CondError::FloatLiteral(lit_debug(lit)));
                }
                if matches!(lit, LiteralValue::UInt(_)) {
                    // uint is not a float, but the schema is int-only; keep
                    // the surface tight and reject it too (write a plain int).
                    return Err(CondError::FloatLiteral(format!(
                        "uint literal {} (int-only)",
                        lit_debug(lit)
                    )));
                }
                Expr::Literal(lit.clone())
            }
            Expr::Ident(name) => {
                check_ident(name)?;
                Expr::Ident(name.clone())
            }
            Expr::Select(sel) => {
                if let Expr::Ident(root) = &sel.operand.expr {
                    let dotted = format!("{root}.{}", sel.field);
                    // VOCAB lookup covers `trigger.kind` (alias of `kind`),
                    // `chain.count` (alias of `effect.count`), and the plain
                    // `root.field` spellings -- rewrite to the table's flat id.
                    if let Some(n) = vocab::by_cel(&dotted) {
                        if matches!(n.scope, Scope::Func { .. }) {
                            return Err(CondError::UnknownVar(dotted));
                        }
                        // Presence tests on schema fields are useless (they
                        // always exist); reject rather than silently
                        // mis-evaluate after the flat rewrite.
                        if sel.test {
                            return Err(CondError::UnknownVar(format!(
                                "has({dotted}): schema fields always exist; test the value instead"
                            )));
                        }
                        return Ok(IdedExpr {
                            id: e.id,
                            expr: Expr::Ident(n.flat.into()),
                        });
                    }
                    if root == "trigger" {
                        return Err(CondError::UnknownVar(format!("trigger.{}", sel.field)));
                    }
                    if field_ok(root, &sel.field) {
                        if sel.test {
                            return Err(CondError::UnknownVar(format!(
                                "has({root}.{}): schema fields always exist; test the value instead",
                                sel.field
                            )));
                        }
                        return Ok(IdedExpr {
                            id: e.id,
                            expr: Expr::Ident(format!("{root}_{}", sel.field)),
                        });
                    }
                    if vocab::struct_roots().iter().any(|(r, _)| *r == root) {
                        return Err(CondError::UnknownVar(format!("{root}.{}", sel.field)));
                    }
                }
                // Select on a non-ident operand (e.g. `money(owner).x`) --
                // recurse and keep.
                Expr::Select(cel::common::ast::SelectExpr {
                    operand: Box::new(rewrite(&sel.operand)?),
                    field: sel.field.clone(),
                    test: sel.test,
                })
            }
            Expr::Call(call) => {
                let name = call.func_name.as_str();
                if FLOAT_FNS.contains(&name) {
                    return Err(CondError::FloatOp(name.to_string()));
                }
                // `effect.has(x)` / `chain.has(x)` -> `chain_has(x)`
                // `effect.hits(x)` / `chain.hits(x)` -> `chain_hits(x)`
                if let Some(target) = &call.target {
                    if let Expr::Ident(root) = &target.expr {
                        if (root == "effect" || root == "chain")
                            && (name == "has" || name == "hits")
                        {
                            if call.args.len() != 1 {
                                return Err(CondError::UnknownVar(format!(
                                    "effect.{name} takes exactly 1 argument"
                                )));
                            }
                            let arg = rewrite(&call.args[0])?;
                            return Ok(IdedExpr {
                                id: e.id,
                                expr: Expr::Call(cel::common::ast::CallExpr {
                                    func_name: format!("chain_{name}"),
                                    target: None,
                                    args: vec![arg],
                                }),
                            });
                        }
                    }
                }
                if !is_operator(name) && !is_macro(name) && !functions().contains(&name) {
                    // A member call on a rewritten/unknown target still has a
                    // func_name we must know.
                    return Err(CondError::UnknownFn(name.to_string()));
                }
                let target = match &call.target {
                    Some(t) => Some(Box::new(rewrite(t)?)),
                    None => None,
                };
                let mut args = Vec::with_capacity(call.args.len());
                for a in &call.args {
                    args.push(rewrite(a)?);
                }
                Expr::Call(cel::common::ast::CallExpr {
                    func_name: call.func_name.clone(),
                    target,
                    args,
                })
            }
            Expr::List(list) => {
                let mut elements = Vec::with_capacity(list.elements.len());
                for el in &list.elements {
                    elements.push(rewrite(el)?);
                }
                Expr::List(cel::common::ast::ListExpr {
                    elements,
                    optional_indices: list.optional_indices.clone(),
                })
            }
            Expr::Map(map) => {
                let mut entries = Vec::with_capacity(map.entries.len());
                for ent in &map.entries {
                    entries.push(cel::common::ast::IdedEntryExpr {
                        id: ent.id,
                        expr: match &ent.expr {
                            cel::common::ast::EntryExpr::MapEntry(me) => {
                                cel::common::ast::EntryExpr::MapEntry(cel::common::ast::MapEntryExpr {
                                    key: rewrite(&me.key)?,
                                    value: rewrite(&me.value)?,
                                    optional: me.optional,
                                })
                            }
                            cel::common::ast::EntryExpr::StructField(sf) => {
                                cel::common::ast::EntryExpr::StructField(
                                    cel::common::ast::StructFieldExpr {
                                        field: sf.field.clone(),
                                        value: rewrite(&sf.value)?,
                                        optional: sf.optional,
                                    },
                                )
                            }
                        },
                    });
                }
                Expr::Map(cel::common::ast::MapExpr { entries })
            }
            Expr::Struct(st) => {
                // Struct literals need a registered type; the schema has none.
                return Err(CondError::UnknownVar(format!(
                    "struct literal {} is not in the schema",
                    st.type_name
                )));
            }
            Expr::Comprehension(_) => {
                // Macros (`all`, `exists`, `map`, `filter`) expand here. The
                // residual vocabulary is bigger than the guards need and the
                // int-only lint cannot see inside the accu vars reliably; keep
                // G1 to scalar predicates. `effect.has` covers the list scans.
                return Err(CondError::UnknownFn(
                    "comprehension macros (all/exists/map/filter) are not in the G1 schema".into(),
                ));
            }
        },
    })
}

fn check_ident(name: &str) -> Result<(), CondError> {
    if ident_known(name) {
        return Ok(());
    }
    Err(CondError::UnknownVar(name.to_string()))
}

fn lit_debug(lit: &LiteralValue) -> String {
    match lit {
        LiteralValue::Double(d) => format!("{}", d.inner()),
        LiteralValue::UInt(u) => format!("{}u", u.inner()),
        LiteralValue::Int(i) => format!("{}", i.inner()),
        _ => format!("{lit:?}"),
    }
}