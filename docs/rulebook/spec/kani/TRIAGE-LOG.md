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

1. **Per-card body dispatch** (`generated/body_dispatch_fn.rs`,
   `tools/kani/gen_body_dispatch.py`). `entry_idx` under `cfg(kani)` goes
   through `call_body_idx`: one match arm per RULESET index, each
   `inline(always) call_body(&BAND[i], …)` on a `&'static` constant so
   LLVM emits a **direct** call (or at worst a 1-target site).
2. **`Msg::new` skips `intern()`** under `cfg(kani)`. The host's
   `Op::log` / `Op::effect` only bump a narration counter and never read
   the key; `ctx::s()` names the host compares still intern.

No card-body edits. Production (`cfg(not(kani))`) `entry_idx` / `Msg`
paths unchanged. `cargo build -p game-rules -p rules-native -p rules-kani`
green; `cargo test -p rules-kani` green; `cargo test -p rules-cond
--features lower` green; wasm32 workspace check green.

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
