// Run with: node --test webui/src/app/installHint.test.ts
// The install cover is a gate, not chrome: it must fire only on a phone
// browser tab (never a desktop or tablet -- "not standalone" is true there
// too, so the phone gate is the only thing holding it back), in either
// orientation, it must sit in front of the rotate cover rather than beside
// it, and the user's override must be able to clear it for good. These
// checks pin that decision table so a broader trigger cannot leak the
// prompt onto desktops or tablets.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { showInstallHint, showRotateHint } from "./installGate.ts";

const phoneTab = { phone: true, standalone: false, dismissed: false };

test("install cover: phone browser tab, either orientation", () => {
  // The table has no orientation field on purpose -- a handset already
  // turned sideways is still a tab worth installing -- so no portrait-only
  // filter can creep back in through the env shape.
  assert.equal(showInstallHint(phoneTab), true);
  assert.deepEqual(Object.keys(phoneTab).sort(), ["dismissed", "phone", "standalone"]);
});

test("install cover: never on a desktop or tablet", () => {
  // Both are just as "not standalone" as a handset -- the phone gate is the
  // only thing keeping the cover off them.
  assert.equal(showInstallHint({ ...phoneTab, phone: false }), false);
});

test("install cover: never inside an installed PWA", () => {
  assert.equal(showInstallHint({ ...phoneTab, standalone: true }), false);
});

test("install cover: the override clears it for good", () => {
  assert.equal(showInstallHint({ ...phoneTab, dismissed: true }), false);
});

test("install cover sits in front of the rotate cover", () => {
  // Both would show; the rotate cover steps aside until the install one is done.
  const installCover = showInstallHint(phoneTab);
  assert.equal(installCover, true);
  assert.equal(
    showRotateHint({ portrait: true, forced: false, installCover }),
    false,
  );
});

test("rotate cover returns once the install cover is gone", () => {
  assert.equal(
    showRotateHint({ portrait: true, forced: false, installCover: false }),
    true,
  );
});

test("rotate cover: forced rotation still wins", () => {
  // A user who already asked for the forced turn must not be bounced back.
  assert.equal(
    showRotateHint({ portrait: true, forced: true, installCover: false }),
    false,
  );
});

test("rotate cover: stays down in landscape", () => {
  assert.equal(
    showRotateHint({ portrait: false, forced: false, installCover: false }),
    false,
  );
});