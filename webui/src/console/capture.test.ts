import { strict as assert } from "node:assert";
import { test } from "node:test";
import { clearLogs, getLogs, installLogCapture } from "./log.ts";

test("browser capture keeps native output and records runtime failures", async (t) => {
  // A native EventTarget exercises listener registration/dispatch without a DOM.
  const browser = new EventTarget();
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", { configurable: true, value: browser });
  const levels = ["log", "info", "warn", "error", "debug"] as const;
  const native = new Map(levels.map((level) => [level, t.mock.method(console, level, () => {})]));
  t.after(() => {
    t.mock.restoreAll();
    if (originalWindow) Object.defineProperty(globalThis, "window", originalWindow);
    else Reflect.deleteProperty(globalThis, "window");
    clearLogs();
  });
  installLogCapture();

  await t.test("all native console methods retain their arguments and receiver", () => {
    clearLogs();
    const value = { money: 50 };
    for (const level of levels) {
      console[level]("value", value);
      const calls = native.get(level)!.mock.calls;
      assert.equal(calls.length, 1);
      assert.deepEqual(calls[0].arguments, ["value", value]);
      assert.equal(calls[0].this, console);
    }
    assert.deepEqual(getLogs().map(({ source, level }) => ({ source, level })),
      levels.map((level) => ({ source: "browser", level: level === "debug" ? "log" : level })));
    value.money = 99;
    assert.ok(getLogs().every((entry) => entry.text.includes('"money": 50')));
  });

  await t.test("installing capture twice never duplicates methods or listeners", () => {
    clearLogs();
    installLogCapture();
    console.warn("one warning");
    const event = new Event("error");
    Object.defineProperty(event, "error", { value: new Error("one error") });
    browser.dispatchEvent(event);
    assert.equal(native.get("warn")!.mock.calls.length, 2);
    assert.equal(getLogs().length, 2);
    assert.equal(getLogs()[0].text, "one warning");
    assert.match(getLogs()[1].text, /one error/);
  });

  await t.test("uncaught Error objects retain the message and stack", () => {
    clearLogs();
    const error = new Error("render failed");
    const event = new Event("error");
    Object.defineProperty(event, "error", { value: error });
    browser.dispatchEvent(event);
    assert.equal(getLogs().length, 1);
    assert.equal(getLogs()[0].source, "browser");
    assert.equal(getLogs()[0].level, "error");
    assert.equal(getLogs()[0].text, error.stack);
  });

  await t.test("fallback error locations and promise rejection reasons are captured", () => {
    clearLogs();
    const error = new Event("error");
    for (const [field, value] of Object.entries({ message: "load failed", filename: "game.js", lineno: 4, colno: 7 })) {
      Object.defineProperty(error, field, { value });
    }
    browser.dispatchEvent(error);
    const rejection = new Event("unhandledrejection");
    Object.defineProperty(rejection, "reason", { value: { code: "offline", retry: 1n } });
    browser.dispatchEvent(rejection);
    assert.equal(getLogs()[0].text, "load failed (game.js:4:7)");
    assert.match(getLogs()[1].text, /offline/);
    assert.match(getLogs()[1].text, /1n/);
    assert.ok(getLogs().every((entry) => entry.source === "browser" && entry.level === "error"));
  });
});
