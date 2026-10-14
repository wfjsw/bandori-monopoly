// Deed-popup tile-text winner picking (`docs/TILES.md` 「Tile text」). Run with:
//   node --test webui/src/scenes/board/tileText.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { winningTileText } from "./tileText.ts";
import type { TileTextEntry } from "../../core/types.ts";

function e(from: string, slot: string, key: string): TileTextEntry {
  return { from, slot, key };
}

test("missing list → empty result", () => {
  assert.deepEqual(winningTileText(undefined), {});
});

test("empty list → empty result", () => {
  assert.deepEqual(winningTileText([]), {});
});

test("replacements before base: first wins", () => {
  const entries = [
    e("event:协助CiRCLE重建", "settle", "deed.note.rebuildSettle"),
    e("event:协助CiRCLE重建", "pass", ""),
    e("tile:circle", "settle", "deed.note.circleSettle"),
    e("tile:circle", "pass", "deed.note.circlePass"),
  ];
  assert.deepEqual(winningTileText(entries), {
    settle: "deed.note.rebuildSettle",
    // The empty key on the replacement suppresses the pass slot; the base
    // `tile:circle` cannot re-claim it.
    // pass: suppressed
  });
});

test("first claim wins even when a later speaker has a real key", () => {
  const entries = [
    e("event:协助CiRCLE重建", "pass", ""),
    e("tile:circle", "pass", "deed.note.circlePass"),
  ];
  assert.deepEqual(winningTileText(entries), {});
});

test("empty key suppresses (renders nothing)", () => {
  const entries = [e("tile:circle", "settle", "")];
  assert.deepEqual(winningTileText(entries), {});
});

test("base texts win when nothing replaces them", () => {
  const entries = [
    e("tile:circle", "settle", "deed.note.circleSettle"),
    e("tile:circle", "pass", "deed.note.circlePass"),
  ];
  assert.deepEqual(winningTileText(entries), {
    settle: "deed.note.circleSettle",
    pass: "deed.note.circlePass",
  });
});

test("a replacement that does not declare a slot has no opinion", () => {
  const entries = [
    e("card:Parking", "settle", "deed.note.dealer"),
    e("tile:circle", "settle", "deed.note.circleSettle"),
    e("tile:circle", "pass", "deed.note.circlePass"),
  ];
  assert.deepEqual(winningTileText(entries), {
    settle: "deed.note.dealer",
    pass: "deed.note.circlePass",
  });
});

test("unknown slots are ignored", () => {
  const entries = [
    e("tile:event", "settle", "deed.note.drawEvent"),
    e("tile:event", "other", "deed.note.draw"),
  ];
  assert.deepEqual(winningTileText(entries), { settle: "deed.note.drawEvent" });
});