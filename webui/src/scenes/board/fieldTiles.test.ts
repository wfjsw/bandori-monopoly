// Field-row tile bindings (`fieldTiles.ts`): the hold-to-highlight is attached
// only when this returns a non-empty list. Run with:
//   node --test webui/src/scenes/board/fieldTiles.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { designatedTiles, eventTiles, fieldCardTiles } from "./fieldTiles.ts";
import type { TileMark } from "../../core/types.ts";

function mark(partial: Partial<TileMark>): TileMark {
  return {
    uid: 1,
    tile: 0,
    kind: "mark:designate",
    category: "designate",
    owner: 0,
    count: 1,
    card: "",
    src: -1,
    instance: -1,
    note: { k: "" },
    ...partial,
  };
}

test("no marks → no tiles (the hold effect stays off)", () => {
  assert.deepEqual(designatedTiles(undefined, fieldCardTiles(7)), []);
  assert.deepEqual(designatedTiles([], fieldCardTiles(7)), []);
});

test("a field card binds its own instance's designate marks only", () => {
  const marks = [
    mark({ tile: 3, src: 7 }),
    mark({ tile: 5, instance: 7 }),
    mark({ tile: 9, src: 8 }),
    mark({ tile: 11, src: 7, category: "cp" }),
  ];
  assert.deepEqual(designatedTiles(marks, fieldCardTiles(7)), [3, 5]);
  assert.deepEqual(designatedTiles(marks, fieldCardTiles(8)), [9]);
});

test("one tile twice is listed once", () => {
  const marks = [mark({ tile: 4, src: 2 }), mark({ tile: 4, instance: 2 })];
  assert.deepEqual(designatedTiles(marks, fieldCardTiles(2)), [4]);
});

test("an event binds by rule id or bare roster id", () => {
  const marks = [
    mark({ tile: 1, card: "event:修复公告", src: 40 }),
    mark({ tile: 2, card: "修复公告", src: 41 }),
    mark({ tile: 3, card: "event:别的", src: 42 }),
  ];
  assert.deepEqual(designatedTiles(marks, eventTiles("修复公告")), [1, 2]);
  assert.deepEqual(designatedTiles(marks, eventTiles("event:修复公告")), [1, 2]);
  assert.deepEqual(designatedTiles(marks, eventTiles("别的")), [3]);
  assert.deepEqual(designatedTiles(marks, eventTiles("没有的")), []);
});