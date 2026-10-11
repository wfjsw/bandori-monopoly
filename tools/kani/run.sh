#!/usr/bin/env bash
# tools/kani/run.sh — run rules-kani harnesses with bounded parallelism.
#
# Fail-closed: any FAILED / TIMEOUT / UNMODELED verdict exits nonzero.
# No #[ignore], no skip list. See docs/rulebook/spec/KANI.md and
# docs/rulebook/spec/KANI-FOUNDATION.md.
#
# Usage:
#   tools/kani/run.sh [--unit pp] [--harness name] [--jobs 4] [--timeout 900] [--out DIR]
#
# Env:
#   JOBS     default 4 (box: 24 cores / 16 GB; CBMC uses several GB/harness)
#   TIMEOUT  default 900 seconds per harness
#   CARGO_TARGET_DIR  forced to /home/monopoly/kani-target-f (NOT kani-target)
#   CARGO_BUILD_JOBS  forced to 8

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# --- fixed build settings (do not change without updating KANI.md §1) ---
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/monopoly/kani-target-f}"
export CARGO_BUILD_JOBS=8
# Keep the required path unless the caller already pointed elsewhere on purpose
# via --out only. The target dir is deliberately NOT /home/monopoly/kani-target
# (owned by another agent).
if [[ "$CARGO_TARGET_DIR" == "/home/monopoly/kani-target" ]]; then
  echo "refusing CARGO_TARGET_DIR=/home/monopoly/kani-target (owned elsewhere); use /home/monopoly/kani-target-f" >&2
  exit 2
fi

UNWIND=80
NO_UNWIND_CHECKS=1
JOBS="${JOBS:-4}"
TIMEOUT="${TIMEOUT:-900}"
OUT_DIR=""
UNIT_FILTER=()
HARNESS_FILTER=()

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --unit)     UNIT_FILTER+=("$2"); shift 2 ;;
    --harness)  HARNESS_FILTER+=("$2"); shift 2 ;;
    --jobs)     JOBS="$2"; shift 2 ;;
    --timeout)  TIMEOUT="$2"; shift 2 ;;
    --out)      OUT_DIR="$2"; shift 2 ;;
    --unwind)   UNWIND="$2"; shift 2 ;;
    -h|--help)  usage 0 ;;
    *)
      echo "unknown arg: $1" >&2
      usage 2
      ;;
  esac
done

# Comma-separated --unit / JOBS values are accepted.
split_csv() {
  local IFS=,
  # shellcheck disable=SC2206
  local parts=($1)
  printf '%s\n' "${parts[@]}"
}

if [[ ${#UNIT_FILTER[@]} -gt 0 ]]; then
  EXPANDED=()
  for u in "${UNIT_FILTER[@]}"; do
    while IFS= read -r line; do
      [[ -n "$line" ]] && EXPANDED+=("$line")
    done < <(split_csv "$u")
  done
  UNIT_FILTER=("${EXPANDED[@]}")
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$ROOT/target/kani-results"
fi
mkdir -p "$OUT_DIR"
LOG_DIR="$OUT_DIR/logs"
mkdir -p "$LOG_DIR"

RESULTS_JSON="$OUT_DIR/results.json"
RESULTS_MD="$OUT_DIR/results.md"

# ---------------------------------------------------------------------------
# Discover harnesses: every `pub fn name` under a #[kani::proof] (directly or
# via clause_proof! / clause_mutant!). Unit = proofs module stem.
# ---------------------------------------------------------------------------
PROOF_DIR="$ROOT/rules/kani/src/proofs"
if [[ ! -d "$PROOF_DIR" ]]; then
  echo "missing $PROOF_DIR" >&2
  exit 2
fi

HARNESS_LIST="$OUT_DIR/harnesses.tsv"   # unit \t harness \t file
: > "$HARNESS_LIST"

discover() {
  python3 - "$PROOF_DIR" "$HARNESS_LIST" <<'PY'
import re, sys
from pathlib import Path

proof_dir = Path(sys.argv[1])
out = Path(sys.argv[2])

# Accept:
#   #[kani::proof]
#   #[kani::proof]
#   #[kani::should_panic]
#   pub fn name()
# and
#   clause_proof! { ... pub fn name() { ... } }
#   clause_mutant! { ... pub fn name() { ... } }
proof_attr = re.compile(r'#\[kani::proof\]')
fn_re = re.compile(r'\b(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(')
macro_re = re.compile(r'\b(clause_proof|clause_mutant)!\s*\{', re.M)

rows = []
for path in sorted(proof_dir.glob('*.rs')):
    if path.name == 'mod.rs':
        continue
    unit = path.stem
    src = path.read_text()
    # strip line comments so commented-out proofs are not discovered
    src_nc = re.sub(r'//.*', '', src)

    names = []
    # 1) #[kani::proof] ... fn name
    for m in proof_attr.finditer(src_nc):
        window = src_nc[m.end(): m.end() + 400]
        fm = fn_re.search(window)
        if fm:
            names.append(fm.group(1))

    # 2) clause_proof! / clause_mutant! blocks
    for m in macro_re.finditer(src_nc):
        window = src_nc[m.end(): m.end() + 800]
        fm = fn_re.search(window)
        if fm:
            names.append(fm.group(1))

    # unique, keep order
    seen = set()
    for n in names:
        if n not in seen:
            seen.add(n)
            rows.append((unit, n, str(path)))

with out.open('w') as f:
    for unit, name, path in rows:
        f.write(f'{unit}\t{name}\t{path}\n')
print(len(rows))
PY
}

N_DISC="$(discover)"
echo "discovered $N_DISC harness(es) under rules/kani/src/proofs"

# Apply filters
SELECTED="$OUT_DIR/selected.tsv"
: > "$SELECTED"
while IFS=$'\t' read -r unit name file; do
  if [[ ${#UNIT_FILTER[@]} -gt 0 ]]; then
    ok=0
    for u in "${UNIT_FILTER[@]}"; do
      [[ "$u" == "$unit" ]] && ok=1
    done
    [[ $ok -eq 1 ]] || continue
  fi
  if [[ ${#HARNESS_FILTER[@]} -gt 0 ]]; then
    ok=0
    for h in "${HARNESS_FILTER[@]}"; do
      [[ "$h" == "$name" ]] && ok=1
    done
    [[ $ok -eq 1 ]] || continue
  fi
  printf '%s\t%s\t%s\n' "$unit" "$name" "$file" >> "$SELECTED"
done < "$HARNESS_LIST"

N_SEL="$(wc -l < "$SELECTED" | tr -d ' ')"
echo "selected $N_SEL harness(es)  jobs=$JOBS  timeout=${TIMEOUT}s  unwind=$UNWIND"
echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo "  out=$OUT_DIR"

if [[ "$N_SEL" -eq 0 ]]; then
  echo "no harnesses matched" >&2
  # Fail-closed: empty selection is a failure.
  exit 1
fi

# ---------------------------------------------------------------------------
# Run one harness
# ---------------------------------------------------------------------------
run_one() {
  local unit="$1" name="$2" log="$3"
  local start end dt verdict exit_code
  start=$(date +%s.%N)

  # cargo kani from the worktree root; exact flags per KANI.md §1
  set +e
  timeout --signal=TERM --kill-after=30 "${TIMEOUT}s" \
    cargo kani -p rules-kani \
      --harness "$name" \
      --default-unwind "$UNWIND" \
      $( [[ "$NO_UNWIND_CHECKS" == 1 ]] && echo --no-unwinding-checks ) \
    >"$log" 2>&1
  exit_code=$?
  set -e

  end=$(date +%s.%N)
  dt=$(python3 -c "print(f'{max(0.0, float('$end')-float('$start')):.3f}')")

  if [[ $exit_code -eq 124 || $exit_code -eq 137 ]]; then
    verdict="TIMEOUT"
  elif grep -q 'unmodeled host op' "$log" 2>/dev/null; then
    verdict="UNMODELED"
  elif grep -qi 'VERIFICATION:- SUCCESSFUL' "$log" 2>/dev/null; then
    verdict="SUCCESSFUL"
  elif grep -qi 'VERIFICATION:- FAILED' "$log" 2>/dev/null; then
    verdict="FAILED"
  elif [[ $exit_code -eq 0 ]]; then
    # Some kani versions only print CHECKS; accept exit 0 as successful
    # only when the log does not claim a failure.
    if grep -qi 'FAILED\|error\[\|panic' "$log" 2>/dev/null; then
      verdict="FAILED"
    else
      verdict="SUCCESSFUL"
    fi
  else
    # Compile error / harness missing / other: fail closed as UNMODELED
    # when the harness cannot be found, else FAILED.
    if grep -qi 'harness.*not found\|no harness\|unknown harness' "$log" 2>/dev/null; then
      verdict="UNMODELED"
    else
      verdict="FAILED"
    fi
  fi

  # Emit one TSV result line on stdout (collected by the driver)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$unit" "$name" "$verdict" "$dt" "$UNWIND" "$log"
}

# ---------------------------------------------------------------------------
# Bounded parallel driver (JOBS concurrent; each harness gets TIMEOUT)
# ---------------------------------------------------------------------------
RESULTS_TSV="$OUT_DIR/results.tsv"
: > "$RESULTS_TSV"
export RESULTS_TSV TIMEOUT UNWIND NO_UNWIND_CHECKS CARGO_TARGET_DIR CARGO_BUILD_JOBS

# Worker pool: read selected.tsv, run at most JOBS at a time.
# Uses a temp dir of running PIDs.
WORKDIR="$OUT_DIR/.work"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

running=0
idx=0
while IFS=$'\t' read -r unit name file; do
  idx=$((idx + 1))
  log="$LOG_DIR/${unit}__${name}.log"
  # throttle
  while [[ $running -ge $JOBS ]]; do
    if wait -n 2>/dev/null; then
      running=$((running - 1))
    else
      # bash without wait -n: poll
      sleep 0.5
      running=$(jobs -rp | wc -l | tr -d ' ')
    fi
  done
  (
    set +e
    line="$(run_one "$unit" "$name" "$log")"
    set -e
    flock "$RESULTS_TSV" bash -c "printf '%s\n' \"$line\" >> \"$RESULTS_TSV\""
  ) &
  running=$((running + 1))
done < "$SELECTED"
wait

N_DONE="$(wc -l < "$RESULTS_TSV" | tr -d ' ')"
if [[ "$N_DONE" -ne "$N_SEL" ]]; then
  echo "internal error: ran $N_DONE of $N_SEL" >&2
  # still write whatever we have; exit will be nonzero
fi

# ---------------------------------------------------------------------------
# Write JSON + markdown
# ---------------------------------------------------------------------------
python3 - "$RESULTS_TSV" "$RESULTS_JSON" "$RESULTS_MD" "$UNWIND" "$JOBS" "$TIMEOUT" "$CARGO_TARGET_DIR" <<'PY'
import json, sys, datetime, collections
from pathlib import Path

tsv, out_json, out_md, unwind, jobs, timeout, target = sys.argv[1:8]
rows = []
for line in Path(tsv).read_text().splitlines():
    if not line.strip():
        continue
    unit, name, verdict, dt, uw, log = line.split('\t')
    rows.append({
        "unit": unit,
        "harness": name,
        "verdict": verdict,
        "time_sec": float(dt),
        "unwind": int(uw),
        "log": log,
    })

rows.sort(key=lambda r: (r["unit"], r["harness"]))
summary = collections.Counter(r["verdict"] for r in rows)
for k in ("SUCCESSFUL", "FAILED", "TIMEOUT", "UNMODELED"):
    summary.setdefault(k, 0)

payload = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "crate": "rules-kani",
    "target_dir": target,
    "unwind": int(unwind),
    "no_unwinding_checks": True,
    "jobs": int(jobs),
    "timeout_sec": int(timeout),
    "summary": dict(summary),
    "fail_closed": True,
    "results": rows,
}
Path(out_json).write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")

lines = []
A = lines.append
A("# Kani run results")
A("")
A(f"* crate: `rules-kani`")
A(f"* target: `{target}`")
A(f"* unwind: `{unwind}` + `--no-unwinding-checks`")
A(f"* jobs: `{jobs}`  timeout: `{timeout}s` per harness")
A(f"* generated: {payload['generated_at']}")
A(f"* fail-closed: any non-SUCCESSFUL is a failure (no ignores)")
A("")
A("## Summary")
A("")
A("| verdict | count |")
A("|---|---:|")
for k in ("SUCCESSFUL", "FAILED", "TIMEOUT", "UNMODELED"):
    A(f"| {k} | {summary[k]} |")
A(f"| **total** | **{len(rows)}** |")
A("")
A("## Harnesses")
A("")
A("| unit | harness | verdict | time (s) | unwind | log |")
A("|---|---|---|---:|---:|---|")
for r in rows:
    log_rel = Path(r["log"]).name
    A(f"| `{r['unit']}` | `{r['harness']}` | **{r['verdict']}** | {r['time_sec']:.1f} | {r['unwind']} | `logs/{log_rel}` |")
A("")
Path(out_md).write_text("\n".join(lines) + "\n")

bad = sum(summary[k] for k in ("FAILED", "TIMEOUT", "UNMODELED"))
print(f"wrote {out_json}")
print(f"wrote {out_md}")
print(f"SUCCESSFUL={summary['SUCCESSFUL']} FAILED={summary['FAILED']} "
      f"TIMEOUT={summary['TIMEOUT']} UNMODELED={summary['UNMODELED']}")
sys.exit(1 if bad else 0)
PY
STATUS=$?

echo ""
echo "results:"
echo "  $RESULTS_JSON"
echo "  $RESULTS_MD"
if [[ $STATUS -ne 0 ]]; then
  echo "FAIL-CLOSED: one or more harnesses FAILED / TIMEOUT / UNMODELED" >&2
fi
exit $STATUS