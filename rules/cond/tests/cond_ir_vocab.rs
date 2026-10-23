//! The IR variable table is checked against `rules-cond`'s `VOCAB`.
//! Adding a VOCAB scalar without extending `rules-cond-ir::vars` fails here.

use rules_cond::vocab::{Scope, Ty, VOCAB};
use rules_cond_ir::vars::{VarTy, SCALAR_COUNT, VARS};

#[test]
fn ir_vars_match_vocab() {
    let scalars: Vec<_> = VOCAB
        .iter()
        .filter(|n| !matches!(n.scope, Scope::Func { .. }))
        .collect();
    assert_eq!(
        scalars.len(),
        SCALAR_COUNT,
        "VOCAB scalar count != IR SCALAR_COUNT"
    );
    assert_eq!(VARS.len(), SCALAR_COUNT);
    for (i, n) in scalars.iter().enumerate() {
        let (id, flat, ty) = VARS[i];
        assert_eq!(
            id as usize, i,
            "VARS[{i}] id out of order (expected index {i})"
        );
        assert_eq!(flat, n.flat, "VARS[{i}] flat name mismatch");
        let expect = match n.ty {
            Ty::Int => VarTy::Int,
            Ty::Bool => VarTy::Bool,
            Ty::OptInt => VarTy::OptInt,
            Ty::Str => VarTy::Str,
        };
        assert_eq!(ty, expect, "VARS[{i}] ({flat}) type mismatch");
    }
}

#[test]
fn ir_fn_ids_match_vocab_funcs() {
    use rules_cond::lower::all_fn_ids;
    let funcs: Vec<_> = VOCAB
        .iter()
        .filter(|n| matches!(n.scope, Scope::Func { .. }))
        .collect();
    let ids = all_fn_ids();
    assert_eq!(
        funcs.len(),
        ids.len(),
        "VOCAB func count != IR FnId count: vocab={:?} ir={:?}",
        funcs.iter().map(|n| n.flat).collect::<Vec<_>>(),
        ids.iter().map(|(n, _)| *n).collect::<Vec<_>>()
    );
    for n in &funcs {
        assert!(
            ids.iter().any(|(f, _)| *f == n.flat),
            "VOCAB function {} has no IR FnId",
            n.flat
        );
    }
    for (f, _) in ids {
        assert!(
            funcs.iter().any(|n| n.flat == *f),
            "IR FnId {f} has no VOCAB entry"
        );
    }
}
