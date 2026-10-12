# Kani sound-proof tuning — cheap proofs without losing soundness

Branch `kani-tune` at `/home/monopoly/kani-tune-wt`.
Goal: keep unwinding assertions **ON** (sound) and bring representative
harnesses under ~5 min, after the old `--no-unwinding-checks` runs turned
out to be vacuous (truncated paths, unreachable covers).

---

## 1. Why the old numbers were lies

`--no-unwinding-checks` lets CBMC stop unwinding a loop and still print
`VERIFICATION:- SUCCESSFUL`. Every `assert` / `cover` below the cut is then
unreachable. The Morfonica unit caught this (unwind 24 over `invoke::find`'s
RULESET scan); the money pilot's 85 s "SUCCESS" was produced the same way.

With assertions ON, a too-small bound is a **loud** `unwinding assertion`
failure (or, at a large bound, an explosion of path splits). Neither is
acceptable: we need the *minimal sound* bound per harness, and a symbolic
model whose loops are actually that small.

**Key fact:** unwinding assertions are about the **static trip count**.
`while i < runtime_n` with early exit still demands
`unwind >= max(runtime_n)`. Only a constant bound (or a chunked rewrite)
moves the needle.

---

## 2. Model bound changes (this branch)

| site | before | after | why safe |
|---|---|---|---|
| `card-sdk/src/intern.rs` `intern_bytes` | 32-slot scan × up to 48-byte `bytes_eq` (kept scanning after mismatch) | open-addressed 64-slot table, **8 probes**, FNV hash-first, **8×8** byte walks, `MAX_NAME=56` | hash is unique-checked; probe exhaustion / overlong name → loud `INVALID` (`intern_or_fail` panics) |
| `board::tile_named[_bytes]` | 60-name scan | FNV table + binary search (~6 int compares), byte-compare the hit | hashes strictly sorted & unique (`tests/model_bounds.rs`) |
| `invoke::by_idx` | 17-band integer walk | const `BAND_START[18]` + binary search | `band_starts_match_ruleset` pins offsets to the live tables |
| `world::NameMap` | `MAX_KEYS=32`, `while i < self.n` (u8!) | `MAX_KEYS=8`, loops bound on `MAX_KEYS` | full map overwrites slot 0 (already documented); `conf_model` never needs >8 keys |
| `host::name_eq` | `while i < n` (≤64) | 8×8 chunk walk | same compare, fixed trip count |
| `host::owned_count` / `owned_at` | `while i < 60` | 8×8 chunk walk over tiles | same result, bound 8 |
| `cond-ir` `named_get` / `bin_get` / `*_set` | `iter().find()` | `while` + early exit | same semantics; iterator chains unroll badly |
| `cond-ir` `eval` `CounterIs` | `by_flat("counter_name")` string scan | const `VarId::CounterName as u16` | one true index |
| `ir_fill::id_of_bytes` / `owned_tiles` / `fill_board_sets` / VARS OptInt sweep / 60-entry `tile_named` fill | 60 / 43 trip loops | 8×8 chunk walks | same values |
| `ir_fill::fill_for_guard` | (new) | walks the static IR arena (fixed 32 nodes) and fills **only** what the guard reads; skips geometry bins / board sets / 60-name fill unless the program uses those `FnId`s | `conf_fill_diff` still pins full `fill_from_world` |
| `cond-ir::id_of` | `for b in name.as_bytes()` (CJK 16–52) | `const fn` 8×8 chunk; `TILE_NAME_IDS` / `ID_TOK_PP_*` precomputed | same FNV hash |
| `cond-ir` `named_get`/`named_set` | 80-slot walk for every insert | `n ≤ 4` fast path (4-iter walk); 10×8 walk only for the full 60-name table | same semantics |
| `host` `player_count` | returns raw `n_players` | `kani::assume(N ∈ 2..=4)` at the ABI | card bodies write `0..player_count()`; an unconstrained `i32` Range hit *"Not unwinding … iteration 24"* on `poorest` |

`fill_pay_add` (the money-shave light path) is unchanged.

---

## 3. Per-harness unwind

`tools/kani/run.sh` resolves the bound in this order:

1. CLI `--unwind N` (forces every selected harness)
2. `rules/kani/unwind_table.toml` → `harness = N`
3. `#[kani::unwind(N)]` on the harness (also accepted inside `clause_proof!`)
4. `DEFAULT_UNWIND=24` (was a global 80)

Unwinding assertions stay ON. A too-small bound is verdict **UNWINDING**
(loud, fail-closed) — never a silent `SUCCESSFUL`. `--no-unwinding-checks`
caps the verdict at **UNSOUND** and can never be reported as a proof.

### Calibration

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target-tune CARGO_BUILD_JOBS=3
python3 tools/kani/calibrate.py --unit money_shave \
    --harness aya_haruka_02_shave_iff --timeout 3600 \
    --mutate 2 --budget 7200
```

Binary-searches N in `2..256` for the smallest sound bound, writes
`rules/kani/unwind_table.toml`, and **mutation-checks**: a deliberately
too-small N must FAIL with an unwinding assertion (exit 3 if it does not —
that would mean the pipeline could silently regress to vacuous proofs).

---

## 4. Timing table

Filled in as runs complete. Method: one serial CBMC job
(`bash tools/kani/run.sh --unit U --harness NAME --jobs 1 --timeout 3600`),
machine-wide cap 2 (one slot reserved for the concurrent central pass).

### Profile of `aya_haruka_02_shave_iff` (sound, unwind 24, mid-tuning)

CBMC `Unwinding loop …` lines, counted by function (iterations × loops):

| function | first profile | after intern+id_of+player_count | what |
|---|---:|---:|---|
| `rules_cond_ir::ctx::id_of` | 144 | **0** | FNV over CJK literals; now `const fn` + `TILE_NAME_IDS` / `ID_TOK_PP_*` |
| `card_sdk::intern::intern_bytes` / `fnv1a` / `bytes_eq` | 3600 s stall | **0** | 8-probe open addressing, 8×8 byte walks |
| `IrCtx::named_set` | 144 | 180 → ~4 | 4-iter fast path when `n ≤ 4` (light fills) |
| `poorest` / `Range::try_fold` | 23 (hit unwind 24: *"Not unwinding"*) | **4** | card body `(0..player_count()).all(...)`; `player_count` now `kani::assume(N∈2..=4)` at the ABI |
| `memcmp` | 14 | 14 | `String` / slice compares in unmodified bodies — remaining cliff |
| `invoke::by_idx` | 10 | 8 | binary search |

| harness | before (sound, global 80) | after (tuned model + per-harness unwind) | verdict | covers |
|---|---|---|---|---|
| `money_shave::aya_haruka_02_shave_iff` | >50 min stuck in `intern_bytes` | _in flight_ | | |
| `wrong_guard::wrong_guard_too_loose_is_caught` | attr unwind 6 | _pending_ | | |
| `wrong_guard::real_guard_matches_clause_iff` | attr unwind 6 | _pending_ | | |
| `teleport::tae_sound_teleport_and_fire_iff` | attr unwind 80 | _pending_ | | |
| `ppp::wait_aoki_01_place_on_field_iff` | 195 s | _pending_ | | |
| `ppp::bang_dream_01_crystal_iff` | attr unwind 32 | _pending_ | | |
| `general::fever_04_*` | FAILED unwinding assertion at `fill_from_world` | now `fill_for_guard` | | |
| `hostops_smoke::rent_of_property_table` | FAILED unwinding assertion at `intern_bytes` | intern is 8-probe | | |
| `prompt::aya_haruka_01_empty_discard` | ~194 s | _pending_ | | |

---

## 5. Vacuity checks

For each re-timed harness, confirm:

1. covers are **SATISFIED** (`kani::cover!` counts, no "unreachable");
2. code after the card body is reachable — temporarily `assert!(false)`
   after the invoke and confirm the run **FAILS**.

Record results here. A proof that cannot fail the post-invoke assert is
vacuous and must not be reported as SUCCESSFUL.

---

## 6. What remains expensive

| cliff | who | why |
|---|---|---|
| `String` / `Vec` drop-glue + `memcmp` in **unmodified card bodies** | `aya_haruka_01_prompt_all_picks`, `riot_02_03_cycle`, any `ids.contains(&c)` path | heap `String` length stays symbolic; needs a `cfg(kani)` interned-str facade in card bodies or a 10× CPU budget. **Not a model-loop problem.** |
| `fill_geometry_bins` (60 × seats × radius) | guards using `players_on` / `owned_within` / `on_path` | still a full board walk; `fill_for_guard` skips it unless the IR calls those `FnId`s |
| `fill_from_world` full path | proofs that have not yet moved to `fill_for_guard` | kept for `conf_fill_diff`; proofs should call `fill_for_guard` |
| multi-turn / chain / settle | — | **PROTOCOL-LEVEL** (`KANI.md` §6); Kani proves a straight-line body only |

---

## 7. Verification checklist

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target-tune CARGO_BUILD_JOBS=3
cargo build -p game-rules -p rules-native -p rules-kani
cargo test -p rules-kani            # 3× deterministic
cargo test -p rules-cond --features lower
cargo check --manifest-path rules/Cargo.toml --workspace --target wasm32-unknown-unknown
```

`rules-kani` suites: `conf_fill_diff`, `conf_hostops2`, `conf_model`,
`pilot_guards`, `model_bounds`. No `#[ignore]`, no skip list.

---

## 8. Rules we did not break

* Unwinding assertions stay ON. `--no-unwinding-checks` is exploration-only
  and can never be reported as SUCCESSFUL.
* Every bound that truncates real behaviour is a loud `INVALID` / panic /
  documented `NameMap` overflow, never a silent zero.
* Card bodies under `rules/cards/**` are unmodified.
* `conf_fill_diff` / `conf_model` / `conf_hostops2` / `pilot_guards` green.
* Spec text still comes from the 规则书 (clause docs), not engine behaviour.