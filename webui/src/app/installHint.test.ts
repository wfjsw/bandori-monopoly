// Run with: node --test webui/src/app/installHint.test.ts
// The install cover is a gate, not chrome: it must fire only on a phone at
// the rotate-cover moment (never a desktop or tablet -- "not standalone" is
// true there too, and a small tablet can even pass the rotate query, so both
// gates matter), it must sit in front of the rotate cover rather than beside
// it, and the user's override must clear it for good. These checks pin that
// decision table so a broader trigger cannot leak the prompt onto desktops
// or tablets.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { showInstallHint, showRotateHint } from "./installGate.ts";

const phonePortraitTab = { phone: true, portrait: true, standalone: false, dismissed: false };

test("install cover: phone browser tab at the rotate-cover moment", () => {
  assert.equal(showInstallHint(phonePortraitTab), true);
});

test("install cover: never on a desktop or tablet", () => {
  // Both are just as "not standalone" as a handset, and a small tablet can
  // even match the rotate query -- the phone gate is what keeps them clean.
  assert.equal(showInstallHint({ ...phonePortraitTab, phone: false }), false);
});

test("install cover: never inside an installed PWA", () => {
  assert.equal(showInstallHint({ ...phonePortraitTab, standalone: true }), false);
});

test("install cover: the override clears it for good", () => {
  assert.equal(showInstallHint({ ...phonePortraitTab, dismissed: true }), false);
});

test("install cover: lands on the rotate-cover moment only", () => {
  // Landscape is either already playing or a real turn of the phone -- no
  // full-screen cover should drop over the board there.
  assert.equal(showInstallHint({ ...phonePortraitTab, portrait: false }), false);
});

test("install cover sits in front of the rotate cover", () => {
  // Both would show; the rotate cover steps aside until the install one is done.
  const installCover = showInstallHint(phonePortraitTab);
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

test("rotate cover: stays down in landscape once install is done", () => {
  assert.equal(
    showRotateHint({ portrait: false, forced: false, installCover: false }),
    false,
  );
});