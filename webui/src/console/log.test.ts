import { strict as assert } from "node:assert";
import { test } from "node:test";
import { appendLog, clearLogs, formatValue, getLogs, LOG_LIMIT, subscribeLogs } from "./log.ts";

test("log capture holds a bounded tail and snapshots values at log time", () => {
  clearLogs();
  const value = { money: 1 };
  appendLog("log", "browser", value);
  value.money = 99;
  assert.match(getLogs()[0].text, /1/);
  for (let i = 0; i < LOG_LIMIT + 10; i++) appendLog("info", "game", i);
  assert.equal(getLogs().length, LOG_LIMIT);
  assert.equal(getLogs().at(-1)?.text, String(LOG_LIMIT + 9));
  appendLog("error", "browser", "x".repeat(100_000));
  assert.ok(getLogs().at(-1)!.text.length < 12_010);
});

test("errors, circular objects and bigint are safe to inspect", () => {
  const value: { self?: unknown; n: bigint } = { n: 1n };
  value.self = value;
  assert.match(formatValue(value), /Circular/);
  assert.match(formatValue(value), /1n/);
  assert.match(formatValue(new Error("failed")), /failed/);
  assert.equal(formatValue(undefined), "undefined");
  assert.equal(formatValue({ toJSON() { throw new Error("getter failed"); } }), "[Unserializable]");
});

test("listeners are deferred and batched, and can detach", async () => {
  let called = 0;
  const off = subscribeLogs(() => called++);
  appendLog("info", "console", "test");
  clearLogs();
  assert.equal(called, 0);
  await new Promise<void>((resolve) => queueMicrotask(resolve));
  assert.equal(called, 1);
  off();
  clearLogs();
  await new Promise<void>((resolve) => queueMicrotask(resolve));
  assert.equal(called, 1);
});
