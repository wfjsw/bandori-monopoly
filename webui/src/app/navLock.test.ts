// Run with: node --test webui/src/app/navLock.test.ts
// The arm / re-arm rules behind the match's navigation lock.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { isNavLockEntry, isNavLockFor, rearmNavLock, type NavLockHistory, type NavLockEntry } from "./navLock.ts";

function fakeHistory(state: unknown = null): NavLockHistory & { pushed: { state: unknown; url: string | null | undefined }[] } {
  const pushed: { state: unknown; url: string | null | undefined }[] = [];
  return {
    state,
    pushed,
    pushState(data, _unused, url) {
      pushed.push({ state: data, url });
      (this as { state: unknown }).state = data;
    },
  };
}

const lock = (path: string): NavLockEntry => ({ __navLock: 1, path });

test("arming pushes a sentinel when the top entry is a real screen", () => {
  const h = fakeHistory(null);
  rearmNavLock(h, { pathname: "/play/solo" }, "/play/solo");
  assert.equal(h.pushed.length, 1);
  assert.deepEqual(h.pushed[0]?.state, lock("/play/solo"));
  assert.equal(h.pushed[0]?.url, "/play/solo");
  assert.equal(isNavLockEntry(h.state), true);
});

test("arming is a no-op when the sentinel already tops the same path", () => {
  const h = fakeHistory(lock("/play/solo"));
  rearmNavLock(h, { pathname: "/play/solo" }, "/play/solo");
  assert.equal(h.pushed.length, 0, "a second sentinel would leave a dead Back later");
});

test("a pop back to the real entry re-arms the sentinel", () => {
  // Back popped the sentinel: same path, but the top is the real entry again.
  const h = fakeHistory(null);
  rearmNavLock(h, { pathname: "/play/solo" }, "/play/solo");
  assert.equal(h.pushed.length, 1);
});

test("a pop onto another route is pulled back to the locked path", () => {
  const h = fakeHistory(null);
  rearmNavLock(h, { pathname: "/menu" }, "/play/solo");
  assert.equal(h.pushed.length, 1);
  assert.deepEqual(h.pushed[0]?.state, lock("/play/solo"));
  assert.equal(h.pushed[0]?.url, "/play/solo", "the restored entry must carry the locked path");
});

test("a sentinel for a different path is not mistaken for this one", () => {
  const h = fakeHistory(lock("/play/other"));
  rearmNavLock(h, { pathname: "/play/solo" }, "/play/solo");
  assert.equal(h.pushed.length, 1);
  assert.deepEqual(h.pushed[0]?.state, lock("/play/solo"));
});

test("isNavLockEntry only answers the sentinel shape", () => {
  assert.equal(isNavLockEntry(null), false);
  assert.equal(isNavLockEntry(undefined), false);
  assert.equal(isNavLockEntry({}), false);
  assert.equal(isNavLockEntry({ __navLock: 0, path: "/play/solo" }), false);
  assert.equal(isNavLockEntry({ __navLock: 1, path: "/play/solo" }), true);
});

test("isNavLockFor requires the sentinel to name this path", () => {
  assert.equal(isNavLockFor(lock("/play/solo"), "/play/solo"), true);
  assert.equal(isNavLockFor(lock("/play/other"), "/play/solo"), false);
  assert.equal(isNavLockFor(null, "/play/solo"), false);
});