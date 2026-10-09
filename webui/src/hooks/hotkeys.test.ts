// Run with: node --test webui/src/hooks/hotkeys.test.ts
// The pure hotkey matcher behind `useHotkeys` (no DOM beyond the target tags).

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { matchHotkey, type Hotkey, type HotkeyEventLike } from "./pure.ts";

const fakeEl = (tagName: string, isContentEditable = false) => ({ tagName, isContentEditable });

const ev = (over: Partial<HotkeyEventLike> = {}): HotkeyEventLike => ({
  key: " ",
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  repeat: false,
  isComposing: false,
  target: null,
  ...over,
});

const hk = (over: Partial<Hotkey> = {}): Hotkey => ({ key: " ", run: () => {}, ...over });

test("matches by key and ignores other keys", () => {
  assert.equal(matchHotkey(hk(), ev()), true);
  assert.equal(matchHotkey(hk(), ev({ key: "x" })), false);
  assert.equal(matchHotkey(hk({ key: ["h", "H"] }), ev({ key: "H" })), true);
});

test("modifier chords pass only when the binding opts in", () => {
  assert.equal(matchHotkey(hk(), ev({ ctrlKey: true })), false, "ctrl+Space is the browser's");
  assert.equal(matchHotkey(hk({ mods: true }), ev({ ctrlKey: true })), true);
  assert.equal(matchHotkey(hk(), ev({ metaKey: true })), false);
  assert.equal(matchHotkey(hk(), ev({ altKey: true })), false);
});

test("typing in a field swallows the shortcut unless the binding opts in", () => {
  const input = fakeEl("INPUT");
  assert.equal(matchHotkey(hk(), ev({ target: input })), false);
  assert.equal(matchHotkey(hk({ typing: true }), ev({ target: input })), true);
  const div = fakeEl("DIV", true);
  assert.equal(matchHotkey(hk(), ev({ target: div })), false, "contentEditable counts as typing");
});

test("Space leaves a focused button its click", () => {
  const button = fakeEl("BUTTON");
  assert.equal(matchHotkey(hk(), ev({ target: button })), true, "default keeps controls in");
  assert.equal(matchHotkey(hk({ onControl: false }), ev({ target: button })), false, "Space is handed back to the button");
  // Arrows simply do not set `onControl: false`, so they still seek on.
  assert.equal(matchHotkey(hk({ key: "ArrowLeft" }), ev({ key: "ArrowLeft", target: button })), true);
});

test("held-key repeat is ignored only when the binding says once", () => {
  assert.equal(matchHotkey(hk({ key: "ArrowLeft" }), ev({ key: "ArrowLeft", repeat: true })), true, "arrows want the repeat");
  assert.equal(matchHotkey(hk({ key: "h", once: true }), ev({ key: "h", repeat: true })), false);
  assert.equal(matchHotkey(hk({ key: "h", once: true }), ev({ key: "h", repeat: false })), true);
});

test("an IME composition swallows every shortcut", () => {
  assert.equal(matchHotkey(hk(), ev({ isComposing: true })), false, "Space mid-composition is the IME's");
  assert.equal(matchHotkey(hk({ key: "ArrowLeft" }), ev({ key: "ArrowLeft", isComposing: true })), false);
});

test("the first matching binding wins (useHotkeys loop order)", () => {
  // Documented contract: a more specific binding is listed first.
  const order = [hk({ key: "h", once: true }), hk({ key: ["h", "H"] })];
  assert.equal(order.find((b) => matchHotkey(b, ev({ key: "h" }))), order[0], "the first of two hits wins");
  assert.equal(order.find((b) => matchHotkey(b, ev({ key: "h", repeat: true }))), order[1], "and a held key falls through to the next");
});