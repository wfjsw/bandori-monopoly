#!/usr/bin/env python3
"""Smoke test for rules-worker: long-running, many requests, no shared state.

Spawns one worker process and drives it over stdio, threading the match blob
through the caller the way a real pool would. Asserts the three properties the
design promises:

  1. long running  -- one process answers every request, including after a
                      malformed one (which must not kill it)
  2. multi-request -- ops beyond the first keep working on the same process
  3. stateless     -- the caller owns the state; `view` does not echo a blob
                      and a second process fed the same blob behaves the same

  python tools/rules-worker-smoke.py
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "debug" / "rules-worker.exe"


def members():
    return [
        {"id": 1, "player": "a", "character": "", "cnId": "", "ready": True, "host": True, "bot": False, "away": False},
        {"id": 2, "player": "b", "character": "", "cnId": "", "ready": True, "host": False, "bot": True, "away": False},
    ]


def start():
    return subprocess.Popen(
        [str(BIN)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        cwd=ROOT,
    )


class Worker:
    def __init__(self):
        self.p = start()
        self.n = 0

    def call(self, req):
        self.n += 1
        req.setdefault("id", self.n)
        self.p.stdin.write(json.dumps(req, ensure_ascii=False) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        if not line:
            raise AssertionError(f"worker died on request {req.get('id')}: {req.get('op')}")
        return json.loads(line)

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def main():
    if not BIN.exists():
        sys.exit(f"build it first: cargo build -p rules-worker ({BIN} missing)")
    ok = 0

    w = Worker()

    # 1 + 2: one process, several requests.
    for _ in range(3):
        r = w.call({"op": "ping"})
        assert r["ok"] is True, r
        ok += 1

    # A malformed request must be answered, not fatal.
    w.p.stdin.write("this is not json\n")
    w.p.stdin.flush()
    bad = json.loads(w.p.stdout.readline())
    assert bad["ok"] is False, bad
    ok += 1
    assert w.call({"op": "ping"})["ok"] is True, "worker died after a bad request"
    ok += 1

    # 3: create a match. The caller owns the blob from here on.
    r = w.call({"op": "new", "members": members(), "seed": 42, "mode": 0, "weights": {"money": 1, "property": 1, "houses": 1}})
    assert r["ok"] is True and r["state"], r
    state = r["state"]
    ok += 1
    print(f"  save blob: {len(state)} bytes")

    # Read-only ops must not hand back a blob -- that is what "stateless" means
    # for the caller: it already has the only copy.
    v = w.call({"op": "view", "state": state, "member": 1})
    assert v["ok"] is True and "view" in v and "state" not in v, v
    assert v["view"]["you"] == 1, v["view"]
    ok += 1

    e = w.call({"op": "events", "state": state, "since": 0})
    assert e["ok"] is True and "events" in e and "state" not in e, e
    ok += 1

    # A mutating op returns a new blob, and the old one still works -- the
    # worker holds nothing, so two callers can hold two versions of the same
    # match and both stay valid.
    t = w.call({"op": "tick", "state": state, "dt": 0.05})
    assert t["ok"] is True and t.get("changed") is True and t["state"], t
    state2 = t["state"]
    ok += 1
    assert w.call({"op": "view", "state": state, "member": 1})["ok"] is True, "the pre-tick blob stopped working"
    ok += 1

    # Same blob into a *second* process must behave the same: nothing is shared.
    w2 = Worker()
    v1 = w.call({"op": "view", "state": state2, "member": 1})
    v2 = w2.call({"op": "view", "state": state2, "member": 1})
    assert v1["view"] == v2["view"], "two workers disagree on the same state"
    ok += 1

    # And the second process is live on its own.
    assert w2.call({"op": "ping"})["ok"] is True
    ok += 1

    w.close()
    w2.close()
    print(f"rules-worker smoke: {ok} checks passed")


if __name__ == "__main__":
    main()