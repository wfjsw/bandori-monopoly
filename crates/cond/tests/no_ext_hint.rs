//! Install-forgotten hint (brief 9.3): this binary never installs a
//! `CondExt`, so a content sugar name must fail with `UnknownFn` naming the
//! spelling and the `CondExt not installed` hint.

use rules_cond::{compile, CondError};

#[test]
fn unknown_fn_names_spelling_and_install_hint() {
    let err = compile("charge(owner) > 0").unwrap_err();
    match &err {
        CondError::UnknownFn(s) => assert_eq!(s, "charge"),
        other => panic!("expected UnknownFn, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(msg.contains("charge"), "must name the spelling: {msg}");
    assert!(
        msg.contains("CondExt not installed"),
        "must say CondExt not installed: {msg}"
    );
}

#[test]
fn unknown_var_names_spelling_and_install_hint() {
    let err = compile("owner.charge > 0").unwrap_err();
    match &err {
        CondError::UnknownVar(s) => assert_eq!(s, "owner.charge"),
        other => panic!("expected UnknownVar, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(msg.contains("owner.charge"), "must name the spelling: {msg}");
    assert!(
        msg.contains("CondExt not installed"),
        "must say CondExt not installed: {msg}"
    );
}