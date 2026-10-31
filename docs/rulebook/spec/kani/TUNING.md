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

## 10. String facade (branch `kani-strfacade`)

Goal: make string-keyed code in card bodies cheap under Kani **without
editing card bodies** (byte-for-byte unmodified, wasm32 + native unchanged
in behaviour). All changes are `cfg(kani)`-gated or live in `rules-kani`.

### Method

| piece | what |
|---|---|
| `card-sdk/src/intern.rs` | `fnv1a` / `bytes_eq` / probe / insert are **zero-loop** under `cfg(kani)` (64-way `macro_rules!` steps, `.get` not `[i]` — no `slice_index_fail` panic paths). `intern_by_hash(h, s)` takes a precomputed FNV so an inlined `s()` constant-folds a literal's hash. Production (`cfg(not(kani))`) keeps master's 8×8 loops byte-for-byte. |
| `card-sdk/src/ctx.rs` `s()` | under `cfg(kani)` returns `(INTERN_TAG \| NameId, len)` — the handle *is* the intern id. No arena `Vec`, no host re-hash. `#[cfg_attr(kani, inline(always))]` on every `&str` wrapper so a card body's `ctx::tok(p, "literal")` constant-folds the hash through the chain. |
| `card-sdk/src/native.rs` `read` | tagged handles resolve from the intern table (static bytes). Untagged handles stay arena out-buffers. |
| `rules/kani/src/host.rs` | `name_id` decodes the tag (O(1)). `Known` pre-interns pilot names at `install()`; `key == intern::intern(TOK_PP_POS)` becomes `key == known().tok_pp_pos` (`u16` compare). `card_slot` / `card_u16` / `card_slot_of_id` are NameId compares. |
| `rules/kani/src/world.rs` | `sync_views` / `rent_houses` cache `NameId`s once per proof. `card_slot_of_bytes` zero-loop. |
| `rules/kani/src/invoke.rs` | `hook_idx` / `hook_kind_matches` parse `"PaySplitAdd"` / `"Hook"` to `HookKind` / `KindTag` and compare integers — kills the 14 `memcmp`s of short ASCII tags. |
| `rules/kani/src/board.rs` | `bytes_eq` zero-loop (same bits as the old 8×8 walk). |

Card bodies still see `Vec<String>` / `String`. `ctx::cards_in` already
returns 1-byte facade strings under `cfg(kani)` (HOSTOPS-ROUND2); combined
with the NameId ABI, `ids.contains(&c)` and `take_card(..., &id)` are
1-byte / `u16` compares.

### What is now cheap

| was | now |
|---|---|
| `fnv1a` 144 unrolls, `bytes_eq` 127, `intern_bytes` 15 (900 s profile of `aya_haruka_02_shave_iff`) | **0** unwind (zero-loop); hash of a literal constant-folds through `inline(always)` |
| `intern::intern(CONST)` on every `count_held` / `set_held` / `crystals` check | pre-interned `Known` ids — `u16` compare |
| 14 `memcmp`s of `"PaySplitAdd"` etc. in `invoke::hook_idx` | integer `KindTag` / `HookKind` match |
| `copy_from_slice` `len_mismatch_fail` panic paths in intern insert | zero-loop `.get` byte copy |
| host `name_eq` 8×8 byte walk over CJK | `NameId` compare for the four pilots; `.get` walk otherwise |

### Measurements (one serial CBMC job, unwinding assertions ON)

| harness | before (TUNING.md §4) | after | verdict |
|---|---|---|---|
| `hostops_smoke::rent_of_property_table` | **FAILED** unwinding at `intern_bytes` | **SUCCESSFUL 59 s** at unwind 12 | SUCCESSFUL |
| `money_shave::aya_haruka_02_shave_iff` | **TIMEOUT 900 s** (fnv1a 144 + bytes_eq 127 + memcmp 14) | **TIMEOUT 1800 s** at unwind 32; profile has **0** fnv1a/bytes_eq/intern_bytes/memcmp unwinds | TIMEOUT — string cliff gone; solver-bound on symbolic world + `poorest` `Range::try_fold` |
| `prompt::aya_haruka_01_empty_discard` | ~194 s (old, not re-timed) | **SUCCESSFUL 573 s** at unwind 24 | SUCCESSFUL (under 10 min) |
| `prompt::aya_haruka_01_prompt_all_picks_iff` | timeout / `Vec<String>` cliff | _not re-timed_ | — |
| `ras::riot_02_03_cycle_iff` | TIMEOUT 600 s | _not re-timed_ | — |
| `ppp::wait_aoki_01_place_on_field_iff` | 195 s SUCCESSFUL | **SUCCESSFUL 414 s** at unwind 32 | SUCCESSFUL (under 10 min) |
| `teleport::tae_sound_teleport_and_fire_iff` | 68 s (unsound history) | _not re-timed_ | — |
| `general::fever_04_turn_start_iff` | FAILED unwinding at `fill_from_world` | _not re-timed_ | — |
| `money_shave::aya_haruka_02_mutant_is_detected` | attr 64 | table 32 | _not re-timed_ |

`aya_haruka_02_shave_iff` profile after the facade: **no** `fnv1a` /
`bytes_eq` / `intern_bytes` / `memcmp` unwinds. Remaining cost is the
symbolic world (money/out/pos of 4 seats), `poorest`'s `0..player_count()`
`Range::try_fold` (4 unrolls), and `invoke::by_idx` (binary search). The
string cliff is gone; the proof is now model-bound.

### Per-card body trampolines (batch 3)

`invoke::entry_idx` under `cfg(kani)` dispatches through `call_body_idx` →
`call_card_N`, generated at build time by `rules/kani/build.rs`
(`gen_body_dispatch`) into `$OUT_DIR/body_dispatch_fn.rs` (never committed).
A shared `body_of(on)(player)` `code*` site made CBMC expand an IF-chain over
every address-taken `fn(i32) -> Asked` (874–4313 targets) — the
`duobang_01_02` SAT cliff (680 s → TIMEOUT). See TRIAGE-LOG batch 3.

### Remaining intractable patterns

| pattern | why | affected clauses |
|---|---|---|
| `Vec<String>` / `String` drop glue in card bodies (`ids: Vec<String>`, `push`, `swap_remove`) | heap metadata stays symbolic to CBMC; cannot replace `String` without editing the body's type annotation (`Vec<alloc::string::String>` is named). **Mitigated** by `kani_alloc` bump (see §11) + static 1-byte facade strings | `aya_longing` Play (prompt class), any `cards_in` consumer |
| `ids.contains(&c)` over a growable `Vec` | `Vec::contains` unrolls the grow path; 1-byte facade makes each `eq` cheap but the iteration bound is still `ids.len()` | prompt / all-picks / riot cycle |
| `ctx::player_count()` **value** (not the loop) | documented §7 caveat — unchanged | `chuchu_music`, `sakiko_lead`, `kokoro_circle`, `clear_cp`, `umiri_card`, `tsukushi_garden`, `hallucination` |
| multi-turn / chain / settle | PROTOCOL (`KANI.md` §6) | all chain windows |

### Wasm / native hash note

`node tools/build-ruleset.mjs` **without** `CARGO_TARGET_DIR`, compared
against the **merge-base** (`10ddefe`) `card-sdk` (swap
`intern.rs`/`ctx.rs`/`native.rs` only — those are the files this branch
touches):

| artifact | result |
|---|---|
| `dist/cards/conds-*.bin` | **identical** |
| `dist/fixtures/*.wasm` | **identical** |
| `dist/cards/*.wasm` (`card_all`) | **differs by 21 bytes** (411940 vs 411919) |

The 21-byte delta is residual codegen / panic-string drift from
`cfg(kani)` scaffolding in shared `card-sdk` files. Non-kani logic is the
merge-base code (`cfg(not(kani))` twins are byte-for-byte). Card behaviour
on wasm is unchanged. Method: `sha256sum dist/cards/* dist/fixtures/*`
after two `build-ruleset` runs.

### Unwind table updates

```toml
aya_haruka_02_shave_iff = 32          # was 64
aya_haruka_02_mutant_x_off_by_one = 32
aya_haruka_02_mutant_is_detected = 32
rent_of_property_table = 12           # MAX_FIELD × NameMap nested scans
```

Re-calibrate with `tools/kani/calibrate.py --mutate 2` (too-small N must
FAIL with an unwinding assertion).

**Measured mutation check** (`hostops_smoke::rent_of_property_table`, manual
attribute patch):

| N | verdict |
|---:|---|
| 12 (sound) | SUCCESSFUL 59 s |
| **2 (deliberate)** | **UNWINDING** (unwinding assertion) — pipeline cannot silently regress |


---

## 11. String/Vec facades (branch `kani-strvec`)

Goal: kill the remaining `extend_with` / `drop_glue` / `memcmp` / `ask_card`
unwinds that time out the prompt / money_shave / front_or_back rows. All
`cfg(kani)` (production dist hashes **byte-identical** to master). See
TRIAGE-LOG `strvec` for the before/after table.

| piece | what |
|---|---|
| `card-sdk/kani_alloc.rs` | static 16 KiB bump `#[global_allocator]`: `alloc`=cursor, `dealloc`=no-op. Unmodified bodies' `String`/`Vec` grow/drop is array arithmetic, not CBMC heap objects |
| `card-sdk/native.rs` arena | fixed `[[[u8;64];8];8]` frames (was `Vec<Vec<Vec<u8>>>`); zero-loop intern/read/write/pad |
| `out_buf`/`out_read` | no `vec![0u8; cap]` (`extend_with` 31); empty scratch + fixed arena pull |
| `ask_*` (pick/card/player/tile/number) | 4-slot branch trees instead of `for` over symbolic-length slices (`ctx::ask_card` 31) |
| `split_pay` / `draw` | zero-loop filter/leg/draw walks |
| `cards_in` facade strings | static 1-byte buffers (`String::from_raw_parts`), no heap; `eq` is one byte |
| `cond-ir` `named_get`/`named_set` | zero-loop 80-step scan (`.get`, no index panic) |
| `invoke` hook-kind match | zero-loop 4-slot (`kinds.iter().any` gone) |
| events specs / `moca_02` scan | zero-loop step macros over the effect log |

Residual (cannot fix without a body edit): the bodies' own `Vec<String>`
(`aya_longing` `ids`) and `Vec<i32>` (`front_or_back` `front`/`back`/`winners`)
still have `Vec::len`-bounded loops. The bump allocator makes each iteration
cheap; the loop bound is the global unwind if `len` is not folded.


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

---

## 12. class-E deep dive — library stubs + global kani-bump (branch `kani-triage3`)

Goal: a **sound general lever** for the persistent class-E TIMEOUT rows
(fmt/memchr, RawVec grow, panic-message assembly). Stubs are **library
only** — never a card body, skill body, event body, host arm, or spec.

### Levers

| lever | what | soundness |
|---|---|---|
| **kani-bump always-on** | `card-sdk/kani_alloc` under every `cfg(kani)` build (was opt-in per band-mygo/band-ras) | validated on `skills_mygo::band_04` (SUCCESSFUL 3/3 covers) and sumimi twoinone before the flip. Bump arena returns real pointers; `dealloc` is a no-op. Feature `kani-bump` stays as a no-op alias. |
| **`LIB_COST` stub set** | `memchr`/`memchr_naive`/`memchr_aligned` → 128-step `.get` scan; `slice_index_fail` / `handle_alloc_error` / `capacity_overflow` / `handle_error` → `kani::assert(false)` + diverge | pure search / loud-fail paths. Panic and OOM are loud fails either way; only the fmt/memchr of the *message* is dropped. |
| **`LIB_COST_NOFMT`** | `LIB_COST` + `panic_fmt` + `alloc::fmt::format` | `panic_fmt` stub is **not** on `#[kani::should_panic]` mutants (they need a real `panic!` event). `format!` keeps `as_str()` exact; argument formats use a 64-byte `String` + `write_fmt` (no `estimated_capacity` loop). Key bytes stay get/set-consistent within a proof; the proof never pins key bytes to production. |
| **runner** | `tools/kani/run.sh` always passes `-Z stubbing`; `KANI_EXTRA_ARGS` exploration hook (default empty) | |

Every `#[kani::proof]` gets `#[kani::use_stub_set(crate::kani_stubs::LIB_COST[_NOFMT])]`
(603 non-mutant → NOFMT, 41 `should_panic` mutants → LIB_COST).

### Tried and rejected

| idea | why not |
|---|---|
| `Formatter::pad` / `pad_integral` stubs | Kani rejects: `mismatch in the number of generic parameters` (`Formatter::<'a>::pad` counts 1 generic; a free `fn<'a>` stub counts 0). |
| `PadAdapter::write_str` stub | `core::fmt::builders` is private — cannot name the type in the stub signature. |
| `--no-unwinding-checks` | unsound (vacuous proofs); never used. |
| `-Z stubbing` on production bodies | not allowed (card bodies stay unmodified). |

### Profile delta — `skills_pp::aya_02_pay_discount_iff` (TIMEOUT 1800 s central)

| metric | central (no stubs) | + kani-bump + LIB_COST | + NOFMT (panic_fmt + format) |
|---|---|---|---|
| `memchr_naive` unrolls | 1868 | **0** | **0** |
| `memchr_aligned` | 574 | **0** | **0** |
| `memcmp` | 558 | (see run) | (see run) |
| `slice_index_fail` | 38 | **0** | **0** |
| `RawVec handle_error` | 27 | stubbed (assert) | stubbed |
| `PadAdapter` recursion | 1 | 4–6 | gone from early BMC |
| aborting paths | 5477 | 34–53 | 25 |
| top unwind | memchr 1868 | fmt/PadAdapter | `sparse_play_world` 2 + `fmt::write` 2 |
| BMC reached? | no (died in symex) | yes | yes (`Starting Bounded Model Checking`) |

The string/fmt cliff is gone. Residual is `fmt::write` (2 unrolls via
`format!`'s `write_fmt`) + `RawVec::with_capacity_in` abort paths + the
sparse symbolic world itself. See TRIAGE-LOG `class-E deep dive` for the
per-row verdict table.

### Soundness argument per stub (coordinator item C)

| stub | what the real function does | why the replacement is equivalent | what it is NOT allowed to change |
|---|---|---|---|
| `memchr` / `memchr_naive` / `memchr_aligned` | first index of byte `n` in `text`, or `None` | pure search. Stub is a 128-step `.get` scan; same `Option<usize>` for every haystack of length ≤ 128. **Model bound:** every `format!` key / panic snippet / `split('\n')` piece in this corpus is ≤ 128 bytes (`skill.tsukushi.rec:3` = 20; panic texts ~50). Longer haystacks are a **loud** `kani::assert(false)` — never a silent wrong index. | result value |
| `slice_index_fail(start,end,len)` | formats "index out of bounds: …" then panics (`-> !`) | reachable OOB is a loud fail either way. Stub: `kani::assert(false)` + diverge. Only the fmt/memchr of the *message* is dropped. | reachability of the OOB |
| `handle_alloc_error(layout)` | formats the layout then aborts (`-> !`) | reachable OOM is a loud fail either way. Stub: `kani::assert(false)` + diverge. | reachability of OOM |
| `capacity_overflow` / `handle_error` | panics / dispatches to the two above (`-> !`) | same: loud fail either way. `handle_error`'s `match` is the cost; the arms are already stubbed. | reachability of grow failure |
| `panic_fmt(fmt)` | formats `fmt` then panics (`-> !`) | reachable panic is a loud fail either way for **iff/no-op** harnesses. Stub: `kani::assert(false)` + diverge — the proof still **FAILS** on a non-mutant panic. **Not applied** to `#[kani::should_panic]` mutants (they need a real `panic!` event; those keep `LIB_COST` without `panic_fmt`). | (a) iff/no-op: panic ⇒ FAIL; (b) mutants: panic still observed |
| `alloc::fmt::format` (`format!`) | `as_str().to_owned()` or `write_fmt` into a `String` | `as_str()` branch is **exact**. Argument branch: 64-byte `String` + `write_fmt` (skips `estimated_capacity` loop). Key bytes are get/set-consistent within a proof because every `format!` site shares this stub. The proof never pins key bytes to production spellings. | get/set round-trip of the same `format!` expression |
| `<{u32,i32,i64} as Display>::fmt` | `_fmt_inner` → `pad_integral` → possible `PostPadding`/`PadAdapter` | same decimal digits / sign as production for `{}` and `format!("{p}")` (the only forms in the corpus). Loud FAIL if `width`/`precision` is set (corpus never uses `{:.width$}` on these). | the string produced for no-width `{}` |

**Non-mutant panic:** `panic_fmt` stub is `kani::assert(false, …)` — a failed
CBMC property. `VERIFICATION` is **FAILED**, never SUCCESSFUL. Covers on the
panic path stay unreachable (same as a real panic aborting the run).

**memchr > 128:** loud FAIL, not a wrong index. If a future body formats a
string longer than 128, the proof fails closed and the stub cap is raised
with a comment.

### Non-vacuity

Covers + post-invoke `assert!(false)` probes must be re-checked on the
first row that *finishes*. A run that still TIMEOUTs never reaches a
property verdict (covers unevaluated). The stubs themselves are fail-closed
(`kani::assert(false)` on OOB / OOM / panic / newline pad).

### Soundness kept

* Unwinding assertions ON. No `#[ignore]`, no allowlist, no weakened specs.
* Card bodies unmodified. Only `rules/kani/**` + `cfg(kani)` in `crates/card-sdk`.
* `panic_fmt` stub excluded from `should_panic` mutants.
* Production (`cfg(not(kani))`) untouched by stubs (`-Z stubbing` is a Kani
  compile flag; `kani_alloc` is `cfg(kani)` only).
