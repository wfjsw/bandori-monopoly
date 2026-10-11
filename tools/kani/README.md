# tools/kani — formal-verification runner

Runs the `rules-kani` Kani harnesses (`#[kani::proof]`) with a per-harness
timeout and bounded parallelism. Fail-closed: any `FAILED` / `TIMEOUT` /
`UNMODELED` verdict exits nonzero. No `#[ignore]`, no skip list.

## Usage

```sh
tools/kani/run.sh [--unit pp] [--harness name] [--jobs 4] [--timeout 900] [--out DIR]
```

| flag | default | meaning |
|---|---|---|
| `--unit NAME` | all | proofs module stem (`pp`, `ppp`, `money_shave`, `skills_bands`, …). Repeatable or comma-separated. |
| `--harness NAME` | all | exact harness fn name. Repeatable. |
| `--jobs N` / `JOBS` | `4` | concurrent harnesses (box: 24 cores / 16 GB; CBMC uses several GB each). |
| `KANI_GLOBAL_SLOTS` | `3` | machine-wide cap across ALL run.sh invocations (flock slots in `$KANI_SLOT_DIR`, default `~/.kani-slots`); always run proofs through run.sh so they count against it. |
| `--timeout SEC` / `TIMEOUT` | `900` | wall-clock per harness. |
| `--out DIR` | `target/kani-results` | JSON + markdown + per-harness logs. |
| `--unwind N` | `80` | passed as `--default-unwind`. Unwinding assertions are ON (insufficient unwind = FAILED, never a silent vacuous pass). `--no-unwind-checks` is a loud exploration-only opt-out whose runs report UNSOUND. |
| verdicts | | SUCCESSFUL, FAILED, TIMEOUT, UNMODELED, VACUOUS (a `kani::cover!` was proven unreachable: the proof never reached its interesting path), UNSOUND. Only SUCCESSFUL counts; the runner exits non-zero otherwise. |

## How it runs

From the worktree root:

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target-f   # NOT kani-target
export CARGO_BUILD_JOBS=8
cargo kani -p rules-kani --harness <name> --default-unwind 80
```

See `docs/rulebook/spec/KANI.md` §1 for the toolchain and flag rationale, and
`docs/rulebook/spec/KANI-FOUNDATION.md` for the harness-writing template.

## Output

* `results.json` — machine-readable rows: harness, unit, verdict, time, unwind, log path.
* `results.md` — the same as a markdown table.
* `logs/<unit>__<harness>.log` — full `cargo kani` stdout/stderr.

Verdicts: `SUCCESSFUL` | `FAILED` | `TIMEOUT` | `UNMODELED`.
`UNMODELED` covers a missing harness and a proof that hit
`unmodeled host op` in `rules/kani/src/host.rs`.

## Notes

* Harness names are discovered from `rules/kani/src/proofs/*.rs`
  (`#[kani::proof]` and `clause_proof!` / `clause_mutant!`).
* Parallel jobs share one `CARGO_TARGET_DIR`; cargo may serialize the compile
  phase on the target-dir lock, then CBMC runs. Keep `--jobs` ≤ 4 on this box.
* Pilot harnesses live in `money_shave.rs`, `teleport.rs`, `prompt.rs`
  (treat those stems as units).