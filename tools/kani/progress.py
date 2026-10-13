#!/usr/bin/env python3
"""Live progress of a Kani central pass.

  python3 tools/kani/progress.py [--out DIR] [--watch SECONDS] [--tail N]

run.sh sends each harness's full output to its own log file and writes
results.tsv one line per FINISHED harness (live); results.json/md only appear
when a whole unit finishes. central.log carries just unit headers. This reads
results.tsv + selected.tsv + the logs to show where the pass is right now.
"""
import argparse, glob, os, re, subprocess, sys, time, collections

def rd(p):
    try:
        return open(p, errors="replace").read()
    except OSError:
        return ""

def units_in_order(out):
    log = rd(os.path.join(out, "central.log"))
    order = []
    for m in re.finditer(r"^=== (\S+) (tier\d|retry-\d+)", log, re.M):
        if m.group(1) not in order:
            order.append(m.group(1))
    for d in sorted(glob.glob(os.path.join(out, "*", "selected.tsv"))):
        u = os.path.basename(os.path.dirname(d))
        if u not in order:
            order.append(u)
    return order

def unit_state(out, u):
    base = os.path.join(out, u)
    sel = [l.split("\t") for l in rd(os.path.join(base, "selected.tsv")).splitlines() if l]
    names = [s[1] for s in sel if len(s) > 1]
    done = {}
    for l in rd(os.path.join(base, "results.tsv")).splitlines():
        f = l.split("\t")
        if len(f) >= 4:
            done[f[1]] = (f[2], float(f[3] or 0))
    finished = os.path.isfile(os.path.join(base, "results.json"))
    running = [n for n in names if n not in done]
    return names, done, finished, running

def newest_log(out, u, names, done):
    best = None
    for n in names:
        if n in done:
            continue
        p = os.path.join(out, u, "logs", f"{u}__{n}.log")
        if os.path.isfile(p):
            m = os.path.getmtime(p)
            if best is None or m > best[0]:
                best = (m, n, p)
    return best

def last_progress_line(p):
    t = rd(p)
    lines = [l for l in t.splitlines() if l.strip()]
    for l in reversed(lines[-200:]):
        if "Unwinding loop" in l or "Running" in l or "Checking" in l or "Generated" in l or "SAT" in l or "Solving" in l or "Converting" in l:
            return l.strip()[:120]
    return (lines[-1].strip()[:120] if lines else "")

def cbmc():
    out = []
    try:
        for pid in subprocess.check_output(["pgrep", "-x", "cbmc"]).split():
            pid = int(pid)
            rss = int(rd(f"/proc/{pid}/status").split("VmRSS:")[1].split()[0]) // 1024
            et = subprocess.check_output(["ps", "-o", "etimes=", "-p", str(pid)]).decode().strip()
            out.append((pid, rss, int(et)))
    except Exception:
        pass
    return out

def show(out, tail):
    order = units_in_order(out)
    tot = collections.Counter(); ndone = ntotal = 0
    print(f"central pass: {out}   ({time.strftime('%F %T')})")
    print(f"{'unit':<20}{'done/total':>11}  verdicts")
    cur = None
    for u in order:
        names, done, finished, running = unit_state(out, u)
        c = collections.Counter(v for v, _ in done.values())
        tot.update(c); ndone += len(done); ntotal += len(names)
        mark = "done" if finished else ("RUNNING" if names else "starting")
        vs = " ".join(f"{k}={v}" for k, v in sorted(c.items()))
        print(f"{u:<20}{len(done):>5}/{len(names):<5} {mark:<9} {vs}")
        if not finished and names:
            cur = (u, names, done)
    print(f"{'TOTAL (listed units)':<20}{ndone:>5}/{ntotal:<5}           " + " ".join(f"{k}={v}" for k, v in sorted(tot.items())))
    if cur:
        u, names, done = cur
        nl = newest_log(out, u, names, done)
        if nl:
            age = int(time.time() - nl[0])
            print(f"\ncurrent: {u}::{nl[1]}  (log last written {age}s ago)")
            print(f"  {last_progress_line(nl[2])}")
    for pid, rss, et in cbmc():
        print(f"cbmc pid {pid}: {rss} MB, running {et // 60}m{et % 60:02d}s")
    if tail:
        print("\nlast finished harnesses:")
        rows = []
        for u in order:
            for l in rd(os.path.join(out, u, "results.tsv")).splitlines():
                f = l.split("\t")
                if len(f) >= 4:
                    p = f[5] if len(f) > 5 else ""
                    rows.append((os.path.getmtime(p) if os.path.isfile(p) else 0, f))
        for _, f in sorted(rows)[-tail:]:
            print(f"  {f[0]}::{f[1]}  {f[2]}  {float(f[3]):.0f}s")

ap = argparse.ArgumentParser()
ap.add_argument("--out", default=os.environ.get("KANI_CENTRAL_OUT", "/home/monopoly/kani-central-results3"))
ap.add_argument("--watch", type=int, default=0)
ap.add_argument("--tail", type=int, default=5)
a = ap.parse_args()
while True:
    show(a.out, a.tail)
    if not a.watch:
        break
    time.sleep(a.watch)
    print("\n" + "-" * 60)
