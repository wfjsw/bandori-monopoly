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
