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
#   KANI_EXTRA_ARGS  optional extra `cargo kani` flags (default empty; exploration only)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# --- fixed build settings (do not change without updating KANI.md §1) ---
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/monopoly/kani-target-f}"
# Honour a caller-provided CARGO_BUILD_JOBS (this tuning pass uses 3 so a
# concurrent central pass still has cores); default remains 8 for full sweeps.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}"
# Keep the required path unless the caller already pointed elsewhere on purpose
# via --out only. The target dir is deliberately NOT /home/monopoly/kani-target
# (owned by another agent).
if [[ "$CARGO_TARGET_DIR" == "/home/monopoly/kani-target" ]]; then
  echo "refusing CARGO_TARGET_DIR=/home/monopoly/kani-target (owned elsewhere); use /home/monopoly/kani-target-f" >&2
  exit 2
fi

# Default global unwind. Deliberately small: the symbolic model's loops are
# bounded by MAX_* constants that this branch tightened; a harness that needs
# more declares `#[kani::unwind(N)]` or gets a row in
# rules/kani/unwind_table.toml (calibrated by tools/kani/calibrate.py).
# --unwind on the CLI forces every selected harness to one value.
DEFAULT_UNWIND=24
UNWIND=""
# Unwinding assertions stay ON by default: with them off (the old default) an
# insufficient --default-unwind silently truncates paths and CBMC still prints
# "VERIFICATION:- SUCCESSFUL" with every assert/cover UNREACHABLE (found by the
# Morfonica unit: unwind 24 over invoke::find's RULESET scan was vacuous).
# --no-unwind-checks is an explicit, loud opt-out for exploration only; such a
# run can never be reported as SUCCESSFUL (it is capped at UNSOUND).
NO_UNWIND_CHECKS=0
JOBS="${JOBS:-4}"
TIMEOUT="${TIMEOUT:-900}"
OUT_DIR=""
UNIT_FILTER=()
HARNESS_FILTER=()

# Per-harness unwind table (harness = N). Table entry beats the source
# attribute so a calibration win is what the runner actually uses.
UNWIND_TABLE="$ROOT/rules/kani/unwind_table.toml"

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
    --no-unwind-checks) NO_UNWIND_CHECKS=1; shift ;;
    --unwind)   UNWIND="$2"; shift 2 ;;
    --no-unwind-checks) NO_UNWIND_CHECKS=1; shift ;;
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

HARNESS_LIST="$OUT_DIR/harnesses.tsv"   # unit \t harness \t file \t unwind_attr
: > "$HARNESS_LIST"

discover() {
  python3 - "$PROOF_DIR" "$HARNESS_LIST" <<'PY'
import re, sys
from pathlib import Path

proof_dir = Path(sys.argv[1])
out = Path(sys.argv[2])

# Accept:
#   #[kani::proof]
#   #[kani::unwind(N)]
#   pub fn name()
# and
#   clause_proof! { #[kani::unwind(N)] pub fn name() { ... } }
#   clause_mutant! { ... pub fn name() { ... } }
proof_attr = re.compile(r'#\[kani::proof\]')
fn_re = re.compile(r'\b(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(')
unwind_re = re.compile(r'#\[kani::unwind\((\d+)\)\]')
macro_re = re.compile(r'\b(clause_proof|clause_mutant)!\s*\{', re.M)

rows = []
for path in sorted(proof_dir.glob('*.rs')):
    if path.name == 'mod.rs':
        continue
    unit = path.stem
    src = path.read_text()
    # strip line comments so commented-out proofs are not discovered
    src_nc = re.sub(r'//.*', '', src)

    names = []  # (name, unwind_or_empty)
    # 1) #[kani::proof] ... fn name  (unwind attr may sit before or after proof)
    for m in proof_attr.finditer(src_nc):
        window = src_nc[m.end(): m.end() + 400]
        fm = fn_re.search(window)
        if fm:
            # look back a little for #[kani::unwind(N)] before the proof,
            # and forward for one sitting between proof and fn.
            pre = src_nc[max(0, m.start() - 120): m.start()]
            um = unwind_re.search(pre) or unwind_re.search(window)
            names.append((fm.group(1), um.group(1) if um else ''))

    # 2) clause_proof! / clause_mutant! blocks
    for m in macro_re.finditer(src_nc):
        window = src_nc[m.end(): m.end() + 800]
        fm = fn_re.search(window)
        if fm:
            um = unwind_re.search(window)
            names.append((fm.group(1), um.group(1) if um else ''))

    # unique, keep order
    seen = set()
    for n, uw in names:
        if n not in seen:
            seen.add(n)
            rows.append((unit, n, str(path), uw))

with out.open('w') as f:
    for unit, name, path, uw in rows:
        f.write(f'{unit}\t{name}\t{path}\t{uw}\n')
print(len(rows))
PY
}

N_DISC="$(discover)"
echo "discovered $N_DISC harness(es) under rules/kani/src/proofs"

# Apply filters. Columns: unit, name, file, unwind_attr (may be empty).
SELECTED="$OUT_DIR/selected.tsv"
: > "$SELECTED"
while IFS=$'\t' read -r unit name file attr; do
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
  printf '%s\t%s\t%s\t%s\n' "$unit" "$name" "$file" "$attr" >> "$SELECTED"
done < "$HARNESS_LIST"

N_SEL="$(wc -l < "$SELECTED" | tr -d ' ')"
echo "selected $N_SEL harness(es)  jobs=$JOBS  timeout=${TIMEOUT}s  unwind=${UNWIND:-auto}"
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
# Machine-wide concurrency cap shared by EVERY run.sh invocation on this box
# (several agents run proofs at once; CBMC peaks at several GB each and the box
# has 16 GB). A slot is an flock on $KANI_SLOT_DIR/slotN held for the harness's
# lifetime; the per-harness timeout starts only after the slot is acquired.
KANI_GLOBAL_SLOTS="${KANI_GLOBAL_SLOTS:-2}"
KANI_SLOT_DIR="${KANI_SLOT_DIR:-$HOME/.kani-slots}"
mkdir -p "$KANI_SLOT_DIR"

acquire_slot() {
  local i fd
  while true; do
    for ((i = 0; i < KANI_GLOBAL_SLOTS; i++)); do
      exec {fd}>"$KANI_SLOT_DIR/slot$i"
      if flock -n "$fd"; then
        SLOT_FD="$fd"
        return 0
      fi
      exec {fd}>&-
    done
    sleep 3
  done
}

# Resolve the unwind bound for one harness.
# Priority: CLI --unwind > unwind_table.toml > #[kani::unwind(N)] attr > DEFAULT.
resolve_unwind() {
  local name="$1" attr="$2"
  if [[ -n "$UNWIND" ]]; then
    echo "$UNWIND"
    return
  fi
  local t
  t="$(python3 - "$UNWIND_TABLE" "$name" <<'PY'
import re, sys
from pathlib import Path
table, name = sys.argv[1], sys.argv[2]
p = Path(table)
if not p.exists():
    print("")
    raise SystemExit(0)
for line in p.read_text().splitlines():
    line = line.strip()
    if not line or line.startswith('#') or line.startswith('['):
        continue
    m = re.match(r'^([A-Za-z0-9_:.]+)\s*=\s*(\d+)\s*$', line)
    if m and m.group(1) == name:
        print(m.group(2))
        raise SystemExit(0)
print("")
PY
)"
  if [[ -n "$t" ]]; then
    echo "$t"
    return
  fi
  if [[ -n "$attr" ]]; then
    echo "$attr"
    return
  fi
  echo "$DEFAULT_UNWIND"
}

# Map unit name -> cargo features of rules-kani (one unit's crates only).
# Shrinks CBMC's address-taken `fn(i32)->Asked` set (874 → tens).
unit_features() {
  case "$1" in
    pp|money_shave|prompt) echo "band-pp,skills-groups" ;;
    ppp|teleport) echo "band-ppp,skills-personas,skills-groups" ;;
    r) echo "band-roselia,skills-personas,skills-groups" ;;
    ag) echo "band-ag,skills-personas,skills-groups" ;;
    hhw) echo "band-hhw,skills-personas,skills-groups" ;;
    mor) echo "band-morfonica" ;;
    ras) echo "band-ras" ;;
    mygo) echo "band-mygo" ;;
    mujica) echo "band-mujica" ;;
    crychic) echo "band-crychic" ;;
    sumimi) echo "band-sumimi,skills-personas" ;;
    events) echo "events" ;;
    tiles|markers) echo "tiles" ;;
    skills_pp) echo "band-pp,skills-personas,skills-groups" ;;
    skills_ppp) echo "band-ppp,skills-personas,skills-groups" ;;
    skills_r) echo "band-roselia,skills-personas,skills-groups" ;;
    skills_ag) echo "band-ag,skills-personas,skills-groups" ;;
    skills_hhw) echo "band-hhw,skills-personas,skills-groups" ;;
    skills_mor) echo "band-morfonica,skills-groups,skills-personas" ;;
    skills_ras) echo "band-ras,skills-personas" ;;
    skills_mygo) echo "band-mygo,skills-groups" ;;
    skills_mujica) echo "band-mujica,skills-groups" ;;
    skills_crychic) echo "band-crychic,skills-groups,skills-personas" ;;
    skills_sumimi) echo "band-sumimi,skills-groups" ;;
    skills_general|general) echo "band-general" ;;
    skills_characters) echo "skills-personas" ;;
    skills_bands) echo "skills-groups" ;;
    *) echo "all" ;;
  esac
}

run_one() {
  local unit="$1" name="$2" log="$3" attr="$4"
  local start end dt verdict exit_code
  local unwind
  unwind="$(resolve_unwind "$name" "$attr")"
  acquire_slot
  start=$(date +%s.%N)

  # cargo kani from the worktree root; exact flags per KANI.md §1.
  # Pass --default-unwind matching the resolved bound so the CLI value and any
  # #[kani::unwind] attribute cannot disagree (attribute wins in Kani; we
  # keep them equal). Unwinding assertions stay ON unless --no-unwind-checks.
  # `-Z stubbing` enables `#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]`
  # (library-cost stubs; semantics-preserving, see rules/kani/src/kani_stubs.rs).
  # KANI_EXTRA_ARGS is an optional exploration hook (default off/empty).
  set +e
  timeout --signal=TERM --kill-after=30 "${TIMEOUT}s" \
    cargo kani -p rules-kani \
      -Z stubbing \
      --no-default-features \
      --features "$(unit_features "$unit")" \
      --harness "$name" \
      --default-unwind "$unwind" \
      $( [[ "$NO_UNWIND_CHECKS" == 1 ]] && echo --no-unwinding-checks ) \
      ${KANI_EXTRA_ARGS:-} \
    >"$log" 2>&1
  exit_code=$?
  set -e

  end=$(date +%s.%N)
  dt=$(python3 -c "print(f'{max(0.0, float('$end')-float('$start')):.3f}')")

  if [[ $exit_code -eq 124 || $exit_code -eq 137 ]]; then
    verdict="TIMEOUT"
  elif grep -qi 'VERIFICATION:- SUCCESSFUL' "$log" 2>/dev/null; then
    # A CBMC verdict line always wins: the host's `unmodeled host op` assert
    # string is compiled into every rules-kani binary and shows up in the
    # check list of every run, so it is NOT evidence of an unmodeled op.
    verdict="SUCCESSFUL"
    # Vacuity gate: a cover that CBMC proved unreachable means the proof's
    # interesting path never ran (truncated unwinding, contradictory assumes,
    # or a dead branch). Fail closed.
    if grep -qE 'cover properties satisfied \([0-9]+ unreachable\)' "$log" 2>/dev/null \
       || grep -qE '\*\* 0 of [1-9][0-9]* cover properties satisfied' "$log" 2>/dev/null; then
      verdict="VACUOUS"
    elif [[ "$NO_UNWIND_CHECKS" == 1 ]]; then
      verdict="UNSOUND"
    fi
  elif grep -qi 'VERIFICATION:- FAILED' "$log" 2>/dev/null; then
    if grep -qi 'unwinding assertion' "$log" 2>/dev/null; then
      # Loud: the bound is too small. This is the calibration signal, not a
      # spec failure. Surface it distinctly so calibrate.py can react.
      verdict="UNWINDING"
    elif grep -qE 'Failed Checks:.*unmodeled host op' "$log" 2>/dev/null; then
      verdict="UNMODELED"
    else
      verdict="FAILED"
    fi
  elif grep -qi 'unwinding assertion' "$log" 2>/dev/null; then
    verdict="UNWINDING"
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
    "$unit" "$name" "$verdict" "$dt" "$unwind" "$log"
}

# ---------------------------------------------------------------------------
# Bounded parallel driver (JOBS concurrent; each harness gets TIMEOUT)
# ---------------------------------------------------------------------------
RESULTS_TSV="$OUT_DIR/results.tsv"
: > "$RESULTS_TSV"
export RESULTS_TSV TIMEOUT UNWIND NO_UNWIND_CHECKS CARGO_TARGET_DIR CARGO_BUILD_JOBS
export DEFAULT_UNWIND UNWIND_TABLE

# Worker pool: read selected.tsv, run at most JOBS at a time.
# Uses a temp dir of running PIDs.
WORKDIR="$OUT_DIR/.work"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

running=0
idx=0
while IFS=$'\t' read -r unit name file attr; do
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
    line="$(run_one "$unit" "$name" "$log" "$attr")"
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
python3 - "$RESULTS_TSV" "$RESULTS_JSON" "$RESULTS_MD" "${UNWIND:-auto}" "$JOBS" "$TIMEOUT" "$CARGO_TARGET_DIR" <<'PY'
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
for k in ("SUCCESSFUL", "FAILED", "TIMEOUT", "UNMODELED", "VACUOUS", "UNSOUND", "UNWINDING"):
    summary.setdefault(k, 0)

payload = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "crate": "rules-kani",
    "target_dir": target,
    "unwind": unwind,
    "no_unwinding_checks": False,
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
A(f"* unwind: `{unwind}` (unwinding assertions on unless --no-unwind-checks; covers must be reachable)")
A(f"* jobs: `{jobs}`  timeout: `{timeout}s` per harness")
A(f"* generated: {payload['generated_at']}")
A(f"* fail-closed: any non-SUCCESSFUL is a failure (no ignores)")
A("")
A("## Summary")
A("")
A("| verdict | count |")
A("|---|---:|")
for k in ("SUCCESSFUL", "FAILED", "TIMEOUT", "UNMODELED", "VACUOUS", "UNSOUND", "UNWINDING"):
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

bad = sum(summary[k] for k in ("FAILED", "TIMEOUT", "UNMODELED", "VACUOUS", "UNSOUND", "UNWINDING"))
print(f"wrote {out_json}")
print(f"wrote {out_md}")
print(f"SUCCESSFUL={summary['SUCCESSFUL']} FAILED={summary['FAILED']} "
      f"TIMEOUT={summary['TIMEOUT']} UNMODELED={summary['UNMODELED']} VACUOUS={summary['VACUOUS']} UNSOUND={summary['UNSOUND']} UNWINDING={summary['UNWINDING']}")
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