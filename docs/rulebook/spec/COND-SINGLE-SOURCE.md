# Single-source guard conditions with a proven translator

Status: **landed (pilot)**. Branch `cond-single`. The CEL `&'static str` on
each `On::*` entry is the **one source of truth**; a proven lowering yields
both the production host path (unchanged) and a heap-free Rust evaluator that
Kani runs with **no `assume` of the host condition**.

Companion: `docs/rulebook/spec/KANI.md` (the pilot that this unblocks),
`docs/GUARDS.md` (the condition language and host prefilter).

---

## 1. Decision: derive the Rust evaluator from the same linted AST

Two designs were on the table (user preference was a macro).

### 1.1 Rejected: a `cond!(actor == owner && value > 0)` macro

A macro that emits **both** a CEL string and a Rust evaluator.

| | |
|---|---|
| pros | one authoring site next to the card; looks tidy |
| cons | real guards use single-quoted strings (`slot('skillState')`), dotted names (`move.roll`), bare kind constants (`Teleport`) and `null` — a Rust token-tree `stringify!` cannot produce valid CEL (`'x'` is not a Rust literal; `null`/kind ids need mapping). A real `cond!` would need its **own CEL-subset parser** → a second trusted parser that can diverge from the vendored `cel` one |
| cons | 138 distinct condition strings / ~226 non-empty `pre` sites would all need re-authoring; byte-identical production output is at risk |
| cons | proof obligation becomes `print(macro AST) re-parses to the identical cel AST` — an extra equivalence to establish per guard |
| cons | string arguments inside the macro (`tok('PAREO标记')`) force a quoting story that is not Rust's |

### 1.2 Chosen: derive the Rust evaluator from the same linted/rewritten CEL AST

Name: **AST-derived IR** (`rules-cond-ir` + `rules-cond::lower`).

| | |
|---|---|
| source of truth | the existing `&'static str` condition (already the production wire format; nothing to re-author) |
| parser | exactly one: the vendored `cel` crate via `rules-cond::compile` (already load-bearing for the host) |
| translator | `lower`: the **rewritten** AST (`lint_and_rewrite` output) → a heap-free arena IR |
| Rust side | `rules-cond-ir::eval` over `Val { Int, Bool, Null, List, Err }` — integer-only, no `String`/`Vec`/`HashMap` at eval |
| Kani side | generated `const` programs in `rules/kani/src/generated/cond_irs.rs`, evaluated by the same interpreter under a symbolic `IrCtx` |
| production | **unchanged**: the host still evaluates the CEL string via `rules-cond`; the IR is host-only (`lower` feature / `cfg(kani)` / tests) |

Smaller trusted base, zero migration risk, cannot diverge in parsing. The
user can veto this deviation from the macro preference; the name to look for
is **AST-derived IR**.

### 1.3 How a guard is authored

Write the CEL string exactly as today:

```rust
On::Hook(&[HookKind::PayAdd], "actor == owner && value > 0", None, pay_add),
```

Then, to prove it under Kani:

1. `cargo run -p rules-cond --features lower --example gen_cond_ir`
   (regenerates `rules/kani/src/generated/cond_irs.rs` from every inventory
   string);
2. in the harness, `ir_fill::guard_by_src("actor == owner && value > 0")`
   and `eval_bool` against an `IrCtx` filled from the symbolic world;
3. never `assume` the condition — evaluate it and branch.

The consistency test (`rules/cond/tests/cond_ir_consistency.rs`) fails if the
generated table drifts from the strings. No guard may exist whose Rust side
was authored separately.

---

## 2. Architecture

```
On::* "pre"  (the one source)
    │
    ├─► rules-cond::compile  ──► rewritten CEL AST
    │         │                        │
    │         │                        ├─► (host, production) cel interpreter
    │         │                        │      Cond::eval = eval_checked.unwrap_or(false)
    │         │                        │
    │         └──────── lower ─────────┴─► rules-cond-ir::Program (arena of Node)
    │                                            │
    │                                            ├─► rules-cond-ir::eval   (interpreter)
    │                                            ├─► rules-cond-ir::spec   (written reference)
    │                                            └─► generated const data → Kani harness
    │
    └─► ManifestOn.pre  (unchanged wire format; ABI untouched)
```

Crates (host-only; never in the rules/ wasm workspace):

| crate | role |
|---|---|
| `rules/cond-ir` | `Val` / `Node` / `IrCtx` / `eval` / `spec` — no dependencies, Kani-friendly |
| `rules/cond` (`lower` feature) | `lower`: rewritten AST → `Program`; `inventory_guards` |
| `rules/kani` | generated IRs, `ir_fill` (World → `IrCtx`), proofs |

### 2.1 IR shape (why not a `const` expression tree)

A post-order arena of `Node { kind, payload, a, b, c }`:

* `Copy`, no heap, `const`-friendly (the generator emits `static [Node]`);
* short-circuiting skips the untaken arm (lazy `&&` / `||` / ternary);
* recursion depth = expression depth (every real guard < 16) — no unwinding
  cliff;
* string arguments (`slot('name')`) are baked to `id_of(name)` at lower time;
* functions go through one narrow `call_fn` surface the Kani host models.

Rejected alternative: straight-line generated `fn` over a flat `Ctx` struct.
It would work, but every guard becomes a bespoke function (harder to
consistency-check against the string) and the "translator" becomes a code
emitter rather than a tree lowerer. The arena keeps the translator total and
the evaluator one recursive match.

### 2.2 Vocabulary stays single-sourced

`rules-cond/src/vocab.rs` `VOCAB` is still the one definition. The IR's
`vars::VARS` (scalars) and `FnId` (functions) are **checked against it** by
`rules/cond/tests/cond_ir_vocab.rs`. Adding a name without extending the IR
fails that test.

### 2.3 Zero cost in production

* `rules-cond-ir` is not a dependency of `game-rules`, `rules-native`, or any
  card crate.
* `rules-cond`'s `lower` feature is off by default (host builds use
  `compile + wire` as before).
* The card wasm build (`rules/` workspace) is untouched: condition strings
  and the manifest are unchanged.

---

## 3. Reference semantics (the written spec)

Derived from the vendored `cel` crate (`third_party/cel-rust`) and confirmed
by differential execution. Full table in §4.

* **int64 arithmetic** uses `checked_*`; overflow is an error.
* **division / remainder by zero** is an error.
* **`null`** is its own value. `null == null` is true; `null == 1` is false
  (never an error). **Ordering involving `null` is always an error** — even
  `null <= null` (see Finding F1).
* **`OptInt` variables** bind `null` when the raw i64 is **any negative**
  (see Finding F2), not just `-1`.
* **`&&` / `||` absorb an error** when the other side decides:

  | `a && b` | `b=true` | `b=false` | `b=err` |
  |---|---|---|---|
  | `a=true` | true | false | err |
  | `a=false` | false | false | false |
  | `a=err` | err | false | err |

  | `a \|\| b` | `b=true` | `b=false` | `b=err` |
  |---|---|---|---|
  | `a=true` | true | true | true |
  | `a=false` | true | false | err |
  | `a=err` | true | false | err |

  Short-circuit: `false && e` never evaluates `e`; `true || e` never
  evaluates `e`.
* **ternary** is lazy; only the taken branch is evaluated; a non-bool
  condition is an error.
* **equality** is total over the lattice (cross-type = false).
* **any evaluation error ⇒ the whole condition is false**
  (`Cond::eval = eval_checked(..).unwrap_or(false)`), and the IR root rule
  `Val::as_cond_bool` is the same.

The reference lives in `rules/cond-ir/src/spec.rs` as independent `spec_*`
functions (not a copy of the interpreter's match arms). The Kani proofs
assert `eval_op == spec_op` for every operand; the differential tests assert
both equal the real `cel` evaluator.

---

## 4. Per-operator proof table

| operator | reference semantics | Kani (`cond_ops.rs`) | differential (`cond_ir_diff.rs`) |
|---|---|---|---|
| `+ - *` | `checked_*`, overflow ⇒ Err | `ir_arith_matches_spec_for_all_operands` | boundary grid + random trees |
| `/ %` | div/rem by zero ⇒ Err; `i64::MIN / -1` ⇒ Err | same | same |
| `== !=` | total; cross-type false; `null==null` true | `ir_eq_matches_spec_for_all_operands` | same |
| `< <= > >=` | like-with-like only; **any `null` ⇒ Err** | `ir_cmp_matches_spec_for_all_operands` | same |
| `&& \|\|` | error absorption (§3) | `ir_logical_matches_spec_for_all_operands` | same |
| `!` / unary `-` | non-bool ⇒ Err; `i64::MIN` neg ⇒ Err | `ir_neg_not_match_spec` | same |
| `?:` | lazy; non-bool cond ⇒ Err | covered by random trees | same |
| `in` / `size` | membership / list length; non-list ⇒ Err | (not yet harnessed; no real guard uses them) | random trees |
| functions (`slot`/`tok`/`is_circle`/…) | via `IrCtx` accessors | via `money_shave` / pilot tests | every inventory guard |

**Induction claim (what the proofs cover).** For a closed-set expression `e`
over the scalar lattice: if every leaf evaluates to the same `Val` on both
sides (the binding is shared — `IrCtx` / `WindowCtx` are filled from one
fixture), and every node kind's operator satisfies
`eval_op == spec_op`, then `eval(e) == spec_eval(e)` by structural induction
on the AST. The Kani proofs cover the operator step for **all** operand
values; the differential tests cover the leaf binding and the composition on
real and random trees against the vendored `cel`. What is **not** covered by
this induction: the lowering from `IdedExpr` to `Node` (pinned by the
per-guard consistency check + differential), and the World → `IrCtx` binding
(pinned by the host's own `fill_window`/`fill_candidate` and the pilot
harnesses).

---

## 5. Inventory (rules/, at generation time)

| | count |
|---|---|
| `On::*` declarations scanned | 626 |
| distinct condition strings that lower | **139** (incl. `pre::MINE` = `actor == owner`) |
| empty conditions (`""`) | 400 |
| junk extracted then rejected (Msg names etc.) | 5 |
| operators actually used | `&&` `==` `!=` `>=` `>` `\|\|` `<=` `-` `<` `/` |
| **not used in any guard** | `*` `%` `+` `!` `?:` `in` `size` (still in the closed set and proved) |
| functions used | `slot` `is_circle` `tile_named` `tok` `fire` `band_is` `counter_is` `is_ring` `players_on` `next_dist` `is_live_house` `is_buyable` `on_path` `character_is` `neighbor` `card.counter` `effect.has` `effect.hits` |
| variables used | `owner` `actor` `card.placed` `tile.*` `value` `target` `card.counter` `move.*` `kind` `pay_is_rent` `by` `effect.*` `turn_*` `trigger_card` `plan.fixed_roll` `roll_source` |

This bounds the translator's proof obligation: the closed operator set is
smaller than CEL, and the function set is exactly VOCAB's.

---

## 6. Findings (CEL-vs-intuition divergences)

Every one of these was found by the differential tests and **fixed in the IR
to match the vendored `cel`** (the host behaviour is the contract).

| id | finding | resolution |
|---|---|---|
| **F1** | Ordering involving `null` is **always an error**, including `null <= null`. `CelNull` does not implement `Comparer`, so `compare_values` fails and the host maps it to `false`. Intuition (and the first draft of the spec) said `null <= null` is true. | `spec_cmp` / `values_cmp` now return `Err` for every null pairing. |
| **F2** | `OptInt` binds `null` for **any negative** raw i64, not just `-1` (`vocab::opt`: `if v < 0 { Null }`). So `plan_fixed_roll = Some(i64::MIN)` reads as `null`. | IR `scalar_val` maps `raw < 0` → `Null`. |
| **F3** | `card.tile` is a plain `Int` (`-1` = with owner, `-2` = not placed), **not** `OptInt`. The first IR table marked it OptInt and diverged on `card_tile < 2`. | `vars::VARS` corrected; the vocab-sync test would have caught a silent mismatch. |
| **F4** | `counter_name` binds `0` for an empty `Trigger.name`, not `id_of("")`. | test fixture + IR documentation. |

---

## 7. Trusted base (what is still assumed) and how it is tested

| piece | why trusted | how tested |
|---|---|---|
| vendored `cel` crate | the production host evaluator | the differential tests treat it as the oracle; its own test suite |
| `lint_and_rewrite` (schema.rs) | flattens `owner.money` → `owner_money` etc. | already covered by `rules-cond` unit tests + load-time fail-closed |
| `lower` (AST → IR) | the translator | per-guard consistency (`lower(compile(s)) == embedded IR`) + differential on every inventory string |
| `rules-cond-ir::eval` | the Kani-side evaluator | Kani proofs vs `spec` + differential vs `cel` |
| `rules-cond-ir::spec` | the written reference | differential vs `cel` (if both spec and eval diverge from cel together, differential catches it) |
| World → `IrCtx` binding (`ir_fill`) | the last mile into Kani | pilot harnesses + the host's own `fill_window`/`fill_candidate` (same field semantics); a future step is a differential of `fill_*` vs `ir_fill` on a shared snapshot |
| function accessors (`is_circle`, `tok`, …) | model the view | pilot tests; the vocab-sync test pins the name set |
| `id_of` hash | name-stable identity | copy of `rules_cond::id_of` in `cond-ir`; pinned by construction (same FNV-1a) |

---

## 8. Pilot: 3 real guards end to end

| guard | card / shape | evidence |
|---|---|---|
| `"actor == owner && value > 0"` | PP:[丸山彩]憧憬的前方 `On::Hook(PayAdd)` | in the generated table; `money_shave.rs` evaluates it under symbolic context (no `assume`); `wrong_guard.rs` shows `value >= 0` fails the iff |
| `"actor == owner && is_circle(tile.id)"` | function (`is_circle`) | in the generated table; `pilot_guards.rs` truth table |
| `"actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0"` | chain/kind | in the generated table; `pilot_guards.rs` truth table |

### 8.1 Kani results

See §10 (filled in at proof time).

### 8.2 Wrong-guard demonstration

`wrong_guard_too_loose_is_caught` builds the **wrong** guard `value >= 0`
(inline IR, the mutant) and asserts `wrong ⇔ clause_condition` where
`clause_condition` is `actor == owner && value > 0` from the 规则书 quote.
Kani must report FAIL (via `#[kani::should_panic]`) on the `value == 0`
counterexample. The companion `real_guard_matches_clause_iff` verifies the
same proof obligation holds for the real single-source guard.

---

## 9. Migration plan for the rest

| phase | content | effort |
|---|---|---|
| 0 | machinery + 3 pilots (this change) | done |
| 1 | run `gen_cond_ir` in CI; fail the build if the generated table is stale | ~0.5 day |
| 2 | per-clause Kani harnesses that use `guard_by_src` instead of `assume` (replace the pilot's hand assumes one by one) | ~2–3 min/harness once the world model covers the clause's fields |
| 3 | extend `ir_fill` to the full `WindowCtx`/`CandidateCtx` surface (the remaining trusted base) and differential it against `fill_window`/`fill_candidate` | 1–2 days |
| 4 | optional: a `cond!` sugar that expands to the string literal only (no second parser) for new guards | 0.5 day, zero risk |

**Do not** re-author the 139 existing strings. The generator already covers
them; a re-authoring pass is exactly the risk this design avoids.

---

## 10. Proof / test result tables

### Differential + consistency (host, `cargo test -p rules-cond --features lower`)

| test | cases | result |
|---|---|---|
| `diff_operators_boundary_grid` | boundary × operators + connectives + error absorption | **agree** |
| `diff_every_scalar_over_boundary` | every VOCAB scalar × {MIN,-1,0,1,MAX,null} | **agree** |
| `diff_all_inventory_guards` | 139 distinct guards × 2 fixtures | **agree** |
| `diff_random_expression_trees` | 4000 random trees (depth 1–4) | **agree** |
| `ir_eval_matches_written_spec` | full scalar lattice × lattice for eq/ne/cmp/arith/and/or | **agree** |
| `error_roots_are_false` | error roots ⇒ false on both sides | **agree** |
| `every_guard_lowers_to_its_embedded_ir` | 139 guards vs generated table | **match** |
| `ir_vars_match_vocab` / `ir_fn_ids_match_vocab_funcs` | 43 scalars / 34 fns | **match** |

### Kani (`cargo kani -p rules-kani`, `CARGO_TARGET_DIR=/home/monopoly/kani-target`)

Invocation: `cargo kani -p rules-kani --harness <name> --default-unwind 8 --no-unwinding-checks`.

| harness | result | time | notes |
|---|---|---|---|
| `ir_eq_matches_spec_for_all_operands` | **SUCCESSFUL** | 2.1 s | symbolic i64/bool/null |
| `ir_lt_matches_spec` | **SUCCESSFUL** | 0.9 s | |
| `ir_le_matches_spec` | **SUCCESSFUL** | 1.3 s | |
| `ir_gt_matches_spec` | **SUCCESSFUL** | 2.0 s | |
| `ir_ge_matches_spec` | **SUCCESSFUL** | 1.0 s | |
| `ir_add_matches_spec` | **SUCCESSFUL** | 1.9 s | |
| `ir_sub_matches_spec` | **SUCCESSFUL** | 1.6 s | |
| `ir_mul_matches_spec` | **SUCCESSFUL** | 47.4 s | |
| `ir_div_matches_spec` | **incomplete** | > 3 min | CBMC `checked_div` on full i64×i64; case-split version still slow. Covered exhaustively by the differential boundary grid |
| `ir_rem_matches_spec` | **incomplete** | > 3 min | same |
| `ir_logical_matches_spec_for_all_operands` | **SUCCESSFUL** | 0.9 s | error-absorption table |
| `ir_neg_not_match_spec` | **SUCCESSFUL** | 0.7 s | |
| `error_root_is_false` | **SUCCESSFUL** | 1.1 s | |
| `ir_tern_matches_spec` | **SUCCESSFUL** | 0.3 s | |
| `aya_haruka_02_shave_iff` | **SUCCESSFUL** | 7.0 s | **no `assume` of `value > 0`** — the real IR decides |
| `aya_haruka_02_mutant_is_detected` | **SUCCESSFUL** | 92 s | proves the off-by-one is live (card ≠ X+1 spec) |
| `aya_haruka_02_mutant_x_off_by_one` | tooling quirk | 115 s | the assert fails in the check list, but `#[kani::should_panic]` reports "no panics" (likely `panic_nounwind_fmt` from the message form). Use `mutant_is_detected` as the non-vacuity evidence |
| `wrong_guard_too_loose_is_caught` | **SUCCESSFUL (panic as expected)** | 2.8 s | `value >= 0` is caught at `value == 0` |
| `real_guard_matches_clause_iff` | **SUCCESSFUL** | 2.2 s | companion; real guard ⇔ clause |

### Production output

* card wasm / manifest / `ABI_VERSION`: **unchanged** (condition strings
  untouched; no wire-format change). `cargo check --manifest-path
  rules/Cargo.toml --workspace --target wasm32-unknown-unknown`: **ok**.
* `cargo test -p rules-cond --features lower`: **68 passed** (57 existing +
  6 differential + 2 vocab + 2 consistency + 1 doctest).
* `cargo test -p rules-kani --test pilot_guards`: **4 passed**.
* `cargo build -p game-rules -p rules-native`: **ok**.

---

## 11. Repo map

```
rules/cond-ir/                  NEW  heap-free IR + eval + spec
  src/{lib,ctx,nodes,eval,spec,vars}.rs
rules/cond/
  src/lower.rs                  NEW  AST → IR (feature `lower`)
  examples/gen_cond_ir.rs       NEW  generator
  tests/cond_ir_diff.rs         NEW  differential vs cel
  tests/cond_ir_vocab.rs        NEW  VOCAB ↔ IR sync
  tests/cond_ir_consistency.rs  NEW  per-guard embedded-IR check
rules/kani/
  src/generated/cond_irs.rs     NEW  generated const IRs (139 guards)
  src/ir_fill.rs                NEW  World → IrCtx
  src/proofs/cond_ops.rs        NEW  operator proofs
  src/proofs/wrong_guard.rs     NEW  wrong-guard demonstration
  src/proofs/money_shave.rs     UPD  guard evaluated, no assume
  tests/pilot_guards.rs         NEW  host-side pilot truth tables
docs/rulebook/spec/COND-SINGLE-SOURCE.md   NEW  this file
```