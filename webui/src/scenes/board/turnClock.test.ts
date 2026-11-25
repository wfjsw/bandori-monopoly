// Run with: node --test webui/src/scenes/board/turnClock.test.ts
// The acting seat's clock: bonus drains first, and within a turn the count
// only moves down -- refills (an effect granting time) excepted (`./turnClock`).

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { clockText, freshHold, turnClock, type TurnClockInput } from "./turnClock.ts";
import type { MatchPlayer, MatchState } from "../../core/types.ts";

const seat = (ai = false) => ({ ai, bankrupt: false, left: false }) as unknown as MatchPlayer;

function st(over: Partial<MatchState> = {}): MatchState {
  return {
    phase: "play", turn: 0, round: 1, step: 2, busy: false, prompt: { id: 0 },
    baseTime: 15, bonusTime: 12, clockPaused: false,
    players: [seat(), seat()],
    ...over,
  } as unknown as MatchState;
}

/** An input plus its own cross-render memory. */
function input(o: Partial<TurnClockInput> = {}): TurnClockInput {
  return {
    state: st(), clock: null, asking: false, solo: false, elapsed: 0, held: false, frozen: null, hold: freshHold(), ...o,
  };
}

/** Run a sequence of frames through one hold, returning each printed second. */
function frames(states: Partial<TurnClockInput>[]): (string | null)[] {
  const hold = freshHold();
  return states.map((o) => {
    const t = turnClock({ ...input(o), hold });
    return t && clockText(t);
  });
}

test("the pool drains bonus first, from the published deadlines", () => {
  const now = performance.now();
  const clock = { baseDeadlineMs: now + 25_000, bonusDeadlineMs: now + 10_000 };
  const t = turnClock(input({ clock }));
  assert.equal(t?.captionKey, "board.bonusTime");
  assert.equal(t?.tone, "", "an ordinary bonus ring");
  // +100 ms expiry epsilon: the display never races the server.
  assert.equal(t && clockText(t), "11");
});

test("an emptied bonus hands the ring to the base pool, then to 0", () => {
  const now = performance.now();
  const base = turnClock(input({ clock: { baseDeadlineMs: now + 15_000, bonusDeadlineMs: now } }));
  assert.equal(base?.captionKey, "board.baseTime");
  assert.equal(base?.tone, "base");
  const spent = turnClock(input({ clock: { baseDeadlineMs: now, bonusDeadlineMs: now } }));
  assert.equal(spent && clockText(spent), "0");
});

test("re-stamped deadlines within a turn never raise the count", () => {
  // Every frame re-anchors the deadline to `now + pool`; a raw read of that
  // would step the count back up whenever the client clock slips.
  const now = performance.now();
  const a = turnClock(input({ clock: { baseDeadlineMs: now + 20_000, bonusDeadlineMs: now + 5_000 } }));
  assert.equal(a && clockText(a), "6");
  const b = turnClock({ ...input({ clock: { baseDeadlineMs: now + 20_400, bonusDeadlineMs: now + 5_400 } }), hold: input().hold });
  assert.ok(Number(b && clockText(b)) <= 6);
});

test("the ring drains: a frame's pool is not re-read as the full mark", () => {
  const now = performance.now();
  const hold = freshHold();
  // The frame's pool is the deadline's remaining: that is how the server
  // builds it (`deadline = now + pool`).
  const at = (left: number) =>
    turnClock({
      ...input({ state: st({ bonusTime: left / 1000 }), clock: { baseDeadlineMs: now + 20_000, bonusDeadlineMs: now + left } }),
      hold,
    });
  const first = at(10_000);
  assert.equal(first?.frac, 1, "10 s of 10 s left");
  const later = at(4_000);
  assert.ok(later && later.frac < 0.5, `4 s left must not read as a full ring (got ${later?.frac})`);
});

test("a held frame that already carries the next seat keeps the shown value", () => {
  // The refill for the next turn lands before the hold; the ring still sits on
  // the seat that finished, so its fresh 30 must not appear yet.
  const hold = freshHold();
  const t0 = turnClock({ ...input({ state: st({ bonusTime: 7 }) }), hold });
  assert.equal(t0 && clockText(t0), "7");
  const held = st({ turn: 1, bonusTime: 30, baseTime: 15, clockPaused: true });
  const t = turnClock({ ...input({ state: st({ bonusTime: 7 }), held: true, frozen: held }), hold });
  assert.equal(t && clockText(t), "7", "no reset to the next seat's refill");
});

test("a next-seat paused frame without a client hold keeps it too", () => {
  const hold = freshHold();
  turnClock({ ...input({ state: st({ clockPaused: true, bonusTime: 7 }) }), hold });
  const pausedNext = st({ turn: 1, bonusTime: 30, clockPaused: true });
  const t = turnClock({ ...input({ state: st({ clockPaused: true, bonusTime: 7 }), frozen: pausedNext }), hold });
  assert.equal(t && clockText(t), "7");
});

test("the turn catches up: the next seat's refilled pool is adopted", () => {
  // Same hold, now drawn on the new seat: the ring moved, so its full clock.
  const out = frames([
    { state: st({ bonusTime: 7 }) },
    { state: st({ turn: 1, bonusTime: 30 }), frozen: st({ turn: 1, bonusTime: 30 }) },
  ]);
  assert.deepEqual(out, ["7", "30"]);
});

test("an effect granting bonus time is allowed to raise the count", () => {
  const out = frames([
    { state: st({ bonusTime: 4 }) },
    { state: st({ bonusTime: 4, clockPaused: true }) },
    // effect: +10 s, same seat, same turn
    { state: st({ bonusTime: 14, clockPaused: true }) },
  ]);
  assert.deepEqual(out, ["4", "4", "14"]);
});

test("a settle or an open prompt freezes the no-deadline count", () => {
  const out = frames([
    { state: st({ bonusTime: 12 }), elapsed: 5 },
    { state: st({ bonusTime: 12, busy: true }), elapsed: 6 },
    { state: st({ bonusTime: 12, busy: true }), elapsed: 9, asking: true },
    { state: st({ bonusTime: 10 }), elapsed: 4 }, // a new frame re-stamps `at`
  ]);
  assert.deepEqual(out, ["7", "7", "7", "6"]);
});

test("a drained turn shows its base pool, never a restart", () => {
  const out = frames([
    { state: st({ bonusTime: 9 }) },
    { state: st({ bonusTime: 0, baseTime: 14 }) },
  ]);
  assert.deepEqual(out, ["9", "14"]);
});

test("the bonus handing over to the base is not capped by the last bonus second", () => {
  // The base's own maximum may already be seeded from an earlier base phase,
  // so this is not a `grew` -- the pool kind is what allows the jump.
  const out = frames([
    { state: st({ bonusTime: 1, baseTime: 15 }) },
    { state: st({ bonusTime: 0, baseTime: 15 }) },
  ]);
  assert.deepEqual(out, ["1", "15"]);
});

test("a past deadline recovers once a real deadline arrives again", () => {
  const now = performance.now();
  const out = frames([
    { clock: { baseDeadlineMs: now, bonusDeadlineMs: now } },
    { clock: { baseDeadlineMs: now + 20_000, bonusDeadlineMs: now + 5_000 } },
  ]);
  assert.deepEqual(out, ["0", "6"]);
});

test("with no shown value yet, a next-seat held frame falls back to its pools", () => {
  const held = st({ turn: 1, bonusTime: 30, clockPaused: true });
  const t = turnClock(input({ state: st(), held: true, frozen: held }));
  assert.equal(t && clockText(t), "30");
});

test("a same-seat held frame reads the newest pool (bonus, then base)", () => {
  const hold = freshHold();
  const frozen = st({ bonusTime: 7, clockPaused: true });
  const t = turnClock({ ...input({ held: true, frozen }), hold });
  assert.equal(t && clockText(t), "7");
  const drained = st({ bonusTime: 0, baseTime: 14, clockPaused: true });
  const b = turnClock({ ...input({ held: true, frozen: drained }), hold });
  assert.equal(b?.captionKey, "board.baseTime");
});

test("no clock for a machine seat, solo, or outside the play phase", () => {
  assert.equal(turnClock(input({ state: st({ players: [seat(true), seat()] }) })), null);
  assert.equal(turnClock(input({ solo: true })), null);
  assert.equal(turnClock(input({ state: st({ phase: "ended" }) })), null);
  assert.equal(turnClock(input({ state: st({ turn: -1 }) })), null);
});

test("without a deadline the shown pool interpolates from the frame's arrival", () => {
  // Replay / older server path: local elapsed, never negative.
  const t = turnClock(input({ state: st({ bonusTime: 12 }), elapsed: 5 }));
  assert.equal(t && clockText(t), "7");
  const base = turnClock(input({ state: st({ bonusTime: 4, baseTime: 15 }), elapsed: 9 }));
  assert.equal(base?.captionKey, "board.baseTime");
  assert.equal(base?.tone, "base");
  assert.equal(base && clockText(base), "10");
});
