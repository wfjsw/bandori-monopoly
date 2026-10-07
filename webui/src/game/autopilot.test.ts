// Unit tests for the 托管 suggestion policy. Run with:
//   node --test webui/src/game/autopilot.test.ts
// (Node's built-in runner; no test framework is installed in webui.)

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { suggest, plan, CHAOS_RESERVE, type AutopilotCtx } from "./autopilot.ts";
import type { MatchPlayer, MatchState, MatchView, TileData } from "../core/types.ts";

// ---------------------------------------------------------------- fixtures

const tile = (index: number, price: number, house: number, kind = "property"): TileData => ({
  index,
  name: `T${index}`,
  shortName: `T${index}`,
  kind,
  group: 0,
  color: "#000",
  tier: 0,
  area: "a",
  price,
  house,
  rent: [10, 20, 30, 40, 50],
  note: "",
});

const TILES: TileData[] = [
  tile(0, 0, 0, "start"),
  tile(1, 1000, 500),
  tile(2, 2000, 600),
  tile(3, 3000, 700),
];

function player(over: Partial<MatchPlayer> = {}): MatchPlayer {
  return {
    member: 1,
    player: "Me",
    bot: false,
    ai: false,
    roll: 0,
    banDone: false,
    ban: "",
    character: "",
    deckReady: false,
    money: 10000,
    pos: 0,
    hand: 0,
    draw: 0,
    discard: [],
    mulligan: false,
    bankrupt: false,
    left: false,
    outOrder: 0,
    assets: 0,
    score: 0,
    rank: 0,
    state: { handLimit: { value: 5, min: 0, max: 99 } },
    skillCharacter: "",
    bands: "",
    tokens: [],
    skillNote: { k: "" },
    field: [],
    actions: [],
    ...over,
  };
}

function state(over: Partial<MatchState> = {}): MatchState {
  return {
    phase: "play",
    matchId: 1,
    seq: 1,
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
    plan: { playerId: 0, from: 0, steps: 0, started: false, reach: [], canBuild: true },
    timeLeft: 0,
    shield: 0,
    bank: 0,
    bought: false,
    built: false,
    players: [player()],
    bans: [],
    owners: [-1, -1, -1, -1],
    houses: [0, 0, 0, 0],
    mortgaged: [false, false, false, false],
    embers: [0, 0, 0, 0],
    marks: [],
    tileColors: [0, 0, 0, 0],
    eventDeck: 0,
    eventTop: [],
    eventDiscard: [],
    eventActive: [],
    prompt: {
      id: 0,
      kind: "",
      title: { k: "" },
      text: { k: "" },
      card: "",
      options: [],
      fallback: 0,
      players: [],
      answers: [],
      timeLeft: 0,
      tile: -1,
      bid: 0,
      bidder: -1,
      items: [],
      count: 0,
    },
    vote: { id: 0, by: -1, players: [], answers: [], timeLeft: 0 },
    events: [],
    endReason: "",
    winner: -1,
    scoreMoney: 1,
    scoreProperty: 1,
    scoreHouses: 1,
    ...over,
  };
}

function view(over: Partial<MatchView> = {}): MatchView {
  return {
    state: state(),
    hand: [],
    handNotes: [],
    draw: [],
    you: 1,
    playerId: 0,
    aiAnswer: null,
    playable: [],
    ...over,
  };
}

/** Deterministic RNG from a fixed sequence of [0, 1) values. */
function rng(...values: number[]): () => number {
  let i = 0;
  return () => values[i++ % values.length];
}

const ctx = (over: Partial<AutopilotCtx> = {}): AutopilotCtx => ({
  tiles: TILES,
  characters: [{ name: "A", cnId: "a" }, { name: "B", cnId: "" }],
  playedThisTurn: 0,
  deckPreset: () => ["c1", "c2"],
  deckRandom: () => ["r1", "r2", "r3"],
  random: rng(0.1),
  ...over,
});

// ---------------------------------------------------------------- tests

test("on your turn in 运营 it suggests roll", () => {
  const cmd = suggest(view(), ctx());
  assert.deepEqual(cmd, { act: "roll" });
});

test("after landing on an affordable unowned tile it suggests buy", () => {
  // 结束, standing on tile 1 (1000), money 10000 -> 10000 - 2400 >= 2000.
  const v = view({
    state: state({
      step: 4,
      landed: 1,
      bought: false,
      players: [player({ pos: 1 })],
    }),
  });
  const cmd = suggest(v, ctx());
  assert.deepEqual(cmd, { act: "buy", value: 1 });
});

test("it answers a prompt with the AI answer", () => {
  const v = view({
    state: state({
      busy: true,
      prompt: {
        id: 7,
        kind: "choice",
        title: { k: "ask.circle.title" },
        text: { k: "ask.circle.text" },
        card: "",
        options: [{ k: "ask.circle.money" }, { k: "ask.circle.card" }],
        fallback: 0,
        players: [0],
        answers: [-1],
        timeLeft: 10,
        tile: -1,
        bid: 0,
        bidder: -1,
        items: [],
        count: 0,
      },
    }),
    aiAnswer: { answer: 1, picked: [], worth: 0 },
  });
  const cmd = suggest(v, ctx());
  assert.deepEqual(cmd, { act: "answer", prompt: 7, value: 1 });
});

test("it falls back to the prompt's fallback without an aiAnswer", () => {
  const v = view({
    state: state({
      busy: true,
      prompt: {
        id: 8,
        kind: "choice",
        title: { k: "ask.counteract.title" },
        text: { k: "ask.counteract.text" },
        card: "",
        options: [{ k: "ask.counteract.play" }, { k: "ask.counteract.skip" }],
        fallback: 1,
        players: [0],
        answers: [-1],
        timeLeft: 12,
        tile: -1,
        bid: 0,
        bidder: -1,
        items: [],
        count: 0,
      },
    }),
    aiAnswer: null,
  });
  const cmd = suggest(v, ctx());
  assert.deepEqual(cmd, { act: "answer", prompt: 8, value: 1 }, "counteract declines");
});

test("it does nothing when the turn is not yours", () => {
  const v = view({
    state: state({
      turn: 1,
      roller: 1,
      players: [player(), player({ member: 2, bot: true })],
    }),
  });
  assert.equal(suggest(v, ctx()), null);
});

test("over the hand limit it discards first", () => {
  const v = view({ hand: ["x", "y", "z", "w", "v", "u"], state: state({ step: 2 }) });
  const cmd = suggest(v, ctx({ random: rng(0) }));
  assert.equal(cmd?.act, "discard");
});

test("skip-move in 运营 ends the turn instead of rolling", () => {
  const cmd = suggest(view({ state: state({ step: 2, skipMove: true }) }), ctx());
  assert.deepEqual(cmd, { act: "end" });
});

test("it prefers redeeming over rolling when a deed leaves the reserve", () => {
  const v = view({
    state: state({
      step: 2,
      owners: [-1, 0, -1, -1],
      mortgaged: [false, true, false, false],
      players: [player({ money: 10000 })],
    }),
  });
  const cmd = suggest(v, ctx());
  // tile 1: redeem cost 600 -> 10000 - 600 >= 4000.
  assert.deepEqual(cmd, { act: "redeem", value: 1 });
});

test("a buy prompt is answered by the buy threshold", () => {
  const prompt = {
    id: 3,
    kind: "choice",
    title: { k: "ask.buy.title" },
    text: { k: "ask.buy.text" },
    card: "",
    options: [{ k: "ask.buy.yes" }, { k: "ask.no_buy" }],
    fallback: 1,
    players: [0],
    answers: [-1],
    timeLeft: 15,
    tile: 1,
    bid: 0,
    bidder: -1,
    items: [],
    count: 0,
  };
  // 10000 - 1000 >= 2000 -> buy (0).
  assert.deepEqual(suggest(view({ state: state({ busy: true, prompt }) }), ctx()), {
    act: "answer",
    prompt: 3,
    value: 0,
  });
  // 1500 - 1000 < 2000 -> decline (1).
  const poor = view({ state: state({ busy: true, prompt, players: [player({ money: 1500 })] }) });
  assert.deepEqual(suggest(poor, ctx()), { act: "answer", prompt: 3, value: 1 });
});

test("auction bids within the precomputed worth and passes above it", () => {
  const prompt = {
    id: 4,
    kind: "auction",
    title: { k: "ask.auction.title" },
    text: { k: "ask.auction.text" },
    card: "",
    options: [],
    fallback: 0,
    players: [0],
    answers: [-1],
    timeLeft: 10,
    tile: 1,
    bid: 0,
    bidder: -1,
    items: [],
    count: 0,
  };
  // worth 2000, no bids yet -> min 100, bid 100..300.
  const cmd = suggest(view({ state: state({ busy: true, prompt }) }), ctx({ random: rng(0.5) }));
  assert.equal(cmd?.act, "answer");
  assert.equal(cmd?.prompt, 4);
  assert.ok((cmd?.value ?? -1) >= 100 && (cmd?.value ?? -1) <= 300);
  // worth below the minimum -> pass.
  const poor = suggest(
    view({ state: state({ busy: true, prompt, players: [player({ money: 150 })] }) }),
    ctx(),
  );
  assert.deepEqual(poor, { act: "answer", prompt: 4, value: -1 });
});
// ---------------------------------------------------------------- chaos

test("chaos plays a playable card instead of rolling in 运营", () => {
  const v = view({
    hand: ["c1", "c2"],
    playable: [true, false],
    state: state({ step: 2 }),
  });
  const cmd = suggest(v, ctx({ random: rng(0.9) }), "chaos");
  assert.deepEqual(cmd, { act: "play", card: "c1" });
});

test("chaos declares a counteract when offered", () => {
  const v = view({
    state: state({
      busy: true,
      prompt: {
        id: 9,
        kind: "choice",
        title: { k: "ask.counteract.title" },
        text: { k: "ask.counteract.text" },
        card: "",
        // Two [反击] cards plus the skip option; the fallback is skip.
        options: [{ k: "ask.counteract.play" }, { k: "ask.counteract.play" }, { k: "ask.counteract.skip" }],
        fallback: 2,
        players: [0],
        answers: [-1],
        timeLeft: 12,
        tile: -1,
        bid: 0,
        bidder: -1,
        items: [],
        count: 0,
      },
    }),
    aiAnswer: null,
  });
  const cmd = suggest(v, ctx({ random: rng(0.5) }), "chaos");
  assert.equal(cmd?.act, "answer");
  assert.equal(cmd?.prompt, 9);
  assert.ok((cmd?.value ?? -1) >= 0 && (cmd?.value ?? -1) <= 1, "picks a card, never the skip option");
});

test("chaos picks a non-default prompt option", () => {
  const v = view({
    state: state({
      busy: true,
      prompt: {
        id: 5,
        kind: "choice",
        title: { k: "ask.circle.title" },
        text: { k: "ask.circle.text" },
        card: "",
        options: [{ k: "ask.circle.money" }, { k: "ask.circle.card" }],
        fallback: 0,
        players: [0],
        answers: [-1],
        timeLeft: 10,
        tile: -1,
        bid: 0,
        bidder: -1,
        items: [],
        count: 0,
      },
    }),
    aiAnswer: null,
  });
  const cmd = suggest(v, ctx({ random: rng(0.5) }), "chaos");
  assert.deepEqual(cmd, { act: "answer", prompt: 5, value: 1 }, "the non-default option");
});

test("chaos buys when money - price >= 1000 and declines below it", () => {
  const prompt = {
    id: 6,
    kind: "choice",
    title: { k: "ask.buy.title" },
    text: { k: "ask.buy.text" },
    card: "",
    options: [{ k: "ask.buy.yes" }, { k: "ask.no_buy" }],
    fallback: 1,
    players: [0],
    answers: [-1],
    timeLeft: 15,
    tile: 1,
    bid: 0,
    bidder: -1,
    items: [],
    count: 0,
  };
  // tile 1 costs 1000: 2000 - 1000 >= 1000 -> buy.
  const ok = view({ state: state({ busy: true, prompt, players: [player({ money: 2000 })] }) });
  assert.deepEqual(suggest(ok, ctx(), "chaos"), { act: "answer", prompt: 6, value: 0 });
  // 1999 - 1000 < 1000 -> decline.
  const tight = view({ state: state({ busy: true, prompt, players: [player({ money: 1999 })] }) });
  assert.deepEqual(suggest(tight, ctx(), "chaos"), { act: "answer", prompt: 6, value: 1 });
});

test("chaos buys after landing only while the reserve survives", () => {
  // 结束 on tile 1 (1000): 2000 - 1000 >= 1000 -> buy; 1999 - 1000 < 1000 -> end.
  const at = (money: number) =>
    view({
      state: state({
        step: 4,
        landed: 1,
        players: [player({ pos: 1, money })],
      }),
    });
  assert.deepEqual(suggest(at(2000), ctx(), "chaos"), { act: "buy", value: 1 });
  assert.deepEqual(suggest(at(1999), ctx(), "chaos"), { act: "end" });
  assert.equal(CHAOS_RESERVE, 1000);
});

test("chaos lists every playable card before rolling", () => {
  const v = view({
    hand: ["c1", "c2", "c3"],
    playable: [true, true, false],
    state: state({ step: 2 }),
  });
  const list = plan(v, ctx({ random: rng(0.5) }), "chaos");
  assert.deepEqual(
    list.map((c) => c.act),
    ["play", "play", "roll"],
  );
});
