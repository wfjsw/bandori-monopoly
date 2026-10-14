#!/usr/bin/env bash
# Serial per-harness sweep of bands-b units. One CBMC at a time.
# Appends each verdict to results-all.tsv immediately.
set -u
export CARGO_TARGET_DIR=/home/monopoly/kani-target-bands-b
export CARGO_BUILD_JOBS=3
ROOT=/home/monopoly/kani-bands-b-wt
OUT=/home/monopoly/kani-bands-b-results
MASTER="$OUT/results-all.tsv"
cd "$ROOT"
mkdir -p "$OUT"
[[ -f "$MASTER" ]] || echo -e "unit\tharness\tverdict\ttime_sec\tunwind\tlog" > "$MASTER"

# Discover harnesses once
python3 - "$ROOT/rules/kani/src/proofs" "$OUT/selected-bands-b.tsv" <<'PY'
import re, sys
from pathlib import Path
proof_dir, out = Path(sys.argv[1]), Path(sys.argv[2])
UNITS = {"ras","skills_ras","skills_mygo","mygo","sumimi","skills_sumimi",
         "crychic","skills_crychic","mujica","skills_mujica","mor","skills_mor"}
rows = []
for path in sorted(proof_dir.glob("*.rs")):
    unit = path.stem
    if unit not in UNITS:
        continue
    src = re.sub(r"//.*", "", path.read_text())
    seen = set()
    for m in re.finditer(r"#\[kani::proof\]", src):
        window = src[m.end(): m.end()+400]
        fm = re.search(r"\b(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", window)
        if fm and fm.group(1) not in seen:
            seen.add(fm.group(1))
            rows.append((unit, fm.group(1)))
with out.open("w") as f:
    for u, n in rows:
        f.write(f"{u}\t{n}\n")
print(f"selected {len(rows)} harnesses")
PY

while IFS=$'\t' read -r unit name; do
  # skip already-recorded
  if grep -qP "^${unit}\t${name}\t" "$MASTER" 2>/dev/null; then
    echo "SKIP $unit::$name (already recorded)"
    continue
  fi
  echo "===== RUN $unit::$name $(date -Is) ====="
  bash tools/kani/run.sh --unit "$unit" --harness "$name" --timeout 1800 --out "$OUT" || true
  if [[ -f "$OUT/results.tsv" ]]; then
    # last data row
    row=$(grep -P "^${unit}\t${name}\t" "$OUT/results.tsv" | tail -1)
    if [[ -n "$row" ]]; then
      echo "$row" >> "$MASTER"
      echo "RECORDED $row"
    fi
  fi
  echo "--- master counts ---"
  awk -F'\t' 'NR>1{c[$3]++} END{for (k in c) printf "  %s %d\n", k, c[k]}' "$MASTER"
done < "$OUT/selected-bands-b.tsv"
echo "===== SWEEP COMPLETE $(date -Is) ====="
