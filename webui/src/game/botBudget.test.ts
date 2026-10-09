// Budget policy for the advanced bot (`docs/BOT.md` §3.6, user ruling
// 2026-10-07). Run with:
//   node --test webui/src/game/botBudget.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import {
  EXPECTED_DECISIONS_PER_TURN,
  MAX_BUDGET_MS,
  MIN_BUDGET_MS,
  SAFETY_MARGIN_MS,
  SOLO_CAP_MS,
  decisionAt,
  decisionSeed,
  decideBudgetMs,
  nextTurnNear,
  outerDeadlineMs,
  ponderBudgetMs,
  seedForThread,
} from "./botBudget.ts";

// ---------------------------------------------------------------- solo cap

test("solo (no deadline) uses the generous configurable cap", () => {
  assert.equal(decideBudgetMs({ timed: false }), SOLO_CAP_MS);
  assert.equal(decideBudgetMs({ timed: false, soloCapMs: 1500 }), 1500);
  assert.equal(decideBudgetMs({ timed: false, soloCapMs: 60_000 }), MAX_BUDGET_MS);
  assert.equal(decideBudgetMs({ timed: false, soloCapMs: 1 }), MIN_BUDGET_MS);
});

// ---------------------------------------------------------------- timed

test("timed: prompt clock minus the safety margin, spread over the turn", () => {
  // 5 s on the prompt clock, first decision of a 3-decision turn -> 1/3.
  const ms = decideBudgetMs({ timed: true, promptTimeLeft: 5, turnTimeLeft: 30, decisionsThisTurn: 0 });
  assert.equal(ms, Math.round((5_000 - SAFETY_MARGIN_MS) / EXPECTED_DECISIONS_PER_TURN));
});

test("timed: the turn clock binds when it is tighter than the prompt clock", () => {
  const ms = decideBudgetMs({ timed: true, promptTimeLeft: 30, turnTimeLeft: 4, decisionsThisTurn: 0 });
  assert.equal(ms, Math.round((4_000 - SAFETY_MARGIN_MS) / EXPECTED_DECISIONS_PER_TURN));
});

test("timed: the turn surface uses the turn clock alone", () => {
  const ms = decideBudgetMs({ timed: true, promptTimeLeft: null, turnTimeLeft: 6, decisionsThisTurn: 0 });
  assert.equal(ms, Math.round((6_000 - SAFETY_MARGIN_MS) / EXPECTED_DECISIONS_PER_TURN));
});

test("timed: the budget is spread across the turn's expected decisions", () => {
  const one = decideBudgetMs({ timed: true, turnTimeLeft: 9, decisionsThisTurn: 0 });
  const two = decideBudgetMs({ timed: true, turnTimeLeft: 9, decisionsThisTurn: 1 });
  const three = decideBudgetMs({ timed: true, turnTimeLeft: 9, decisionsThisTurn: 2 });
  // User ruling: never spend it all on the first one -- 1/3, then 1/2 of the
  // rest, then the last share.
  assert.equal(one, Math.round((9_000 - SAFETY_MARGIN_MS) / 3));
  assert.equal(two, Math.round((9_000 - SAFETY_MARGIN_MS) / 2));
  assert.equal(three, Math.round((9_000 - SAFETY_MARGIN_MS) / 1));
  assert.ok(one < two && two < three, "later decisions get a bigger share of what is left");
});

test("timed: never below the floor or above the ceiling", () => {
  assert.equal(decideBudgetMs({ timed: true, promptTimeLeft: 0.2, turnTimeLeft: 30 }), MIN_BUDGET_MS);
  assert.equal(decideBudgetMs({ timed: true, turnTimeLeft: 600, decisionsThisTurn: 2 }), MAX_BUDGET_MS);
});

test("timed: a missing clock falls back to the cap rather than stalling", () => {
  assert.equal(decideBudgetMs({ timed: true, promptTimeLeft: null, turnTimeLeft: null }), SOLO_CAP_MS);
});

// ---------------------------------------------------------------- deadline

test("the outer deadline leaves room for one iteration overrun", () => {
  assert.equal(outerDeadlineMs(1_000), 1_000 + 2_000);
  assert.equal(outerDeadlineMs(12_000), 12_000 + 2_000);
  // Always wider than the budget itself.
  assert.ok(outerDeadlineMs(500) > 500);
});

// ---------------------------------------------------------------- decisionAt

const prompt = (id: number, players: number[], answers: number[]) => ({ id, players, answers });

test("decisionAt: setup phases are engine-side (no ask)", () => {
  const st = { phase: "deck", busy: false, turn: 1, step: 2, prompt: prompt(0, [], []) };
  assert.equal(decisionAt(st, 1), null);
});

test("decisionAt: an open prompt waiting on the seat", () => {
  const st = { phase: "play", busy: false, turn: 1, step: 2, prompt: prompt(7, [1, 2], [-1, -1]) };
  assert.equal(decisionAt(st, 1), 7);
  assert.equal(decisionAt(st, 2), 7);
  assert.equal(decisionAt(st, 3), null, "a seat the prompt is not asking");
});

test("decisionAt: a prompt the seat has already answered is closed", () => {
  const st = { phase: "play", busy: false, turn: 1, step: 2, prompt: prompt(7, [1], [0]) };
  assert.equal(decisionAt(st, 1), null);
});

test("decisionAt: the turn surface on 运营 / 结束", () => {
  const ops = { phase: "play", busy: false, turn: 1, step: 2, prompt: prompt(0, [], []) };
  assert.equal(decisionAt(ops, 1), 0, "my turn, 运营");
  assert.equal(decisionAt(ops, 2), null, "not my turn");
  const end = { phase: "play", busy: false, turn: 1, step: 4, prompt: prompt(0, [], []) };
  assert.equal(decisionAt(end, 1), 0, "my turn, 结束");
  const mid = { phase: "play", busy: false, turn: 1, step: 3, prompt: prompt(0, [], []) };
  assert.equal(decisionAt(mid, 1), null, "移动 is running");
  const busy = { phase: "play", busy: true, turn: 1, step: 2, prompt: prompt(0, [], []) };
  assert.equal(decisionAt(busy, 1), null, "mid-routine");
});

// ------------------------------------------------------- nextTurnNear / ponder

const seat = (bankrupt = false, left = false) => ({ bankrupt, left });

test("nextTurnNear: our turn is always near (the next surface is ours)", () => {
  const st = { phase: "play", busy: false, turn: 1, step: 2, prompt: prompt(0, [], []), players: [seat(), seat(), seat()] };
  assert.equal(nextTurnNear(st, 1), true);
  const mid = { ...st, busy: true, step: 3 };
  assert.equal(nextTurnNear(mid, 1), true, "mid-routine on our turn");
});

test("nextTurnNear: the current player's turn ending makes ours next", () => {
  const st = { phase: "play", busy: false, turn: 0, step: 4, prompt: prompt(0, [], []), players: [seat(), seat(), seat()] };
  assert.equal(nextTurnNear(st, 1), true, "结束 -- about to hand over");
  assert.equal(nextTurnNear(st, 2), true, "also near for the seat after us");
});

test("nextTurnNear: the next active seat in the ring", () => {
  const st = { phase: "play", busy: false, turn: 0, step: 2, prompt: prompt(0, [], []), players: [seat(), seat(), seat()] };
  assert.equal(nextTurnNear(st, 1), true, "we are next");
  assert.equal(nextTurnNear(st, 2), false, "two seats away -- not near");
  // Out seats are skipped in the ring.
  const withOut = { ...st, players: [seat(), seat(true), seat()] };
  assert.equal(nextTurnNear(withOut, 2), true, "the out seat is skipped, so 2 is next");
});

test("nextTurnNear: setup phases and a dead match are never near", () => {
  const st = { phase: "deck", busy: false, turn: 1, step: 2, prompt: prompt(0, [], []), players: [seat(), seat(), seat()] };
  assert.equal(nextTurnNear(st, 1), false);
});

test("ponderBudgetMs is a slice of the decision budget, never a runaway", () => {
  const solo = ponderBudgetMs({ timed: false });
  assert.equal(solo, Math.round(SOLO_CAP_MS / 3));
  assert.ok(ponderBudgetMs({ timed: false, soloCapMs: 8_000 }) <= 1_000, "capped");
  assert.ok(ponderBudgetMs({ timed: true, promptTimeLeft: 1, turnTimeLeft: 1 }) >= MIN_BUDGET_MS);
  assert.ok(ponderBudgetMs({ timed: true }) <= 1_000, "the server's speculative slice");
});

// ---------------------------------------------------------------- seeds

test("decisionSeed is deterministic and separates identities", () => {
  const a = decisionSeed("solo", 2, 10, 0);
  assert.equal(a, decisionSeed("solo", 2, 10, 0), "same identity, same seed");
  assert.notEqual(a, decisionSeed("solo", 2, 11, 0), "different seq");
  assert.notEqual(a, decisionSeed("solo", 3, 10, 0), "different member");
  assert.notEqual(a, decisionSeed("R1", 2, 10, 0), "different room");
  assert.notEqual(a, decisionSeed("solo", 2, 10, 5), "different prompt");
});

test("seedForThread: worker 0 keeps the seed, the others split away", () => {
  const seed = 12345;
  assert.equal(seedForThread(seed, 0), seed >>> 0, "N=1 is the plain search");
  const s1 = seedForThread(seed, 1);
  const s2 = seedForThread(seed, 2);
  assert.notEqual(s1, seed);
  assert.notEqual(s1, s2);
  assert.equal(s1, seedForThread(seed, 1), "deterministic");
});

test("seedForThread matches bot_core::seed_for_thread (BigInt splitmix64)", () => {
  // The reference values come from `bot_core::seed_for_thread` (the same
  // function `bot-glue` re-exports); if the TS port drifts, the root-parallel
  // merge would silently diverge from `search_root_parallel`.
  const cases: [number, number, number][] = [
    [0, 0, 0],
    [12345, 0, 12345],
    [12345, 1, 3033129047],
    [12345, 2, 459700571],
    [1, 3, 3506550201],
    [0xffffffff, 1, 898848118],
  ];
  for (const [seed, thread, want] of cases) {
    assert.equal(seedForThread(seed, thread), want >>> 0, `seed=${seed} thread=${thread}`);
  }
});

// ---------------------------------------------------------------- spread sanity

test("the spread keeps the turn inside its clock", () => {
  // A 12 s turn with the expected number of decisions: each share leaves the
  // safety margin, and the live clock advances by the share actually spent.
  let left = 12_000;
  for (let i = 0; i < EXPECTED_DECISIONS_PER_TURN; i++) {
    const ms = decideBudgetMs({ timed: true, turnTimeLeft: left / 1000, decisionsThisTurn: i });
    assert.ok(ms <= left - SAFETY_MARGIN_MS + 1, `decision ${i} leaves the margin`);
    left -= ms;
    assert.ok(left >= 0);
  }
  assert.ok(left <= SAFETY_MARGIN_MS + 1, "the margin is what is left over");
});