//! Round-trip: parse → `wire::Compiled` (postcard) → `Program::from_ast` →
//! eval, equal to compile+eval.
//!
//! The expression table covers every AST node kind the interpreter
//! evaluates, plus a sample of the expressions the crate's own unit tests
//! use. Size and decode-time numbers for the FORK.md / GUARDS.md notes come
//! from `print_wire_sizes` / `print_decode_time` (`--nocapture`).

use cel::common::ast::{EntryExpr, Expr, IdedExpr};
use cel::wire::{Compiled, FORMAT_VERSION};
use cel::{Context, Program, Value};

/// Expressions that evaluate over the empty context.
const NO_VARS: &[&str] = &[
    // literals
    "null", "true", "false", "42", "-7", "1u", "18446744073709551615u", "3.5", "-2.0", "'hello'",
    "\"world\"", "b'bytes'", "b\"\\x00\\xff\"", "'un\\tescaped\\n'",
    // arithmetic / comparison / logic
    "1 + 1", "(1 + 1)", "40 + 2", "2 - 3", "6 * 7", "7 / 2", "7 % 3", "1 < uint(2)",
    "uint(0) > -10", "1.0 > 0.0", "'a' < 'b'", "true && false", "true || false", "!true",
    "1 == 1", "1 != 2", "1 <= 1", "2 >= 3",
    // lists
    "[]", "[1, 2, 3]", "[1, 2, 3][0]", "[1, 2, 3][-1]", "size([1, 2, 3]) == 3", "size([]) == 3",
    // maps
    "{}", "{'a': 1, 'b': 2}", "{'a': 1, 'b': 2}['a']", "{'a': 1, 'a': 2}['a'] == 2",
    "{1: 'a', 2: 'b', 'c': 3}[2] == 'b'", "{-1: 'a', 18446744073709551615u: 'b'}[-1] == 'a'",
    "{} == []",
    // struct literals
    "type(cel.MyStruct{}) == cel.MyStruct",
    "cel.MyStruct { some: 'value' }.some == 'value'",
    "cel.MyStruct { some: 'value', here: 'totally' }.here == 'totally'",
    "has(cel.MyStruct{name: 'foo', value: 1}.name)",
    "has(cel.MyStruct{}.name)",
    "geo.Point{x: 1, y: 2} == geo.Point{y: 2, x: 1}",
    // macros (expanded at parse time into comprehensions)
    "[1, 2, 3].exists(x, x > 2)",
    "[1, 2, 3].all(x, x > 0)",
    "[1, 2, 3].exists_one(x, x > 2)",
    "[1, 2, 3].map(x, x * 2)[2] == 6",
    "[1, 2, 3, 4].filter(x, x % 2 == 0) == [2, 4]",
    "[1, 1].map(x, x * 2)[0] == 2",
    "[1, 2, 3].all(x, x < 10) && [1, 2, 3].exists(x, x == 2)",
    // ternary / conditional
    "true ? 1 : 2", "false ? 'y' : 'n'", "1 > 0 ? (2 > 1 ? 'a' : 'b') : 'c'",
    // conversions
    "int('42') == 42", "string(1) == '1'", "uint(1) == 1u", "double('NaN') == double('NaN')",
    "1.0 > double('NaN')", "bool(1)", "bytes('ab') == b'ab'",
    "type(1) == int", "type('x') == string", "type(1u) == uint", "type(1.0) == double",
    "type(b'x') == bytes", "type(true) == bool", "type(null) == null_type", "type([]) == list",
    "type({}) == map", "type(type(1)) == type",
    // string functions (stdlib)
    "size('abc') == 3", "'hello'.startsWith('he')", "'hello'.endsWith('lo')",
    "'hello'.contains('ell')", "'hello'.matches('h.*o')", "matches('hello', 'h.*o')",
    "'hello'[1] == 'e'",
    // optional syntax
    "optional.of(1).hasValue()", "optional.none().hasValue() == false", "optional.of(1).value() == 1",
    "optional.of(1).orValue(5) == 1", "optional.none().orValue(5) == 5",
    "optional.of(1).or(optional.of(2)).value() == 1",
    "optional.none().or(optional.of(2)).value() == 2",
    "[1, 2, ?optional.of(3), 4] == [1, 2, 3, 4]", "[1, 2, ?optional.none(), 4] == [1, 2, 4]",
    "[?optional.of(1), ?optional.none(), ?optional.of(3)] == [1, 3]",
    "{'a': 1, ?'b': optional.of(2)} == {'a': 1, 'b': 2}",
    "{'a': 1, ?'b': optional.none()} == {'a': 1}",
    "optional.of([1, 2, 3])[1].orValue(99) == 2", "optional.of([1, 2, 3])[4].orValue(99) == 99",
    "optional.none()[1].orValue(99) == 99", "optional.of([1, 2, 3])[?1].orValue(99) == 2",
    // nested / mixed
    "[{'k': 1}, {'k': 2}].map(m, m.k)[1] == 2", "{'xs': [1, 2, 3]}.xs[2] == 3",
    "size([1, [2, 3], {'a': 4}]) == 3", "[1, 2, 3].map(x, x * 2).filter(y, y > 2) == [4, 6]",
    "[1, 2, 3, 4].map(x, x + 1).all(y, y > 1)", "[[1], [2, 3]].exists(l, l.exists(x, x == 3))",
];

/// The sample conditions of docs/GUARDS.md §8.3 — the shape that will
/// actually be serialized for the guard prefilter.
const GUARD_SAMPLES: &[&str] = &[
    "actor == owner",
    "actor != owner && owner.money >= 500",
    "effect.hits(owner) && value >= 5000",
    "tile.owner == owner && tile.houses > 0",
    "move.kind == Walk && move.roll != null",
    "effect.has(Pay)",
    "slot('asUsualTurn') != turn_key",
    "money(owner) >= 500",
];

/// Expressions over a small variable context (`foo`, `bar`, `data`, `list`).
const WITH_VARS: &[&str] = &[
    "foo", "foo.bar == 1", "foo.bar", "foo.bar + 1 == 2", "bar", "bar == 'hi'", "has(data.x)",
    "has(data.x) && data.x.startsWith(\"foo\")", "data.x == 'foo'", "has(data.missing) == false",
    "list[0] == 10", "list[2]", "size(list) == 3", "list.exists(x, x > 20)",
    "foo.bar > 0 && bar == 'hi'", "foo.bar >= 1 ? list[1] : 0",
];

fn var_ctx() -> Context<'static, 'static> {
    use std::collections::HashMap;
    let mut ctx = Context::default();
    ctx.add_variable_from_value("foo", HashMap::from([("bar", 1i64)]));
    ctx.add_variable_from_value("bar", "hi".to_string());
    ctx.add_variable_from_value("data", HashMap::from([("x", "foo".to_string())]));
    ctx.add_variable_from_value("list", vec![10i64, 20, 30]);
    ctx
}

fn encode(program: &Program, with_source: bool) -> Vec<u8> {
    postcard::to_allocvec(&program.to_compiled(with_source)).expect("postcard encode")
}

fn decode(bytes: &[u8]) -> Result<Program, String> {
    let compiled: Compiled = postcard::from_bytes(bytes).map_err(|e| e.to_string())?;
    compiled.into_program().map_err(|e| e.to_string())
}

fn roundtrip(src: &str, ctx: &Context) {
    let program = cel::Env::stdlib().compile(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    let direct = program.execute(ctx);

    // With source: tree and result survive.
    let loaded = decode(&encode(&program, true)).unwrap_or_else(|e| panic!("{src}: {e}"));
    assert_eq!(direct, loaded.execute(ctx), "wire round-trip changed the result of {src}");
    assert_eq!(program.expression(), loaded.expression(), "wire round-trip changed the tree of {src}");
    assert_eq!(loaded.source_info().source, src, "wire round-trip dropped the source of {src}");

    // Without source: same tree, same result, no source text on the wire.
    let lean = encode(&program, false);
    let loaded = decode(&lean).unwrap_or_else(|e| panic!("{src}: {e}"));
    assert_eq!(direct, loaded.execute(ctx), "lean wire round-trip changed the result of {src}");
    assert_eq!(program.expression(), loaded.expression(), "lean wire round-trip changed the tree of {src}");
    assert_eq!(loaded.source_info().source, "", "lean wire kept the source of {src}");
}

#[test]
fn wire_round_trip_preserves_eval() {
    let ctx = Context::default();
    for src in NO_VARS.iter().chain(GUARD_SAMPLES.iter()) {
        roundtrip(src, &ctx);
    }
    let ctx = var_ctx();
    for src in WITH_VARS {
        roundtrip(src, &ctx);
    }
}

/// Every distinct node kind in the table above must actually appear in some
/// expression's tree — the round-trip is only meaningful if we cover the AST.
#[test]
fn the_table_covers_every_ast_node_kind() {
    fn rec(e: &IdedExpr, seen: &mut std::collections::HashSet<&'static str>) {
        match &e.expr {
            Expr::Unspecified => {
                seen.insert("Unspecified");
            }
            Expr::Literal(_) => {
                seen.insert("Literal");
            }
            Expr::Ident(_) => {
                seen.insert("Ident");
            }
            Expr::Select(s) => {
                seen.insert("Select");
                if s.test {
                    seen.insert("Select.test");
                }
                rec(&s.operand, seen);
            }
            Expr::Call(c) => {
                seen.insert("Call");
                if c.target.is_some() {
                    seen.insert("Call.member");
                }
                if let Some(t) = &c.target {
                    rec(t, seen);
                }
                for a in &c.args {
                    rec(a, seen);
                }
            }
            Expr::List(l) => {
                seen.insert("List");
                if !l.optional_indices.is_empty() {
                    seen.insert("List.optional");
                }
                for x in &l.elements {
                    rec(x, seen);
                }
            }
            Expr::Map(m) => {
                seen.insert("Map");
                for en in &m.entries {
                    rec_entry(&en.expr, seen);
                }
            }
            Expr::Struct(s) => {
                seen.insert("Struct");
                for en in &s.entries {
                    rec_entry(&en.expr, seen);
                }
            }
            Expr::Comprehension(c) => {
                seen.insert("Comprehension");
                rec(&c.iter_range, seen);
                rec(&c.accu_init, seen);
                rec(&c.loop_cond, seen);
                rec(&c.loop_step, seen);
                rec(&c.result, seen);
            }
        }
    }
    fn rec_entry(e: &EntryExpr, seen: &mut std::collections::HashSet<&'static str>) {
        match e {
            EntryExpr::StructField(f) => rec(&f.value, seen),
            EntryExpr::MapEntry(m) => {
                rec(&m.key, seen);
                rec(&m.value, seen);
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    for src in NO_VARS.iter().chain(WITH_VARS.iter()).chain(GUARD_SAMPLES.iter()) {
        let p = cel::Env::stdlib().compile(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        rec(p.expression(), &mut seen);
    }
    for kind in [
        "Literal", "Ident", "Select", "Select.test", "Call", "Call.member", "List",
        "List.optional", "Map", "Struct", "Comprehension",
    ] {
        assert!(seen.contains(kind), "table never exercises {kind}: {seen:?}");
    }
}

#[test]
fn a_version_bump_fails_loudly() {
    let program = cel::Env::stdlib().compile("1 + 2").unwrap();
    let mut bytes = encode(&program, false);
    // `version` is the first field: postcard writes it as a single byte.
    assert_eq!(bytes[0], FORMAT_VERSION);
    bytes[0] = FORMAT_VERSION.wrapping_add(1);
    let err = decode(&bytes).unwrap_err();
    assert!(
        err.contains("format version") && err.contains(&FORMAT_VERSION.to_string()),
        "version mismatch must name both versions, got: {err}"
    );
    // And the typed error, not just the Display string.
    let compiled: Compiled = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(
        compiled.check(),
        Err(cel::wire::WireError::Version {
            found: FORMAT_VERSION.wrapping_add(1),
            expected: FORMAT_VERSION,
        })
    );
}

#[test]
fn decode_rejects_garbage() {
    assert!(decode(b"not postcard").is_err());
    assert!(decode(b"").is_err());
}

#[test]
fn from_ast_evaluates_a_hand_built_tree_without_a_parser() {
    let expr = IdedExpr {
        id: 1,
        expr: Expr::Call(cel::common::ast::CallExpr {
            func_name: "_+_".to_string(),
            target: None,
            args: vec![
                IdedExpr {
                    id: 2,
                    expr: Expr::Literal(cel::common::ast::LiteralValue::Int(40.into())),
                },
                IdedExpr {
                    id: 3,
                    expr: Expr::Literal(cel::common::ast::LiteralValue::Int(2.into())),
                },
            ],
        }),
    };
    let program = Program::from_compiled(Compiled::bare(expr)).unwrap();
    assert_eq!(program.execute(&Context::default()), Ok(Value::Int(42)));
}

#[test]
fn print_wire_sizes() {
    // `cargo test --manifest-path third_party/cel-rust/Cargo.toml \
    //    --features parser,serde-ast --test wire -- --nocapture`
    println!("source_bytes lean_bytes with_source_bytes expr");
    for src in GUARD_SAMPLES.iter().chain(&[
        "[1, 2, 3].exists(x, x > 2) && {'a': 1}['a'] == 1",
        "has(data.x) ? data.x : 'default'",
    ]) {
        let program = cel::Env::stdlib().compile(src).unwrap();
        let lean = encode(&program, false).len();
        let full = encode(&program, true).len();
        println!("{} {} {} {src}", src.len(), lean, full);
    }
}

#[test]
fn print_decode_time() {
    use std::time::Instant;
    let src = "actor != owner && owner.money >= 500";
    let program = cel::Env::stdlib().compile(src).unwrap();
    let bytes = encode(&program, false);
    let n = 100_000;
    // Warm up.
    for _ in 0..1000 {
        let _ = decode(&bytes).unwrap();
    }
    let start = Instant::now();
    for _ in 0..n {
        let _ = decode(&bytes).unwrap();
    }
    let elapsed = start.elapsed();
    println!(
        "decode {src}: {} B, {n} decodes in {elapsed:?}, {:.2} ns each",
        bytes.len(),
        elapsed.as_nanos() as f64 / n as f64
    );
}