// Online seat-view extras: cache, merge, notify, failure. Run with:
//   node --test webui/src/game/viewExtras.test.ts
//
// The `extras` op itself is covered by `botPool.test.ts` against the real
// built `bot-glue` bundle. This file pins the page-side cache + merge contract
// (`OnlineExtras` / `emitOnlineView`, which `OnlineSession.emitView` wires to
// its subscriber set) with a fake pool -- no wasm needed.

import { strict as assert } from "node:assert";
import { test } from "node:test";

import {
  emitOnlineView,
  extrasKey,
  mergeExtras,
  OnlineExtras,
  type ViewExtras,
} from "./viewExtras.ts";
import { BotPool, type BotWorkerLike, type BotPoolOpts } from "./botPool.ts";
import type { MatchView, MatchState, MatchPlayer, SkillAction } from "../core/types.ts";

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
  const players = over.players ?? [player(), player({ member: 2, player: "B1", bot: true, ai: true })];
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
      price: -1,
      prices: [],
    },
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
    hand: over.hand ?? ["c1", "c2"],
    handNotes: [],
    draw: [],
    you: over.you ?? 1,
    playerId: over.playerId ?? 0,
    ...over,
    state: over.state ?? state(),
  } as MatchView;
}

function skill(id: string, enabled = true): SkillAction {
  return {
    id,
    source: "0:skill",
    title: { k: `card.${id}.title` },
    text: { k: `card.${id}.text` },
    enabled,
    reason: enabled ? { k: "" } : { k: "skills.unavailable" },
  };
}

function extrasBlob(over: Partial<ViewExtras> = {}): ViewExtras {
  return {
    playable: [true, false],
    estCost: [100, 200],
    skills: [skill("skill:0:one")],
    aiAnswer: { answer: 1, picked: ["c1"], worth: 42 },
    ...over,
  };
}

/** A pool whose `extras` settles the way the test wants (and counts calls). */
function fakePool(
  extras: (v: MatchView, seed: number) => Promise<ViewExtras>,
): BotPool & { calls: { view: MatchView; seed: number }[] } {
  const calls: { view: MatchView; seed: number }[] = [];
  return {
    calls,
    extras: async (v: MatchView, seed: number) => {
      calls.push({ view: v, seed });
      return extras(v, seed);
    },
    decide: async () => {
      throw new Error("not used");
    },
    ponder: async () => {},
    invalidate: async () => {},
    ensureLoaded: async () => {},
    dispose: () => {},
    workers: 1,
  } as unknown as BotPool & { calls: { view: MatchView; seed: number }[] };
}

function delay<T>(v: T, ms = 0): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(v), ms));
}

// ---------------------------------------------------------------- key / merge

test("extrasKey is seq + prompt.id", () => {
  const a = view({ state: state({ seq: 7, prompt: { id: 3 } as MatchState["prompt"] }) });
  assert.equal(extrasKey(a), "7:3");
  const b = view({ state: state({ seq: 7, prompt: { id: 0 } as MatchState["prompt"] }) });
  assert.equal(extrasKey(b), "7:0");
});

test("mergeExtras writes the four fields and leaves the rest alone", () => {
  const v = view({ hand: ["a", "b"] });
  const e = extrasBlob();
  const m = mergeExtras(v, e);
  assert.equal(m.hand, v.hand);
  assert.equal(m.state, v.state);
  assert.equal(m.playable, e.playable);
  assert.equal(m.estCost, e.estCost);
  assert.equal(m.skills, e.skills);
  assert.equal(m.aiAnswer, e.aiAnswer);
  assert.notEqual(m, v, "a new object so React re-renders");
});

// ---------------------------------------------------------------- emit cycle

test("extras result merges into the view and notifies subscribers", async () => {
  const pool = fakePool(async () => delay(extrasBlob(), 5));
  const extras = new OnlineExtras("room-1", 1, pool);
  const notified: MatchView[] = [];
  let live: MatchView | null = null;
  const emit = (m: MatchView) => {
    live = m;
    notified.push(m);
  };
  const v = view();
  const done = emitOnlineView(extras, v, emit, () => live);

  // First emit: no extras yet (fields absent -- consumers tolerate that).
  assert.equal(notified.length, 1);
  assert.equal(notified[0].playable, undefined);
  assert.equal(notified[0].skills, undefined);
  assert.equal(notified[0].aiAnswer, undefined);

  // Extras land: the live view is re-emitted with the four fields filled in.
  await done;
  assert.equal(notified.length, 2, "subscribers notified again");
  assert.deepEqual(notified[1].playable, [true, false]);
  assert.deepEqual(notified[1].estCost, [100, 200]);
  assert.equal(notified[1].skills?.length, 1);
  assert.deepEqual(notified[1].aiAnswer, { answer: 1, picked: ["c1"], worth: 42 });
  assert.equal(notified[1].hand, v.hand);
});

test("cache: same seq+promptId does not recompute", async () => {
  const pool = fakePool(async () => delay(extrasBlob(), 5));
  const extras = new OnlineExtras("room-1", 1, pool);
  const v1 = view({ state: state({ seq: 4, prompt: { id: 2 } as MatchState["prompt"] }) });
  const v2 = view({ state: state({ seq: 4, prompt: { id: 2 } as MatchState["prompt"] }) });
  assert.equal(extrasKey(v1), extrasKey(v2));

  const got: ViewExtras[] = [];
  const p1 = extras.request(v1, (e) => got.push(e));
  const p2 = extras.request(v2, (e) => got.push(e)); // joins the in-flight one
  assert.equal(p1, p2, "same key joins the in-flight promise");
  await p1;
  assert.equal(pool.calls.length, 1);
  assert.equal(got.length, 1);

  // A third frame on the same decision hits the cache (no worker round-trip)
  // and `mergeCached` fills the fields immediately.
  const merged = extras.mergeCached(v2);
  assert.deepEqual(merged.playable, [true, false]);
  assert.equal(pool.calls.length, 1);

  // A new prompt on the same state is a new decision.
  const v3 = view({ state: state({ seq: 4, prompt: { id: 3 } as MatchState["prompt"] }) });
  await extras.request(v3, () => {});
  assert.equal(pool.calls.length, 2);
});

test("failure: missing extras leaves the view usable", async () => {
  const pool = fakePool(async () => {
    throw new Error("worker exploded");
  });
  const extras = new OnlineExtras("room-1", 1, pool);
  const notified: MatchView[] = [];
  let live: MatchView | null = null;
  const emit = (m: MatchView) => {
    live = m;
    notified.push(m);
  };
  const v = view();
  await emitOnlineView(extras, v, emit, () => live);

  // Still just the one emit; the frame is intact and has no extras fields.
  assert.equal(notified.length, 1);
  assert.equal(notified[0].playable, undefined);
  assert.deepEqual(notified[0].hand, ["c1", "c2"]);
  assert.equal(notified[0].state.seq, 10);
  // mergeCached is a no-op -- the view is handed on as-is.
  assert.equal(extras.mergeCached(v), v);
  // The failed key is not retried.
  await extras.request(v, () => {});
  assert.equal(pool.calls.length, 1);
});

test("a late extras reply does not overwrite a newer decision", async () => {
  const release: Array<() => void> = [];
  const extras = new OnlineExtras("room-1", 1, fakePool(async () => {
    const e = extrasBlob();
    return new Promise<ViewExtras>((resolve) => {
      release.push(() => resolve(e));
    });
  }));
  const notified: MatchView[] = [];
  let live: MatchView | null = null;
  const emit = (m: MatchView) => {
    live = m;
    notified.push(m);
  };

  const v1 = view({ state: state({ seq: 1 }) });
  const p1 = emitOnlineView(extras, v1, emit, () => live);
  // A newer frame lands before the extras reply.
  const v2 = view({ state: state({ seq: 2 }) });
  const p2 = emitOnlineView(extras, v2, emit, () => live);

  // Let both worker calls start (they park on `release`).
  await delay(0);
  assert.equal(release.length, 2);
  release[0]();
  await p1;
  assert.equal(notified.length, 2, "no re-emit for the stale key");
  assert.equal(notified[1].state.seq, 2);
  assert.equal(notified[1].playable, undefined);

  // The live key's reply does land.
  release[1]();
  await p2;
  assert.equal(notified.length, 3);
  assert.equal(notified[2].state.seq, 2);
  assert.deepEqual(notified[2].playable, [true, false]);
});

test("seed comes from the decision identity, never a constant", async () => {
  const pool = fakePool(async () => extrasBlob());
  const extras = new OnlineExtras("room-A", 3, pool);
  await extras.request(view({ state: state({ seq: 11, prompt: { id: 4 } as MatchState["prompt"] }) }), () => {});
  await extras.request(view({ state: state({ seq: 12, prompt: { id: 4 } as MatchState["prompt"] }) }), () => {});
  assert.equal(pool.calls.length, 2);
  assert.notEqual(pool.calls[0].seed, pool.calls[1].seed, "different seq -> different seed");
  for (const c of pool.calls) {
    assert.ok(Number.isFinite(c.seed) && c.seed >= 0 && c.seed <= 0xffffffff, "u32");
  }
});

// ---------------------------------------------------------------- BotPool.extras

test("BotPool.extras dispatches the extras op on one worker", async () => {
  const ops: { op: string; seed?: number }[] = [];
  const workers: BotWorkerLike[] = [];
  const factory = (): BotWorkerLike => {
    let onmessage: ((e: { data: unknown }) => void) | null = null;
    const w: BotWorkerLike = {
      postMessage(msg: unknown) {
        const m = msg as { id: number; op: string; seed?: number };
        ops.push({ op: m.op, seed: m.seed });
        setTimeout(() => {
          if (m.op === "init") onmessage?.({ data: { id: m.id, ok: true, value: { modules: 0, info: {} } } });
          else if (m.op === "extras") onmessage?.({ data: { id: m.id, ok: true, value: extrasBlob() } });
        }, 0);
      },
      terminate() {},
      get onmessage() {
        return onmessage;
      },
      set onmessage(fn: ((e: { data: unknown }) => void) | null) {
        onmessage = fn;
      },
      onerror: null,
    };
    workers.push(w);
    return w;
  };
  const pool = new BotPool({
    workers: 3,
    baseUrl: "http://localhost/assets/engine/bot-glue/",
    workerFactory: factory,
  } satisfies BotPoolOpts);
  try {
    const e = await pool.extras(view(), 0xabcdef01);
    assert.deepEqual(e.playable, [true, false]);
    // Init on every slot, but extras on just one (no root-parallel).
    assert.equal(ops.filter((o) => o.op === "init").length, 3);
    assert.equal(ops.filter((o) => o.op === "extras").length, 1);
    assert.equal(ops.find((o) => o.op === "extras")?.seed, 0xabcdef01);
  } finally {
    pool.dispose();
  }
});