# Kani coverage dashboard

Computed 2026-10-12 from the per-unit tables in `docs/rulebook/spec/kani/*.md`
and the central serial pass under `/home/monopoly/kani-central-results2/`
(partial and still growing). This is a **status snapshot**, not a proof
ledger: a "written, verdict pending" row is not a proof.

## Legend

### Clause classification (unit tables)

| tag | meaning |
|---|---|
| **PROVEN** | iff body proof + non-vacuous mutant, both SUCCESSFUL under the sound runner |
| **PROVEN*** / pending | harness (or pure contract) written; CBMC verdict not landed yet |
| **FAILS** | plain failing proof — body ≠ book; finding tagged **LESS** / **MORE** / **WRONG** |
| **PROTOCOL-LEVEL** | not expressible as a straight-line body proof (multi-turn, [反击] window, settle pipeline, move-plan scheduling). **Not covered by Kani** |
| **BLOCKED-HOST** | host-model gap or protocol stub blocks the proof (see `KANI-COVERAGE.md`, `HOSTOPS-ROUND2.md`) |
| **OPEN** | book text undefined / ambiguous; only what the text pins is contracted |
| **TIMEOUT/OOM** | harness written; CBMC hit the time / memory cliff |

### Harness verdicts (central runner / unit run tables)

| verdict | meaning |
|---|---|
| **SUCCESSFUL** | all checks passed, covers reachable, unwinding assertions held (sound runner) |
| **FAILED** | a property check disagreed (a finding, or a stale spec) |
| **TIMEOUT** | hit the per-harness timeout (default 1800 s) |
| **UNWINDING** | unwinding assertion failed (bound too low) — fail-closed, **not** a proof |
| **VACUOUS** | some `cover!` unreachable — fail-closed, **not** a proof |
| **not-run / pending** | harness written, no verdict row yet |

Older unit-doc lines that predate the sound runner (unwinding checks off, or
covers not gated) are **unconfirmed** — see Soundness caveats.

## Per-unit clause classification

Counts as printed in each unit table (they use slightly different PROVEN
breakdowns; the notes column keeps the unit's own split).

| unit | PROVEN (verdict in) | PROVEN pending / written | FAILS | PROTOCOL-LEVEL | BLOCKED-HOST | OPEN | TIMEOUT/OOM | clause rows (approx) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `ag` | 2 | 24 | 0 | 14 | 12 | 11 | 0 | 60 |
| `crychic` | 6 (leaf iff) | 15 written / pending | 2 | 18 | 7 | 7 | 0 | — |
| `events` | ~30 (partial) | (included left) | 1 | ~18 | ~10 | ~12 | 0 (slot-starved) | 55 harnesses |
| `general` | 1 body + ~14 formula/mutant | 6 body | 0 | 12 | 3 | 8 refs | 0 | — |
| `hhw` | 5 | ~25 | 2 (+2 CANDIDATE) | ~20 | 3+1 | ~15 | 1 (`athletic_place_three`) | — |
| `mor` | 1 non-vacuous | 21 | 2 (+1 CANDIDATE `rainbow_01`) | 16 | 12 | 20 refs | 0 | — |
| `mujica` | 18 | 0 landed this pass | 3 | 29 | 3 | 8 | 0 | — |
| `mygo` | 3 | 8 | 3 summary (+4 skill black-box) | 18 | 8 | 6 (+refs) | 0 | ~40 |
| `pp` | 2 pilot + 1 formula | 29 | 2 (+1 body hint) | 10 | 6 | 24 | 0 | 74 |
| `ppp` | 1 pilot + 3 | ~30 | 2 (+1 `returns-c4`) | 22 | 14 | 8–11 | 1→0 (later SUCCESSFUL) | — |
| `r` | 2 | 16 | 0 | 34 | 6 | 6 | 0 | — |
| `ras` | 1 | 4 leaf | 0 (+1 CANDIDATE `chuchu-skill-02`) | 35 | 11 | 9 | 2 | 62 |
| `sumimi` | 3 formula | 12 | 2 | 14 | 5 | 6–8 | 0 | ~35 |
| `tiles` + `tile_marks` | 8–14 | 2 observable-half | 0 (2 expected once unblocked) | 6 | 8 | 5 refs | 0 | — |

**Totals (approximate, unit tables as printed):** FAILS ≈ **23** clause rows
(+ several CANDIDATE / expected-once-unblocked); PROTOCOL-LEVEL ≈ **246**;
BLOCKED-HOST ≈ **103**; OPEN ≈ **130+** refs. PROVEN-with-verdict is still a
minority of written harnesses — the serial central pass is the bottleneck
(machine-wide CBMC slot cap).

## Per-unit harness verdicts

### Unit-doc run tables (as recorded in `kani/*.md`)

| unit | SUCCESSFUL | FAILED | TIMEOUT | UNWINDING | VACUOUS | pending / not-run | notes |
|---|---:|---:|---:|---:|---:|---:|---|
| `ag` | 2 | 0 | 0 | 0 | 0 | 24+ | `half_speed_01_place_iff`, `colors_01_wrap_formula` |
| `crychic` | 1 | 0 | 0 | 0 | 0 | 21 | `soyoreturn_02_hand_iff`; `ifonly_02_*` expected FAILS not run |
| `events` | (unit doc TBD) | — | — | — | — | most of 55 | slot-starved; see central row below |
| `general` | ~15 | 0 | 0 | 0 | 0 | ~10 | formulas + recruit/great mutants; body iff pending then landed centrally |
| `hhw` | 7 | 0* | 1 | 0 | 0 | ~25 | *`athletic_place_three_crystals_iff` intern cliff (TIMEOUT class) |
| `mor` | 4 | 1† | 0 | 0 | 2‡ | ~50 | †`rainbow_01_half_iff` FAILED on spec CeilTo gap (CANDIDATE); ‡unwind-24 vacuous |
| `mujica` | 0 | 0 | 0 | 0 | 0 | 37 | all pending (RAM / slot flood) |
| `mygo` | 3 | 0 | 0 | 0 | 0 | ~13 | `rinpuu_01_*`, `journey_01_mutant` |
| `pp` | 1 (+pilot) | 0 | 0 | 0 | 0 | 29+ | `dream_ahead_03_formula`; pilot `money_shave`/`prompt` |
| `ppp` | ~24 | 0 | 1→0 | 0 | 0 | ~30 | many ~1 s pure specs; `tae_sound` pilot in `teleport.rs` |
| `r` | 2 | 0 | 0 | 0 | 0 | ~22 | `resolve_play_iff` + mutant |
| `ras` | 3 | 0 | 1 | 0 | 0 | 6 | `riot_02_03_cycle_iff` TIMEOUT (Vec drop) |
| `sumimi` | 3 | 0 | 0 | 0 | 0 | 12 | formula trio; `idolband_reset_iff` verdict disputed |
| `tiles` | 5 | 0 | 0 | 0 | 0 | 15 | later landed centrally |

### Central serial pass (`/home/monopoly/kani-central-results2/`)

Partial, still growing (`events` mid-run). Counts from `results.json` /
`results.tsv` + `tools/kani/central_summary.py`.

| unit | SUCCESSFUL | FAILED | TIMEOUT | UNWINDING | VACUOUS | total | non-success names |
|---|---:|---:|---:|---:|---:|---:|---|
| `cond_ops` | 12 | 0 | 2 | 0 | 0 | 14 | TIMEOUT `ir_div_matches_spec`, `ir_rem_matches_spec` |
| `events` | 5 | 3 | 10 | 4 | 0 | 22 | FAILED `aao_02_hand_veto_is_armed` (confirms LESS), `pico_swap_01_rotates_positions`, `pico_swap_02_skips_exiled_players` (CANDIDATE vs unit PROVEN); UNWINDING `tears_01/02`, `tears_mutant`, `moca_01_activation`, `aao_01_activation`; 10 TIMEOUT |
| `general` | 22 | 0 | 0 | 3 | 0 | 25 | UNWINDING `fever_04_not_placed_guard_rejects`, `fever_04_turn_start_iff`, `fever_04_wrong_turn_noop` |
| `hostops_smoke` | 7 | 0 | 0 | 0 | 0 | 7 | — |
| `money_shave` | 0 | 0 | 3 | 0 | 0 | 3 | TIMEOUT `aya_haruka_02_shave_iff`, `aya_haruka_02_mutant_x_off_by_one`, `aya_haruka_02_mutant_is_detected` |
| `teleport` | 1 | 1 | 0 | 0 | 0 | 2 | FAILED `tae_sound_teleport_and_fire_iff` |
| `verify-fix1` | 2 | 0 | 0 | 0 | 0 | 2 | supersedes `teleport/`: `tae_sound_teleport_and_fire_iff` SUCCESSFUL + mutant |
| `tile_marks` | 6 | 0 | 0 | 0 | 0 | 6 | — |
| `tiles` | 14 | 0 | 0 | 0 | 0 | 14 | — |
| `wrong_guard` | 5 | 0 | 0 | 0 | 0 | 5 | — |

**Central totals (snapshot):** SUCCESSFUL 76 · FAILED 4 · TIMEOUT 15 ·
UNWINDING 7 · VACUOUS 0 · total 100 rows.

Notable central FAILED rows:

* `aao_02_hand_veto_is_armed` — confirms the E-aao-02 LESS finding.
* `pico_swap_01_rotates_positions` / `pico_swap_02_skips_exiled_players` —
  unit table listed these as covered; treat as **CANDIDATE** unit-vs-central
  disagreement until re-read against the sound runner.
* `tae_sound_teleport_and_fire_iff` (`teleport/`) — superseded by
  `verify-fix1/` SUCCESSFUL after the fix; do not count as a live finding.

## Soundness caveats

1. **`player_count()` is the constant 4 under Kani** (`kani/TUNING.md` §7).
   Loops `for p in 0..ctx::player_count()` stay exact for any `n_players`
   (seats `>= N` report `player_out`), but bodies that read the **value** of
   `player_count()` see 4 even in a 2- or 3-player world. The seven named
   bodies are:
   * `card-ras/chuchu_music.rs:139`
   * `card-crychic/sakiko_lead.rs:41`
   * `card-hhw/kokoro_circle.rs:36`
   * `card-general/clear_cp.rs:89`
   * `card-mujica/umiri_card.rs:185`
   * `card-morfonica/tsukushi_garden.rs:31`
   * `events/hallucination.rs:46`

   A proof about one of those holds **for 4-player worlds only**. Harnesses
   must `kani::assume(w.n_players == 4)` and the unit row must say "n=4 only".
   Do not report such a clause as fully PROVEN for 2..=4 players.

2. **Unit results recorded before the sound runner are unconfirmed.**
   Older `--no-unwinding-checks` runs can print `VERIFICATION:- SUCCESSFUL`
   while asserts/covers below the unwind cut are unreachable (vacuous). The
   sound runner keeps unwinding assertions on (too-low bound ⇒ loud
   **UNWINDING**) and treats unsatisfied `cover!`s as **VACUOUS**. Pre-sound
   lines in the unit tables (notably the money-shave pilot "85 s SUCCESS",
   mor unwind-24 covers, early `tae_sound`) must not be counted as proofs
   until re-run.

3. **PROTOCOL-LEVEL clauses are NOT covered by Kani.** Multi-turn chains,
   [反击] windows, settle pipelines, move-plan scheduling, bankruptcy,
   mortgage, opening deal, and engine play-gate exclusives need a protocol
   model (TLA+/Alloy/SMT over `docs/ENGINE.md`) with Kani only as leaf
   refinement (`KANI.md` §6, `KANI-FOUNDATION.md`). Protocol stubs that
   return 0 never hit `unmodeled`, but a clause that needs their real
   semantics is still PROTOCOL-LEVEL — do not fake it with a silent zero.

4. **Pending ≠ proven.** The shared box has a machine-wide CBMC slot cap
   (about 3); many harnesses are "written, verdict pending" and the central
   pass is still growing under `/home/monopoly/kani-central-results2/`.

5. **HOSTOPS-ROUND2 unblocked many rows.** Unit tables may still list
   `BLOCKED-HOST` for ops that `KANI-COVERAGE.md` (2026-10-11) reports as
   modeled. Protocol stubs remain for engine-scheduling surfaces.

## How to refresh

From a rules/kani worktree (not while the serial proof pass is saturating the
box — coordinate first):

```bash
# 1. Serial central pass (resumable, tiered, jobs=1). Machine-wide slot cap.
#    Writes $OUT/<unit>/{results.json,results.md,results.tsv,logs/}.
bash tools/kani/central.sh --out /home/monopoly/kani-central-results2 \
    --timeout 1800 <unit>...

# 2. Summarise / select rows. Newest row across base, retry-N, unwind-N wins.
python3 tools/kani/central_summary.py --out /home/monopoly/kani-central-results2
python3 tools/kani/central_summary.py --out /home/monopoly/kani-central-results2 \
    --unit <unit> --list-nonsuccess

# 3. Re-run only non-SUCCESSFUL rows at the current timeout.
bash tools/kani/central.sh --out /home/monopoly/kani-central-results2 \
    --retry-nonsuccess <unit>

# 4. Single harness (spot check; sound defaults — keep unwinding checks on).
#    Do NOT set CARGO_TARGET_DIR to a shared stale dir for ruleset builds;
#    kani builds use their own CARGO_TARGET_DIR per unit.
export CARGO_TARGET_DIR=/home/monopoly/kani-target-<unit>
bash tools/kani/run.sh --unit <unit> --harness <name> --jobs 1 --timeout 1800
```

Then re-derive this file's per-unit tables from
`docs/rulebook/spec/kani/*.md` (classification) plus
`central_summary.py` (verdicts), and bump the date.

## See also

* `KANI.md` — toolchain, backend design, pilot clauses
* `KANI-FOUNDATION.md` — host model, PROTOCOL-LEVEL taxonomy
* `KANI-COVERAGE.md` — host-op coverage (284/284 modeled as of 2026-10-11)
* `kani/TUNING.md` — sound-proof tuning + `player_count` caveat
* `kani/HOSTOPS-ROUND2.md` — host ops fixed in the 2026-10-11 pass
* `FINDINGS.md` — consolidated book-vs-implementation discrepancies
* `OPEN-QUESTIONS.md` — consolidated OPEN-* items