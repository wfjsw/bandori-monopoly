// Outside-click deselect (`pick.ts`). Run with:
//   node --test webui/src/hooks/pick.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { isPickedSlot, PICK_ATTR } from "./pick.ts";

/** A minimal element chain (no jsdom): `closest` walks `parentElement`. */
function el(pick: string | null, parent: unknown = null): HTMLElement {
  const node = {
    dataset: pick != null ? { pick } : {},
    parentElement: parent,
    closest(sel: string) {
      let cur: { dataset: { pick?: string }; parentElement: unknown } | null = node as never;
      while (cur) {
        if (sel === `[${PICK_ATTR}]` && cur.dataset.pick != null) return cur;
        cur = cur.parentElement as typeof cur;
      }
      return null;
    },
  };
  return node as unknown as HTMLElement;
}

test("the picked slot's own target is not elsewhere", () => {
  const child = el(null, el("c7"));
  assert.equal(isPickedSlot(child, "c7"), true);
});

test("a different slot's target is elsewhere", () => {
  const inner = el("c8", el("c7"));
  assert.equal(isPickedSlot(inner, "c7"), false);
  assert.equal(isPickedSlot(inner, "c8"), true);
});

test("a bare element is elsewhere", () => {
  assert.equal(isPickedSlot(el(null), "c7"), false);
});

test("null / non-element targets are elsewhere", () => {
  assert.equal(isPickedSlot(null, "c7"), false);
  assert.equal(isPickedSlot({} as EventTarget, "c7"), false);
});