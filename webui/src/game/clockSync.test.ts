// Offset estimator: Cristian/NTP math, lowest-RTT retention, slew.
// Run with: node --test webui/src/game/clockSync.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import {
  ClockOffset,
  estimateOffset,
  displayRemainingMs,
  ResyncLimiter,
  uncertaintyMs,
} from "./clockSync.ts";

test("estimateOffset is the Cristian/NTP formula", () => {
  // Perfect link, server 1000ms ahead: t1=10, t2=1010, t3=1011, t4=11
  // rtt = (11-10) - (1011-1010) = 0
  // offset = ((1010-10) + (1011-11)) / 2 = 1000
  const { offset, rtt } = estimateOffset({ t1: 10, t2: 1010, t3: 1011, t4: 11 });
  assert.equal(rtt, 0);
  assert.equal(offset, 1000);
});

test("estimateOffset folds half the round trip into the offset", () => {
  // 20ms rtt, server 500ms ahead. t1=0, t4=20; server stamps 1ms apart in the middle.
  // t2=510, t3=511  =>  rtt = 20 - 1 = 19
  // offset = ((510-0) + (511-20)) / 2 = 500.5
  const { offset, rtt } = estimateOffset({ t1: 0, t2: 510, t3: 511, t4: 20 });
  assert.equal(rtt, 19);
  assert.equal(offset, 500.5);
});

test("lowest-RTT sample wins; a noisier one is ignored", () => {
  const o = new ClockOffset();
  // t1=0, t4=1, t2=1001, t3=1001 -> rtt = 1, offset = 1000.5
  assert.ok(o.push({ t1: 0, t2: 1001, t3: 1001, t4: 1 }));
  assert.equal(o.targetOffset, 1000.5);
  // Noisier sample with a wild offset: rejected.
  assert.ok(!o.push({ t1: 10, t2: 50_000, t3: 50_100, t4: 120 })); // rtt 110
  assert.equal(o.targetOffset, 1000.5);
  // Better RTT replaces it.
  // t1=0, t4=0, t2=2000, t3=2000 -> rtt 0, offset 2000
  assert.ok(o.push({ t1: 0, t2: 2000, t3: 2000, t4: 0 }));
  assert.equal(o.targetOffset, 2000);
  assert.equal(o.bestRoundTrip, 0);
});

test("the first sample is taken whole, however far off", () => {
  const o = new ClockOffset();
  // Server clock 5 minutes behind the page (server restarted after page load).
  o.push({ t1: 400_000, t2: 100_000, t3: 100_000, t4: 400_000 });
  assert.equal(o.offset, -300_000);
});

test("a change over SNAP_MS snaps on the next step", () => {
  const o = new ClockOffset();
  o.push({ t1: 0, t2: 0, t3: 0, t4: 10 }); // rtt 10, offset -5
  o.push({ t1: 0, t2: 100_000, t3: 100_000, t4: 0 }); // rtt 0, offset 100000
  assert.equal(o.offset, -5);
  assert.equal(o.step(2), 100_000);
});

test("slew approaches the target without jumping", () => {
  const o = new ClockOffset();
  // offset 0, rtt 4: t1=0, t4=4, t2=2, t3=2
  o.push({ t1: 0, t2: 2, t3: 2, t4: 4 });
  // A better-RTT sample 20ms away: t1=0, t4=2, t2=21, t3=21 -> rtt 2, offset 20
  o.push({ t1: 0, t2: 21, t3: 21, t4: 2 });
  assert.equal(o.offset, 0);
  assert.equal(o.step(2), 2);
  assert.equal(o.step(2), 4);
  for (let i = 0; i < 20; i++) o.step(2);
  assert.equal(o.offset, 20);
});

test("remainingMs maps a server deadline through the offset", () => {
  const o = new ClockOffset();
  // server is 1000 ahead: t1=0, t4=0, t2=1000, t3=1000
  o.push({ t1: 0, t2: 1000, t3: 1000, t4: 0 });
  for (let i = 0; i < 1000; i++) o.step(1000); // snap
  // local now 50 -> server now 1050; deadline 2000 -> 950 left
  assert.equal(o.remainingMs(2_000, 50), 950);
});

test("rejects nonsense samples", () => {
  const o = new ClockOffset();
  assert.ok(!o.push({ t1: 0, t2: 0, t3: -5, t4: 10 })); // t3 < t2
  assert.ok(!o.push({ t1: 0, t2: 10, t3: 5, t4: 20 })); // t3 < t2 again
  assert.ok(!o.push({ t1: 0, t2: NaN, t3: 0, t4: 0 }));
  // Negative rtt: long server processing that exceeds the client gap.
  assert.ok(!o.push({ t1: 0, t2: 0, t3: 50, t4: 10 }));
  assert.ok(!o.hasSample);
});

test("displayRemainingMs never races ahead of the server", () => {
  // deadline 2000, local now 1900, offset 0 -> 100ms real left, displayed
  // with the 100ms epsilon so zero is never shown early.
  assert.equal(displayRemainingMs(2_000, 1_900, 0), 200);
  // 50ms past the deadline: still shows 50ms so we do not hit 0 before the
  // server expires the turn (sole authority).
  assert.equal(displayRemainingMs(2_000, 2_050, 0), 50);
  // 150ms past: past the 100ms epsilon, display is 0.
  assert.equal(displayRemainingMs(2_000, 2_150, 0), 0);
});

test("ResyncLimiter is a token bucket", () => {
  const l = new ResyncLimiter(2, 1_000);
  assert.ok(l.take(0));
  assert.ok(l.take(0));
  assert.ok(!l.take(0), "bucket empty");
  assert.ok(l.take(1_000), "refills over time");
});

test("uncertainty is ±RTT/2", () => {
  assert.equal(uncertaintyMs(20), 10);
  assert.equal(uncertaintyMs(0), 0);
  assert.equal(uncertaintyMs(-5), 0);
});