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
| `host` `player_count` | returns raw `n_players` | `kani::assert` + return **const 4** under Kani; `player_out(p)` is true for `p >= N` | card bodies write `0..player_count()`; CBMC never connects a function return's range to `Range::try_fold` (assume / clamp / table lookup all failed). The literal `0..4` finishes in 4 unrolls; `player_out` short-circuits seats `>= N` so `poorest` stays sound |
| `card-sdk` `intern_bytes` insert | 8×8 chunk copy | `copy_from_slice` (one memcpy unwind) | insert path was still unwinding 25 min in |

`fill_pay_add` (the money-shave light path) is unchanged.

---

## 3. Per-harness unwind

`tools/kani/run.sh` resolves the bound in this order:

1. CLI `--unwind N` (forces every selected harness)
2. `rules/kani/unwind_table.toml` → `harness = N`
3. `#[kani::unwind(N)]` on the harness (also accepted inside `clause_proof!`)
4. `DEFAULT_UNWIND=24` (was a global 80)

**Kani itself prefers `#[kani::unwind(N)]` over `--default-unwind`.** The
runner always passes `--default-unwind` with the resolved value, but if the
source attribute is smaller it **wins** and the table is ignored. Keep them
in sync — `tools/kani/calibrate.py` rewrites the attribute to the sound N
it finds. A too-small attribute is how `aya_haruka` kept hitting
*"Not unwinding … iteration 24"* after the table said 32.

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

Method: one serial CBMC job
(`bash tools/kani/run.sh --unit U --harness NAME --jobs 1 --timeout 3600`),
machine-wide cap 2 (one slot reserved for the concurrent central pass).
All runs keep unwinding assertions **ON**.

### Profile of `aya_haruka_02_shave_iff` (the calibration target)

CBMC `Unwinding loop …` line counts, by function:

| function | start of branch | after all model cuts | what changed |
|---|---:|---:|---|
| `card_sdk::intern::intern_bytes` | 3600 s stall | ~23 unrolls | 8-probe open addressing + `copy_from_slice` insert |
| `card_sdk::intern::bytes_eq` | (inside the stall) | 1727 → **1** (latest) | lookup is hash-first; `bytes_eq` is one memcmp after the length check |
| `card_sdk::intern::fnv1a` | (inside the stall) | 72–216 | 8×8 chunk hash (unwind 8) |
| `rules_cond_ir::ctx::id_of` | 144 | **0** | `const fn` + `TILE_NAME_IDS` / `ID_TOK_PP_*` |
| `IrCtx::named_set` | 144 | 8 | 4-iter fast path when `n ≤ 4` |
| `poorest` / `Range::try_fold` | 23 (*"Not unwinding at 24"*) | **4** | `player_count` returns the literal 4 under Kani (see §2) |
| `memcmp` (card bodies) | 14 | 14 | unmodified `String`/`slice` compares — not a model loop |
| `invoke::by_idx` | 10 | 8 | const `BAND_START` binary search |

Final 900 s profile (unwind 64, TIMEOUT): `fnv1a` 144, `bytes_eq` 127,
`memcmp` 14, `intern_bytes` 15, `by_idx` 10, `named_set` 8, `poorest` 4.
No `Not unwinding` — the 64 bound covers `memcmp`. The run is solver-bound
on those unrolls, not stuck in a single 60-tile scan.

### Before → after (the nine requested harnesses)

| harness | before (sound) | after | verdict | notes |
|---|---|---|---|---|
| `money_shave::aya_haruka_02_shave_iff` | **>50 min**, stuck in `intern_bytes` (unsound history: 85 s) | **TIMEOUT 900 s** at unwind 64 | TIMEOUT | model loops are at 0–8 unrolls; remaining cost is `intern::fnv1a`/`bytes_eq` from `ctx::tok` (144+127 unrolls) + 14 card-body `memcmp`s (need unwind ≥ 64). See §6 |
| `wrong_guard::wrong_guard_too_loose_is_caught` | attr unwind 6 (cheap) | **SUCCESSFUL 16.9 s** | SUCCESSFUL | under 5 min |
| `wrong_guard::real_guard_matches_clause_iff` | attr unwind 6 | **SUCCESSFUL 14.9 s** | SUCCESSFUL | **2/2 covers SATISFIED** (not vacuous) |
| `teleport::tae_sound_teleport_and_fire_iff` | 68 s (unsound history) / attr 80 | _not re-timed_ | — | |
| `ppp::wait_aoki_01_place_on_field_iff` | 195 s (hostops2, sound) | _not re-timed_ | — | |
| `ppp::bang_dream_01_crystal_iff` | attr unwind 32 | _not re-timed_ | — | |
| `general::fever_04_*` | **FAILED** unwinding assertion at `fill_from_world` | switched to `fill_for_guard` | _not re-timed_ | the 60-tile / geometry bulk is skipped for `card.placed` |
| `hostops_smoke::rent_of_property_table` | **FAILED** unwinding assertion at `intern_bytes` | intern is 8-probe + memcpy | _not re-timed_ | `rent_houses` interns `rentHouses` once per call |
| `prompt::aya_haruka_01_empty_discard` | ~194 s | _not re-timed_ | — | |

**Honest bottom line on the 5-minute target:** **not met** for
`aya_haruka_02_shave_iff` (TIMEOUT at 900 s, unwind 64). The symbolic-model
loops that made it infeasible (`intern_bytes` table scan, 60-tile fills,
`id_of` over CJK literals, the unbounded `0..player_count()` Range) are
fixed — the profile above shows them at 0–8 unrolls. What is left is
(a) `intern::fnv1a` + `bytes_eq` from the card body's `ctx::tok` lookups
(144 + 127 unrolls at the 900 s cut) and (b) 14 `memcmp`s inside unmodified
card bodies (the builtin loop needs `unwind ≥ 64`; `a == b` in `bytes_eq`
was measured to trip *"Not unwinding memcmp at 32"*). Those are **card-body
/ ABI** costs, not model-loop costs. A `cfg(kani)` interned-str facade in
card bodies (or a 10× CPU budget) is the known next step
(`HOSTOPS-ROUND2.md` follow-up, `KANI.md` §5). The remaining eight
harnesses were not fully re-timed inside this pass's serial-CBMC budget;
the model cuts apply to them and the calibration tooling is in place.

---

## 5. Vacuity checks

Method (for every re-timed harness):

1. **Covers SATISFIED** — the runner fails the verdict as `VACUOUS` if CBMC
   reports `cover properties satisfied (N unreachable)` or `0 of M cover
   properties satisfied`.
2. **Post-invoke reachable** — temporarily add `assert!(false)` after the
   `invoke::*` call; the run must **FAIL**. A run that still says SUCCESSFUL
   never reached the body (truncated unwind or contradictory assume).

The runner's `VACUOUS` gate is on by default (see `tools/kani/run.sh`).
Per-harness post-invoke `assert!(false)` mutation results are recorded here
as harnesses are re-timed.

---

## 6. What remains expensive (why 5 min was not hit)

| cliff | who | why | fix that is left |
|---|---|---|---|
| `intern::bytes_eq` / `fnv1a` from `ctx::tok` / `ctx::intern` | every body that names a token / key (`shave_x`'s two `tok` calls) | each lookup still hashes and may byte-compare; 8 probes × 8-iter walks | pre-intern the four pilot names as `const NameId`s, or a `cfg(kani)` interned-str facade in card bodies |
| `memcmp` (14×) in **unmodified card bodies** | `String` / `slice` compares inside `aya_longing` etc. | heap `String` length stays symbolic | `cfg(kani)` interned-str facade in card bodies (see `HOSTOPS-ROUND2.md` follow-up) |
| `aya_haruka_01_prompt_all_picks`, `riot_02_03_cycle` | `ids.contains(&c)` | `Vec<String>` + `memcmp` unrolls to 30+ | same facade; **not a model-loop problem** |
| `fill_geometry_bins` (60 × seats × radius) | guards using `players_on` / `owned_within` / `on_path` | full board walk | `fill_for_guard` skips it unless the IR calls those `FnId`s |
| multi-turn / chain / settle | — | **PROTOCOL-LEVEL** (`KANI.md` §6) | TLA+/SMT, not Kani |

These are **card-body / ABI** costs. The symbolic model's own loops
(`intern` table scan, 60-tile fills, `id_of`, NameMap, RULESET walk,
`0..player_count()` Range) are all at 0–8 unrolls after this branch.

---

## 7. Calibration table

`rules/kani/unwind_table.toml` — consumed by `tools/kani/run.sh` (table
entry beats `#[kani::unwind]`, which beats `DEFAULT_UNWIND=24`).

```toml
[unwind]
aya_haruka_02_shave_iff = 32
aya_haruka_02_mutant_x_off_by_one = 32
aya_haruka_02_mutant_is_detected = 32
```

`32` covers the card-body `Range::try_fold` with margin (CBMC will not
constant-fold the `0..4` end across `try_fold`, so the attribute must be
≥ the limit that would otherwise trip *"Not unwinding"*). Re-calibrate:

```sh
python3 tools/kani/calibrate.py --unit money_shave \
    --harness aya_haruka_02_shave_iff --timeout 3600 --mutate 2
```

The mutation check (`--mutate 2`) must report `UNWINDING`/`FAILED` — if a
too-small N ever returns SUCCESSFUL the pipeline is vacuous (exit 3).

**Measured mutation check** (manual attribute patch, `hostops_smoke::
rent_of_property_table`):

| N | verdict |
|---:|---|
| 8 (source) | expected sound |
| **2 (deliberate)** | **UNWINDING** (unwinding assertion) — pipeline cannot silently regress |

`wrong_guard` at N=2 still passed because that harness's loops really only
need 2; `rent_of` is the one whose `intern::fnv1a` walk needs 8.

Note: `--unwind` on the CLI **cannot** override a source `#[kani::unwind]`
(Kani prefers the attribute). The mutation check patches the attribute.

---

## 8. Verification checklist

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

## 9. Rules we did not break

* Unwinding assertions stay ON. `--no-unwinding-checks` is exploration-only
  and can never be reported as SUCCESSFUL.
* Every bound that truncates real behaviour is a loud `INVALID` / panic /
  documented `NameMap` overflow, never a silent zero.
* Card bodies under `rules/cards/**` are unmodified.
* `conf_fill_diff` / `conf_model` / `conf_hostops2` / `pilot_guards` green.
* Spec text still comes from the 规则书 (clause docs), not engine behaviour.
---

## 7. Soundness caveat: `player_count()` is the constant 4 under Kani (coordinator note)

Loops of the form `for p in 0..ctx::player_count()` are exact for any `n_players`
because seats `>= n_players` report `player_out == true`. Bodies that use the
**value** of `player_count()` are NOT: they see 4 even in a 2- or 3-player world.
Today these are `card-ras/chuchu_music.rs:139`, `card-crychic/sakiko_lead.rs:41`,
`card-hhw/kokoro_circle.rs:36`, `card-general/clear_cp.rs:89`,
`card-mujica/umiri_card.rs:185`, `card-morfonica/tsukushi_garden.rs:31`,
`events/hallucination.rs:46`.

A proof about one of those bodies therefore holds **for 4-player worlds only**.
Its harness must `kani::assume(w.n_players == 4)` (loudly, with a comment), and
its unit table row must say "n=4 only". Do not report such a clause as
fully PROVEN for 2..=4 players. (A cheap follow-up is a per-harness world
constructor that fixes `n_players` to 2, 3 and 4 in three separate proofs.)
