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
| general::fever_04_turn_start_iff | UNWINDING 314 s (old base; bound at `guard_needs` 32-loop) | A (bound too low; model loop) | 8×8 chunk `guard_needs` + 8×8 OptInt mask walk in `fill_for_guard` | pending |
| general::fever_04_wrong_turn_noop | UNWINDING 266 s | A | same | pending |
| general::fever_04_not_placed_guard_rejects | UNWINDING 103 s | A | same | pending |
| general::fever_04_mutant_is_detected | SUCCESSFUL 417 s | — | re-verify after model cut | pending |
| general::perfect_01_02_03_iff | SUCCESSFUL 416 s (old base; brief said TIMEOUT) | — | re-verify on current master | pending |
| hostops_smoke::rent_of_property_table | SUCCESSFUL 185 s (old base; string facade) | — | re-verify + record | pending |
| teleport::tae_sound_teleport_and_fire_iff | FAILED in `teleport/`, SUCCESSFUL 503 s in `verify-fix1/` after fix | — | re-verify fix on current master | pending |
| teleport::tae_sound_mutant_extra_fire | SUCCESSFUL 526 s (`verify-fix1/`) | — | re-verify | pending |
| cond_ops::ir_div_matches_spec | TIMEOUT 1800 s | E (intractable two-divider circuit equivalence) | reformulate (error class + Euclidean characterisation + mutants) | pending |
| cond_ops::ir_rem_matches_spec | TIMEOUT 1800 s | E | same | pending |

