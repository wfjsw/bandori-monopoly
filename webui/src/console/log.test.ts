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

test("log snapshots stay stable while IDs remain unique across clearing", () => {
  clearLogs();
  const empty = getLogs();
  assert.equal(getLogs(), empty);
  appendLog("command", "console", "> status");
  const first = getLogs();
  assert.equal(empty.length, 0);
  assert.equal(getLogs(), first);
  appendLog("result", "console", { round: 2 });
  assert.equal(first.length, 1);
  assert.equal(getLogs().length, 2);
  const lastId = getLogs().at(-1)!.id;
  clearLogs();
  assert.deepEqual(getLogs(), []);
  assert.equal(first[0].text, "> status");
  appendLog("info", "game", "new scene");
  assert.ok(getLogs()[0].id > lastId);
});

test("the bounded log retains exactly the newest entries in order", () => {
  clearLogs();
  for (let index = 0; index < LOG_LIMIT + 3; index++) appendLog("info", "game", index);
  assert.deepEqual(getLogs().map((entry) => entry.text), Array.from({ length: LOG_LIMIT }, (_, index) => String(index + 3)));
  const ids = getLogs().map((entry) => entry.id);
  assert.equal(new Set(ids).size, LOG_LIMIT);
  assert.ok(ids.every((id, index) => index === 0 || id > ids[index - 1]));
});

test("formatting preserves primitive values and marks truncated text", () => {
  for (const [value, expected] of [[null, "null"], [false, "false"], [0, "0"], [1n, '"1n"'], [Symbol("test"), "Symbol(test)"]] as const) {
    assert.equal(formatValue(value), expected);
  }
  clearLogs();
  appendLog("log", "browser", "a".repeat(12_000));
  assert.equal(getLogs()[0].text, "a".repeat(12_000));
  appendLog("log", "browser", "b".repeat(12_001));
  assert.equal(getLogs()[1].text, "b".repeat(12_000) + "\n…");
  appendLog("log", "browser", "value", 0, false, null);
  assert.equal(getLogs()[2].text, "value 0 false null");
});
