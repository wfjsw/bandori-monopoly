// Run with: node --test webui/src/scenes/board/returnsSelection.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { MatchPrompt } from "../../core/types.ts";
import { returnsPickCount, submitReturnsSelection } from "./returnsSelection.ts";

const pool = Array.from({ length: 11 }, (_, i) => `PPP:card${i}`);
function prompt(n: number, cards = pool): MatchPrompt {
  return {
    id: n, kind: "choice", title: { k: "cards:card-ppp.returns_add_eight_title" },
    text: { k: "cards:card-ppp.returns_add_eight_pick", a: { n: { i: n } } },
    options: cards.map((card) => ({ k: "ask.cardOption", a: { card: { card } } })),
    card: "", players: [0], answers: [-1], timeLeft: 15, bid: 0, bidder: -1,
    items: [], count: 0, price: -1, prices: [],
  };
}

function table(start = 1, delayed = false) {
  let p = prompt(start);
  const listeners = new Set<() => void>();
  const added: string[] = [];
  const commands: { prompt: number; value: number }[] = [];
  const source = {
    current: () => p,
    subscribe: (changed: () => void) => {
      listeners.add(changed);
      changed();
      return () => { listeners.delete(changed); };
    },
    answer: async (id: number, value: number) => {
      assert.equal(id, p.id, "always answer the newly published prompt");
      commands.push({ prompt: id, value });
      const cards = p.options.map((o) => (o.a!.card as { card: string }).card);
      added.push(cards.splice(value, 1)[0]);
      const publish = () => {
        p = id < 8 ? prompt(id + 1, cards) : { ...prompt(9, []), text: { k: "ask.mulligan.text" } };
        listeners.forEach((changed) => changed());
      };
      if (delayed) setTimeout(publish, 1); else publish();
      return true;
    },
  };
  return { source, added, commands, listeners };
}

test("one confirmation submits eight distinct cards despite shifting pool indices", async () => {
  const t = table();
  const selected = [pool[10], pool[0], pool[7], pool[1], pool[9], pool[2], pool[6], pool[3]];
  const progress: number[] = [];
  assert.equal(await submitReturnsSelection(t.source, selected, (n) => progress.push(n)), true);
  assert.deepEqual(t.added, selected);
  assert.equal(t.commands.length, 8);
  assert.deepEqual(progress, [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.equal(t.listeners.size, 0);
});

test("online submission waits for the next state after each acknowledgement", async () => {
  const t = table(1, true);
  assert.equal(await submitReturnsSelection(t.source, pool.slice(0, 8)), true);
  assert.deepEqual(t.added, pool.slice(0, 8));
  assert.equal(t.listeners.size, 0);
});

test("a partially saved setup picks only its remaining cards", async () => {
  const t = table(3);
  assert.equal(returnsPickCount(t.source.current()), 6);
  assert.equal(await submitReturnsSelection(t.source, pool.slice(0, 6)), true);
  assert.equal(t.commands.length, 6);
});

test("wrong counts, duplicates and unavailable cards send no answers", async () => {
  for (const cards of [pool.slice(0, 7), Array(8).fill(pool[0]), [...pool.slice(0, 7), "missing"]]) {
    const t = table();
    assert.equal(await submitReturnsSelection(t.source, cards), false);
    assert.equal(t.commands.length, 0);
  }
});

test("failed acknowledgements and missing SSE stop without leaving listeners", async () => {
  for (const accepted of [false, true]) {
    const t = table();
    const source = { ...t.source, answer: async () => accepted };
    assert.equal(await submitReturnsSelection(source, pool.slice(0, 8), undefined, 5), false);
    assert.equal(t.listeners.size, 0);
  }
});
