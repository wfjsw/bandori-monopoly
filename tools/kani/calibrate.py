#!/usr/bin/env python3
"""Minimal sound unwind search for one rules-kani harness.

For a given harness, binary-search the smallest N in [lo, hi] such that a
sound run (unwinding assertions ON) reports no 'unwinding assertion' failure.
Writes the winner to `rules/kani/unwind_table.toml` (harness -> N).

Usage (from the worktree root):
  export CARGO_TARGET_DIR=/home/monopoly/kani-target-tune CARGO_BUILD_JOBS=3
  python3 tools/kani/calibrate.py --unit money_shave \
      --harness aya_haruka_02_shave_iff --timeout 3600

Options:
  --lo / --hi          search range (default 2..256)
  --budget SECONDS     wall-clock budget for the whole search (default 7200)
  --timeout SECONDS    per-run timeout (default 3600)
  --dry-run            print the plan, do not run CBMC or write the table
  --force              overwrite an existing table entry
  --mutate N           also run once at N (deliberately too small if N < min)
                      and assert it FAILS with an unwinding assertion, so the
                      pipeline cannot silently regress to vacuous proofs

Exit codes:
  0  calibrated (or dry-run)
  1  no sound N found inside the range/budget
  2  usage / environment error
  3  mutation check failed (too-small N did NOT fail)
"""

from __future__ import annotations

import argparse
import datetime
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE = ROOT / "rules" / "kani" / "unwind_table.toml"
RUN = ROOT / "tools" / "kani" / "run.sh"

UNWIND_FAIL_RE = re.compile(r"unwinding assertion", re.I)
SUCCESS_RE = re.compile(r"VERIFICATION:\s*-\s*SUCCESSFUL", re.I)
FAILED_RE = re.compile(r"VERIFICATION:\s*-\s*FAILED", re.I)
COVER_FAIL_RE = re.compile(r"cover properties satisfied \([0-9]+ unreachable\)")


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--unit", required=True)
    p.add_argument("--harness", required=True)
    p.add_argument("--lo", type=int, default=2)
    p.add_argument("--hi", type=int, default=256)
    p.add_argument("--budget", type=int, default=7200)
    p.add_argument("--timeout", type=int, default=3600)
    p.add_argument("--out-dir", default=str(ROOT / "target" / "kani-calibrate"))
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--force", action="store_true")
    p.add_argument("--mutate", type=int, default=None,
                   help="run once at this N and require an unwinding-assertion FAIL")
    return p.parse_args()


def run_one(unit: str, harness: str, unwind: int, timeout: int, out_dir: Path) -> dict:
    """One serial CBMC job via tools/kani/run.sh --jobs 1."""
    tag = f"{harness}__u{unwind}"
    od = out_dir / tag
    od.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = env.get("CARGO_TARGET_DIR", "/home/monopoly/kani-target-tune")
    env["CARGO_BUILD_JOBS"] = env.get("CARGO_BUILD_JOBS", "3")
    cmd = [
        "bash", str(RUN),
        "--unit", unit,
        "--harness", harness,
        "--jobs", "1",
        "--timeout", str(timeout),
        "--unwind", str(unwind),
        "--out", str(od),
    ]
    t0 = time.time()
    print(f"[calibrate] unwind={unwind} -> {' '.join(cmd)}", flush=True)
    try:
        proc = subprocess.run(cmd, cwd=str(ROOT), env=env, timeout=timeout + 120,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        rc = proc.returncode
        log_text = proc.stdout
    except subprocess.TimeoutExpired as e:
        rc = 124
        log_text = (e.stdout or "") if isinstance(e.stdout, str) else ""
    dt = time.time() - t0

    # Prefer the per-harness log the runner wrote.
    logs = sorted(od.glob("logs/*.log"))
    body = ""
    if logs:
        body = logs[0].read_text(errors="replace")
    if not body:
        body = log_text

    unwinding = bool(UNWIND_FAIL_RE.search(body))
    successful = bool(SUCCESS_RE.search(body)) and not unwinding
    failed = bool(FAILED_RE.search(body)) or unwinding
    vacuous = bool(COVER_FAIL_RE.search(body))
    if rc == 124:
        verdict = "TIMEOUT"
    elif unwinding:
        verdict = "UNWINDING"
    elif successful and not vacuous:
        verdict = "SOUND"
    elif successful and vacuous:
        verdict = "VACUOUS"
    elif failed:
        verdict = "FAILED"
    else:
        verdict = "UNKNOWN"

    print(f"[calibrate]   -> {verdict} in {dt:.1f}s rc={rc}", flush=True)
    return {
        "unwind": unwind,
        "verdict": verdict,
        "seconds": round(dt, 2),
        "rc": rc,
        "out": str(od),
        "unwinding": unwinding,
    }


def load_table() -> dict[str, int]:
    """Minimal TOML subset: `harness = N` lines under `[unwind]`."""
    if not TABLE.exists():
        return {}
    text = TABLE.read_text()
    out: dict[str, int] = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("["):
            continue
        m = re.match(r'^([A-Za-z0-9_:.]+)\s*=\s*(\d+)\s*$', line)
        if m:
            out[m.group(1)] = int(m.group(2))
    return out


def write_table(entries: dict[str, int]) -> None:
    TABLE.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Minimal sound unwind bound per harness (calibrated).",
        "# Written by tools/kani/calibrate.py — do not hand-edit numbers without",
        "# re-running the calibration; a too-small N FAILS the unwinding assertion",
        "# (mutation-checked), it never silently truncates.",
        "#",
        "# Consumed by tools/kani/run.sh: table entry > #[kani::unwind] attribute >",
        "# global default.",
        "",
        "[unwind]",
    ]
    for k in sorted(entries):
        lines.append(f"{k} = {entries[k]}")
    TABLE.write_text("\n".join(lines) + "\n")


def main() -> int:
    args = parse_args()
    if args.lo < 2 or args.hi < args.lo:
        print("bad --lo/--hi", file=sys.stderr)
        return 2

    existing = load_table()
    key = args.harness
    if key in existing and not args.force and args.mutate is None:
        print(f"[calibrate] {key} already in table as {existing[key]}; use --force to redo")
        print(f"  table: {TABLE}")
        return 0

    print(f"[calibrate] harness={args.harness} unit={args.unit} "
          f"range=[{args.lo},{args.hi}] budget={args.budget}s")
    if args.dry_run:
        print("[calibrate] dry-run: no CBMC, no table write")
        return 0

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    deadline = time.time() + args.budget

    # Mutation check first (optional): a deliberately too-small N must FAIL.
    if args.mutate is not None:
        if time.time() > deadline:
            print("[calibrate] budget exhausted before mutation check", file=sys.stderr)
            return 1
        r = run_one(args.unit, args.harness, args.mutate, args.timeout, out_dir)
        if r["verdict"] not in ("UNWINDING", "FAILED", "TIMEOUT"):
            print(f"[calibrate] MUTATION CHECK FAILED: N={args.mutate} gave {r['verdict']} "
                  f"(expected UNWINDING/FAILED) — pipeline could silently regress",
                  file=sys.stderr)
            return 3
        print(f"[calibrate] mutation check OK: N={args.mutate} -> {r['verdict']}")

    # Binary search for the smallest sound N.
    lo, hi = args.lo, args.hi
    best: dict | None = None
    trials: list[dict] = []
    while lo <= hi:
        if time.time() > deadline:
            print("[calibrate] budget exhausted mid-search", file=sys.stderr)
            break
        mid = (lo + hi) // 2
        r = run_one(args.unit, args.harness, mid, args.timeout, out_dir)
        trials.append(r)
        if r["verdict"] == "SOUND":
            best = r
            hi = mid - 1
        elif r["verdict"] in ("UNWINDING", "FAILED"):
            lo = mid + 1
        else:
            # TIMEOUT / VACUOUS / UNKNOWN: cannot prove soundness at this N.
            # Treat as "not sound here" and search higher (more unwind may
            # finish paths) but do not record as a winner.
            lo = mid + 1

    if best is None:
        print("[calibrate] no sound N found in range/budget", file=sys.stderr)
        print(f"[calibrate] trials: {trials}", file=sys.stderr)
        return 1

    n = best["unwind"]
    entries = load_table()
    entries[key] = n
    write_table(entries)
    # Kani prefers #[kani::unwind(N)] over --default-unwind, so keep the
    # source attribute in sync or the table entry is ignored.
    patched = patch_unwind_attr(args.harness, n)
    print(f"[calibrate] wrote {TABLE}: {key} = {n}  ({best['seconds']}s)")
    if patched:
        print(f"[calibrate] patched #[kani::unwind({n})] in {patched}")
    else:
        print("[calibrate] note: no #[kani::unwind] attr found; "
              "the table entry is the only bound")
    print(f"[calibrate] trials: {[(t['unwind'], t['verdict'], t['seconds']) for t in trials]}")
    return 0


def patch_unwind_attr(harness: str, n: int) -> str | None:
    """Rewrite `#[kani::unwind(M)]` above `fn harness` to `#[kani::unwind(n)]`."""
    proofs = ROOT / "rules" / "kani" / "src" / "proofs"
    for path in proofs.glob("*.rs"):
        src = path.read_text()
        if f"fn {harness}(" not in src:
            continue
        # Replace the unwind attribute closest above the fn.
        pattern = re.compile(
            r"(#\[kani::unwind\()(\d+)(\)\])"
            r"((?:\s*#\[[^\]]+\])*\s*(?:pub\s+)?fn\s+"
            + re.escape(harness)
            + r"\s*\()"
        )
        new, cnt = pattern.subn(rf"\g<1>{n}\g<3>\g<4>", src, count=1)
        if cnt:
            path.write_text(new)
            return str(path)
        # No attribute: insert one just before `#[kani::proof]` / the fn.
        pattern2 = re.compile(
            r"(#\[kani::proof\])"
            r"((?:\s*#\[[^\]]+\])*\s*(?:pub\s+)?fn\s+"
            + re.escape(harness)
            + r"\s*\()"
        )
        new, cnt = pattern2.subn(rf"#[kani::unwind({n})]\n\g<1>\g<2>", src, count=1)
        if cnt:
            path.write_text(new)
            return str(path)
    return None


if __name__ == "__main__":
    sys.exit(main())