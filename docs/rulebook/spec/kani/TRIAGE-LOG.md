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
| money_shave::aya_haruka_02_shave_not_poorest_noop | (new split) | E | documented state cut + clause-term assume | TIMEOUT/OOM-bound at 19 min / 3.8 GB (killed to free the slot); profile: `Vec::extend_with` 31, `memcmp` 10, `by_idx` 10 — still solver-bound |
| money_shave::aya_haruka_02_* | TIMEOUT 1800 s | E | fans cut + split | not re-run to completion; remaining cliff is `Msg`/`Vec<u8>` drop+extend in the card body's `ctx::log` path |
| events::heat_head_01_activation_phase_up | FAILED | B + C | harness read `key_get(0,"crystals")` but `set_crystals` writes the field-instance counter; also `World::self_crystals` fallback for event instances with no field slot | **SUCCESSFUL 423 s** |
| events::heat_head_03_phase_end_files_away | FAILED | B + C | same | **SUCCESSFUL 692 s** |
| events::circle_rebuild_01_activation_five_crystals | FAILED | B + C | same crystals read + host fallback | **SUCCESSFUL** (see ev-circle) |
| events::asukayama_02_status_spread_blocked_neighbor | FAILED | B | harness assumed stub `neighbor` returned tiles; host now returns the next present **player seat** (book 「上一位/下一位玩家」). Rewrote to prev=3 stunStart=2, next=1 noHand=2, seat 2 give_exile(1,-1) | pending re-run |
| events::pico_swap_01 | FAILED | B + F | `log_is_teleports` exact-length vs latch `SetState`; cover `n_log==3` vacuous for the same reason. Filter `SetState`; cover counts Teleports | **SUCCESSFUL 1767 s** (covers sat) |
| events::aao_02_hand_veto_is_armed | FAILED | **D GENUINE E-aao-02** | keep failing; do not fix | FAILED (unchanged) |
| events::embers_03_settle_costs_ten_per_copy | FAILED | OPEN / D? | [触发结算] undefined (OPEN-触发结算); not re-run | pending |
| events::* TIMEOUT (chart_rush, front_or_back, never_ends, …) | TIMEOUT | E / A | `Vec` drop_glue, symbolic-Vec `for &p in &winners`, fmt recursion; bear_cookie unwind 16→32 | pending |

### Host-model defect fixed (class C)

`World::set_field_crystals` was a silent no-op when `last_uid` had no `field[]`
slot. Events (`heat_head` phase, `circle_rebuild`'s 5 miracle crystals) call
`ctx::set_crystals` on the running instance. Added `World::self_crystals` as
the fallback for that case; `conf_hostops2::self_crystals_roundtrip_without_field_slot`
pins it. `conf_fill_diff` / `conf_model` / `conf_hostops2` / `pilot_guards` /
`model_bounds` stay green (3×).
