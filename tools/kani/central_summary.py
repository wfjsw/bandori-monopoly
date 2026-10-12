#!/usr/bin/env python3
"""Summarise / select rows across the tiered central-pass result dirs.
Newest results.json (by dir mtime order: base, retry-N, unwind-N) wins per harness."""
import argparse, glob, json, os, re, sys, collections

def load(out, unit):
    rows = {}
    dirs = [os.path.join(out, unit)] + sorted(
        glob.glob(os.path.join(out, unit, "retry-*")) + glob.glob(os.path.join(out, unit, "unwind-*")),
        key=lambda p: os.path.getmtime(p))
    for d in dirs:
        f = os.path.join(d, "results.json")
        if not os.path.isfile(f):
            continue
        j = json.load(open(f))
        for r in (j.get("rows") or j.get("results") or j.get("harnesses") or []):
            r = dict(r); r["_dir"] = d
            rows[r.get("harness") or r.get("name")] = r
    return rows

def unwind_only(row):
    log = row.get("log") or ""
    log = log if os.path.isabs(log) else os.path.join(row["_dir"], log)
    try: t = open(log, errors="replace").read()
    except OSError: return False
    fails = re.findall(r"Failed Checks: (.*)", t)
    return bool(fails) and all(f.startswith("unwinding assertion") for f in fails)

ap = argparse.ArgumentParser()
ap.add_argument("--out", required=True); ap.add_argument("--unit")
ap.add_argument("--list-nonsuccess", action="store_true")
ap.add_argument("--list-unwind-only", action="store_true")
ap.add_argument("--skip", default="")
a = ap.parse_args()
skip = set(filter(None, a.skip.split(",")))
units = [a.unit] if a.unit else sorted(os.path.basename(p) for p in glob.glob(os.path.join(a.out, "*")) if os.path.isdir(p))
if a.list_nonsuccess or a.list_unwind_only:
    for h, r in load(a.out, a.unit).items():
        if h in skip: continue
        if a.list_nonsuccess and r["verdict"] != "SUCCESSFUL": print(h)
        if a.list_unwind_only and r["verdict"] == "FAILED" and unwind_only(r): print(h)
    sys.exit(0)
tot = collections.Counter()
print("| unit | harness | verdict | time(s) |\n|---|---|---|---:|")
for u in units:
    for h, r in sorted(load(a.out, u).items()):
        tot[r["verdict"]] += 1
        print(f"| {u} | {h} | {r['verdict']} | {float(r.get('time_sec', 0)):.0f} |")
print("\nTOTAL", dict(tot))
