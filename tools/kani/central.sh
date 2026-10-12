#!/usr/bin/env bash
# Central serial verification pass. One harness at a time (--jobs 1) through
# run.sh (machine-wide slot cap applies). Resumable and tiered:
#   tier 1  every harness of the unit at unwind $UNWIND, timeout $TIMEOUT
#   tier 2  harnesses that FAILED *only* on "unwinding assertion" checks are
#           re-run at unwind $UNWIND2 (an unwinding failure is a bound that is
#           too low, never a spec violation)
#   tier 3  (optional, --retry-nonsuccess) re-run every non-SUCCESSFUL row of an
#           already-finished unit at the current timeout, in $OUT/<unit>/retry-N
# Verdict per harness = newest row across $OUT/<unit>/{.,retry-*,unwind-*}
# (tools/kani/central_summary.py prints the table).
#   tools/kani/central.sh [--out DIR] [--timeout SEC] [--unwind N] [--unwind2 N]
#                         [--retry-nonsuccess] [--skip H1,H2] [unit ...]
set -u
OUT=${KANI_CENTRAL_OUT:-$PWD/target/kani-central}
TIMEOUT=1800; UNWIND=""; UNWIND2=256; RETRY=0; SKIP=""
while [[ $# -gt 0 ]]; do case "$1" in
  --out) OUT="$2"; shift 2;; --timeout) TIMEOUT="$2"; shift 2;;
  --unwind) UNWIND="$2"; shift 2;; --unwind2) UNWIND2="$2"; shift 2;;
  --retry-nonsuccess) RETRY=1; shift;; --skip) SKIP="$2"; shift 2;; *) break;; esac; done
UNITS=("$@")
[[ ${#UNITS[@]} -eq 0 ]] && mapfile -t UNITS < <(ls rules/kani/src/proofs | sed 's/\.rs$//' | grep -v '^mod$')
mkdir -p "$OUT"
H=$(dirname "$0")

run_set() { # unit dir unwind harness...
  local u="$1" d="$2" uw="$3"; shift 3
  local uwf=(); [[ -n "$uw" ]] && uwf=(--unwind "$uw")
  local hs=(); for h in "$@"; do hs+=(--harness "$h"); done
  bash tools/kani/run.sh --unit "$u" "${hs[@]}" --jobs 1 --timeout "$TIMEOUT" "${uwf[@]}" --out "$d" \
    || echo "unit $u: non-success rows (see $d/results.md)"
}

for u in "${UNITS[@]}"; do
  if [[ ! -f "$OUT/$u/results.json" ]]; then
    echo "=== $u tier1 $(date +%T)"
    bash tools/kani/run.sh --unit "$u" --jobs 1 --timeout "$TIMEOUT" ${UNWIND:+--unwind "$UNWIND"} --out "$OUT/$u" \
      || echo "unit $u: non-success rows (see $OUT/$u/results.md)"
  elif [[ $RETRY == 1 ]]; then
    n=1; while [[ -e "$OUT/$u/retry-$n" ]]; do n=$((n+1)); done
    mapfile -t todo < <(python3 "$H/central_summary.py" --out "$OUT" --unit "$u" --list-nonsuccess --skip "$SKIP")
    [[ ${#todo[@]} -gt 0 ]] && { echo "=== $u retry-$n (${#todo[@]} harnesses) $(date +%T)"; run_set "$u" "$OUT/$u/retry-$n" "$UNWIND" "${todo[@]}"; }
  else
    echo "skip $u (done)"
  fi
  mapfile -t uwf < <(python3 "$H/central_summary.py" --out "$OUT" --unit "$u" --list-unwind-only)
  if [[ ${#uwf[@]} -gt 0 ]]; then
    n=1; while [[ -e "$OUT/$u/unwind-$n" ]]; do n=$((n+1)); done
    echo "=== $u tier2 unwind=$UNWIND2 (${#uwf[@]} harnesses) $(date +%T)"
    run_set "$u" "$OUT/$u/unwind-$n" "$UNWIND2" "${uwf[@]}"
  fi
done
echo "central pass finished $(date +%T)"
