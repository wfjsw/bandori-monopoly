import assert from "node:assert/strict";
import { test } from "node:test";
import { describeWorkerError } from "./workerError.ts";

test("bare Event keeps the worker url instead of [object Event]", () => {
  const s = describeWorkerError({ type: "error" }, new URL("http://h/assets/engine/bot-worker.js"));
  assert.match(s, /no message/);
  assert.match(s, /bot-worker\.js/);
  assert.doesNotMatch(s, /object Event/);
});

test("ErrorEvent fields are all kept", () => {
  const s = describeWorkerError({ message: "boom", filename: "http://h/w.js", lineno: 3, colno: 9, type: "error" });
  assert.equal(s, "boom at http://h/w.js:3:9 [error]");
});
