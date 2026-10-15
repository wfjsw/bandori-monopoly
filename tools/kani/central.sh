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

# A toolchain/source break (e.g. another session added a host import that
# sys_kani.rs lacks) makes every harness "FAIL" in seconds; never burn through
# the remaining units that way. Abort the whole pass loudly instead.
build_break() { # unit
  local d="$OUT/$1/logs" n=0 bad=0 f
  [[ -d "$d" ]] || return 1
  for f in "$d"/*.log; do
    [[ -f "$f" ]] || continue
    n=$((n+1))
    grep -qE 'could not compile|error\[E[0-9]+\]' "$f" && bad=$((bad+1))
  done
  [[ $n -gt 0 && $bad -eq $n ]]
}

# Pre-flight: compile-check the first harness of each unit before any CBMC
# time is spent. A rename/regression break stops the pass immediately with
# the first error instead of burning through every unit's logs.
preflight() {
  local u feat h
  echo "=== preflight compile check $(date +%T)"
  for u in "${UNITS[@]}"; do
    # Resolve the unit's features the same way run.sh does.
    case "$u" in
      pp|money_shave|prompt) feat="band-pp" ;;
      ppp|teleport) feat="band-ppp,skills-personas,skills-groups" ;;
      r) feat="band-roselia,skills-personas,skills-groups" ;;
      ag) feat="band-ag,skills-personas,skills-groups" ;;
      hhw) feat="band-hhw,skills-personas,skills-groups" ;;
      mor) feat="band-morfonica" ;;
      ras) feat="band-ras" ;;
      mygo) feat="band-mygo" ;;
      mujica) feat="band-mujica" ;;
      crychic) feat="band-crychic" ;;
      sumimi) feat="band-sumimi,skills-personas" ;;
      events) feat="events" ;;
      tiles|tile_marks) feat="tiles" ;;
      skills_pp) feat="band-pp,skills-personas,skills-groups" ;;
      skills_ppp) feat="band-ppp,skills-personas,skills-groups" ;;
      skills_r) feat="band-roselia,skills-personas,skills-groups" ;;
      skills_ag) feat="band-ag,skills-personas,skills-groups" ;;
      skills_hhw) feat="band-hhw,skills-personas,skills-groups" ;;
      skills_mor) feat="band-morfonica,skills-groups,skills-personas" ;;
      skills_ras) feat="band-ras,skills-personas" ;;
      skills_mygo) feat="band-mygo,skills-groups" ;;
      skills_mujica) feat="band-mujica,skills-groups" ;;
      skills_crychic) feat="band-crychic,skills-groups,skills-personas" ;;
      skills_sumimi) feat="band-sumimi,skills-groups" ;;
      skills_general|general) feat="band-general" ;;
      skills_characters) feat="skills-personas" ;;
      skills_bands) feat="skills-groups" ;;
      *) feat="all" ;;
    esac
    echo "  $u (features: $feat)"
    if ! cargo kani --only-codegen -p rules-kani --no-default-features --features "$feat" 2>&1 | tee /tmp/kani-preflight-$u.log | grep -qE 'error\[E[0-9]+\]'; then
      : # no errors
    else
      echo "PREFLIGHT FAIL: unit $u does not compile." >&2
      echo "First compile errors:" >&2
      grep -E 'error\[E[0-9]+\]' /tmp/kani-preflight-$u.log | head -5 >&2
      exit 3
    fi
  done
  echo "=== preflight OK $(date +%T)"
}

# Compile-check every unit before spending any CBMC time.
preflight

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
  if build_break "$u"; then
    echo "BUILD BREAK: every harness of unit $u failed to compile (see $OUT/$u/logs). Aborting the pass; fix the source, delete $OUT/$u and re-run." >&2
    # Print the first compile error from the first log to speed up diagnosis.
    first_log=$(ls "$OUT/$u/logs"/*.log 2>/dev/null | head -1)
    if [[ -n "$first_log" ]]; then
      echo "First compile error ($first_log):" >&2
      grep -E 'error\[E[0-9]+\]|could not compile' "$first_log" | head -5 >&2
    fi
    exit 2
  fi
  mapfile -t uwf < <(python3 "$H/central_summary.py" --out "$OUT" --unit "$u" --list-unwind-only)
  if [[ ${#uwf[@]} -gt 0 ]]; then
    n=1; while [[ -e "$OUT/$u/unwind-$n" ]]; do n=$((n+1)); done
    echo "=== $u tier2 unwind=$UNWIND2 (${#uwf[@]} harnesses) $(date +%T)"
    run_set "$u" "$OUT/$u/unwind-$n" "$UNWIND2" "${uwf[@]}"
  fi
done
echo "central pass finished $(date +%T)"
