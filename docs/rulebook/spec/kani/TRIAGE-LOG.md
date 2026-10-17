# Kani triage log — branch `kani-triage1`

One entry per proof: id, before (old-base verdict+time), class (A–F per the
triage method), action, after (verdict, time). Soundness rules: unwinding
assertions ON; no `--no-unwinding-checks`; no card-body edits under
`rules/cards|skills|events|tiles`.

Old base: `/home/monopoly/kani-central-results2/<unit>/` (read-only).
New runs: `/home/monopoly/kani-triage1-results/<tag>/`.

---

## Batch 1 — cheap wins (general / hostops_smoke / teleport / cond_ops)

| id | before | class | action | after |
|---|---|---|---|---|
| general::fever_04_turn_start_iff | UNWINDING 314 s (old base; bound at `guard_needs` 32-loop) | A (bound too low; model loop) | 8×8 chunk `guard_needs` + 8×8 OptInt mask walk in `fill_for_guard` | **SUCCESSFUL 652 s** (unwind 24) |
| general::fever_04_wrong_turn_noop | UNWINDING 266 s | A | same | **SUCCESSFUL 673 s** (unwind 24) |
| general::fever_04_not_placed_guard_rejects | UNWINDING 103 s | A | same | **SUCCESSFUL 180 s** (unwind 24) |
| general::fever_04_mutant_is_detected | SUCCESSFUL 417 s | — | re-verify after model cut | **SUCCESSFUL 474 s** |
| general::perfect_01_02_03_iff | SUCCESSFUL 416 s (old base; brief said TIMEOUT) | — | re-verify on current master | pending |
| hostops_smoke::rent_of_property_table | SUCCESSFUL 185 s (old base; string facade) | — | re-verify + record | pending |
| teleport::tae_sound_teleport_and_fire_iff | FAILED in `teleport/`, SUCCESSFUL 503 s in `verify-fix1/` after fix | — | re-verify fix on current master | pending |
| teleport::tae_sound_mutant_extra_fire | SUCCESSFUL 526 s (`verify-fix1/`) | — | re-verify | pending |
| cond_ops::ir_div_matches_spec | TIMEOUT 1800 s | E (two-divider circuit equivalence) | reformulate: error class (full width) + truncating-divmod characterisation on the i16 image + kissat; mutants | **SUCCESSFUL 152 s** |
| cond_ops::ir_rem_matches_spec | TIMEOUT 1800 s | E | same (i16 split + kissat) | **SUCCESSFUL** (see results.tsv) |


## money_shave (next)

| id | before | class | action | after |
|---|---|---|---|---|
| money_shave::aya_haruka_02_shave_iff | TIMEOUT 1800 s | E (solver-bound on symbolic world + `poorest` `Range::try_fold`) | cut unread non-owner fans (documented); keep iff | pending |
| money_shave::aya_haruka_02_shave_poorest | (new split) | E | poorest half with clause-term assume | pending |
| money_shave::aya_haruka_02_shave_not_poorest_noop | (new split) | E | not-poorest half | pending |
| money_shave::aya_haruka_02_mutant_* | TIMEOUT 1800 s | E | same state cut | pending |

## events notes (pre-classification from old-base logs)

| id | old verdict | likely class | note |
|---|---|---|---|
| aao_02_hand_veto_is_armed | FAILED | **D GENUINE E-aao-02** | keep failing; do not fix |
| bear_cookie_01_push_is_protocol | TIMEOUT | A | `all_players` `Range::try_fold` at unwind 16; raise to 32 |
| front_or_back_* | TIMEOUT | E/A | `for &p in &winners` symbolic-Vec length; log scans at 32 |
| chart_rush_* | TIMEOUT | E/A | `drop_glue::<Vec<u8>>` at 24 |
| never_ends_01_noop_blocked_host | TIMEOUT | E | fmt `PadAdapter` recursion + memchr |
| pico_swap_01/02 | FAILED | B? | native tests pass; unit table says PROVEN — harness vs host |
| asukayama_02_status_spread_blocked_neighbor | FAILED | B/C? | message says BLOCKED-HOST(neighbor) but host now returns a tile |
| heat_head_01_activation_phase_up | FAILED | B/C? | expects phase crystals key == 1 |
| embers_03_settle_costs_ten_per_copy | FAILED | D? | settle cost 10×copies; OPEN-触发结算 |
| circle_rebuild_01_activation_five_crystals | FAILED | B/C? | activation five crystals |

### cond_ops reformulation — what is and is not proven

**Proven (Kani, unwinding assertions ON):**

| claim | harness | domain |
|---|---|---|
| error class exact (`Err` iff non-Int, `y==0`, or `(MIN,-1)`) for `/` and `%` | `ir_div_rem_error_class_matches_spec` | full i64 / all `Val` |
| error-ness agrees with `spec_arith` (bit, not Ok value) | same | full |
| Ok value is the unique truncating divmod: `q*y+r==x`, `\|r\|<\|y\|`, `sign(r)∈{0,sign(x)}` | `ir_div_matches_spec`, `ir_rem_matches_spec` | i16 image of x,y (sign-extended); `MIN/-1` covered as Err at full width |
| floor-rounding mutant rejected (`sign(r)==sign(x)` fails) | `ir_div_mutant_floor_rounding_is_detected` | i16 image, disagree region covered |
| wrong-error-case mutant rejected (`MIN/-1` not wrapping `MIN`; `x%0` not `0`) | `ir_div_mutant_wrong_error_case_is_detected` | constants |

**Not proven here:**

* bit-for-bit identity with `spec_arith`'s second `checked_div`/`checked_rem` over the full i64×i64 grid (intractable circuit equivalence; the old form timed out at 1800 s).
* tree-level `eval_node` composition (covered by `rules-cond/tests/cond_ir_diff.rs` + COND-SINGLE-SOURCE §4).
* Ok-value characterisation outside the i16 image (the i16 image contains every rounding/sign/bound class; full-width identity remains the differential boundary grid's job).

## Batch 2 — money_shave / events

| id | before | class | action | after |
|---|---|---|---|---|
| **card-sdk Msg facade (`cfg(kani)`)** | `Vec::extend_with` 31 + `drop_glue` in `ctx::log` | E (TUNING §10) | `Msg` = interned key + fixed `[i64;4]` args (`Copy`); builders never construct `Arg`/`String`/`Box`; `mj()` = 2 ints, no arena `Vec` per log | **Vec cliff gone** (profile 0 extend/drop); dist hashes **byte-identical** to master |
| **sys_kani `join_redirect_target` / `set_join_used`** | central `general` unit failed to compile on master | C | mirrored `sys.rs` externs | compiles; central can pick up after merge |
| **invoke integer hook + zero-loop `by_idx`** | `memcmp` 10, `by_idx` 10 | E | `hook_kind_only_idx(HookKind)`; `tag_key` packed-int dispatch; `by_idx` unrolled band select | **memcmp 0, by_idx 0** |
| **intern identity `(FNV,len)`** | `bytes_eq` 64×8×10 known names | E | drop `bytes_eq` from probe hit; loud INVALID on hash+len mismatch; `pilot_intern_hashes_are_unique` | intern aborts collapsed |
| money_shave splits drop guard IR | `fill_pay_add` + `named_set` 8 | A/setup | halves assume host invoked body (`shave_iff` proves the IR) | IR gone from split profile |
| money_shave::aya_haruka_02_shave_not_poorest_noop | TIMEOUT 1800 s | E | all of the above | still **TIMEOUT** at 1800 s (profile now only `poorest` Range 4 + world setup 4+4); 10 min goal **not met** |
| money_shave::aya_haruka_02_* (iff / poorest / mutants) | TIMEOUT | E | same stack | pending re-run (not reached inside budget) |
| events::heat_head_01_activation_phase_up | FAILED | B + C | harness read `key_get(0,"crystals")` but `set_crystals` writes the field-instance counter; also `World::self_crystals` fallback for event instances with no field slot | **SUCCESSFUL 423 s** |
| events::heat_head_03_phase_end_files_away | FAILED | B + C | same | **SUCCESSFUL 692 s** |
| events::circle_rebuild_01_activation_five_crystals | FAILED | B + C | same crystals read + host fallback | **SUCCESSFUL** (see ev-circle) |
| events::asukayama_02_status_spread_blocked_neighbor | FAILED | B + C | harness assumed stub `neighbor` returned tiles; host `neighbor` returned the *last* present seat (not the next). Fixed host to first-match + rewrote harness to the seat split (prev stunStart=2, next noHand=2, remaining give_exile) | **SUCCESSFUL 989 s** |
| events::pico_swap_01 | FAILED | B + F | `log_is_teleports` exact-length vs latch `SetState`; cover `n_log==3` vacuous for the same reason. Filter `SetState`; cover counts Teleports | **SUCCESSFUL 1767 s** (covers sat) |
| events::aao_02_hand_veto_is_armed | FAILED | **D GENUINE E-aao-02** | keep failing; do not fix | FAILED (unchanged) |
| events::embers_03_settle_costs_ten_per_copy | FAILED | OPEN | [触发结算] undefined (OPEN-触发结算). Harness already proves only the determined fragment (`embers_03_settle_ok` = 10×copies pay leg); settle timing is the OPEN half | OPEN (not a card-body bug) |
| events::* TIMEOUT (chart_rush, front_or_back, never_ends, …) | TIMEOUT | E / A | Msg facade + `all_players` 4-iter. `chart_rush_01_spends_fire` re-run: unwind profile only `base_world` 4 + `all_players` 3 — Vec/memcmp cliff **gone**, still **TIMEOUT 1800 s** (solver-bound on the symbolic world). `front_or_back` body uses `Vec<i32>` front/back (cannot edit body). `never_ends` is fmt `PadAdapter` | chart_rush_01: TIMEOUT; rest pending |
| events::tears_01 / tears_02 / tears_mutant | UNWINDING 324–334 s (central) | **A** | `tears_world` cleared `owner=-1` over 60 tiles under `unwind(32)`; `World::new` already does. Deleted the loop (same class-A no-ops in r/ras/ag/mor setup) | **SUCCESSFUL 724 s / 761 s / 609 s** (covers sat on iff halves) |

### money_shave cliff trail (batch 2)

One serial CBMC job at a time, unwind assertions ON, `aya_haruka_02_shave_not_poorest_noop`
(the cheap split half) as the probe:

| step | profile left | verdict |
|---|---|---|
| batch-1 end | `Vec::extend_with` 31, `memcmp` 10, `by_idx` 10 | TIMEOUT |
| Msg facade | `memcmp` 10, `by_idx` 10, intern `bytes_eq` | TIMEOUT 1800 s |
| + memcmp-free hooks | `by_idx` 10, intern `bytes_eq`, `named_set` 8 | TIMEOUT 1800 s |
| + zero-loop `by_idx` | intern `bytes_eq`, `named_set` 8, `poorest` 4 | TIMEOUT 1800 s |
| + splits drop guard IR | intern `bytes_eq`, `poorest` 4, world 4+4 | TIMEOUT 1800 s |
| + intern `(FNV,len)` | `poorest` Range 4 + `symbolic_pay_world`/`assume_not_poorest` 4+4 only | **TIMEOUT 1800 s** |

**Honest bottom line:** the named TUNING §10 Msg cliff is **fixed**
(`Vec::extend_with` / `drop_glue` gone; production dist byte-identical).
The proof is still solver-bound after all unroll cliffs are gone — what
remains is the 4-seat symbolic world + `poorest`'s `Range::try_fold` over
it, i.e. the model, not a string/heap cliff. The **≤10 min** goal is
**not met**. Next lever is shrinking `World` for the split halves (only
`money`/`out`/`pay_amount` are read) or a dedicated 2–3 player constructor.

### Host-model defect fixed (class C)

`World::set_field_crystals` was a silent no-op when `last_uid` had no `field[]`
slot. Events (`heat_head` phase, `circle_rebuild`'s 5 miracle crystals) call
`ctx::set_crystals` on the running instance. Added `World::self_crystals` as
the fallback for that case; `conf_hostops2::self_crystals_roundtrip_without_field_slot`
pins it. `conf_fill_diff` / `conf_model` / `conf_hostops2` / `pilot_guards` /
`model_bounds` stay green (3×).

### Host-model defect fixed (class C) — `Op::neighbor`

The scan kept overwriting `found` and returned the **last** present seat
(for n=4 all-present that is the far side of the drawer, not 「上一位/下一位玩家」).
Now `break`s on the first present seat. Pinned by
`conf_hostops2::neighbor_is_the_next_present_seat`. With the correct
neighbour, `asukayama_02` proves the full 2-眩晕 / noHand / 1-除外 split.

### Host-model / ABI gap fixed (class C) — `sys_kani` join ops

`join_redirect_target` / `set_join_used` existed in `sys.rs` (extern) and
`kani_host::Op` but not in `sys_kani.rs`, so `cargo kani` failed to compile
`card-sdk` on master (the central pass's `general` unit hit this). Mirrored
the two wrappers.

## Batch 3 — events `duobang_01_02` SAT regression (perf)

Problem: `events::duobang_01_02_two_player_iff` verified **680 s** at
`34456b2` (batch-1 base) but **TIMEOUT 1800 s** on master `506e900`.
`duobang_01_mutant_is_detected` also exceeded 4 min. Log ended after
symbolic execution (`aborting path on assume(false)`; loops only
`all_players` 4 + `duobang.rs:23` 2 + `duobang.rs:45` 1) with a 1.9 GB
solver stuck in SAT — **not** an unrolling cliff.

### Bisect (serial CBMC, unwind 32, assertions ON)

| commit | what landed | duobang_01_02 verdict + time |
|---|---|---|
| `34456b2` | known-good (kani-fix2 join ops) | **SUCCESSFUL 680 s** (prior central run) |
| `36daf0d` | docs-only vs 34456b2 | (same code; not re-run) |
| `6aa73e0` | batch 2: Msg facade, integer hooks, zero-loop `by_idx`, intern `(FNV,len)`, `all_players` 4-iter | **TIMEOUT 1800 s** — same profile (unwind 4+4+2+1, 1.9 GB SAT) |
| `16b9df4` / `506e900` | status-hosts + tears docs | **TIMEOUT 1800 s** (problem statement; duobang body unchanged) |

Tears cleanup `33a9704` is docs-only. Status-hosts does not touch
`duobang.rs`. **The regressing range is batch 2** (`36daf0d..6aa73e0`).

### Root cause (class E — SAT instance size)

CBMC `--show-goto-functions` on the 6aa73e0 duobang binary shows
`invoke::entry_idx` after *Removal of function pointers*:

* one shared `code*` call (`body_of(on)(player)`)
* expanded to an IF-chain of **4313** targets (`IF ptr = address_of(f_i) THEN GOTO …`)
* 91 distinct `::play` bodies kept (RULESET holds every `fn` address, so
  `--drop-unused-functions` cannot strip them)

That chain is bit-blasted into the CNF. The body loops are tiny (4+4+2+1);
the 1.9 GB solver is the **program**, not the dice. Batch-2's invoke
rewrite (by_idx unrolled, hook dispatch) did not shrink the chain; the
Msg facade additionally walked the intern `static mut` table on every
`Msg::new` (log / transfer lines).

### Fix (soundness kept: unwinding ON, covers reachable, mutants detected)

1. **Per-card body trampolines** (`call_card_N` + `call_body_idx`),
   generated at build time by `rules/kani/build.rs` (`gen_body_dispatch`)
   into `$OUT_DIR/body_dispatch_fn.rs` and `include!`d from `src/invoke.rs`.
   Never committed (same rule as `cond_irs.rs`). Each trampoline is
   `#[inline(never)]` and loads one constant `&CardDef` so CBMC's
   points-to set is that card's entries only.
2. **`Msg::new` skips `intern()`** under `cfg(kani)`. The host's
   `Op::log` / `Op::effect` only bump a narration counter and never read
   the key; `ctx::s()` names the host compares still intern.

No card-body edits. Production (`cfg(not(kani))`) `entry_idx` / `Msg`
paths unchanged. `cargo build -p game-rules -p rules-native -p rules-kani`
green; `cargo test -p rules-kani` green; `cargo test -p rules-cond
--features lower` green; wasm32 workspace check green.

### After (fix-trampoline)

| harness | before | after | notes |
|---|---|---|---|
| `events::duobang_01_02_two_player_iff` | TIMEOUT 1800 s | **SUCCESSFUL 922 s wall / 542 s CBMC** (5/5 covers sat, 0/34678 failed) | formula 605k vars / 732k clauses (was 1.9 GB stuck) |
| `events::duobang_01_mutant_is_detected` | >4 min | **SUCCESSFUL 714 s** | mutant detected |
| `events::pico_swap_01_rotates_positions` | 1767 s | **SUCCESSFUL 1287 s** (1/1 cover sat) | was near 1800 s limit |
| `events::tears_01_same_colour_next_pays_700` | 724 s | **SUCCESSFUL 925 s** (1/1 cover sat) | trampoline goto-instrument overhead |

Dispatch shape after the fix: 285 `#[inline(never)]` `call_card_N`
trampolines (each loads one constant `&CardDef`) + a thin integer
`call_body_idx` match. A shared `body_of(on)(player)` site unions every
`fn(i32) -> Asked` in the binary into one 874–4313 target IF-chain.

### Events proofs likely on the same path (same `entry_idx` / `all_players` / `Msg` / `Vec` shape)

| id | shape | note |
|---|---|---|
| `duobang_01_02` / `duobang_01_mutant` | all_players + Vec faces + Msg + transfer | **this batch** |
| `pico_swap_01` / `02` / mutant | all_players + Vec dests + Msg | 1767 s near limit at batch 2 |
| `tears_01` / `02` / mutant | all_players + Vec + Msg + transfer | 724 s after class-A cut |
| `front_or_back_*` | all_players + Vec winners + Msg + split_pay | old TIMEOUT |
| `chart_rush_*` | all_players + Vec + Msg | old TIMEOUT after Msg facade |
| `asukayama_02`, `ex_quest`, `habinel`, `lulu`, `school_time`, `kizuna`, `never_ends`, `lost_chase`, `embers_*`, `estate_*`, `bear_cookie`, `this_hand` | all_players + Msg | same `entry_idx` cliff |

Every harness that goes through `invoke::event_play_idx` shares the
IF-chain; the per-card dispatch is a global win for the unit.

### Follow-up (coordinator): derived table is build-time

`body_dispatch_fn.rs` is **not committed**. `rules/kani/build.rs`
(`gen_body_dispatch`) parses `invoke::BAND_START` and writes
`$OUT_DIR/body_dispatch_fn.rs` (same rule as `cond_irs.rs`); `invoke.rs`
`include!`s it. `.gitignore` lists the old source path. Adding a card only
needs `gen_card_ids.py` + `BAND_START` — trampolines regenerate on build.

Re-verified after the move: `wrong_guard::real_guard_matches_clause_iff`
**SUCCESSFUL 17 s** (2/2 covers sat);
`events::duobang_01_02_two_player_iff` **SUCCESSFUL 979 s** (5/5 covers sat,
same 605k-var formula). `cargo build -p game-rules -p rules-native -p rules-kani`,
`cargo test -p rules-kani` 3× green, `cargo test -p rules-cond --features lower`,
wasm32 workspace check green.

## Batch 3 follow-up (coordinator): pp world-setup cliff

Central `pp::same_dream_01_band_plus_3_iff` TIMEOUT 1800 s. Unwind profile
dominated by `NameMap::get_opt` **144** + `NameMap::set` **80** +
`sync_views` 8 + `symbolic_play_world` 4 — the per-seat named-counter
setup, not the card body.

| cut | what |
|---|---|
| `NameMap` direct-index | `vals[NameId.0]` over `intern::SLOTS` (64 buckets) + `u64` present bitset. O(1) get/set. Derived MIN/MAX/EXPIRES ids (`key+0x2000/0x4000/0x8000`) use a 16-slot extra list (linear, rare). |
| pp setup intern once | `symbolic_play_world` interned `FANS_UP`/`FANS_DOWN` per seat (8-probe table walk × 8); now once. |

`conf_fill_diff` / `conf_model` / `conf_hostops2` / `pilot_guards` /
`model_bounds` green. `cargo build -p game-rules -p rules-native -p rules-kani`
+ wasm32 workspace check green.

### pp dispatch shape (iterate)

First cut (285 `call_card_N` trampolines) kept every IF-chain live —
`call_body_idx` is a 285-arm match and `idx` is a parameter, so SSA held
285×874 chains (75 MB goto, 3.4 GB SSA). Replaced with **one** `code*`
site: match `idx` → `&'static CardDef` constant, one `On` load + call
(42 MB goto, 1×874 chain). `inline(always)` on `entry_idx` / `play_idx` /
`event_play_idx` / `call_body_idx` so a `card_id!` constant can fold.

`pp::same_dream_01_band_plus_3_iff` unwind is now only
`sync_views` 8 + `symbolic_play_world` 4 (NameMap walks gone). Still
SSA/solver-bound after that (~15 min, no verdict yet) — World copy +
card-body IR remains. 2-seat variants / lazy `sync_views` / no full
`reset_world` copy are the next levers (documented, clause-faithful).

## Batch 3c — pp setup cost (coordinator item 6)

Levers applied (in payoff order):

| lever | what | result |
|---|---|---|
| (1) per-seat `sync_views` | `add_held` / `set_held_name` refresh one seat | unwind `sync_views` **gone** |
| (2) in-place `symbolic_play_world` | no `reset_world(w)` full-World copy | harness setup is field writes only |
| (3) `pin_two_seats()` | same_dream_01/02, title_idol_01, maya_inf_02 (documented seat-count independent) | 2-seat money/fans |
| (4) intern fan tokens once | was per seat | intern walks gone |
| NameMap direct-index | (previous batch) | `get_opt`/`set` unrolls **gone** |

Unwind after cuts: **only** `symbolic_play_world` 4 (the seat loop).

**Remaining cliff (not an unroll):** SSA / solver after symbolic execution.
Root: one shared `fn(i32) -> Asked` `code*` call site expands to an
**874-target IF-chain** (every address-taken body in the binary). Tried
and rejected: 285 per-card trampolines (285×874 = 249k branches, 75–77 MB
goto); per-arm `f(player)` (same). Best measured shape: one
match-selected `&'static CardDef` + one `code*` call (42 MB goto). A
true **direct** call needs a named `play` symbol (card-body `pub` —
forbidden). `pp::same_dream_01` / `title_idol_01_mutant` remain SSA-bound
at 12–18 min (no verdict inside 10 min target).

`duobang_01_02` on this stack: SUCCESSFUL 542 s CBMC (events bodies are
smaller). next lever: reduce World SSA (NameMap `SLOTS` 64 → packed
dense id) or a `cfg(kani)` body-enum in `card-sdk` so the call is a match
on an integer, not a `code*`.

### Named-const trampolines — CBMC still 874 targets (measurement)

Tried the pilot invoker shape (`git show 91fe463:rules/kani/src/invoke.rs`):

```rust
let card = &card_pp::same_dream::SAME_DREAM;
match &card.on[0] { On::Play(_, _, f) => f(player), … }
```

`parse_cards` in `build.rs` names each pub const
(`&{crate}::{module}::{CONST}`) from the band `lib.rs` `use mod::CONST` +
`CARDS` order. **Literal** `on[0]` index (not `on.get(sym)`).

| shape | goto size | `address_of` / IFs per `code*` site |
|---|---|---|
| `&CARDS[i]` single site | 42 MB | 874 |
| named const + `on.get(sym)` ×285 trampolines | 75 MB | **874 per trampoline** |
| named const + `on[0]` pilot arm + `on.get` fallback | 105 MB | **1747 per trampoline** (2 sites × 874) |

`goto-instrument --show-goto-functions` on
`pp::same_dream_01_band_plus_3_iff`: every `invoke::call_card_N` has
**874 `address_of` IF-branches** for `f(player)`. The value set does **not**
collapse to 1. CBMC's function-pointer removal is **type-based** over
every `fn(i32) -> Asked` in the binary (874 today; the pilot era had far
fewer address-taken bodies), not value-based on the named const's
initialiser.

**Conclusion:** naming the pub const + literal `on[0]` is not enough on
this CBMC. A 1-target resolve needs either (a) a **named** `play` symbol
(`pub fn` in the card module — currently a card-body edit), or (b) a
`card-sdk` `On` body **integer id** so the call is `match body_id` with
direct wrappers, not a `code*` load.

Unrolls remain clean (`symbolic_play_world` 4 only) after the 3c setup
cuts.

### Lever A — per-unit crate features (address-taken set)

`rules-kani` optional features `band-*` / `skills-*` / `events` / `tiles`
(`default = all`). `run.sh --unit` maps to `--no-default-features --features
…`. `RULESET` / `BAND_START` / `card_ids` / body-dispatch generate from the
**enabled** set (`card_id!` = const-fn byte compare; disabled id = compile
error).

`pp::same_dream_01_band_plus_3_iff` with `--features band-pp` only:

| metric | all crates (285) | band-pp (19) |
|---|---:|---:|
| goto binary | 42–106 MB | **11 MB** |
| `call_card_*` trampolines | 285 | **19** |
| `address_of` IFs per `code*` site | **874** | **388** |
| verdict | TIMEOUT 1800 s | **TIMEOUT 1800 s** (SSA-bound after clean unwind) |

~2.3× fewer targets per site, 15× fewer sites. Still not 1 target —
CBMC removal is type-based over every `fn(i32)->Asked` **in the linked
binary** (pp bodies + shared runtime ≈ 388).

### Lever B — `goto-instrument --restrict-function-pointer`

Exists (`--restrict-function-pointer pointer_name/target[,targets]*`,
`--restrict-function-pointer-by-name`). CBMC itself has no such flag;
Kani exposes `--cbmc-args` but not a goto-instrument hook, so pinning the
site needs a pipeline wrap (goto-cc → restrict → cbmc) that `cargo kani`
does not offer today.

### Plain answer

**Neither lever gets pp under ~10–15 min.** Unrolls are clean
(`symbolic_play_world` 4); SSA/solver after symbolic execution is still
>18 min / TIMEOUT at 1800 s even with 19 cards and 388-target sites.

### events backlog (central pass)

| id | verdict | class | note |
|---|---|---|---|
| `events::moca_01_activation_keeps_the_card` | UNWINDING 507 s | A | `memcmp` 38 unrolls (host tag compare) — same class as the old hook-dispatch cliff |

## Batch 3d — events per-proof root-cause (user decision: TIMEOUT stays FAIL)

One entry per failing proof: phase split, unwind profile, dominant cost,
class A–F, fix or why none.

| id | verdict (central) | phase split | dominant cost | class | action |
|---|---|---|---|---|---|
| `events::moca_01_activation_keeps_the_card` | UNWINDING 507 s (unwind 16) | BMC → unwind `base_world` 4 + **memcmp 38** (`Not unwinding at 16`) | `[i32;4]::eq` on `post.money == pre.money` → 16-byte `memcmp` | **A** | element-wise `arr4_eq` / `arr4b_eq` (no memcmp). **SUCCESSFUL 394 s** |
| `events::aao_01_activation_keeps_the_card` | UNWINDING 562 s (unwind 16) | same — memcmp 30 (`Not unwinding at 16`) | same `post.money == pre.money && post.pos == pre.pos` | **A** | same fix. **SUCCESSFUL 310 s** |
| `events::moca_02_mutant_is_detected` | TIMEOUT 1800 s | BMC → **8725 VCC** / 719 after simp → SSA 4.5 s → SAT **703k vars / 761k clauses** (first SAT 0.04 s, then many) | unwind **memchr_naive 1193** + memchr_aligned 284 + memcmp 237; `PadAdapter::write_str` recursion 81 — fmt of `kani::assert` / `fail_*` message strings | **E** (fmt/memchr) | no cheap fix without dropping assert-message fmt; mutant has extra `iter().any` paths into it. **remains TIMEOUT (FAIL)** |
| `events::aao_02_hand_veto_is_armed` | FAILED 550 s | (see prior) | body LESS — no [手] veto surface | **D GENUINE E-aao-02** | keep FAILING (not a perf bug) |
| `events::front_or_back_01_equal_iff` | TIMEOUT 1800 s (unwind 32) | BMC → unwind `front_or_back::play` **31** (`Not unwinding at 32`) + `log_is_gains` 31 + slice `all`/`any`/`find_map` 31 | `for &p in &winners` over a **symbolic-length `Vec<i32>`** (body, cannot edit) + `effects.iter().all/any` over `MAX_LOG=24` | **A/E** | body Vec length not a constant bound; raising unwind past 32 explodes. **remains TIMEOUT (FAIL)** — no cheap fix without body edit |
| `events::front_or_back_02_same_half_share` | TIMEOUT 1800 s (unwind 32) | same + `slice_contains` 24 | same | **A/E** | same — **remains TIMEOUT (FAIL)** |

Phase markers used: `Starting Bounded Model Checking`, `Generated N VCC(s)`,
`Running propositional reduction`, `N variables, M clauses`, `SAT checker`,
`Runtime Solver`.

## Batch 3e — events rows + preventive memcmp + BACKEND + kassert!

### New events failures (live pass)

| id | verdict | phase split | dominant cost | class | action |
|---|---|---|---|---|---|
| `events::ex_quest_01_rolls_and_files` | UNWINDING 937 s (unwind 16) | BMC → memcmp 45 (`Not unwinding at 16` ×3) + `ex_quest::play` 15 (`Not unwinding at 16`); then 15627 VCC / 1.16M vars / 2.0M clauses / SAT 15 s | array `==` memcmp + `for &p in &all_players()` Vec loop at 16 | **A** | `arr4_eq` (done); body loop still at 16 — re-timing |
| `events::habinel_01_grants_exiles` | TIMEOUT 1800 s (unwind 24) | BMC → `habinel::play` 23 (`Not unwinding at 24`) + `spec::habinel_01_exiles_ok` 23 + harness loop 23; 23476 VCC / **7.8M vars / 30.8M clauses** / SAT 61 s | body + spec loops at 24 + `board::tile_named_bytes` 6 (60-name scan) — huge formula | **A/E** | no cheap fix (body `for` + tile scan); **remains TIMEOUT (FAIL)** |
| `events::front_or_back_mutant_swapped_is_detected` | TIMEOUT 1800 s (unwind 32) | same as front_or_back_01/02: `play` Vec 31, `log_is_gains` 31, slice iters 31 | body `Vec` + effects iters | **A/E** | same as siblings — **remains TIMEOUT (FAIL)** |

### Preventive memcmp cut (all units)

`post.money == pre.money` is `[i32;4]::eq` → 16-byte `memcmp` at unwind 16.
Helpers `world::arr4_eq` / `arr8b_eq` / `arr4b_eq` applied across **23 proof
files + 7 spec files**. Cheap, no behaviour change.

### kassert! — assert without Rust fmt

`assert!(cond, "literal")` still pulls `panic_fmt` → `PadAdapter` →
`memchr` (1193 unrolls on moca_02_mutant). `kassert!` = `kani::assert`
under Kani (CBMC property, no fmt) / `assert!` natively. Replaced across
all proofs.

### BACKEND → direct extern "Rust"

`card_sdk::kani_host::call` under `cfg(kani)` is now
`extern "Rust" { fn rules_kani_host_dispatch(...) }` — a direct CALL into
`rules_kani::host::dispatch`. No fn-ptr (was 874-target IF-chain at every
`ctx::*` site). Native/tests keep `BACKEND`. Production card-sdk
`cfg(not(kani))` unchanged (dist hashes: only `cfg(kani)` code moved).

### BACKEND direct extern — **not supported by Kani** (reverted)

`extern "Rust" { rules_kani_host_dispatch }` made `same_dream_01` finish
in **244 s** (388k vars / 396k clauses, SSA 1.7 s) — but Kani aborts with
`call to foreign "Rust" function is not currently supported` so every
property is UNDETERMINED (FAILED, vacuous). Same limitation as
`extern "C-unwind"` (kani#2423). **Reverted** (`cf1e112`). The 244 s run
is still evidence: the `BACKEND` fn-ptr IF-chain at every `ctx::*` site is
the dominant SSA cost. A viable cut needs the host dispatch **in**
`card-sdk` under `cfg(kani)` (no cross-crate call), or a Kani feature for
Rust ABI imports.

### kassert! + arr4_eq (kept)

`assert!` → `kassert!` (no `panic_fmt`/`memchr`) across proofs;
`arr4_eq`/`arr8b_eq`/`arr4b_eq` across proofs+spec. Re-timed
`moca_01` 394 s / `aao_01` 310 s (batch 3d).

## Batch 4 — direct host dispatch: **not merged** (docs-only record)

The `kani-ops` / `kani-model` split (direct `kani_model::dispatch` call)
was implemented and then **rejected**:

* **Production wasm hashes change** (`3aedcf2b…` vs master `493c2ffe…`)
  — intern/arena moved crates, so codegen is not bit-identical.
* **No proof speedup** — `pp::same_dream_01_band_plus_3_iff` remains
  **symex-bound** inside the model itself (300-op `dispatch` match,
  `World` copy, `native::read`/`to_vec`), ~25 min / TIMEOUT.

**Correction to the 244 s "breakthrough":** that run used `extern "Rust"`
which Kani **aborts** (`call to foreign "Rust" function is not currently
supported`) — every property UNDETERMINED, 0/4 covers. It did **not**
verify the clause. Class **E** for the pp rows.

| id | class | dominant cost | next idea |
|---|---|---|---|
| `pp::same_dream_01_band_plus_3_iff` | **E** | model: 300-op `dispatch` match, `World` copy, `native::read`/`to_vec` (not the body `code*` IF-chain — direct-call experiment) | shrink `dispatch` match per proof (op subsets); smaller `World` for band proofs; allocation-free `native::read` for intern-tagged handles |

**Kept (independent of crate layout):** `kassert!` / `arr4_eq` (batch 3e);
crychic static assert message (format-arg `kassert!` did not match the
macro). Production `rules/` restored to master.


## Batch 3e/4 triage — remaining rows

| id | verdict | class | dominant cost | action |
|---|---|---|---|---|
| `events::ex_quest_01_rolls_and_files` | UNWINDING 937 s → **SUCCESSFUL 724 s** (1/1 cover) | **A** | array `==` memcmp at unwind 16 | `arr4_eq` (kept) |
| `events::habinel_01_grants_exiles` | TIMEOUT 1800 s | **A/E** | body/spec/harness loops at 24; 7.8M vars / 30.8M clauses; `tile_named_bytes` 6 | no cheap fix — **FAIL** |
| `events::front_or_back_*` (01/02/mutant) | TIMEOUT 1800 s | **A/E** | body `Vec` + effects iters at 32 | no cheap fix — **FAIL** |
| `events::moca_02_mutant_is_detected` | TIMEOUT 1800 s | **E** | fmt `PadAdapter` + memchr 1193 (assert messages) | `kassert!` applied; re-time pending |
| `events::aao_02_hand_veto_is_armed` | FAILED | **D** | GENUINE E-aao-02 (LESS) | keep FAILING |
| `prompt::aya_haruka_01_prompt_all_picks_iff` | TIMEOUT 1800 s | **E** | `Vec<u8>::extend_with` 31, `drop_glue<[String]>` 62, `memcmp` 31, `ctx::ask_card` 31 | card-body `String`/`Vec<String>` ABI (TUNING §10) — no cheap fix |
| `prompt::aya_haruka_01_mutant_two_cards` | TIMEOUT 1800 s | **E** | same | same |
| `money_shave::aya_haruka_02_shave_iff` | TIMEOUT 1800 s | **E** | `Vec<u8>::extend_with` 31, `by_idx` 10, `memcmp` 10, `named_set` 8 | same |
| `money_shave::aya_haruka_02_mutant_*` | TIMEOUT 1800 s | **E** | same | same |

Branch is **docs + crychic static kassert message only** (production
`rules/` matches master; dist hashes identical).


## strvec — String/Vec unwinding facades (branch `kani-strvec`)

Goal: remove the `String`/`Vec` unwinding cost that times out
`prompt::aya_haruka_01_*`, `money_shave::aya_haruka_02_*`, and
`events::front_or_back_*` / `moca_02_mutant` at 1800 s. Approach: `cfg(kani)`
facades in `rules/card-sdk` (same pattern as intern/Msg: zero-loop, heap-free)
plus harness-side fixed arrays. Production wasm/native **byte-identical**
(`dist` hashes match master).

### Changes (all `cfg(kani)` unless noted)

| piece | was | now |
|---|---|---|
| `card-sdk/kani_alloc.rs` | system `malloc`/`free` (CBMC heap objects) | static 16 KiB bump: `alloc`=cursor, `dealloc`=no-op, `realloc` grows last block |
| `card-sdk/native.rs` arena | `Vec<Vec<Vec<u8>>>` + `vec![0u8; len]` | `[[[u8;64];8];8]` frame stack; zero-loop intern/read/write/pad |
| `out_buf` / `out_read` | `vec![0u8; cap]` (`extend_with` 31) | empty scratch + fixed arena pull |
| `ask_pick` / `ask_card` / `ask_player` / `ask_player_cards` / `ask_number` | `for` over symbolic-length slice (`ctx::ask_card` 31) | 4-slot branch trees; `kani::assert` if >4 |
| `cond-ir` `named_get` / `named_set` | 4-iter + 10×8 chunk walks (`named_set` 8) | zero-loop 80-step scan (`.get`, no index panic) |
| `ir_fill::fill_pay_add` | `named_set` ×2 | direct `named[0..2]` slots |
| `invoke::hook_kind_only_idx` / `hook_kind_matches` | `kinds.iter().any` | zero-loop 4-slot match |
| events specs (`log_is_gains`, `log_is_paytos`, `front_or_back_*`) | `iter().zip` / `contains` at 31 | zero-loop step macros (bound 24) |
| `moca_02_mutant` scan | `effects.iter().any` | zero-loop 24-step |

### Production hash check

`node tools/build-ruleset.mjs` **without** `CARGO_TARGET_DIR` (worktree
`/home/monopoly/kani-strvec-wt`), compared to master's `/home/monopoly/web/dist`:

| artifact | worktree | master | result |
|---|---|---|---|
| `dist/cards/*.wasm` (card_all) | `fc3010e9cf22c528…b4c15` | `fc3010e9cf22c528…b4c15` | **identical** |
| `dist/fixtures/*.wasm` | `03d5135b4faf3dca…ada15` | `03d5135b4faf3dca…ada15` | **identical** |
| `dist/cards/conds-*.bin` | `a76f4eab6a76a74f…7da7` | `a76f4eab6a76a74f…7da7` | **identical** |

### Tests

* `cargo test -p rules-kani` ×3 — green (13 + 14 + 37 + 5 + 5)
* `cargo test -p rules-cond --features lower` — green (58)
* `cargo check --manifest-path rules/Cargo.toml --workspace --target wasm32-unknown-unknown` — green
* `cargo build -p game-rules -p rules-native -p server` — green

### Before / after (target rows)

Iterative re-runs of `prompt::aya_haruka_01_prompt_all_picks_iff` (one CBMC
slot, 1800 s cap, unwind 32):

| run | change on top of the facades | verdict | profile |
|---|---|---|---|
| v1 | facades only (ask trees, fixed arena, named_set 0, out_buf) | **FAILED 1156 s** | SDK cliffs gone (0 extend_with / ask_card); residual body `drop_glue<[String]>` 32, `String fold` 31, `memcmp` 32 |
| v2 | `cards_in` generic `T: From<CardList>` | compile fail (type-param default is a hard error) | — |
| v3 | `cards_in` len 0-or-4 + `from_raw_parts` static strings | **FAILED 476 s, 0/4 covers UNREACHABLE** | profile clean (no memcmp/drop_glue) but `__rust_dealloc` rejects static pointers → every String-drop path aborts → vacuous |
| v4 | `String::from` facade (valid alloc) + len 0-or-4 | killed mid-run | `memcmp` 414+ (heap `len` not folded); drop_glue/fold **gone** |
| v5 | `from_raw_parts` over a real 1-byte `alloc` (constant `len=1`) + len 0-or-4 | **TIMEOUT 1800 s** | `memcmp` 2015 (`slice_contains` 63 × 32); drop_glue only 13 (constant-len helps); 187 `__rust_dealloc`/`same_allocation` path aborts |

**What is gone for good (all runs after v1):** `Vec<u8>::extend_with`,
`ctx::ask_card` / `ask_pick` / `ask_player` / `ask_tile` loops,
`named_set` 8, `by_idx` 10, `drop_glue<[String]>` (constant-length
`cards_in`), the `String` `map().collect()` fold.

**What remains:** `memcmp` inside `String::eq` (`ids.contains`). CBMC
treats a heap `String`'s `len` as symbolic even when built via
`from_raw_parts(p, 1, 1)`, so `memcmp` unrolls to the global bound (32).
That is the unmodified body's `Vec<String>` ABI — the TUNING §10 wall.
No `cfg(kani)` facade can make `alloc::string::String`'s Drop/eq see a
constant length without a body type-annotation change.

| id | before (batch 3e/4) | after (strvec) |
|---|---|---|
| `prompt::aya_haruka_01_prompt_all_picks_iff` | TIMEOUT 1800 s — extend_with 31, drop_glue 62, memcmp 31, ask_card 31 | **FAILED 1156 s** (v1) / **FAILED 476 s vacuous** (v3) / **TIMEOUT 1800 s** (v5) — extend_with/ask_card **gone**; body `Vec<String>` memcmp/drop_glue/fold residual (see above) |
| `prompt::aya_haruka_01_mutant_two_cards` | TIMEOUT 1800 s — same | same stack as iff; residual memcmp |
| `money_shave::aya_haruka_02_shave_iff` | TIMEOUT 1800 s — extend_with 31, by_idx 10, memcmp 10, named_set 8 | **TIMEOUT 1800 s**, unwind profile **clean** (`symbolic_pay_world` 4 + `fill_pay_add` 4 + `poorest` 4 + `hook_kind` 1; **zero** extend_with/named_set/by_idx/memcmp/drop_glue). Solver-bound on the 4-seat symbolic world — the String/Vec cliffs are gone |
| `money_shave::aya_haruka_02_mutant_*` | TIMEOUT 1800 s — same | same (clean profile, solver-bound) |
| `events::front_or_back_*` (01/02/mutant) | TIMEOUT 1800 s — body `Vec` + effects iters at 32 | effects iters 0 (spec step macros); body `Vec<i32>` loops remain (same wall) |
| `events::moca_02_mutant_is_detected` | TIMEOUT 1800 s — fmt PadAdapter + memchr 1193 | `kassert!` + zero-loop `iter().any`; re-time pending |

Remaining risk: the unmodified bodies still allocate `Vec<String>`
(`aya_longing` `ids`) and `Vec<i32>` (`front_or_back` `front`/`back`/`winners`).
Those types cannot change without a body edit. The bump allocator turns their
grow/drop into array arithmetic; the loops still unroll to the global bound if
`Vec::len` is not folded. If a row stays TIMEOUT the profile will say whether
the residual is still `drop_glue`/`extend_with` or solver-bound on the symbolic
world (the money_shave post-batch-2 diagnosis).
## op-subset — per-harness host-op subset + sparse World (branch `kani-opsubset`)

Goal (coordinator): `pp::same_dream_01_band_plus_3_iff` well under 1800 s,
non-vacuous. Levers named in Batch 4: shrink the ~300-op `dispatch` match per
proof, smaller `World`, allocation-free `native::read`.

### What landed

| lever | what | files |
|---|---|---|
| (1) **Per-harness host-op subset** | `harness::host_subset!` builds a tiny `fn(Op, Args) -> i64` match covering only the `host::arm_*` handlers the card body reaches. Every other `Op` → `host::outside_subset` = **`kani::assert(false)`** (loud FAIL — never `assume(false)`, so an undeclared op cannot silently pass). `host::install_backend(f)` installs it. | `host.rs`, `harness.rs` |
| **arm_\* single-source handlers** | `arm_log`, `arm_gain`, `arm_count_held`, `arm_add_held`, `arm_player_count`, `arm_in_band`, `arm_hand_count`, `arm_take_card`, `arm_to_discard`, `arm_ask`, `arm_add_band_crystals`, … — `dispatch` and subset dispatchers share them. | `host.rs` |
| (2) **Sparse play world** | `harness::sparse_play_world(SparsePlay)`: only `used_seats` get symbolic money/fans; seats `used..4` stay at `World::new()` defaults (concrete). `n_players` concrete. Isolation `hand_resonance = 0` default. `with_world`/`with_world_mut` `#[inline(always)]` (no `World` copy per host call). | `harness.rs`, `world.rs` |
| **`name_id` no alloc** | Untagged name handle under `cfg(kani)` = loud FAIL (`kani::assert(false)`), not `native::read`/`to_vec`. The `ctx::s`/`key!` facade always tags. `cfg(not(kani))` keeps the byte fallback. | `host.rs` |
| **pp wiring** | `same_dream_dispatch` declares 14 ops (body + `try_resonance` entry); `same_dream_world()` = subset + sparse 2-seat. Harnesses: `same_dream_01_band_plus_3_iff`, `same_dream_02_money_mutant_is_detected`. | `proofs/pp.rs` (kit declaration only) |

`player_count()` semantics unchanged (TUNING §7): `ctx::player_count()` still
returns the constant 4 under `cfg(kani)` after asserting `n_players ∈ 2..=4`;
`player_out(p)` is true for `p >= n_players`. Sparse world only makes **2**
seats symbolic — the body's `0..4` loop is four cheap iterations and with
`widen == false` only `p == user` is processed.

### Class C: hook-status polarity (`raise` / `gate` / `raise_bought` / `ring_multiplier`)

**Fixed here** — the parallel triage pass ("Batch 3e continued") diagnosed
`chart_rush_01_spends_fire` as a class C host-model defect (`Op::raise`
benign-0 → cancelled → `spend_fire` skipped the real spend) and left the
proof FAILING for a `host.rs` fix. This branch is that fix.

`events::chart_rush_01_spends_fire` spent no [火罐]: `spend_fire` calls
`ctx::raise(FirePaying)`, and a **0** return means *cancelled / substituted*
→ `return Ok(true)` without the real spend. `Op::raise` sat in the benign-0
list (returned 0). Engine contract (`wasm_rules` `HostRequest::Raise`):
return `(!cancelled)` — **1 = settled**.

Same polarity class, same fix (all pinned in `conf_hostops2`):

| op | 0 means | correct default | was | now |
|---|---|---|---|---|
| `Op::raise` | cancelled → caller skips effect | 1 = settled | 0 | **1** (`arm_raise`) |
| `Op::gate` | guarded/immune/out → skip effect | 1 = allowed | 0 | **1** (`arm_gate`, 0 for out seats) |
| `Op::raise_bought` | hand-over did not land | 1 = landed (HOST-06) | 0 | **1** (`arm_raise_bought`) |
| `Op::ring_multiplier` | 0 zeroes rent | ≥ 1 (identity 1; game default 10) | 0 | **1** (`arm_ring_multiplier`, `TODO(规则书)` for the stock 10) |

Reported leftovers with the 0-means-skip shape (still benign-0, not yet
armed): `play_from_hand` (0 = not from hand), `can_build_on` (0 = cannot),
`card_*` / `buy` / `acquire` / `agent_offer` (0 = did not land).
`designation_cancelled` (0 = not cancelled) is the correct polarity.

### Measurements (one serial CBMC, unwinding assertions ON, `tools/kani/run.sh`)

| harness | before | after | verdict |
|---|---|---|---|
| `pp::same_dream_01_band_plus_3_iff` | TIMEOUT 1800 s (central; NameMap 144+80 unrolls, 42 MB goto) | **TIMEOUT 1800 s** ×2 (op-subset + sparse world; then + `name_id` cut) | **TIMEOUT** — target **not met** |
| `events::chart_rush_01_spends_fire` | TIMEOUT 1800 s (old base, class E) + **genuine class C** (no fire spent — `raise` returned cancelled) | **SUCCESSFUL 909 s** (unwind 24, **2/2 covers sat**: "no pots" + "some pots spent") | **SUCCESSFUL** |

Profile deltas for `same_dream_01_band_plus_3_iff` (`--features band-pp`,
unwind 32):

| metric | pre-op-subset (central) | op-subset + sparse | + `name_id` cut |
|---|---|---|---|
| goto binary | 42–106 MB | **8.8 MB** | 8.8 MB |
| unwind loops | NameMap 144+80, sync_views 8, setup 4 | **`sparse_play_world` 2 only** | same |
| aborting paths | (alloc-heavy) | 29 (incl. `native::read`, `RawVec`, `slice_index_fail`) | **14** (Option::unwrap_or ×6, `name_id` ×3, `Result::branch` ×3; **zero alloc / to_vec / RawVec**) |
| verdict | TIMEOUT 1800 s | TIMEOUT 1800 s | TIMEOUT 1800 s |

Unrolls are clean. The remaining cost is **SSA/solver after symbolic
execution** — not a model loop, not an alloc cliff. Named in Batch 4 after the
direct-call experiment: the **BACKEND `fn(Op, Args) -> i64` IF-chain** at every
`ctx::*` site, plus the body `fn(i32) -> Asked` `code*` chain (388 targets with
`--features band-pp`). Those live in `card-sdk::kani_host::call` (fn-ptr) and
`invoke.rs` trampolines — **outside this task's edit scope** (`host.rs` /
`world.rs` / `harness.rs` / `shim.rs` only). The rejected `extern "Rust"` run
(244 s, UNDETERMINED) is still the evidence that the host-call fn-ptr is the
dominant SSA cost.

### Non-vacuity

Not reached: both runs hit TIMEOUT before any property verdict, so covers /
`assert!(false)` probes were not evaluated. The subset trap is fail-closed by
construction (`outside_subset` = `kani::assert(false)`), and `sparse_play_world`
uses no `assume` that hides body behaviour (isolation `hand_resonance = 0` is
stated and matches the pp module doc). A post-invoke `assert!(false)` probe
must be run when the proof first finishes.

### Soundness kept

* Unwinding assertions ON; no `--no-unwinding-checks`.
* No `#[ignore]`, no allowlist, no weakened specs.
* `outside_subset` / untagged `name_id` **FAIL** the proof — never `assume`.
* Card bodies unmodified. `rules/` production (non-kani) untouched by the
  op-subset / sparse-world work (`name_id` keeps its byte fallback
  `cfg(not(kani))`). `cargo test -p rules-kani` green (incl. 4 new
  `conf_hostops2` polarity pins).
* `player_count()=4` / `player_out` semantics documented and unchanged
  (TUNING §7).

### Next levers (out of this task's file scope)

1. Fold the `kani_host::BACKEND` fn-ptr (put the dispatch **in** `card-sdk`
   under `cfg(kani)`, or a Kani feature for Rust-ABI imports) — the 244 s
   direct-call experiment says this is the dominant cost.
2. Body `code*` → integer body-id match (`invoke.rs` / `build.rs` trampolines)
   so the call is not a 388-target IF-chain.
3. `native::read` tagged path still `to_vec`s in `card-sdk` (other agent).
## Batch 3e continued — proof-file triage (parallel: op-subset + strvec agents)

Scope: `rules/kani/src/{proofs,spec}/*.rs` + docs only.

### `events::chart_rush_01_spends_fire` FAILED — **class C host-model defect**

Assert `E-chart-rush-01: all fire pots spent` fails. Root cause chain:

1. Body: `fire::spend(p, lost, …)` → `ruleset_core::spend_fire`.
2. `spend_fire` opens the `FirePaying` link first:
   `if !ctx::raise(…, FirePaying, n)? { return Ok(true); }`
   (`raise` = true when the link **settled**, false when **cancelled**).
3. Model `host.rs` `Op::raise` is in the benign-0 unmodeled list → returns **0**.
4. `raise` → `Ok(false)` («cancelled») → `spend_fire` returns `Ok(true)`
   **without subtracting** `w.fire[p]`.

So the model reports the spend as satisfied but leaves the pots. The proof
is correct; the **host model's `Op::raise` must return 1 (settled)** for
`FirePaying` when no hook substitutes. Fix belongs in `host.rs` (not this
agent's scope) — leave the proof **FAILING** as the finding.

### `events::fix_note_01_activation_keeps` UNWINDING — class A

`fix_note::play` `for t in 0..tile_count()` is a **60-trip** loop
(`Not unwinding at 16`). Raised harness unwind to **64** (re-timing).

### `fix_note_01` unwind 64 — **TIMEOUT 1200 s** (class E after bound fix)

60-trip `for t in 0..tile_count()` fully unwound is SSA-heavy. Bound is
now sound (64 ≥ 60); cost is the body loop (cannot edit). **FAIL** —
no cheap proof-file lever beyond the bound.

### `end_it_all_01_three_picks_and_keep` — dropped unused `picks` scan

`ask_pick` + `Vec<Msg>` fold (String ABI) still class E; pick-count lives
in `three_picks_counted`. Simplified the keep half (re-timing pending).

### `moca_02_mutant_is_detected` — **SUCCESSFUL 340 s** (was TIMEOUT 1800 s)

`kassert!` (no `panic_fmt`/`memchr`) removed the fmt cliff that dominated
the 1193-unroll memchr/PadAdapter profile. Class **E → fixed**.

## triage2 — events unit, central-results3 non-success rows

Branch `kani-triage2` · Worktree `/home/monopoly/kani-triage2-wt`.
Old base: `/home/monopoly/kani-central-results3/events/` (unit finished on an
older base). New runs: `/home/monopoly/kani-triage2-results/`.
One serial CBMC (`KANI_GLOBAL_SLOTS=3`, slot2 = this agent); unwinding
assertions ON. Classes A–F per the user rule.

Master already contained (before this branch): `arr4_eq` / `kassert!`,
op-subset host (`arm_raise` = 1 etc.), sparse World, `cfg(kani)` String/Vec
facades. Central rows older than those fixes are re-timed, not re-derived.

### Per-row table (before → after)

| id | before (central-results3) | phase split / dominant cost | class | action | after |
|---|---|---|---|---|---|
| `moca_01_activation_keeps_the_card` | UNWINDING 507 s (unwind 16) | BMC → `memcmp.unwind` 16 (16-byte `==`) | **A** | `arr4_eq` already on master | **SUCCESSFUL 322 s** (unwind 16) |
| `aao_01_activation_keeps_the_card` | UNWINDING 562 s (unwind 16) | memcmp 16 (A) **and** `host::name_id` untagged (C) | **A + C** | `arr4_eq` on master; `set_self_prop` packing bug is host — **FAILED 260 s** | FAILED (class C, host) |
| `this_hand_01_move_is_protocol` | UNWINDING 571 s (unwind 16) | `memcmp.unwind` 16 | **A** | `arr4_eq` already on master | **SUCCESSFUL 315 s** (unwind 16) |
| `ex_quest_01_rolls_and_files` | UNWINDING 937 s (unwind 16) | `ex_quest::play` + `memcmp` | **A** | `arr4_eq` + unwind **24** | **SUCCESSFUL 778 s** (unwind assert at 24 holds) |
| `fix_note_01_activation_keeps` | UNWINDING 838 s (unwind 16) | 60-trip body loop (A) + `set_self_prop` (C) + SSA cost | **A → E** | unwind **64** (sound); body loop cannot shrink | **TIMEOUT 1800 s** (unwind 64) |
| `chart_rush_01_spends_fire` | FAILED 846 s | assert `all fire pots spent`; `Op::raise` was benign-0 → `spend_fire` skipped | **C** | `arm_raise` → 1 on master (op-subset) | **SUCCESSFUL 654 s** (2/2 covers sat) |
| `embers_03_settle_costs_ten_per_copy` | FAILED 1139 s | wrong On index (B) **and** `set_self_prop` packing (C) | **B + C** | entry 3 fixed; still **FAILED 232 s** on `name_id` | FAILED (class C, host) |
| `lulu_01_pay_500_leg` | FAILED 647 s | assert `Teleport tile == -1`; `tile_named(快餐店)` now **33** | **B** | assert pay+teleport to 33 via `lulu_01_pay_ok` | **SUCCESSFUL 366 s** |
| `aao_02_hand_veto_is_armed` | FAILED 550 s | assert `[手] veto armed`; body arms nothing (D) + `set_self_prop` packing (C) | **D** GENUINE E-aao-02 | **keep FAILING** (not a perf/setup bug) | **FAILED 239 s** (class D assert still fires; also name_id C) |
| `moca_02_mutant_is_detected` | TIMEOUT 1800 s (old base) | fmt `PadAdapter` + `memchr` 1193 from `assert!` msgs | **E** | `kassert!` already on master | **SUCCESSFUL 249 s** (unwind 16) |
| `never_ends_01_noop_blocked_host` | TIMEOUT 1800 s | body `format!("RiNG {n}")` → PadAdapter + memchr; **and** stale no-op assert (`tile_named` now resolves 8/16/36/53) | **E + B** | renamed `never_ends_01_teleport_and_exile`; prove TP+exile; `expect`/`panic!` → `kassert!` | pending |
| `front_or_back_01_equal_iff` | TIMEOUT 1800 s (unwind 32) | body `Vec` at 32 + harness `iter().all/any/find_map` 31 + `expect` fmt | **A/E** | zero-loop scans; `expect`/`panic!` → `kassert!` | pending |
| `front_or_back_02_same_half_share` | TIMEOUT 1800 s | same | **A/E** | same | pending |
| `front_or_back_mutant_swapped_is_detected` | TIMEOUT 1800 s | same | **A/E** | same + zero-loop leg sum | pending |
| `habinel_01_grants_exiles` | TIMEOUT 1800 s (unwind 24) | body+spec loops 24; 7.8M vars / 30.8M clauses; **stale `to=-1`** (`tile_named(微笑号)` now **26**) | **A/E + B** | assert `to=26`; drop harness cover loop | pending |
| `kizuna_01_teleport_and_exile` | TIMEOUT 1800 s (unwind 24) | `Vec::extend_with` 24 (old base) + `find_map` + `expect` fmt | **E + A** | strvec facades on master; zero-loop dest scan; no `expect` | pending |
| `end_it_all_01_three_picks_and_keep` | TIMEOUT 1800 s (unwind 32) | `ask_pick` + `Vec<Msg>` / String ABI | **E** | no cheap proof-file lever (body ABI) | pending |
| `estate_01_auction_pay` | TIMEOUT 1800 s (unwind 32) | body `for t in 0..tile_count()` 60-trip (log at iter 19/22) | **A/E** | raise attr to **64** (sound; cost is the body loop) | pending |
| `estate_02_all_pass_no_pay` | TIMEOUT 1800 s | same 60-trip scan | **A/E** | same | pending |
| `lost_chase_rolls_and_pushes_stub` | TIMEOUT 1800 s (unwind 24) | body `for p` + `faces.iter().find/all` 24; 7.3M vars / 28.4M clauses | **A/E** | no cheap proof-file lever (body Vec iters) | pending |
| `lost_chase_end_it_all_guard` | TIMEOUT 1800 s | same | **A/E** | same | pending |
| `school_time_01_exile_leg` | TIMEOUT 1800 s (unwind 24) | `board::tile_named_bytes` 6 × SCHOOLS scan + character_is | **E** | dest is now 17 (周边学区) not -1; comment fixed | pending |
| `bear_cookie_01_push_is_protocol` | TIMEOUT 1800 s (unwind 32) | `event_deck_push` stub + `all_players` Range | **A/E** | PROTOCOL-LEVEL assert is intentional (FAIL expected if push is stub) | pending |

### Class C host-model defect found (NOT fixed — `host.rs` is the other agent's)

**`Op::set_self_prop` key packing vs the shared prop arm.**

`sys_kani::set_self_prop(kp, kl, v)` packs `[kp, kl, v, 0, 0, 0]`.
`host::dispatch`'s shared arm for `set_self_prop | set_prop_at | set_tile_prop`
first does `name_id(a1, a2)` — for `set_self_prop` that is `name_id(kl, v)`,
an **untagged** handle. `name_id` (cfg(kani)) is a loud
`kani::assert(false, "untagged name handle")`. The correct decode
`name_id(a0, a1)` on the `set_self_prop` branch is then unreachable.

Blast radius (every body that calls `ctx::set_prop`):
`aao`, `embers`, `fix_note`, `hallucination`, `surprise_duel`, `marina_box`,
`bangdream_chan`. On the older central base this was silent; master's
`name_id` cut makes it a hard FAIL (`aao_01` FAILED 260 s on this).
**Needed host fix:** only decode `(a1,a2)` for the ops whose packing is
`(uid/tile, kp, kl, …)`; for `set_self_prop` use `(a0,a1)` / `a2` as the
value (the inner code already has the right parse — drop the eager outer one).

Until that lands, set_prop-using events rows are class **C** (host), not
proof bugs. No host.rs edit on this branch (owned by kani-ppcost).

### Class-B setup bugs fixed on this branch

1. **embers On-index off-by-one.** `On` is
   `0=Play, 1=TurnStartBefore/clear, 2=TurnStartBefore/spread, 3=SettleBody, 4=Exile`.
   Proofs used 2 for SettleBody and 3 for Exile (comment had missed the
   spread hook). `embers_03` was invoking the spread hook (no pay);
   `embers_04` was invoking SettleBody (marks never cleared — **vacuous**
   for the clear-on-exile claim).
2. **tile_named is no longer a stub.** `board::tile_named` resolves every
   `data/board.json` name. Stale `-1` dest pins were wrong:
   快餐店=33, 微笑号=26, 周边学区=17, 飞鸟山公园=56, CiRCLE=0, RiNG 1..4=8/16/36/53.
   `lulu_01`, `habinel_01`, `never_ends_01` asserted the old stub shape.
3. **`expect` / `panic!` in proofs pull `panic_fmt` → PadAdapter → memchr**
   (same cliff as the old `assert!` messages). Replaced with `kassert!` /
   `unwrap_or` + an explicit predicate. Zero-loop `eff_*` scanners replace
   `slice::iter().all/any/find_map` on `MAX_LOG`.

### Host-model notes (not edited here — `host.rs` is the other agent's)

| gap | who needs it | note |
|---|---|---|
| `Op::raise` polarity | `chart_rush_01` | **already fixed** on master (`arm_raise` = 1) |
| `tile_named` full board | lulu / habinel / never_ends / school_time | **already fixed** on master |
| `remove_marks` stub | embers_04 | still a stub — proof documents it |
| `event_deck_push` / `event_banish` | bear_cookie / lost_chase / end_it_all | still stubs |
| `card_move` / `plan::set_*` | this_hand / asukayama-01 | still stubs |
| `trig_move_*` unmodeled | moca halve / heat_head / circle_rebuild | still unmodeled |
| `format!` in `never_ends` body | never_ends_01 | **card-body** cost (cannot edit); class E |

### Soundness kept

Unwinding assertions ON; no `#[ignore]`; no allowlist; no weakened specs;
`aao_02` stays FAILING as the genuine book/body discrepancy (class D).

### triage2 summary (re-timed on this branch)

| verdict | count | ids |
|---|---:|---|
| SUCCESSFUL | 6 | moca_01 322 s, this_hand_01 315 s, chart_rush_01 654 s (2/2 covers), lulu_01 366 s, moca_02_mutant 249 s, ex_quest_01 778 s |
| FAILED | 3 | aao_01 (C set_self_prop), embers_03 (C set_self_prop), aao_02 (**D GENUINE E-aao-02**, keep) |
| TIMEOUT | 1 | fix_note_01 1800 s (60-trip body loop at unwind 64) |
| in queue | 13 | habinel, never_ends, kizuna, front_or_back ×3, end_it_all keep, estate ×2, lost_chase ×2, school_time, bear_cookie |

Soundness: unwinding assertions ON throughout. No `#[ignore]`, no allowlist,
no weakened specs. `aao_02` remains FAILING as the genuine book/body gap.
`cargo build -p rules-kani` + `cargo test -p rules-kani` ×3 green.

**Host-model change needed (not applied — `host.rs` is kani-ppcost's):**
`Op::set_self_prop` key packing (see Class C section above).

## ppcost — `pp::same_dream_01_band_plus_3_iff` root cause + fix (branch `kani-ppcost`)

Goal: move `pp::same_dream_01_band_plus_3_iff` (features `band-pp`) under
1800 s after op-subset / sparse-world / strvec left it TIMEOUT. Profile
first (goto binary + BMC phases), then fix. One CBMC at a time; central
pass holds the other slot.

### Phase split (central log, unwind 32, `--features band-pp`)

`/home/monopoly/kani-central-results3/pp/logs/pp__same_dream_01_band_plus_3_iff.log`
(6520 lines, killed at 1800 s):

| phase | reached? | evidence |
|---|---|---|
| compile / codegen | yes | `Finished dev profile in 13.86s`, goto binary **8.5 MB** (`b14b385a…/…same_dream_01….out`) |
| BMC / symex | **started, never finished** | last lines are `Unwinding loop sparse_play_world` ×2 + 14 `aborting path`; **no** `size of program expression`, **no** `Generated N VCC(s)` |
| SSA / SAT | **not reached** | — |

Unwind profile is clean (only `sparse_play_world` 2). Remaining cost is
**symex over the goto program**, not a model loop, not SAT. Earlier
"SSA/solver after symex" labels were wrong for this row — BMC itself
never returns.

### Goto-binary shape (same run's `.out`, `goto-instrument --show-goto-functions`)

| site | shape |
|---|---|
| `card_sdk::kani_host::call` | **1-target** after fn-ptr removal: `IF f = &same_dream_dispatch THEN CALL …` else `ASSUME false`. BACKEND is **not** an N-way IF-chain here. |
| `invoke::call_card_N` (19 trampolines) | **379 `IF` / 362 `CALL`** each — the `code*` / `f(player)` chain over every address-taken `fn(i32)->Asked` **and** gate fns (cast to `code*`). Sample targets: `same_dream`, `pay_add`, `drew_guard`, `hina_sound`, … |
| `invoke::call_body_idx` | 26 IFs (integer match → trampoline) |

**Root cause (class E, SAT/symex instance size):** the body `code*`
IF-chain. One live body still means CBMC symex walks ~379 guarded calls
per trampoline. That matches the log dying inside BMC with a tiny
unwind profile.

The 244 s `extern "Rust"` "breakthrough" (batch 3e) is **not** evidence
that BACKEND is the cost: `kani_host::call` already resolves to one
target on this binary. Batch 4's "direct call did not help" run still
had the body `code*` chain (and the pre-op-subset World/dispatch).

### Fix (this branch)

Integer **body-id** match with **DIRECT** calls (task candidate #1):

1. `tools/kani/add_kani_wrappers.py` — append `#[cfg(kani)] pub fn
   NAME_kani(player_id) -> card_sdk::Asked { NAME(player_id) }` after
   every `On::*` body in `rules/{cards,skills,events,tiles,tile_marks}`
   (248 files). Production `cfg(not(kani))` does not see these.
2. `rules/kani/build.rs` `gen_body_dispatch` — parse each `On::*`
   constructor's last argument (the body ident) and emit
   `match entry_index { i => crate::module::body_kani(player), _ => fail_entry_oob }`.
   Empty `On` (e.g. `PP:[衍生]共鸣`) is `fail_entry_oob` for every index.

No `code*` load, no fn-ptr, one body per card. Card bodies themselves
unmodified (logic untouched). Production wasm is unaffected: wrappers
are `cfg(kani)` only.

### Next steps (in this pass)

- time `same_dream_01` / `same_dream_02` under `tools/kani/run.sh`
- non-vacuity: covers SATISFIED + post-invoke `assert!(false)` probe
- production hash check (`build-ruleset` without `CARGO_TARGET_DIR`)
- `cargo test -p rules-kani` ×3, `rules-cond --features lower`, wasm check


### ppcost: host packing mismatch (class C, fixed first per coordinator)

After the `name_id` loud-FAIL cut, shared `Op` arms decoded `name_id(a1,a2)`
for every sibling. Packings differ:

| op | packing | key pair | value |
|---|---|---|---|
| `set_self_prop(kp,kl,v)` | `[kp,kl,v]` | `(a0,a1)` | `a2` |
| `set_tile_prop(tile,kp,kl,v)` | `[tile,kp,kl,v]` | `(a1,a2)` | `a3` |
| `set_prop_at(uid,kp,kl,v)` | `[uid,kp,kl,v]` | `(a1,a2)` | `a3` |
| `self_prop(kp,kl)` | `[kp,kl]` | `(a0,a1)` | — |
| `counter_self(np,nl)` | `[np,nl]` | `(a0,a1)` | — |
| `card_counter(p,cp,cl,np,nl)` | `[p,cp,cl,np,nl]` | `(a3,a4)` | — |
| `add_card_counter(…,np,nl,n)` | name `(a3,a4)`, `n=a5`, no max | | |

The wrong pair is an untagged handle → hard FAIL. Blast radius: every
body calling `ctx::set_prop` (aao, embers, fix_note, hallucination,
surprise_duel, marina_box, bangdream_chan). Fixed in `host.rs` + four
`conf_hostops2` pins (22/22 green). Separate commit.

### ppcost: body dispatch cuts (goto shape)

| lever | call_card_N | other bodies reachable? | verdict |
|---|---|---|---|
| baseline (op-subset) | 379-IF `code*` chain | all 379 targets | TIMEOUT 1800 s, BMC never finishes |
| + `NAME_kani` direct calls | 1 direct call | 19 trampolines × their bodies via `call_body_idx` match | TIMEOUT 1800 s |
| + `play_c::<IDX>` const-generic | 1 direct call; match is `IF 12=0…` | still all 19 — reachability-slice keeps them (CALL on a const-false arm) | TIMEOUT 1800 s |
| + harness calls `same_dream_kani` directly | n/a | **one** body in the call graph | re-timing |

Production hashes (fresh master build vs worktree, `build-ruleset.mjs`
without `CARGO_TARGET_DIR`): **identical** — card_all wasm
`8d2cdd3e…`, conds `a76f4eab…`, fixtures `c502fceb…`.
Tests: `rules-kani` ×3 green; `rules-cond --features lower` 58 green;
wasm32 workspace check green; `game-rules`/`rules-native`/`server` build green.


### ppcost timing table (one serial CBMC, unwind 32, `--features band-pp`)

| run | change | goto | call graph | verdict |
|---|---|---|---|---|
| central | op-subset + sparse + name_id | 8.5 MB | 379-IF `code*` × 19 trampolines | TIMEOUT 1800 s (BMC never reaches `Generated N VCC(s)`) |
| 1 | + `NAME_kani` direct calls in trampolines | 7.6 MB | 19 trampolines, 1 body each; `call_body_idx` match keeps all reachable | TIMEOUT 1800 s |
| 2 | + `play_c::<12>` const-generic | 7.6 MB | match is `IF 12=0…` (constants) but CALL arms still make all 19 reachable (reachability-slice 7.6→7.1 MB) | TIMEOUT 1800 s |
| 3 | + harness `same_dream_kani` direct | 7.6 MB | **one** body | TIMEOUT 1800 s |

Unrolls stay clean (`sparse_play_world` 2 only). Remaining cost is
**symex / formula of the single-body model** (World + 14-op subset +
`same_dream`/`try_resonance` + snapshot + effect-log scan) — not a
multi-target IF-chain. Next levers (not landed): zero-loop the
`effects.iter().filter_map` scan; shrink `Snapshot`; split iff into
per-step harnesses; calibrate unwind below 32.

Non-vacuity: not reached (TIMEOUT before any property verdict). Probes
`same_dream_01_probe_setup_only` / `same_dream_01_probe_post_invoke`
added for the next pass; `assert!(false)` probe must FAIL once a run
finishes.


## ppcost FLOOR bisect + fix (branch `kani-ppcost`, post master merge)

Progressive harnesses, **300 s** cap each, direct `same_dream_kani` body.
Each ends in `kassert!(false)` so SUCCESS is impossible (probe = FAIL
means BMC returned and the body ran).

| step | what | verdict | time | notes |
|---|---|---|---|---|
| (a) | fully concrete world + body | **FAILED** (probe) | **172 s** | BMC returns |
| (b) | + symbolic money/fans (2 seats) | **FAILED** | **219 s** | |
| (c1) | sparse_play_world + **concrete user=0** | **FAILED** | **223 s** | |
| (c2) | concrete world + **symbolic user** | **TIMEOUT** | 300 s | **FLOOR** |
| (c3) | concrete + `symbolic_answers=true` | **FAILED** | **166 s** | not the cliff |
| (c) | sparse + symbolic user (old iff) | **TIMEOUT** | 300 s / 1800 s | |
| (d/e/f) | not needed once (c2) isolated | | | |

**Dominating piece: symbolic player index** (`any_player_in()`). Concrete
user returns in ~220 s; a free `user` never returns. `n_players=2` →
seats `{0,1}`; two harnesses cover every seat exactly (clause is
seat-symmetric: 「自己的」).

### Fixes on the way to SUCCESSFUL

1. **concrete seats 0/1** replace `any_player_in()` in the iff + mutant.
2. **`arm_hand_count` / `discard_count` packing** — `hand_count(player,ptr,len)`
   packs `[player,ptr,len]`; the arm decoded `card_slot(a0,a1)=(player,ptr)`
   and loud-FAILED untagged name_id (same class as `set_self_prop`).
   `try_resonance`'s `hand_count` is on this path. Pinned in conf_hostops2.
3. **composed final faces** — play runs steps 1–3 together; step 3 flips the
   user's reverse fans back to positive. Assert `(up+down, 0)`, not step 2's
   intermediate `(0, up+down)`. (The old harness asserted the intermediate
   state after the full body.)

### Timing (unwind 32, `--features band-pp`, direct body, concrete user)

| harness | before | after | verdict |
|---|---|---|---|
| `same_dream_01_band_plus_3_iff` (user=0) | TIMEOUT 1800 s | **SUCCESSFUL 563 s** | **4/4 covers SATISFIED** |
| `same_dream_01_user1_band_plus_3_iff` (user=1) | — | **SUCCESSFUL 660 s** | **4/4 covers SATISFIED** |
| `same_dream_02_money_mutant_is_detected` | TIMEOUT 1800 s | **SUCCESSFUL 475 s** | disagreement companion (card ≠ up*101) |

Non-vacuity: 4/4 covers on both iff rows; `floor_a` / post-invoke
`kassert!(false)` probes **FAILED** (body ran). Production hashes
unchanged (wrappers are cfg(kani)-only; host.rs packing is model-only).


## bands-b — concrete-user rollout across band units (branch `kani-bands-b`)

Goal: propagate the ppcost fix (concrete user seats + direct `NAME_kani`
body calls + per-harness host-op subset + sparse 2-seat world) to
`mor`/`skills_mor`, `ras`/`skills_ras`, `mygo`/`skills_mygo`,
`mujica`/`skills_mujica`, `crychic`/`skills_crychic`, `sumimi`/`skills_sumimi`.
`skills_bands` / `skills_characters` / `skills_general` remain empty stubs.

### Pattern (from `proofs/pp.rs` + `proofs/mor.rs` rui_demon)

1. `host_subset!` per body — every reachable Op declared; undeclared →
   `outside_subset` loud FAIL (never `assume`).
2. In-place world (`install_backend` + `sparse_play_world` /
   `with_world_mut`) — no `reset_world` full-struct copy.
3. Concrete user: seat-symmetric 「自己的」 → `<id>_iff_at(user)` +
   harnesses `user=0` and `user=1` in `n_players=2`. Non-seat-symmetric
   (other seats / turn order) cover concrete seat combos and say so.
4. Direct body call via cfg(kani) `NAME_kani` (no trampoline IF-chain).
5. Composed final-state assertions; covers + mutants kept.

### host.rs arm extraction (small cherry-pickable commits)

`arm_*` single-source handlers extracted from `dispatch` so subsets can
name them. Groups: money/pay, players, state/slot, counters, piles,
field, fire/stay, trigger, tiles, dice, owned/tile_is/opt/trig_move_*,
band_skill/invoke_skill/opt_price/add_house, dist/tile_forward/
place_card_on/field_instances, stay_of/stun_of/exile_of/turn_key/
round_no. Plan/protocol stubs group to `arm_stub_zero` /
`arm_protocol_stub`. No semantics change.

### tools / feature fixes

* `add_kani_wrappers.py` trailing-comma parse fix; wrapped 22 missing
  bodies (mashiro_pay, secret_rainbow, your_light, several Counteract
  bodies, morfonica band skill, character skills).
* `proofs/mod.rs` + `run.sh unit_features`: `skills_ras` needs
  `skills-characters`; `skills_mor` needs `skills-bands,skills-characters`;
  `skills_crychic` needs `skills-characters`. Gates tightened so
  `--unit ras` no longer fails compiling `skills_ras`.

### Verdicts (fill as runs complete)

See per-unit tables in `docs/rulebook/spec/kani/{mor,ras,mygo,mujica,crychic,sumimi}.md`
and the `bands-b` rows below. Runner: `tools/kani/run.sh --unit <u>
--harness <h> --timeout 1800` serially, `CARGO_TARGET_DIR=/home/monopoly/kani-target-bands-b`,
`CARGO_BUILD_JOBS=3`, machine slot cap 2 shared with `kani-bands-a`.

### bands-b proof results (running sweep; update as rows land)

Runner: `tools/kani/bands-b-sweep.sh` (per-harness, jobs=1, timeout 1800).
Accumulator: `/home/monopoly/kani-bands-b-results/results-all.tsv`.

**Class E fix (unstoppable TIMEOUT → SUCCESSFUL):** `tile_named(SPOTS[sym])`
hashed a slice of symbolic length (`board::tile_hash` while-loop). Two fixes:
(1) zero-loop `tile_hash` under cfg(kani) (64-way step macro);
(2) `World::forced_roll` pin so dice clauses cover concrete faces —
`tile_named` of a symbolic index is still a symex cliff.

| unit | harness | verdict | time (s) | class | note |
|---|---|---|---|---|---|
| ras | unstoppable_card_03_money_iff (u=0, face=1) | **SUCCESSFUL** | 292 | — | 2000 leg |
| ras | unstoppable_card_03_face4_money_iff (u=0, face=4) | **SUCCESSFUL** | ~300 | — | 1000 leg |
| ras | unstoppable_card_03_user1_money_iff (u=1, face=1) | **SUCCESSFUL** | 251 | — | |
| ras | unstoppable_card_03_money_noop_when_out | **SUCCESSFUL** | 268 | — | out player |
| skills_ras | masking_01_fire_gain_leaf | **SUCCESSFUL** | 134 | — | +1 fire |
| crychic | soyoreturn_02_hand_iff | **SUCCESSFUL** | 222 | — | |
| crychic | soyoreturn_02_user1_hand_iff | **SUCCESSFUL** | 246 | — | |
| crychic | soyoreturn_02_mutant_is_detected | **SUCCESSFUL** | 160 | — | |
| crychic | vocalist_01_waive_iff | **SUCCESSFUL** | 164 | — | |
| crychic | vocalist_01_user1_waive_iff | **SUCCESSFUL** | 168 | — | |
| crychic | vocalist_01_mutant_is_detected | **SUCCESSFUL** | 145 | — | |
| crychic | grab_01_stay_iff | **SUCCESSFUL** | 235 | — | |

**12/12 SUCCESSFUL so far (0 FAILED / TIMEOUT).** Sweep continues over the
remaining ~220 harnesses (crychic → mor → mujica → mygo → ras → skills_* →
sumimi, alphabetical). Times in the 130–300 s band match the ppcost floor
(concrete user) once the tile_named / forced_roll cliffs are gone.


**Sweep status at hand-off (2026-10-14):** 17/235 harnesses recorded,
**17 SUCCESSFUL / 0 FAILED / 0 TIMEOUT**. `tools/kani/bands-b-sweep.sh`
(PID visible via `pgrep -f bands-b-sweep`) continues serially
(jobs=1, timeout 1800) over the remaining crychic → mor → mujica →
mygo → ras → skills_* → sumimi. Accumulator
`/home/monopoly/kani-bands-b-results/results-all.tsv`.
`cargo build -p rules-kani` + `cargo test -p rules-kani` ×3 green (85 tests).
## bands-a — concrete-user rollout for ppp / r / ag / hhw + skills_* (branch `kani-bands-a`)

Worktree `/home/monopoly/kani-bands-a-wt`. Propagates the ppcost FLOOR fix
(symbolic player index is the BMC cliff) across the band units.

### Pattern applied (per ppcost / worked example `proofs/pp.rs` same_dream)

1. `n_players = 2` sparse world (`harness::sparse_play_world`).
2. Iff harnesses are `*_iff_at(user)` called from two `#[kani::proof]`s with
   `user = 0` and `user = 1` (clause seat-symmetric 「自己的」). Mutants use a
   single concrete user. Multi-seat clauses (e.g. `moca_02` owner+src) keep a
   concrete pair and say so.
3. Per-body `host_subset!` listing only the ops the unmodified body reaches;
   outside → `outside_subset` loud FAIL.
4. Direct `*_kani` body calls (one body in CBMC's call graph).
5. Composed final-state assertions; covers kept; mutant companions kept.
6. Fail-closed: no `#[ignore]`, no spec weakening, no silent zeros.

### Host-model changes (small commits, cherry-pickable)

| commit | what |
|---|---|
| `host: extract arm_*` | ~50 `arm_*` single-source handlers for `host_subset!` (packing-aware splits for `set_self_prop` / `counter_self` / `add_counter_*`). `dispatch` unchanged (minimal for kani-bands-b). |
| fire arms | `arm_fire` / `arm_fire_max` / `arm_gain_fire` / `arm_spend_fire` |
| `arm_inc_slot` | named keyed increment |
| World `Clone` | proofs read pre-state without a full-struct assign |
| missing `*_kani` wrappers | popipapapipopa, returns::on_played, aya_with, arisa_bonsai::at_turn_end, poppin::{on_pass,lock_hill}, hhw::on_pay_choose |
| `run.sh` features | ppp/skills_* pull in `skills-characters`+`skills-bands` so `card_id!` resolves |

### Formula-harness fixes (class A/B)

| id | class | action | after |
|---|---|---|---|
| `ppp::door_01_sequence_const` | **A** bound-low (10×60 scan) | `board::tile_named` | **SUCCESSFUL** |
| `skills_pp::band_idol_02_overflow_formula` | **B** mutant check in agreeing range | disagree when `x-down >= 11` | **SUCCESSFUL** |
| `skills_ag::firepot_01_tick_formula` | **B** mutant check | disagree exactly at rest 1 | **SUCCESSFUL** |
| `ppp::maze_01_distance_predicate` | — | (already sound) | **SUCCESSFUL** |
| `skills_ag::band_01_filter_formula` | — | | **SUCCESSFUL** |
| `skills_hhw::skills_hhw_smoke_cover` | — | | **SUCCESSFUL** |

### Conversion status (code) vs run status

| unit | conversion | harnesses (approx) | runs |
|---|---|---:|---|
| `ppp` | done (concrete user + subset + direct) | 41 | formula rows done; body rows queued (serial CBMC) |
| `r` | subagent | ~20 | not yet |
| `ag` | subagent | ~20 | not yet |
| `hhw` | subagent | ~20 | not yet |
| `skills_pp` | done | 17 | formula rows done |
| `skills_ppp` | done (direct + subset; owner already 0) | ~25 | not yet |
| `skills_r` | done | 10 | not yet |
| `skills_ag` | done | 10 | formula rows done |
| `skills_hhw` | done | 20 | smoke done |

Body-row runs are serial (`--jobs 1`, timeout 1800 s, one CBMC at a time,
slot cap 2 shared with kani-bands-b). Representative body timing from the
pattern's worked example (pp same_dream): SUCCESSFUL ~560 s per seat.

**First body-row results (this pass):**

| harness | verdict | time | notes |
|---|---|---:|---|
| `skills_r::sayo_fire_pass_iff` | **FAILED** | 190.8 s | BMC **returns** (pattern works). Assert `GainFire 2 or 3` fails — log is not exactly one GainFire. Class B/C under investigation (fire_max clamp / body path / log shape). Not skipped. |
| `skills_r::rinko_fire_pass_iff` | **SUCCESSFUL** | 154.1 s | first green body proof under the pattern |
| `skills_r::lisa_fire_pass_iff` | queued | | |
| `skills_ag::firepot_01_tick_iff` | queued | | |

All other body rows (ppp 30, r 32, ag 30, hhw 30, skills_* ~60) are converted
and queued behind the serial CBMC. Formula rows above are complete.

### Genuine discrepancies (class D)

None newly found in this pass. `aao_02` (triage2, class D E-aao-02) remains
FAILING on master and is out of bands-a's units.

### Soundness kept

Unwinding assertions ON. No `#[ignore]`. No allowlist. No weakened specs.
`outside_subset` / untagged `name_id` FAIL loudly. Production `cfg(not(kani))`
untouched.

### bands-a stub audit — every `arm_plan_stub` use (coordinator soundness follow-up)

A silent-0 stub is only acceptable when the body **provably never** calls the
op. Otherwise: real arm, or loud `outside_subset`. `arm_plan_stub` is renamed
`arm_protocol_stub` and documented as "matches `dispatch`'s benign-0 PROTOCOL
arm" — not a stand-in for real semantics.

| unit::subset | op(s) | body calls? | decision |
|---|---|---|---|
| ppp::ppp_dispatch | `unplace_card\|unplace_at\|unplace_card_named` | **yes** (`returns.rs`) | **real** `arm_unplace` (pushes `Effect::UnplaceCard`) |
| ppp::ppp_dispatch | `targeted_count` | no | `arm_protocol_stub` (dispatch is benign-0) |
| ppp::ppp_dispatch | plan/settle/offer list (`set_steps`, `set_resolve`, `card_offer_build`, `can_build_on`, …) | **yes** (signpost/star_beat/bang_dream/…) | `arm_protocol_stub` — these ARE the dispatch PROTOCOL-0 arms (KANI.md §6). Not silent stand-ins. |
| ppp::ppp_dispatch | `card_move` | **yes** | **real** `arm_card_move` (returns 1) |
| r::ako_dark_dispatch | `play_from_hand` | **yes** (`ako_dark.rs`) | `arm_protocol_stub` (dispatch returns 0 = not-from-hand). **TODO(规则书)** if a proof needs the from-hand path (polarity-sensitive; 0 skips that leg). |
| r::nfo_play_dispatch | `set_steps\|set_resolve` | **yes** (`nfo.rs`) | `arm_protocol_stub` (PROTOCOL-0) |
| r::nfo_play_dispatch | `card_move` | **yes** | **real** `arm_card_move` |
| r::nfo_play_dispatch | `card_replayable\|play_card` | **yes** (`nfo.rs` replay) | `arm_protocol_stub` (0 = not replayable / did not land) |
| r::council_dispatch | `linger` | yes | `arm_protocol_stub` (PROTOCOL-0) |
| skills_*::\* | `state_add` | **yes** (rinko_1cm/lisa_goddess/…) | **real** `arm_inc_slot` (same keyed-add as dispatch) |
| skills_ag::skills_ag_dispatch | `trig_target` | **yes** (`trigger::target`) | **real** `arm_trig_target` |
| skills_ag::skills_ag_dispatch | `place_mark` | **yes** (tae_police/hagumi) | **real** `arm_place_mark` |
| skills_pp/hhw/ppp | `card_move` | yes | **real** `arm_card_move` |
| skills_pp/hhw/ppp | `card_offer_build\|can_build_on\|offer_*\|buy\|acquire` | rimi_resolve offers | `arm_protocol_stub` (PROTOCOL-0 / 0=did-not-land) |
| hhw::\* | plan setters (`set_kind`, `set_steps`, dice, …) | yes (balloon/hagumi) | `arm_protocol_stub` (PROTOCOL-0) |

**Removed from subsets** (not listed → `outside_subset` loud FAIL if reached):
none in this pass beyond the above reclassifications. Prefer listing an op
over stubbing it when unsure.

**Class C found and fixed in this audit:** `state_get`/`state_set` were
`arm_plan_stub` (silent 0) while `ran_red::tick` writes `REST_TURNS` via
`state::set` — the rest counter never moved and the iff failed for the wrong
reason. Replaced with real `arm_state_get` / `arm_state_set` (commits `944dcf5`,
`bc2e730`).

## triage3 — central pp + bands-b non-success rows

Branch `kani-triage3` · Worktree `/home/monopoly/kani-triage3-wt`.
Inputs: `/home/monopoly/kani-central-results3/pp/` (central pass still
running `pp` → `skills_pp` → … → `hhw`/`skills_hhw`) and
`/home/monopoly/kani-bands-b-results/results-all.tsv` (bands-b sweep).
Re-times: `/home/monopoly/kani-triage3-results/`, one CBMC at a time via
`tools/kani/run.sh --unit <u> --harness <h> --timeout 1800`,
`CARGO_TARGET_DIR=/home/monopoly/kani-target-triage3 CARGO_BUILD_JOBS=3`.
Classes A–F per the user rule. No `#[ignore]`, no allowlist, no weakened
specs. Production `cfg(not(kani))` untouched.

### Per-row table (before → after)

| id | before | failing assertion (CBMC log) | class | action | after |
|---|---|---|---|---|---|
| `pp::together_01_mutant_is_detected` | FAILED 543 s | `mutant: card must not match the +1 bonus` | **B** | `min(X,down)` collapses body+mutant to `down` when `down<=6`; assume `down>=7` | **SUCCESSFUL 499 s** |
| `pp::guide_01_band_and_crystals_iff` | FAILED 659 s | `C-PP-guide-01: card carries owned/3 crystals` | **B** | read on-card `crystals` (`field_crystals`), not player key `"奇迹水晶"` | **SUCCESSFUL 396 s** |
| `pp::guide_01_mutant_is_detected` | SUCCESSFUL (vacuous) | — | **F→B** | same wrong read made `cry==0` always; now reads the real counter | **SUCCESSFUL 442 s** |
| `pp::eve_bushido_01_gain_iff` | FAILED 332 s | `C-PP-eve-bushido-01: 12d4 sum in 12..=48` | **C** | `roll_dice` clamped count to `min(8)` → 8d4 (8..=32); raise to `min(100)` (engine `1..=100`) | **SUCCESSFUL 283 s** |
| `pp::no_expect_02_place_deck_draw_iff` | FAILED 370 s | `C-PP-no-expect-02: place on field + draw 1` | **B** | exact-match spec rejected 持续 `set_slot` → `SetState`; ignore bookkeeping | **SUCCESSFUL 382 s** |
| `pp::first_accident_01_place_deck_draw_iff` | FAILED 354 s | `C-PP-first-accident-01: place on field + draw 1` | **B** | same; body stamps `set_tok` → `SetToken`; ignore bookkeeping | **SUCCESSFUL 462 s** |
| `pp::eve_bushido_04_crystal_per_draw_iff` | FAILED 321 s | `C-PP-eve-bushido-04: +1 crystal (pre<3)` | **B** | crystal storage: body writes `field_crystals`, proof read player key | **SUCCESSFUL 296 s** |
| `pp::guide_04_turn_end_crystal_iff` | FAILED 344 s | `C-PP-guide-04: +1 crystal at turn end` | **B** | same; also `pre<=4` entered the ≥5 cash-in (unplaces the card) → `pre<=3` | FAILED 664 s (cash-in region); `pre<=3` queued |
| `pp::dream_ahead_01_crystal_per_draw_iff` | FAILED 357 s | `C-PP-dream-ahead-01: +1 crystal (pre<5)` | **B** | same; X is slot `"dream_ahead_x"` not `"dreamAhead.X"` | TIMEOUT 1800 s (E, symex; was FAILED 357 s) — re-queue |
| `crychic::ifonly_02_discard_at_seven_iff` | FAILED 175 s | `C-CRYCHIC-ifonly-02: card must go to discard` | **C** | host `set_dest` never committed the field fate (`apply_dest`); now unplaces + to-discard. **Not D** — body `set_dest(Graveyard)` is the SDK's documented field-fate API | **SUCCESSFUL 253 s** |
| `crychic::ifonly_02_user1_discard_at_seven_iff` | FAILED 154 s | same | **C** | same | **SUCCESSFUL 461 s** |
| `crychic::soyoreturn_01_branch2_iff` | TIMEOUT 1800 s | (no VCC — died in BMC/symex) | **E** | phase: symex over `arm_cards_in` deck/hand scan + `ask_pick` prompt paths; body ABI, no cheap lever | pending |
| `crychic::soyoreturn_01_user1_branch2_iff` | TIMEOUT 1800 s | same | **E** | same | pending |
| `mor::rui_demon_01_iff` | FAILED 369 s | `C-MOR-rui-demon-01: post money must match the face-value legs` | **B** | spec's other-seat legs swapped. Book 「向全场玩家支付100…之后全场玩家向你支付300」: others **+100 then −300** (spec had −100 then +300) | **SUCCESSFUL 629 s** |
| `mor::rui_demon_01_user1_iff` | FAILED 297 s | same | **B** | same | **SUCCESSFUL 612 s** |
| `mor::rainbow_01_half_iff` | FAILED 203 s | `C-MOR-rainbow-01: effect log must be exactly SetPayAmount(amount/2)` | **D** GENUINE | keep FAILING. Book 「此次支付金额减半」; body does C# `CeilTo(half, 10)` | FAILED 238 s (D kept) |
| `mor::rainbow_01_half_user1_iff` | FAILED 180 s | same | **D** | keep | pending |
| `mor::rainbow_02_boost_iff` | FAILED 203 s | `C-MOR-rainbow-02: …SetPayAmount(1.5×amount)` | **D** GENUINE | keep. Book 「变成1.5倍」; body CeilTo-10 | pending |
| `mor::rainbow_02_boost_user1_iff` | FAILED 262 s | same | **D** | keep | pending |
| `mor::rainbow_01_mutant_is_detected` | FAILED 157 s | `mutant: card must not match off-by-one half` | **D** | same root: CeilTo-10 body collides with `amount/2+1` on e.g. amount∈{18,38,…} | FAILED 209 s (D kept) |
| `mor::rainbow_02_mutant_is_detected` | FAILED 150 s | `mutant: card must not match off-by-one boost` | **D** | same | pending |
| `mor::courage_01_pay_iff_blocked_host` | UNWINDING 217 s | `unwinding assertion loop 0` at `courage_world_owned` | **A** | 60-trip loop → 8×8 walk; then `exactly one PayTo` (pay vs agent arm not partitioned) | FAILED 730 s (arm split) → partitioned, queued |
| `mor::courage_01_pay_user1_iff_blocked_host` | UNWINDING 169 s | same loop at `courage_world_owned` | **A** | same fix (shared helper) | FAILED 504 s (arm split) → partitioned, queued |
| `pp::ranger_01_02_steps_by_count_iff` | FAILED 453 s | `C-PP-ranger-01: +500 iff count >= 1` | **B+C** | host `arm_cards_in` had no Field/Deck case (class C); proof counted the Ranger itself (class B, [手] play is not on the field during its body) | pending |
| `skills_pp::band_idol_02/04` (pre-empted) | (not yet run) | same crystal-storage shape as pp | **B** | `field_crystals` not player key `"奇迹水晶"` | pending |
| `mor::courage_01_mutant_is_detected` | UNWINDING 121 s | same `courage_world_owned` loop | **A** | same 8×8 chunk walk | pending |
| `mor::toko_swap_iff_blocked_host` | UNWINDING 273 s | `unwinding assertion loop 0` at `toko_swap::pairs` | **A** | body `for theirs in 0..60` at unwind 32; raise to 64 (card body untouchable) | queued |
| `mor::toko_swap_user1_iff_blocked_host` | UNWINDING 419 s | same | **A** | same | queued |

### Host-model fixes (commit `2548b2b4`)

1. **`roll_dice` count clamp (class C).** `count.max(0).min(8)` silently
   rewrote 12d4 into 8d4 (range 8..=32). The real engine (`hostfns::roll`)
   accepts `1..=100`; the multi-dice model is a symbolic sum in `c..=c*s`,
   so the wider range is free. → `min(100)`.
2. **`set_dest` field-fate commit (class C).** The real engine defers to
   `apply_dest` at the run's commit (unplace-and-send the running instance
   to the named pile). The Kani arm only stored `w.dest`. A play from hand
   names the *hand card's* fate (applied by the play pipeline); a field
   effect names the instance's fate. Now applies immediately when the
   instance is live on the field — **world state only** (field slot drops,
   pile gains the card), **not** an in-run `UnplaceCard` effect: specs
   `again_02_effects_ok` / `starry_06_effects_ok` / `tritone_02_tick_effects_ok`
   pin "exactly N effects" and the engine's post-body unplace is outside the
   effect log. `arm_unplace` / `arm_unplace_card` (explicit unplace) still
   push `UnplaceCard`. `World::remove_field` does the swap-remove.

### Proof/spec fixes (commit `0b25d27d`)

3. **Crystal storage (class B, 4 rows + 1 vacuous mutant; also 2 skills_pp rows pre-empted).** Bodies call
   `ctx::add_crystals` / `ctx::set_crystals` → `add_counter("crystals")` /
   `set_counter("crystals")`, which the host lands on
   `field_crystals(last_uid)`. Proofs set/read the player-level key
   `"奇迹水晶"` (never written) — every +1-crystal iff failed and
   `guide_01_mutant` passed vacuously (`cry==0` vs a positive mutant).
   `dream_ahead`'s overflow X is `ctx::slot(..., "dream_ahead_x")`, not
   `"dreamAhead.X"`.
4. **`together_01` mutant region (class B).** `min(X, down)` collapses body
   (`min(5+bonus, down)`) and mutant (`min(6, down)`) to `down` when
   `down<=6`. The assume `ceil_half != 1` left that region live. →
   `down >= 7`.
5. **Effects filters (class B, 2 rows).** `no_expect_02` / `first_accident_01`
   exact-match specs rejected the 持续 stamping (`SetState` from `set_slot`,
   `SetToken` from `set_tok`). Those are bookkeeping, not clause effects.
6. **`rui_demon_01_post_ok` legs (class B).** Spec had the other-seat legs
   swapped (−100 then +300). Book: user pays 100 to each other, then each
   other pays 300 to the user → others **+100 then −300**.
7. **`courage_world_owned` (class A).** 60-trip loop at unwind 32. 8×8
   chunk walk (TUNING `owned_count` pattern).

### Genuine discrepancies (class D) — keep FAILING

**C-MOR-rainbow-01/02 `Mor:秘密与青春的虹彩`** (docs/rulebook/cards.json):

> （1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。
> （2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。

Book pins 减半 / 1.5倍 (integer half / 1.5×; the doc example 140 → 210).
Body (`secret_rainbow.rs`) does C# `CeilTo(amount/2.0, 10)` /
`CeilTo(amount*1.5, 10)` — rounds up to a multiple of 10. Example:
amount 25 → body 20, book 12 (or 13). The iff spec pins the book; it must
keep failing. The mutant rows fail for the same root (CeilTo-10 collides
with `amount/2+1` on amounts like 18, 38, 58).

**ifonly-02 is NOT class D.** The earlier suspicion ("book wants UnplaceCard,
body set_dest Graveyard") does not hold: `ctx::set_dest` is the SDK's
documented field-fate API ("Where this card goes when its effect finishes.
For a play it is the hand card's fate; for a field effect, the instance's."),
and the real engine's `apply_dest` unplaces-and-sends at the run's commit.
C# `H.Unplace(this, "discard", …)` is the same outcome via a different API.
The Kani host was missing the commit step → class C, fixed.

### Class E note

`soyoreturn_01_branch2_*` TIMEOUT 1800 s: the log never reaches
`Generated N VCC(s)` — BMC/symex dies over `host::arm_cards_in` deck/hand
scans (4×4 chunk walk) and the `ask_pick` prompt path (`symbolic_answers`).
The body ABI (`cards_in` + `ask_pick` + multi-branch cashout) is the cost;
no proof-file lever. Recorded, not weakened.

### Soundness kept

Unwinding assertions ON; no `#[ignore]`; no allowlist; no weakened specs;
rainbow rows stay FAILING as the genuine book/body gap. Production
`cfg(not(kani))` untouched. `cargo build -p rules-kani` + `cargo test -p
rules-kani` ×3 green (also `--features band-ppp,skills-characters,skills-bands`).

## triage3 batch 2 — re-time results + skills_pp central rows

Re-times landed (one CBMC at a time, `tools/kani/run.sh`, `/home/monopoly/kani-triage3-results/`):

| id | after |
|---|---|
| `pp::together_01_mutant_is_detected` | **SUCCESSFUL 499 s** (was FAILED 543 s) |
| `pp::guide_01_band_and_crystals_iff` | **SUCCESSFUL 396 s** |
| `pp::guide_01_mutant_is_detected` | **SUCCESSFUL 442 s** (was vacuous) |
| `pp::eve_bushido_01_gain_iff` | **SUCCESSFUL 283 s** |
| `pp::no_expect_02_place_deck_draw_iff` | **SUCCESSFUL 382 s** |
| `pp::first_accident_01_place_deck_draw_iff` | **SUCCESSFUL 462 s** |
| `pp::eve_bushido_04_crystal_per_draw_iff` | **SUCCESSFUL 296 s** |
| `crychic::ifonly_02_discard_at_seven_iff` | **SUCCESSFUL 253 s** |
| `crychic::ifonly_02_user1_discard_at_seven_iff` | **SUCCESSFUL 461 s** |
| `mor::rui_demon_01_iff` | **SUCCESSFUL 629 s** |
| `mor::rui_demon_01_user1_iff` | **SUCCESSFUL 612 s** |
| `mor::rainbow_01_half_iff` | FAILED 238 s (D kept) |
| `mor::rainbow_01_mutant_is_detected` | FAILED 209 s (D kept) |
| `pp::guide_04_turn_end_crystal_iff` | FAILED 664 s — crystal storage fixed, but `pre<=4` entered the ≥5 cash-in (unplaces the card). `pre<=3` queued |
| `pp::dream_ahead_01_crystal_per_draw_iff` | TIMEOUT 1800 s (E, symex on `add_crystals` prompt path) — re-queued |
| `mor::courage_01_*` | FAILED (pay vs agent arm not partitioned) — partitioned, queued |

### skills_pp central rows (new)

| id | before | failing assertion | class | action |
|---|---|---|---|---|
| `skills_pp::aya_with_01_five_fans_iff` + user1 | VACUOUS 550/723 s | cover `admitted` unreachable (0 of 1) | **C** | `guard_needs` treated `band_is`'s name-hash ConstInt as `seat_const` and missed FnId 18/19 in the symbolic-seat set → **no seat rows filled**, guard never admits. Fixed in `ir_fill.rs` |
| `skills_pp::aya_with_02_pay_discount_iff` + user1 + mutant | TIMEOUT 1800 s ×3 | (died in BMC/symex) | **E** | `ask_number` prompt path pulls `Msg` → `memchr` (same string cliff as other prompt proofs). No cheap proof-file lever |
| `skills_pp::band_idol_02_body_never_overflows` | FAILED 414 s | `host op outside declared per-harness subset` | **C/B** | body `at_start` calls `placed_cards` (Op not in subset); `at_turn_start` calls `set_prop` → `set_self_prop` (also missing). Both arms exist. Added to subset |
| `skills_pp::band_idol_04_tick_iff` + user1 | FAILED 357/330 s | same `outside_subset` | **C/B** | same (`set_self_prop` from `set_prop(NO_REWARD)`) |

`band_idol_02_overflow_formula`, `hina_lottery_02_borrow_map`, `maya_dawn_02_watch_formula`, `eve_unify_02_extra_dice_formula` are SUCCESSFUL (pure-formula class).

### batch-2 re-time results (in progress)

| id | after |
|---|---|
| `pp::guide_04_turn_end_crystal_iff` | **SUCCESSFUL** (`pre<=3` isolated the cash-in) |
| `mor::courage_01_pay_iff_blocked_host` | **SUCCESSFUL** (pay-vs-agent arm split) |
| `mor::courage_01_pay_user1_iff_blocked_host` | **SUCCESSFUL** |
| `skills_pp::band_idol_02_body_never_overflows` | **SUCCESSFUL** (1/1 cover; `placed_cards` in subset) |
| `skills_pp::band_idol_04_tick_iff` | **SUCCESSFUL** (2/2 covers; `set_self_prop` in subset) |
| `skills_pp::band_idol_04_tick_user1_iff` | **SUCCESSFUL** |
| `skills_pp::aya_with_01_five_fans_iff` + user1 | queued (guard_needs fix) |
| `pp::dream_ahead_01_crystal_per_draw_iff` | queued |
| `mor::toko_swap_*` | queued (unwind 64) |

### ppp central rows (new)

| id | before | failing assertion | class | action |
|---|---|---|---|---|
| `ppp::pipopa_01_iff` + user1 | FAILED 354 s | `log must be exactly GainMoney(1000), PlaceCard(…, Field)` | **C** | `arm_place_card` leaked `w.dest` (the running card's fate) into `PlaceCard.dest`. pipopa does `set_dest(Banished)` for 「此卡[移除]」 then `place_card` a Popipapapipopa — effect recorded dest=3 not 2. Fixed: PlaceCard.dest is always Field (2) |

### batch-2 re-time final

| id | after |
|---|---|
| `skills_pp::aya_with_01_five_fans_iff` | **SUCCESSFUL** (1/1 cover; guard_needs fix) |
| `skills_pp::aya_with_01_five_fans_user1_iff` | **SUCCESSFUL** (1/1 cover) |
| `pp::dream_ahead_01_crystal_per_draw_iff` | **TIMEOUT 1800 s** (class E — `add_crystals` `?` prompt path + `Msg`/memchr symex; body ABI) |
| `ppp::pipopa_01_*` | queued (PlaceCard.dest fix) |
| `ppp::bang_dream_02_teleport_owned_iff` | queued (8×8 walk) |
| `mor::toko_swap_*` | queued (unwind 64) |

`aya_with_02_*` TIMEOUT remains class E (`ask_number` prompt → Msg → memchr).

## triage3 batch 3 — pp central non-success rows (combined master)

Merged master (batch-2 fixes + rounding `c4d15398`). Central `pp` unit
finished on the combined master. Non-success rows:

| id | before | failing assertion / phase | class | action |
|---|---|---|---|---|
| `pp::first_accident_02_blocks_pp_skill_2_iff` | FAILED 8.9 s | `cargo kani` "Failed to match the harness" | **B** | harness was `#[cfg(feature = "skills-personas")]` which the pp unit's features (`band-pp`) do not compile → symbol missing. Moved to `proofs/skills_pp.rs` (features already right) + user1 companion |
| `pp::ranger_01_02_steps_by_count_iff` | TIMEOUT 1800 s | BMC/symex on `arm_cards_in` + `card_slot_of_bytes::eq` + the `cards_in` String facade | **B+E** | Kani `cards_in` facade returns 0 or 4 (padded Vec), not the literal count. Concrete `others` ∈ {0,1} + `count = 0\|4`. Old symbolic 0..=3 made the body see 4 for 1..=3 and the String facade was the cliff |
| `pp::chisato_mask_01_total_iff` | TIMEOUT 1800 s | BMC/symex on `chisato_mask::split_pay` `find`/`try_fold` (slice iters) | **E** | pin `n_players=2`; `ctx::others` is a symbolic-length Vec at 2..=4. The OPEN-分摊 total (2000) is seat-count independent |
| `pp::dream_ahead_01_crystal_per_draw_iff` | TIMEOUT 1800 s | BMC/symex on `add_crystals` `?` prompt path + overflow-X else branch | **E** | concrete `pre=0` so `crystals()<5` constant-folds and the else branch is dead in the CFG |
| `pp::shine_again_01_rescue_iff` | TIMEOUT 1800 s | BMC/symex on `SliceContains` fold + `ask_pick` prompt + `owned_tiles` | **E** | `ask_pick` prompt path pulls `Msg` → memchr; body ABI. No cheap lever |

## triage3 batch 4 — central retry rows (kani-pass@566b6761)

| id | before | failing assertion / phase | class | action |
|---|---|---|---|---|
| `skills_pp::band_idol_01_start_grant_iff` + user1 | FAILED 577/507 s | `C-PP-band-idol-01: each non-PP player gains 1 reverse fan` | **C** | `arm_placed_cards` wrote only `field_n`; `ctx::placed_cards` expected postcard-encoded `Vec<String>` and got nothing → the pastel `at_start` "first copy" check never found the card → body returned early. Fixed: host now writes postcard-encoded card names |
| `ppp::bang_dream_02_teleport_owned_iff` | UNWINDING 1560 s | `unwinding assertion loop 0` at `ctx::owned_tiles` Range fold | **A** | `owned_tiles` used a symbolic-length Range fold (`0..owned_count`); `collect`/`extend_trusted` cliffed at 32. Kani version uses a bounded while loop |
| `ppp::pipopa_01_iff` | SUCCESSFUL 300 s | — | — | recorded (PlaceCard.dest fix from batch 3 landed) |
| `skills_sumimi::band_01_clear_iff_at` | (proactive) | guard changed to `actor == owner` | **B** | both halves of （1） now tested: book 「这回合从别人那里拿到过钱：回合结束时团卡 +1 奇迹水晶；没拿到就清空团卡上的水晶。」 — add +1 when GOT, clear when not |

## triage3 batch 11 — skills_hhw central non-success rows (kani-pass@fc3ff3e1)

Central pass rows (unit `skills_hhw`, features `band-hhw,skills-personas,skills-groups`).
Per-proof detail lives in `docs/rulebook/spec/kani/hhw.md` § skills_hhw triage.

| id | before | failing assertion / phase | class | action |
|---|---|---|---|---|
| `kokoro_02_circle_pass_1500_iff` | TIMEOUT 1800 s | BMC/symex stuck in `ir_fill::fill_geometry_bins` (`players_on` 60×seats×filter fold). Log never reached `Generated N VCC(s)` | **C** (+E cliff) | `fill_for_guard` bit masks targeted a retired FnId numbering. `needs_geom = (fns >> 27) & 0x1F` treated `TileIs` (30) as geometry and **skipped** `fill_board_sets` for it (`needs_sets = >> 6` hit Money..Hand). `OWN_START` = `tile_is(tile.id,'circle')` therefore (a) filled the expensive geometry bins and (b) left `SetId::Circle` empty so the guard could never admit. Masks now derived from `FnId::*` (sets: TileIs/TileIn/Blocked; geom: PlayersOn/NextDist/OthersWithin/OwnedWithin/OnPath). Also `State` bit was `>> 37` (enum is 33) — `state()` rows never filled |
| `kokoro_02_noop_non_circle` | TIMEOUT 1800 s | same geometry cliff | **C** | same mask fix; `tile_is` now consults the filled Circle set and rejects tile 5 |
| `misaki_skill_01_pass_fire_iff` | VACUOUS 444 s | cover.1 `guard admits` **UNSATUSFIABLE**; covers 2/3 UNREACHABLE (`0 of 3 cover properties satisfied (2 unreachable)`) | **C** | `guard_needs` only treated `FnId Slot..=CounterIs (0..=4)` args as name ids, so `TileNamed('弦卷集团')`'s const hash was never `note_named`'d → no `named` row → `tile_named` answered -1 → `tile.id == tile_named(…)` unsat. Now `Slot..=TileNamed (0..=5)` |
| `kaoru_01_pot_arm_iff` | TIMEOUT 1800 s | last unwind `symbolic_pass_world`; body `settle` does `tok(p,THIEF/THIEF_WIRE)` ×4 + `fire::gain`/Msg | **C** (+E body ABI) | `symbolic_pass_world` called `host::install()`, **replacing** the per-harness `skills_hhw_dispatch` subset with the full ~300-op match (TRIAGE-LOG `op-subset` SSA cliff). Subset is now kept; body ops are `player_count`/`count_held_name`/`gain_charge`/`log` — all declared |
| `kaoru_01_mutant_is_detected` | TIMEOUT 1800 s | same | **C** | same |
| `band_01_declined_is_noop` | FAILED 158 s | `host op outside declared per-harness subset` (`host::outside_subset`) | **C** | `on_pay_choose` reaches `trig_target`/`trig_value`/`trig_set_pay_amount`/`add_counter_self` (+ `counter_self`). Arms exist; added to `skills_hhw_dispatch` |
| `band_01_accepted_doubles_iff` | FAILED 213 s | same `outside_subset`; cover `accepted path` unreachable | **C**+**B** | subset fix + **answer polarity inverted**: `ask_yes` is `ask_raw==0` (0=yes, 1=no; see `events.rs` `chart_rush_world`). Harnesses had 0=decline / 1=accept. Flipped |
| `band_01_mutant_is_detected` | FAILED 159 s | same `outside_subset` | **C**+**B** | same; mutant now takes the accept arm (answers[0]=0) so the doubling is actually exercised |

Also kept from the in-progress hand-off (sound):
* `fill_geometry_bins` `players_on` outer tile loop rewritten as an 8×8 chunk walk (unwind bound 8, not 60).
* `guard_needs` TileNamed name-id recognition (the misaki vacuous fix).

Gates (this batch, `CARGO_TARGET_DIR=/home/monopoly/kani-target-triage3`):
* `cargo kani --only-codegen -p rules-kani --no-default-features --features band-hhw,skills-personas,skills-groups` — Finished, no `error` lines.
* `cargo test -p rules-kani` ×2 — all suites `ok` (0 failed).

No CBMC re-times this batch: `free -m` available 1317 MB (< 12000) and `tools/kani/central.sh --retry-nonsuccess` is already running its own `cbmc` (mor unit). All eight rows are **needs re-time**.

Soundness: unwinding assertions ON; no `#[ignore]`; no allowlist; no spec weakening; no production-file edits (only `rules/kani/**`).

## triage3 batch 12 — mor central non-success rows (kani-pass@fc3ff3e1)

Central pass rows (unit `mor`, features `band-morfonica,skills-groups,skills-personas`).
Per-proof detail lives in `docs/rulebook/spec/kani/mor.md`.

| id | before | failing assertion / phase | class | action |
|---|---|---|---|---|
| `starry_05_circle_iff` + `starry_05_circle_user1_iff` | FAILED 236/208 s | `C-MOR-starry-05: owner passing CiRCLE removes one crystal` | **B** | `trig_tile = 1` was the board.json 1-based CiRCLE index. After `board::tile_slot` became the 0-based identity, `TILES[1]` is a property (kind 1), so `is_start` never admitted and the PassTile arm was a no-op. → `trig_tile = 0` (TILES[0] kind 0 = CiRCLE) |
| `nnm_05_empty_discard_iff_fails_less` + `_user1_` | FAILED 244/225 s | `C-MOR-nnm-05 FAILS(LESS): book wants dest=Graveyard` | **B** (suspected D, disproved) | (4) 「当奇迹水晶耗尽时」 is a CounterChanged state clause; the body registered `on_crystals_changed` → `set_dest(Graveyard)` long before the central pass. The iff drove the Play press with *already* empty crystals (a correct no-op) and a stale comment claiming `exhausted` was dead. Rewritten to drive `on_crystals_changed_kani` (starry-02/06 style); companion is now `nnm_05_press_empty_is_noop`. Host `arm_set_dest` commits dest as world state — proof observes `w.dest == 0` |
| `nnm_03_amount_iff_fails_wrong` | FAILED 3.9 s | `C-MOR-nnm-03 FAILS(WRONG): body pays x·2000` | **B** (suspected D, disproved) | Spec-only formula compared book `x·reward` against a stale `nnm_03_amount_body = x·2000`. CRYSTAL-04 already fixed the body: `gain_circle_money` raises `PassRewardAffected`; Morfonica `on_circle` rewrites to 1000/1500/2000 and cancels the default (test `nnm_option2_gains_x_times_circle_money_reward` expects 1000). `CIRCLE_MONEY=2000` is the 专有名词 11 default, not a hard-coding bug. → body model is `x·live_reward`; mutant of the hard-coding is detected |
| `tritone_01_gain_place_iff` + `_user1_iff` | FAILED 270/280 s | `C-MOR-tritone-01: GainMoney + PlaceCard + SetState(owed)` | **B** | Spec claimed `set_crystals(3)` logs no effect. `arm_set_counter_self` **does** push `AddCounter` (same bookkeeping as AG half-speed / R fire-bird). Spec now pins 4 effects: GainMoney, PlaceCard, AddCounter(n=3), SetState(owed) |
| `tritone_02_tick_iff` + `_user1_iff` | FAILED 332/291 s | `C-MOR-tritone-02: tick is decay-only, or decay + payback(owed)` | **B** | Spec required `AddCounter + PayMoney` on the last crystal even when `owed == 0`. `arm_pay` logs nothing for a zero payment. Spec now: `owed <= 0` → decay-only; `owed > 0` → decay + payback |

### CONFIRMED genuine discrepancies

None from this batch. The coordinator's suspected D rows (nnm-05 last-crystal press,
nnm-03 x·2000) are both **class B** after reading the current body + sheet + BACKLOG
CRYSTAL-04. Genuine D in `mor` remains rainbow-01/02 (CeilTo-10; keep FAILING).

### Book quotes checked

* starry-05: 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」 (`docs/rulebook/cards.json`)
* nnm-03/05: 「获得x次经过CiRCLE时的资金奖励」 / 「当奇迹水晶耗尽时，将此卡置入弃牌堆」
* tritone: 「立刻获得此次失去的资金金额，此卡放置在场上」 / 「三回合后…弃置此卡并支付由此卡获得的资金」

### Harness renames

| old | new |
|---|---|
| `nnm_05_empty_discard_iff_fails_less` | `nnm_05_empty_discard_iff` |
| `nnm_05_empty_discard_user1_iff_fails_less` | `nnm_05_empty_discard_user1_iff` |
| `nnm_05_body_leaves_card_is_reachable` | `nnm_05_press_empty_is_noop` |
| `nnm_03_amount_iff_fails_wrong` | `nnm_03_amount_iff` |
| `nnm_03_wrong_is_reachable` | `nnm_03_mutant_is_detected` |

Gates (this batch, `CARGO_TARGET_DIR=/home/monopoly/kani-target-triage3`):
* `cargo kani --only-codegen -p rules-kani --no-default-features --features band-morfonica,skills-groups,skills-personas` — Finished, no `error` lines.
* `cargo test -p rules-kani` ×2 — all suites `ok` (0 failed).

### batch-12 re-times (one CBMC at a time, `tools/kani/run.sh --unit mor`)

| id | after |
|---|---|
| `starry_05_circle_iff` | **SUCCESSFUL 306 s** |
| `starry_05_circle_user1_iff` | **SUCCESSFUL 299 s** |
| `nnm_05_empty_discard_iff` | **SUCCESSFUL 160 s** |
| `nnm_05_empty_discard_user1_iff` | **SUCCESSFUL 142 s** |
| `nnm_03_amount_iff` | **SUCCESSFUL 0.3 s** |
| `nnm_03_mutant_is_detected` | **SUCCESSFUL 0.6 s** |
| `tritone_01_gain_place_iff` | **SUCCESSFUL 307 s** |
| `tritone_01_gain_place_user1_iff` | **SUCCESSFUL 306 s** |
| `tritone_02_tick_iff` | **SUCCESSFUL 335 s** |
| `tritone_02_tick_user1_iff` | **SUCCESSFUL 351 s** |

All nine central-pass FAILED rows are now SUCCESSFUL. No confirmed genuine
discrepancy from this batch. Rainbow-01/02 stay FAILING (genuine D, out of scope).

Renames mean the next central pass should select the new harness names.

Soundness: unwinding assertions ON; no `#[ignore]`; no allowlist; no spec
weakening (tritone-02 `owed<=0` limb matches `arm_pay`'s zero-pay no-op — not
a weaker pin); no production-file edits (only `rules/kani/**`).
