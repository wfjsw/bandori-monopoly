#!/usr/bin/env bash
# Serial sweep of bands-b units. One CBMC at a time (jobs=1).
# Accumulates per-unit results into results-all.tsv.
set -u
export CARGO_TARGET_DIR=/home/monopoly/kani-target-bands-b
export CARGO_BUILD_JOBS=3
ROOT=/home/monopoly/kani-bands-b-wt
OUT=/home/monopoly/kani-bands-b-results
MASTER="$OUT/results-all.tsv"
cd "$ROOT"
mkdir -p "$OUT"
[[ -f "$MASTER" ]] || echo -e "unit\tharness\tverdict\ttime_sec\tunwind\tlog" > "$MASTER"
for unit in ras skills_ras skills_mygo mygo sumimi skills_sumimi crychic skills_crychic mujica skills_mujica mor skills_mor; do
  echo "===== SWEEP $unit $(date -Is) ====="
  bash tools/kani/run.sh --unit "$unit" --jobs 1 --timeout 1800 --out "$OUT" || true
  # run.sh overwrites results.tsv; append to master
  if [[ -f "$OUT/results.tsv" ]]; then
    grep -v '^unit' "$OUT/results.tsv" >> "$MASTER" || true
  fi
  echo "===== DONE $unit $(date -Is) ====="
  echo "--- master so far ---"
  awk -F'\t' 'NR>1{c[$3]++} END{for (k in c) print k, c[k]}' "$MASTER"
done
echo "===== SWEEP COMPLETE $(date -Is) ====="
