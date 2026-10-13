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
| general::fever_04_mutant_is_detected | SUCCESSFUL 417 s | — | re-verify after model cut | running |
| general::perfect_01_02_03_iff | SUCCESSFUL 416 s (old base; brief said TIMEOUT) | — | re-verify on current master | pending |
| hostops_smoke::rent_of_property_table | SUCCESSFUL 185 s (old base; string facade) | — | re-verify + record | pending |
| teleport::tae_sound_teleport_and_fire_iff | FAILED in `teleport/`, SUCCESSFUL 503 s in `verify-fix1/` after fix | — | re-verify fix on current master | pending |
| teleport::tae_sound_mutant_extra_fire | SUCCESSFUL 526 s (`verify-fix1/`) | — | re-verify | pending |
| cond_ops::ir_div_matches_spec | TIMEOUT 1800 s | E (intractable two-divider circuit equivalence) | reformulate (error class + Euclidean characterisation + mutants) | pending |
| cond_ops::ir_rem_matches_spec | TIMEOUT 1800 s | E | same | pending |


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
