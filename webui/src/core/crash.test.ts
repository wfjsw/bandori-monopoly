// Run with: node --test webui/src/core/crash.test.ts
// The crash registry is the root-cause record behind the crash panel: first
// crash wins, a later page-fatal one may still replace noise, and the debug
// blob is what a user pastes into a bug report.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  clearCrash,
  crashDebugText,
  CRASH_DEDUPE_MS,
  CRASH_LOG_TAIL,
  getCrash,
  isRepeatCrash,
  reportCrash,
  subscribeCrash,
  type CrashInfo,
  type CrashKind,
} from "./crash.ts";

const base = {
  time: 1_700_000_000_000,
  url: "https://game.example/play/1",
  ua: "TestBrowser/1.0",
};

function info(kind: CrashKind, message: string, extra: Partial<CrashInfo> = {}): CrashInfo {
  return { kind, message, stack: `Error: ${message}\n    at fn`, ...base, ...extra };
}

test("reportCrash keeps the first crash and ignores later ones", () => {
  clearCrash();
  const first = info("render", "first boom");
  reportCrash(first);
  reportCrash(info("render", "second boom"));
  reportCrash(info("worker", "third boom"));
  assert.equal(getCrash()?.message, "first boom");
  assert.equal(getCrash()?.kind, "render");
});

test("a page-fatal crash may still replace a recorded non-fatal one", () => {
  clearCrash();
  reportCrash(info("rejection", "void promise"));
  reportCrash(info("render", "render boom"));
  assert.equal(getCrash()?.message, "render boom");
  // ...and the reverse never happens: noise cannot bury the real failure.
  reportCrash(info("rejection", "another void"));
  assert.equal(getCrash()?.message, "render boom");
});

// The same bug can arrive twice: once out of a void promise and once out of the
// render it poisoned. The second is the one the user needs to see, so the
// message dedupe must never outrank a page-fatal crash.
test("a rejection does not swallow a later render crash with the same message", () => {
  clearCrash();
  reportCrash(info("rejection", "Cannot read properties of undefined", { time: base.time }));
  reportCrash(info("render", "Cannot read properties of undefined", { time: base.time + 10 }));
  assert.equal(getCrash()?.kind, "render");
  assert.equal(getCrash()?.message, "Cannot read properties of undefined");
});

test("subscribeCrash notifies the kept crash and unsubscribe works", () => {
  clearCrash();
  const seen: string[] = [];
  const stop = subscribeCrash((c) => seen.push(c.message));
  reportCrash(info("error", "one"));
  reportCrash(info("error", "two"));
  assert.deepEqual(seen, ["one"]);
  stop();
  clearCrash();
  reportCrash(info("error", "three"));
  assert.deepEqual(seen, ["one"]);
});

test("crashDebugText includes message, stacks, url and the log tail", () => {
  const crash = info("render", "cannot read length", {
    componentStack: "\n    at SkillBody\n    at App",
  });
  const logs = Array.from({ length: CRASH_LOG_TAIL + 5 }, (_, i) => ({
    time: base.time + i,
    level: i % 2 ? "warn" : "info",
    text: `line ${i}`,
  }));
  const text = crashDebugText(crash, logs);
  assert.match(text, /kind: render/);
  assert.match(text, /url: https:\/\/game\.example\/play\/1/);
  assert.match(text, /ua: TestBrowser\/1\.0/);
  assert.match(text, /message: cannot read length/);
  assert.match(text, /stack:\nError: cannot read length/);
  assert.match(text, /componentStack:\n\s+at SkillBody/);
  // Tail only: the oldest overflow lines are dropped.
  assert.match(text, /line 5\n/);
  assert.doesNotMatch(text, /line 4\n/);
  assert.match(text, /\[warn\] line 29/);
});

test("crashDebugText survives missing optional fields and empty logs", () => {
  const crash = info("rejection", "bare", { stack: undefined, componentStack: undefined });
  const text = crashDebugText(crash, []);
  assert.match(text, /message: bare/);
  assert.doesNotMatch(text, /stack:/);
  assert.doesNotMatch(text, /componentStack:/);
  assert.doesNotMatch(text, /recent logs:/);
  assert.doesNotMatch(text, /undefined/);
});

test("the dedupe helper collapses a repeat of the same message", () => {
  clearCrash();
  assert.equal(isRepeatCrash("same boom", 1_000), false);
  // Inside the window after the first sighting.
  assert.equal(isRepeatCrash("same boom", 1_000 + CRASH_DEDUPE_MS - 1), true);
  // A different message always starts its own window.
  assert.equal(isRepeatCrash("other boom", 1_000 + CRASH_DEDUPE_MS - 1), false);
  assert.equal(isRepeatCrash("other boom", 1_000 + CRASH_DEDUPE_MS - 1 + CRASH_DEDUPE_MS - 1), true);
  // Past the window it is fresh again.
  assert.equal(isRepeatCrash("other boom", 1_000 + CRASH_DEDUPE_MS - 1 + CRASH_DEDUPE_MS + 1), false);
  // And reportCrash itself drops a burst of the same message.
  clearCrash();
  const seen: string[] = [];
  const stop = subscribeCrash((c) => seen.push(c.message));
  reportCrash(info("error", "burst", { time: base.time + 1 }));
  reportCrash(info("error", "burst", { time: base.time + 2 }));
  stop();
  assert.deepEqual(seen, ["burst"]);
});