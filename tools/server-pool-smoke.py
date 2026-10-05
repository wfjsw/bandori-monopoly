#!/usr/bin/env python3
"""End-to-end smoke for the server's rules-worker pool.

Starts the real HTTP server with a worker pool and drives one match through it:
session -> room -> bots -> start -> state -> act -> state. Every match
operation crosses the process boundary, so this is the check that the wiring
works and not just the individual pieces.

  python tools/server-pool-smoke.py
"""

import json
import os
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PORT = 18090
BASE = f"http://127.0.0.1:{PORT}"
SERVER = ROOT / "target" / "debug" / "server.exe"
WORKER = ROOT / "target" / "debug" / "rules-worker.exe"


def req(method, path, token=None, body=None):
    data = None if body is None else json.dumps(body).encode()
    r = urllib.request.Request(BASE + path, data=data, method=method)
    r.add_header("content-type", "application/json")
    if token:
        r.add_header("authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r, timeout=15) as resp:
            return resp.status, json.loads(resp.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def main():
    for exe in (SERVER, WORKER):
        if not exe.exists():
            sys.exit(f"build it first: cargo build -p rules-worker -p server ({exe} missing)")

    env = dict(os.environ, BM_WORKER=str(WORKER))
    srv = subprocess.Popen(
        [str(SERVER), "--port", str(PORT), "--data", "data", "--rules", "dist/cards", "--workers", "4"],
        cwd=ROOT,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        # Wait for the listener.
        for _ in range(50):
            try:
                st, _ = req("GET", "/api/health")
                if st == 200:
                    break
            except Exception:
                time.sleep(0.1)
        else:
            err = srv.stderr.read() if srv.poll() is not None else ""
            sys.exit(f"server did not come up\n{err}")

        ok = 0

        # The pool banner proves workers were spawned.
        time.sleep(0.3)
        # (stderr is piped; the banner is on stderr -- read what is there now)
        # Not blocking on it: the requests below are the real assertion.

        st, sess = req("POST", "/api/session", body={"player": "tester"})
        assert st == 200 and sess.get("token"), (st, sess)
        tok = sess["token"]
        ok += 1

        st, made = req("POST", "/api/rooms", tok, {"name": "pool-test", "ranked": False, "maxPlayers": 6, "password": ""})
        assert st == 200 and made.get("room"), (st, made)
        room = made["room"]["id"]
        ok += 1

        for _ in range(3):
            st, _ = req("POST", f"/api/rooms/{room}/bots", tok, {"op": "add"})
            assert st == 200, st
            ok += 1

        st, started = req("POST", f"/api/rooms/{room}/start", tok, {"force": True})
        assert st == 200 and started.get("playing") is True, (st, started)
        ok += 1

        # The match state comes back out of a worker process.
        st, snap = req("GET", f"/api/rooms/{room}/state", tok)
        assert st == 200, (st, snap)
        m = snap.get("match")
        assert m and m.get("state") and m.get("hand") is not None, snap
        assert m["you"] == 1, m
        print(f"  match phase: {m['state'].get('phase')}  seq: {m['state'].get('seq')}  players: {len(m['state'].get('players', []))}")
        ok += 1

        # An act round-trips through the pool and rewrites the blob.
        before = m["state"].get("seq")
        st, res = req("POST", f"/api/rooms/{room}/act", tok, {"act": "roll"})
        # 200 with ok, or 400 with a rules rejection -- both mean the worker
        # answered. A 500 / timeout would mean the pool did not.
        assert st in (200, 400), (st, res)
        ok += 1

        st, snap2 = req("GET", f"/api/rooms/{room}/state", tok)
        assert st == 200 and snap2.get("match"), (st, snap2)
        after = snap2["match"]["state"].get("seq")
        print(f"  seq {before} -> {after}  (act status {st})")
        ok += 1

        # The ticker drives matches through the same pool. Give it a couple of
        # seconds and the match must have advanced without any client act.
        start_seq = snap2["match"]["state"].get("seq")
        start_phase = snap2["match"]["state"].get("phase")
        moved = False
        for _ in range(30):
            time.sleep(0.2)
            st, s3 = req("GET", f"/api/rooms/{room}/state", tok)
            if st != 200 or not s3.get("match"):
                continue
            st3 = s3["match"]["state"]
            if st3.get("seq") != start_seq or st3.get("phase") != start_phase:
                print(f"  ticker advanced: phase {start_phase} -> {st3.get('phase')}  seq {start_seq} -> {st3.get('seq')}")
                moved = True
                break
        assert moved, "the match never advanced through the ticker/pool"
        ok += 1

        # Two reads are consistent in identity. (Not byte-identical: the ticker
        # may land a tick between them -- the match is live. "Same blob in, same
        # answer out" is what tools/rules-worker-smoke.py checks across two
        # processes.)
        st, a = req("GET", f"/api/rooms/{room}/state", tok)
        st, b = req("GET", f"/api/rooms/{room}/state", tok)
        assert a["match"]["state"]["matchId"] == b["match"]["state"]["matchId"], "match identity changed between reads"
        assert len(a["match"]["state"]["players"]) == len(b["match"]["state"]["players"])
        ok += 1

        print(f"server-pool smoke: {ok} checks passed")
    finally:
        if srv.poll() is None:
            if os.name == "nt":
                srv.terminate()
            else:
                srv.send_signal(signal.SIGTERM)
        try:
            srv.wait(timeout=5)
        except Exception:
            srv.kill()


if __name__ == "__main__":
    main()