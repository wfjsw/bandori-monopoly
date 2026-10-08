# cel-rust fork

A maintained fork of [`cel`](https://crates.io/crates/cel) (cel-rust,
github.com/cel-rust/cel-rust) that can **evaluate a pre-compiled CEL
expression without embedding the parser** — the same general ability the
reference implementations have (cel-java's runtime artifact, cel-cpp building
from a `CheckedExpr`), not a project-specific evaluator.

Consumed by `crates/rules-cond` (docs/GUARDS.md §8) via the workspace root's
`[patch.crates-io]`. `rules-cond` compiles guard-prefilter conditions on the
host and ships the serialized AST to the browser, where a runtime-only build
evaluates it with no antlr4rust in the binary.

## Base

| | |
|---|---|
| upstream | https://github.com/cel-rust/cel-rust |
| version | 0.15.0 (crates.io) |
| commit | `57b2a7e9b835425be0a7320e6bc0461380843f2d` (`.cargo_vcs_info.json`, `path_in_vcs: cel`) |
| license | MIT — see `LICENSE` |
| vendored from | the cargo registry cache of `cel-0.15.0`, unmodified tree as the starting point |

`Cargo.toml` is the fork's working manifest (the crates.io-normalized one from
the registry and the upstream `Cargo.toml.orig` are kept as provenance;
`Cargo.toml.orig` is the upstream author's file).

## Purpose

1. **`parser` feature (default ON)** — the expression parser (antlr4rust) and
   everything parse-time (macro expander, `Program::compile`, `Env::compile` /
   `Env::parser` / `Env::add_macro`) behind one feature. Today's behaviour is
   unchanged with defaults on.
2. **Runtime-only build** — `--no-default-features` gives a pure interpreter:
   no antlr4rust, no nom, no parser modules. Feed it an AST with
   [`Program::from_ast`] or [`wire::Compiled`].
3. **Serialized compiled form** — [`wire::Compiled`], a serde-derived AST in a
   versioned wrapper. This is the **primary** compiled form.

## The serialized form

* **Shape**: `wire::Compiled { version: u8, expression: IdedExpr, source: Option<String> }`.
  Serde derives on the native AST (`serde-ast` feature) — format-agnostic, so
  any serde format works; this project uses **postcard** (same format as the
  ruleset manifest, `crates/rules-cond`).
* **Format version**: `version` is the first field (postcard is not
  self-describing, so it is also the first byte of the blob).
  `Compiled::check` / `Cond::from_bytes` reject any other value with
  `WireError::Version` / `CondError::WireVersion` — a fork bump that changes
  the AST shape **must** bump `wire::FORMAT_VERSION`, and every decode of an
  older blob fails loudly. No cross-version migration.
* **What is on the wire**: only what evaluation needs — the expression tree.
  Source offsets, macro bookkeeping and types are not carried (parse errors
  happen on the host; the interpreter resolves names against the `Context` at
  runtime). The source *text* is optional and omitted for the lean
  runtime-only form (`to_bytes(false)` / `to_compiled(false)`), saving
  16–50 B per condition and the string itself.
* **Sizes** (postcard, `rules-cond` sample conditions): 22–67 B lean,
  38–106 B with source. See docs/GUARDS.md §8.5.

> Protobuf (`cel.expr.ParsedExpr` / `CheckedExpr`) was prototyped and then
> dropped: the user ruled that compatibility with other CEL implementations
> is not a goal here, and postcard of the native AST is both smaller and the
> format the rest of the project already uses. It is not a follow-up item.

## Feature matrix

| feature | default | what it does |
|---|---|---|
| `parser` | **ON** | antlr4rust + `parser` backends (`gen`/`parse`/`parser`/`pratt_parser`), macro expander, `Program::compile`, `Env::compile`/`parser`/`add_macro`. `nom` is **not** part of this: it is only used by the `chrono` duration literal parser (`src/duration.rs`) |
| `serde-ast` | off | serde derives on the AST + the `wire` module |
| `regex` / `chrono` / `ext_encoders` / `json` / `bytes` | `regex`/`chrono`/`ext_encoders` ON | unchanged upstream optionals |
| `parser_pratt` | off | experimental winnow-style parser (upstream), implies `parser` |

Runtime-only build:

```text
cargo build -p cel --manifest-path third_party/cel-rust/Cargo.toml \
  --no-default-features --features serde-ast --target wasm32-unknown-unknown
```

(`-p cel` from the workspace root also works, but feature selection for a
non-member needs the `--manifest-path` form.)

## Diff summary vs 0.15.0

* `Cargo.toml` — features above; `antlr4rust`/`lazy_static`/`nom` optional;
  `prost` never landed (see above). Standalone `[workspace]` table so the
  fork can be tested feature-by-feature without joining the host workspace's
  feature unification.
* `src/lib.rs` — `Program::from_ast`; `Program::compile` / `TryFrom<&str>` /
  `ParseError` re-exports behind `parser`; crate docs for the new features.
* `src/parser/mod.rs` — `Expression` + `ExpressionReferences` always
  available; the antlr4rust backends, `macros.rs` and the `Parser` API behind
  `parser`.
* `src/env.rs` — `macros` field, `parser()` / `compile()` / `add_macro()`
  behind `parser`; **new** `Env::with_minimal_stdlib()` (see below).
* `src/wire.rs` — **new**: the versioned `Compiled` wrapper + `Program` glue.
* `src/common/ast/mod.rs` — serde derives on the AST (`serde-ast`);
  `SourceInfo::offsets()` iterator; parser-dependent unit tests gated.
* `src/common/types/{bool,bytes,double,int,string,uint}.rs` — serde derives on
  the scalar wrappers (`serde-ast`).
* `src/extensions/{lists,math}.rs`, `src/common/types/optional.rs` — macro
  registration and expanders behind `parser` (they are parse-time only).
* Unit tests that drive the parser are `#[cfg(all(test, feature = "parser"))]`
  so `cargo test --no-default-features --lib` compiles.
* `tests/wire.rs` — round-trip (parse → postcard → `from_ast` → eval equals
  compile+eval) across ~130 expressions covering every AST node kind, plus
  the GUARDS.md §8.3 sample conditions; format-version failure; size and
  decode-time hooks.

### `Env::with_minimal_stdlib`

Operators (`+ - * / % == != < <= > >= && || ! ?: []`) are interpreted
directly by the evaluator, not registry overloads, so they need no
registration. `with_minimal_stdlib` registers only the `bool` / `int` /
`uint` / `string` / `type` / `null` libraries (scalar conversions, `size` /
`contains` / `startsWith` / `endsWith` on strings) and the standard *types*
(`list`/`map`/`bytes`/`double`/`optional`/`dyn` names resolve for `type(x) ==
…`) without their function overloads. That is what the `rules-cond` guard
schema needs; it is worth **33–45 KB** of wasm32 (417 KB → 384 KB at
`opt-level=s`+fat LTO+strip for the runtime-only cdylib). Caveat: `size` of a
list or map needs the full `with_stdlib`. This is a fork addition for
size-oriented runtime builds, not upstream API.

## Test / measure

```text
# the fork, all features that matter
cargo test --manifest-path third_party/cel-rust/Cargo.toml --features parser,serde-ast

# runtime-only must build without the parser
cargo build --manifest-path third_party/cel-rust/Cargo.toml \
  --no-default-features --features serde-ast --target wasm32-unknown-unknown

# sizes and decode time (the hooks that print the tables)
cargo test --manifest-path third_party/cel-rust/Cargo.toml \
  --features parser,serde-ast --test wire -- --nocapture

# rules-cond: 29 + 4 tests
cargo test -p rules-cond
```

Doctests that show `Env::compile` need the `parser` feature (they are the
default build). For the runtime-only check use `--lib --tests`.

Note: `parser::parser::tests::malformed_nested_expression_does_not_panic`
overflows the stack on Windows debug builds (upstream: the test caps the
recursion depth at 48 but antlr4rust recurses before that check). Run the
suite with `RUST_MIN_STACK=16777216`.

## Upstreaming

Not a goal of this fork (the user ruled out compatibility work). If any of it
were to go upstream, the natural pieces would be: the `parser` feature split,
`Program::from_ast`, and serde derives on the AST — small, feature-gated,
and independent of the `wire::Compiled` wrapper (which exists for this
project's postcard pipeline).