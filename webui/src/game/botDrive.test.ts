// Driving one seat with the advanced bot (`docs/BOT.md` B6): the fallback
// contract. Run with:
//   node --test webui/src/game/botDrive.test.ts
//
// The search itself is covered by `botPool.test.ts` against the real
// `bot-glue` bundle. This file pins the driver: a failed / timed-out search
// must fall back to the existing bot policy and the match must never stall (§1).

import { strict as assert } from "node:assert";
import { test } from "node:test";

import {
  botSeats,
  decisionKey,
  decisionOf,
  driveSeat,
  needsAnswer,
  type Decision,
  type DriveHooks,
} from "./botDrive.ts";
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
    persona: "户山香澄",
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
    skillOwner: "",
    affiliations: "",
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
    canRollHere: false,
    canEndHere: true,
    plan: { playerId: 0, from: 0, steps: 0, started: false, reach: [], canBuild: false },
    timeLeft: 30,
    baseTime: 0,
    bonusTime: 0,
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

/** A pool whose `decide` / `act` settle the way the test wants. */
function fakePool(
  decide: () => Promise<BotDecideResult>,
  act: () => Promise<Command | null> = async () => null,
): BotPool {
  return {
    decide,
    act,
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

// ---------------------------------------------------------------- botSeats

test("botSeats: every held bot seat, no matter its mentality", () => {
  const v = view({
    state: state({
      players: [
        player({ member: 1, bot: false, ai: false }),
        player({ member: 2, bot: true, ai: false, mentality: "advanced" }),
        // `ai` marks a human under machine control, never a bot; a bot seat is
        // driven whatever the flag says (`docs/BOT-DRIVE.md`).
        player({ member: 3, bot: true, ai: true, mentality: "advanced" }),
        player({ member: 4, bot: true, ai: false, mentality: "standard" }),
        player({ member: 5, bot: true, ai: false, mentality: "chaos" }),
        player({ member: 6, bot: true, ai: false, mentality: "advanced", left: true }), // gone
      ],
    }),
  });
  assert.deepEqual(botSeats(v), [2, 3, 4, 5]);
});

// ------------------------------------------------- answered surfaces

const END = view({ state: state({ seq: 12, step: 4 }) });

test("needsAnswer: an answered 结束 is not asked twice", () => {
  const answered = new Map<number, Decision>();
  assert.equal(needsAnswer(answered, 1, END), true);
  answered.set(1, decisionOf(END));
  // The end phase keeps logging (seq bumps) while it holds: still the same one.
  assert.equal(needsAnswer(answered, 1, END), false);
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 13, step: 4 }) })), false);
  // The next turn is a new surface.
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 14, turn: 1, step: 1 }) })), true);
  // Another seat is unaffected.
  assert.equal(needsAnswer(answered, 2, END), true);
});

test("needsAnswer: 运营 re-arms on the next state", () => {
  const answered = new Map<number, Decision>();
  const ops = view({ state: state({ seq: 12, step: 2 }) });
  answered.set(1, decisionOf(ops));
  assert.equal(needsAnswer(answered, 1, ops), false);
  // A played card bumps the state: the seat may still roll, play, or end.
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 13, step: 2 }) })), true);
});

test("needsAnswer: an answered prompt is not asked twice", () => {
  const answered = new Map<number, Decision>();
  const asked = view({
    state: state({ seq: 12, prompt: { ...state().prompt, id: 4, players: [1], answers: [-1] } }),
  });
  answered.set(1, decisionOf(asked));
  // Another seat's answer bumps the seq; this seat still owes the same one.
  const later = view({
    state: state({ seq: 13, prompt: { ...state().prompt, id: 4, players: [1], answers: [-1] } }),
  });
  assert.equal(needsAnswer(answered, 1, later), false);
  // The prompt closing is a new surface.
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 14 }) })), true);
});

test("needsAnswer: an end at 运营 settles the turn", async () => {
  // Ending at 运营 flips the stage to 结束 before the turn advances; that
  // in-between frame must not look like a new decision.
  const answered = new Map<number, Decision>();
  const v = view({ state: state({ seq: 12, step: 2 }) });
  await driveSeat(
    async () => null,
    () => v,
    1,
    {
      ...hooks(),
      search: false,
      pool: fakePool(async () => okAnswer("end"), async () => ({ act: "end" })),
      onApplied: (m, d, ok) => {
        if (!ok && d.revisitable) answered.delete(m);
        else answered.set(m, d);
      },
    },
  );
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 13, step: 4 }) })), false);
  // The next turn (or an extra turn's 开始) is a new surface.
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 14, turn: 1, step: 4 }) })), true);
  assert.equal(needsAnswer(answered, 1, view({ state: state({ seq: 15, step: 1 }) })), true);
});

test("driveSeat reports the applied decision and its outcome", async () => {
  const v = view({ state: state({ seq: 12 }) });
  const seen: [number, number, boolean][] = [];
  await driveSeat(
    async () => null,
    () => v,
    1,
    {
      ...hooks(),
      search: false,
      pool: fakePool(async () => okAnswer("end"), async () => ({ act: "end" })),
      onApplied: (m, d, ok) => seen.push([m, d.key, ok]),
    },
  );
  // An `end` is recorded against the turn's 结束 surface (see `settled`).
  assert.deepEqual(seen, [[1, decisionKey(END), true]]);
});

test("driveSeat reports a refused answer as not applied", async () => {
  const v = view({ state: state({ seq: 12 }) });
  const seen: [number, number, boolean][] = [];
  let n = 0;
  await driveSeat(
    async () => {
      n++;
      return n === 1 ? ({ k: "err.not_your_turn" } as Msg) : null;
    },
    () => v,
    1,
    {
      ...hooks(),
      search: false,
      pool: fakePool(async () => okAnswer("end"), async () => ({ act: "end" })),
      onApplied: (m, d, ok) => seen.push([m, d.key, ok]),
    },
  );
  assert.deepEqual(seen, [
    [1, decisionKey(END), false],
    [1, decisionKey(END), true],
  ]);
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
    { ...h, pool: fakePool(async () => okAnswer("answer")) },
  );
  assert.equal(outcome, "searched");
  assert.deepEqual(h.sent, [{ act: "answer" }]);
});

const REWARD_PROMPT = {
  id: 4,
  kind: "choice",
  title: { k: "ask.pass_reward.title" },
  text: { k: "" },
  card: "",
  options: [{ k: "ask.pass_reward.money" }, { k: "ask.pass_reward.card" }],
  fallback: 0,
  players: [0],
  answers: [-1],
  timeLeft: 10,
  tile: -1,
  bid: 0,
  bidder: -1,
  items: [],
  count: 0,
};

function rewardView(money: number): MatchView {
  return view({
    state: state({ busy: true, players: [player({ money })], prompt: REWARD_PROMPT }),
  });
}

test("driveSeat forces the card [pass-reward] above 80 000 without searching", async () => {
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => rewardView(200_000),
    1,
    {
      ...h,
      pool: fakePool(async () => {
        throw new Error("must not search");
      }),
    },
  );
  assert.equal(outcome, "heuristic");
  assert.deepEqual(h.sent, [{ act: "answer", prompt: 4, value: 1 }]);
});

test("driveSeat still searches a mid-stack [pass-reward]", async () => {
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => rewardView(50_000),
    1,
    { ...h, pool: fakePool(async () => okAnswer("answer")) },
  );
  assert.equal(outcome, "searched");
  assert.deepEqual(h.sent, [{ act: "answer" }]);
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
    { ...h, pool: fakePool(async () => okAnswer("answer")) },
  );
  assert.equal(outcome, "fallback");
  assert.equal(h.sent.length, 2, "search first, then the policy");
});

test("driveSeat ends a turn whose main move a card already spent", async () => {
  // The engine refuses `roll` with `err.roll_main_used` here, so re-sending
  // the policy's roll candidate wedged the seat forever; the `canRollHere`
  // gate switches the fallback to `end`.
  const v = view({ state: state({ canRollHere: false, canEndHere: true }) });
  const h = hooks();
  const outcome = await driveSeat(
    async (_m, cmd) => {
      h.sent.push(cmd);
      return null;
    },
    () => v,
    1,
    { ...h, search: false, pool: fakePool(async () => okAnswer("roll")) },
  );
  assert.equal(outcome, "heuristic");
  assert.deepEqual(h.sent, [{ act: "end" }]);
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
    { ...h, pool: fakePool(async () => okAnswer("answer")) },
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