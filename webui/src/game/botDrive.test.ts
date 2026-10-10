// Driving one seat with the advanced bot (`docs/BOT.md` B6): the fallback
// contract. Run with:
//   node --test webui/src/game/botDrive.test.ts
//
// The search itself is covered by `botPool.test.ts` against the real
// `bot-glue` bundle. This file pins the driver: a failed / timed-out search
// must fall back to the existing bot policy and the match must never stall (§1).

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { advancedSeats, driveSeat, type DriveHooks } from "./botDrive.ts";
import { decisionAt } from "./botBudget.ts";
import type { Command, MatchView, MatchState, MatchPlayer } from "../core/types.ts";
import type { Msg } from "../i18n/msg.ts";
import type { BotDecideResult, BotPool } from "./botPool.ts";

// ---------------------------------------------------------------- fixtures

function player(over: Partial<MatchPlayer> = {}): MatchPlayer {
  return {
    member: 1,
    player: "P",
    bot: false,
    ai: false,
    roll: 0,
    banDone: true,
    ban: "",
    character: "户山香澄",
    deckReady: true,
    money: 10_000,
    pos: 0,
    hand: 0,
    draw: 0,
    discard: [],
    mulligan: true,
    bankrupt: false,
    left: false,
    outOrder: 0,
    assets: 0,
    score: 0,
    rank: 0,
    mentality: "standard",
    state: {},
    skillCharacter: "",
    bands: "",
    tokens: [],
    skillNote: { k: "" },
    field: [],
    ...over,
  } as MatchPlayer;
}

function state(over: Partial<MatchState> = {}): MatchState {
  const players = over.players ?? [player(), player({ member: 2, player: "B1", bot: true, ai: true, mentality: "advanced" })];
  return {
    phase: "play",
    matchId: 1,
    seq: 10,
    mode: 0,
    turn: 0,
    round: 1,
    step: 2,
    debugOpen: false,
    roller: 0,
    busy: false,
    skipMove: false,
    landed: -1,
    thinkTime: 0,
    buyPrice: -1,
    buildCost: -1,
    plan: { playerId: 0, from: 0, steps: 0, started: false, reach: [], canBuild: false },
    timeLeft: 30,
    shield: 0,
    bank: 0,
    bought: false,
    built: false,
    players,
    bans: [],
    owners: [],
    houses: [],
    mortgaged: [],
    embers: [],
    marks: [],
    eventDeck: 0,
    eventTop: [],
    eventDiscard: [],
    eventActive: [],
    prompt: { id: 0, kind: "", title: { k: "" }, text: { k: "" }, card: "", options: [], fallback: 0, players: [], answers: [], timeLeft: 0, tile: -1, bid: 0, bidder: -1, items: [], count: 0, price: -1, prices: [] },
    vote: { id: 0, by: -1, players: [], answers: [], timeLeft: 0 },
    events: [],
    endReason: "",
    winner: -1,
    scoreMoney: 1,
    scoreProperty: 1,
    scoreHouses: 1,
    ...over,
    players,
  } as MatchState;
}

function view(over: Partial<MatchView> = {}): MatchView {
  return {
    state: over.state ?? state(),
    hand: over.hand ?? [],
    handNotes: [],
    draw: [],
    you: over.you ?? 1,
    playerId: over.playerId ?? 0,
    aiAnswer: over.aiAnswer ?? null,
    playable: over.playable ?? [],
    estCost: over.estCost ?? [],
    ...over,
    state: over.state ?? state(),
  } as MatchView;
}

/** A pool whose `decide` settles the way the test wants. */
function fakePool(decide: () => Promise<BotDecideResult>): BotPool {
  return {
    decide,
    ponder: async () => {},
    ensureLoaded: async () => {},
    dispose: () => {},
    workers: 1,
  } as unknown as BotPool;
}

function hooks(over: Partial<DriveHooks> = {}): DriveHooks & { sent: Command[] } {
  const sent: Command[] = [];
  return {
    sent,
    ctx: {
      tiles: [],
      characters: [],
      playedThisTurn: 0,
      deckPreset: () => [],
      deckSuggest: () => [],
      deckRandom: () => [],
      random: () => 0.5,
    },
    room: "solo",
    timed: false,
    ...over,
  };
}

function okAnswer(act: string, extra: Partial<BotDecideResult> = {}): BotDecideResult {
  return {
    answer: { act } as Command,
    iterations: 3,
    elapsedMs: 12,
    heuristic: false,
    reused: false,
    decisionKey: "00",
    rootStats: [],
    workers: 1,
    merged: false,
    ...extra,
  };
}

// ---------------------------------------------------------------- advancedSeats

test("advancedSeats: only held advanced bot seats", () => {
  const v = view({
    state: state({
      players: [
        player({ member: 1, bot: false, ai: false }),
        player({ member: 2, bot: true, ai: false, mentality: "advanced" }),
        player({ member: 3, bot: true, ai: true, mentality: "advanced" }), // engine-driven
        player({ member: 4, bot: true, ai: false, mentality: "standard" }), // not advanced
        player({ member: 5, bot: true, ai: false, mentality: "advanced", left: true }), // gone
      ],
    }),
  });
  assert.deepEqual(advancedSeats(v), [2]);
});

// ---------------------------------------------------------------- driveSeat

test("driveSeat applies the search's answer through act", async () => {
  const v = view();
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    { ...h, pool: fakePool(async () => okAnswer("buy")) },
  );
  assert.equal(outcome, "searched");
  assert.deepEqual(h.sent, [{ act: "buy" }]);
});

test("driveSeat answers a reused ponder from the cache", async () => {
  const v = view();
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    { ...h, pool: fakePool(async () => okAnswer("end", { reused: true })) },
  );
  assert.equal(outcome, "reused");
  assert.deepEqual(h.sent, [{ act: "end" }]);
});

test("driveSeat falls back to the existing bot policy when the pool throws", async () => {
  const v = view();
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    {
      ...h,
      pool: fakePool(async () => {
        throw new Error("every bot worker failed");
      }),
    },
  );
  assert.equal(outcome, "fallback");
  assert.equal(h.sent.length, 1, "the fallback still answers -- the match never stalls");
  assert.ok(h.sent[0].act, "a legal command");
});

test("driveSeat falls back when the search times out", async () => {
  const v = view();
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    {
      ...h,
      pool: fakePool(
        () =>
          new Promise((_, reject) => {
            setTimeout(() => reject(new Error("bot ask timed out")), 5);
          }),
      ),
    },
  );
  assert.equal(outcome, "fallback");
  assert.equal(h.sent.length, 1);
});

test("driveSeat falls back when the engine refuses the search's answer", async () => {
  const v = view();
  const h = hooks();
  let n = 0;
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      n++;
      // The search's answer is refused; the fallback is accepted.
      return n === 1 ? ({ k: "err.bad_act" } as Msg) : null;
    },
    () => v,
    1,
    { ...h, pool: fakePool(async () => okAnswer("buy")) },
  );
  assert.equal(outcome, "fallback");
  assert.equal(h.sent.length, 2, "search first, then the policy");
});

test("driveSeat: a seat with no decision is a no-op", async () => {
  // Setup phase: `decisionAt` is null.
  const v = view({ state: state({ phase: "deck" }) });
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    { ...h, pool: fakePool(async () => okAnswer("buy")) },
  );
  assert.equal(outcome, "fallback");
  assert.deepEqual(h.sent, [], "nothing to answer");
});

test("driveSeat reports the thinking indicator around the search", async () => {
  const v = view();
  const seen: [number, boolean][] = [];
  await driveSeat(
    async () => null,
    () => v,
    1,
    {
      ...hooks(),
      pool: fakePool(async () => okAnswer("end")),
      onThinking: (m, on) => seen.push([m, on]),
    },
  );
  assert.deepEqual(seen, [
    [1, true],
    [1, false],
  ]);
});

test("driveSeat: a pool that never loads still answers via the policy", async () => {
  const v = view();
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    {
      ...h,
      pool: fakePool(async () => {
        throw new Error("bot pool disposed");
      }),
    },
  );
  assert.equal(outcome, "fallback");
  assert.equal(h.sent.length, 1);
});

// ---------------------------------------------------------------- decisionAt sanity

test("the driver only fires where decisionAt says so", () => {
  const atTurn = view();
  assert.notEqual(decisionAt(atTurn.state, atTurn.playerId), null);
  const notMyTurn = view({ state: state({ turn: 1 }) });
  assert.equal(decisionAt(notMyTurn.state, notMyTurn.playerId), null);
});