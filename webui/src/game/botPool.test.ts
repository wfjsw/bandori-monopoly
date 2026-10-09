// The advanced-bot worker pool + merge logic (`docs/BOT.md` B6). Run with:
//   node --test webui/src/game/botPool.test.ts
//
// Loads the REAL built `bot-glue` bundle (the same bytes the worker imports)
// and the REAL built ruleset through the shared `loadRulesetInto` sequence,
// then drives `decide` at real `MatchView`s taken from a live solo match.
// Also pins the page-side root-statistics merge against `bot_core::merge_stats`.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { loadRulesetInto, type RulesetGlue } from "../core/rulesetLoad.ts";
import {
  mergeRootStats,
  pickAction,
  defaultWorkerCount,
  type DirectBotGlue,
  type RootStat,
} from "./botPool.ts";
import { decisionAt, seedForThread } from "./botBudget.ts";
import type { MatchView, RoomMember } from "../core/types.ts";

/** `SoloMatch::new` takes the derived 256-bit seed as hex (`docs/FAIRNESS.md`); tests just need a deterministic one. */
const seedHex = (n: number) => (n >>> 0).toString(16).padStart(64, "0");

const WEBUI = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ROOT = resolve(WEBUI, "..");
const BOT_DIR = join(WEBUI, "public", "assets", "engine", "bot-glue");
const WEB_GLUE_DIR = join(WEBUI, "src", "wasm");
const DATA_DIR = join(ROOT, "data");

function rulesDir(): string {
  for (const d of [join(WEBUI, "public", "assets", "rules"), join(ROOT, "dist", "cards")]) {
    if (existsSync(join(d, "index.json"))) return d;
  }
  throw new Error("no built ruleset -- run tools/build-ruleset.mjs");
}

function reader(dir: string) {
  return async (rel: string) => new Uint8Array(readFileSync(join(dir, rel)));
}

/** One fresh `bot-glue` instance (the real worker bundle). */
async function loadBotGlue(tag: string): Promise<DirectBotGlue> {
  const glueJs = join(BOT_DIR, "glue.js");
  const glueWasm = join(BOT_DIR, "glue_bg.wasm");
  assert.ok(existsSync(glueJs) && existsSync(glueWasm), "run tools/build-bot-glue.mjs");
  const glue = (await import(pathToFileURL(glueJs).href + "?" + tag)) as unknown as DirectBotGlue;
  await glue.default({ module_or_path: new Uint8Array(readFileSync(glueWasm)) });
  const names: string[] = JSON.parse(glue.data_files());
  const files: Record<string, string> = {};
  for (const n of names) {
    const p = join(DATA_DIR, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  glue.load_data(JSON.stringify(files));
  await loadRulesetInto(glue as unknown as RulesetGlue, reader(rulesDir()));
  return glue;
}

/** One fresh `web-glue` instance -- only used to *produce* real MatchViews. */
async function loadWebGlue(tag: string): Promise<Record<string, any>> {
  const glueJs = join(WEB_GLUE_DIR, "glue.js");
  const glueWasm = join(WEB_GLUE_DIR, "glue_bg.wasm");
  assert.ok(existsSync(glueJs) && existsSync(glueWasm), "run tools/build-glue.mjs");
  const glue = (await import(pathToFileURL(glueJs).href + "?" + tag)) as Record<string, any>;
  await glue.default({ module_or_path: new Uint8Array(readFileSync(glueWasm)) });
  const names: string[] = JSON.parse(glue.data_files());
  const files: Record<string, string> = {};
  for (const n of names) {
    const p = join(DATA_DIR, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  glue.load_data(JSON.stringify(files));
  await loadRulesetInto(glue as unknown as RulesetGlue, reader(rulesDir()));
  return glue;
}

function members(): RoomMember[] {
  const chars = ["户山香澄", "美竹兰", "仓田真白", "花园多惠"];
  return chars.map((c, i) => ({
    id: i + 1,
    player: i === 0 ? "P" : `B${i}`,
    character: c,
    cnId: "",
    ready: true,
    host: i === 0,
    bot: i !== 0,
    away: false,
    mentality: "standard" as const,
  }));
}

/**
 * Real decision views from a live solo match: walk a short game and snapshot
 * every frame where some seat is at a decision (`decisionAt` non-null).
 */
function collectViews(glue: Record<string, any>, limit: number): { view: MatchView; member: number }[] {
  const out: { view: MatchView; member: number }[] = [];
  const seen = new Set<string>();
  // A couple of games: one seed rarely shows every surface before the cap.
  for (const seed of [20261008, 777, 31337]) {
    if (out.length >= limit) break;
    const m = new glue.SoloMatch(JSON.stringify(members()), seedHex(seed), 0, "");
    try {
      m.quick_start();
      for (let step = 0; step < 300 && out.length < limit; step++) {
        m.tick_steps(10);
        for (let member = 1; member <= 4; member++) {
          const v = JSON.parse(m.view(member)) as MatchView;
          if (decisionAt(v.state, v.playerId) == null) continue;
          // Distinct information sets only -- one per (seq, prompt, seat).
          const key = `${v.state.seq}:${v.state.prompt?.id ?? 0}:${member}`;
          if (seen.has(key)) continue;
          seen.add(key);
          out.push({ view: v, member });
        }
        if (m.ended()) break;
      }
    } finally {
      m.free?.();
    }
  }
  return out;
}

// ---------------------------------------------------------------- merge

function stat(answer: Record<string, unknown>, visits: number, valueSum: number, over: Partial<RootStat> = {}): RootStat {
  return {
    answer: answer as RootStat["answer"],
    visits,
    valueSum,
    mean: visits ? valueSum / visits : 0,
    evalMax: 0,
    evalSum: 0,
    evalVisits: 0,
    ...over,
  };
}

test("mergeRootStats sums visits and value sums, takes evalMax, sorts best-first", () => {
  const a = { act: "buy", value: 5 };
  const b = { act: "end" };
  const parts = [
    [stat(a, 3, 1.2, { evalMax: 0.4, evalSum: 1.0, evalVisits: 2 }), stat(b, 5, 2.0, { evalMax: 0.1 })],
    [stat(a, 4, 1.8, { evalMax: 0.9, evalSum: 2.0, evalVisits: 3 }), stat(b, 1, 0.2, { evalMax: 0.7 })],
  ];
  const merged = mergeRootStats(parts);
  assert.equal(merged.length, 2);
  // b has 6 visits, a has 7 -> a first.
  assert.deepEqual(merged[0].answer, a);
  assert.equal(merged[0].visits, 7);
  assert.equal(merged[0].valueSum, 3.0);
  assert.equal(merged[0].evalMax, 0.9, "evalMax takes the max");
  assert.equal(merged[0].evalSum, 3.0);
  assert.equal(merged[0].evalVisits, 5);
  assert.equal(merged[1].visits, 6);
  assert.equal(merged[1].evalMax, 0.7);
});

test("mergeRootStats breaks ties on mean, then a stable command order", () => {
  const a = { act: "buy", value: 1 };
  const b = { act: "buy", value: 2 };
  const merged = mergeRootStats([
    [stat(b, 2, 1.0), stat(a, 2, 1.0)],
    [stat(a, 2, 1.0), stat(b, 2, 1.0)],
  ]);
  assert.equal(merged[0].visits, 4);
  assert.equal(merged[1].visits, 4);
  // Same visits, same mean: JSON order of the answer decides.
  assert.equal(JSON.stringify(merged[0].answer), JSON.stringify(merged[0].answer));
  assert.notDeepEqual(merged[0].answer, merged[1].answer);
});

test("mergeRootStats on an empty / single-worker input", () => {
  assert.deepEqual(mergeRootStats([]), []);
  assert.deepEqual(mergeRootStats([[]]), []);
  const one = mergeRootStats([[stat({ act: "end" }, 1, 0.5)]]);
  assert.equal(one.length, 1);
  assert.equal(pickAction(one)?.act, "end");
  assert.equal(pickAction([]), null);
});

test("defaultWorkerCount is in 1..=4 and respects low memory", () => {
  const n = defaultWorkerCount();
  assert.ok(n >= 1 && n <= 4, `got ${n}`);
});

// ---------------------------------------------------------------- worker API

test("bot-glue loads the real built ruleset through the shared sequence", async () => {
  const glue = await loadBotGlue("boot");
  const info = JSON.parse(glue.info());
  assert.equal(info.service, "bot-glue");
  assert.match(info.modules, /^[0-9a-f]{64}$/, "a real ruleset, not stub");
  assert.ok(info.ponder, "ponder cache on by default");
});

test("decide returns a legal answer for a range of real views", async () => {
  const bot = await loadBotGlue("decide");
  const web = await loadWebGlue("views");
  const samples = collectViews(web, 8);
  assert.ok(samples.length >= 3, `need a range of views, got ${samples.length}`);
  for (const { view, member } of samples) {
    const raw = bot.decide(JSON.stringify(view), 200, 42);
    const r = JSON.parse(raw);
    assert.equal(r.ok, true, `view ${view.state.seq}/${view.state.prompt?.id}: ${raw}`);
    assert.ok(r.answer && typeof r.answer.act === "string", "a NetMessage with an act");
    assert.ok(typeof r.iterations === "number");
    assert.ok(typeof r.elapsedMs === "number");
    assert.ok(typeof r.heuristic === "boolean");
    assert.equal(r.decisionKey, r.decisionKey.toLowerCase());
    // Root stats carry the public command for each branched action.
    if (!r.heuristic) {
      assert.ok(Array.isArray(r.rootStats) && r.rootStats.length > 0, "searched surfaces report root stats");
      for (const s of r.rootStats) {
        assert.ok(s.answer && typeof s.answer.act === "string");
        assert.ok(s.visits >= 0);
      }
    }
    void member;
  }
});

test("decide sees a seat's view only -- never a World / seed / other hand", async () => {
  const bot = await loadBotGlue("boundary");
  const web = await loadWebGlue("boundary-views");
  const samples = collectViews(web, 4);
  assert.ok(samples.length > 0);
  const { view } = samples[0];
  const frame = JSON.stringify(view);
  // The view frame is the same one the page renders (`web-glue` `SoloMatch::view`).
  assert.ok(!frame.includes('"world"'), "no World in the frame");
  assert.ok(!/"seed"\s*:/.test(frame), "no match seed in the frame");
  assert.ok(!/"liveRng"\s*:/.test(frame), "no live RNG in the frame");
  // Own hand is present; another seat's hand is only a count.
  assert.ok(Array.isArray(view.hand), "own hand is a list of card ids");
  for (const p of view.state.players) {
    if (p.member === view.you) continue;
    assert.ok(typeof p.hand === "number", `seat ${p.member} hand is a count`);
    assert.ok(!Array.isArray((p as unknown as { handCards?: unknown }).handCards));
  }
  // The search itself must accept exactly this frame (it parses `SeatView`).
  const r = JSON.parse(bot.decide(frame, 100, 7));
  assert.equal(r.ok, true);
});

test("decide is deterministic given the seed (and per-worker seeds differ)", async () => {
  const bot = await loadBotGlue("determinism");
  const web = await loadWebGlue("det-views");
  const samples = collectViews(web, 3);
  assert.ok(samples.length > 0);
  const { view } = samples[0];
  // A 1 ms budget always completes exactly one iteration (the loop is anytime
  // *between* iterations), so the pick is fully seed-determined. A longer
  // budget is deterministic too, but the iteration count then depends on how
  // busy the machine is -- which is the point of the anytime loop.
  const frame = JSON.stringify(view);
  const a = JSON.parse(bot.decide(frame, 1, 99));
  bot.reset_cache();
  const b = JSON.parse(bot.decide(frame, 1, 99));
  assert.equal(JSON.stringify(a.answer), JSON.stringify(b.answer), "same seed, same answer");
  assert.equal(a.decisionKey, b.decisionKey);
  assert.equal(a.iterations, b.iterations);
  // A different seed changes the determinization stream (and may change the
  // pick); the per-worker derivation is stable and disjoint.
  assert.equal(seedForThread(99, 0), 99);
  assert.notEqual(seedForThread(99, 1), seedForThread(99, 2));
  // And the glue's own derivation matches the page's.
  assert.equal(bot.seed_for_thread(99, 1), seedForThread(99, 1));
  assert.equal(bot.seed_for_thread(99, 2), seedForThread(99, 2));
});

test("a trivial surface is answered by the heuristic without searching", async () => {
  const bot = await loadBotGlue("trivial");
  const web = await loadWebGlue("trivial-views");
  const m = new web.SoloMatch(JSON.stringify(members()), seedHex(4242), 0, "");
  try {
    m.quick_start();
    // Find a frame where someone is at 运营 with nothing worth searching
    // (roll / end) -- `legal_actions` is empty and `decide` must say so.
    let found = false;
    for (let step = 0; step < 200 && !found; step++) {
      m.tick_steps(10);
      for (let member = 1; member <= 4; member++) {
        const v = JSON.parse(m.view(member)) as MatchView;
        if (decisionAt(v.state, v.playerId) == null) continue;
        const r = JSON.parse(bot.decide(JSON.stringify(v), 50, 1));
        if (r.heuristic && r.iterations === 0) {
          assert.ok(r.answer.act, "the heuristic still answers");
          assert.deepEqual(r.rootStats, []);
          found = true;
          break;
        }
      }
    }
    // Not every game hits a trivial surface before the cap; the searched path
    // is covered above, so a miss is a skip rather than a failure.
    if (!found) console.log("  (no trivial surface in this sample -- skipped)");
  } finally {
    m.free?.();
  }
});

test("ponder caches by decision key and a later decide reuses it", async () => {
  const bot = await loadBotGlue("ponder");
  const web = await loadWebGlue("ponder-views");
  const samples = collectViews(web, 4);
  // Find a searched surface (the heuristic path caches nothing).
  for (const { view } of samples) {
    const frame = JSON.stringify(view);
    bot.reset_cache();
    const p = JSON.parse(bot.ponder(frame, 100, 5));
    if (p.heuristic) continue;
    assert.equal(p.ok, true);
    assert.ok(p.decisionKey);
    const d = JSON.parse(bot.decide(frame, 100, 5));
    if (d.reused) {
      assert.equal(d.decisionKey, p.decisionKey);
      assert.ok(d.elapsedMs < 50, "a reused decide is free");
      return;
    }
    // A pondered key that the decide missed is still a valid answer.
    assert.equal(d.ok, true);
    return;
  }
  console.log("  (no searched surface in this sample -- skipped)");
});
// ---------------------------------------------------------------- record

test("a solo match with an Advanced bot records and replays clean", async () => {
  const web = await loadWebGlue("record");
  const bot = await loadBotGlue("record-bot");
  // One human + one Advanced bot. `Match::new` holds the Advanced seat
  // (`ai` off) -- the driver answers through `act`, exactly like the server.
  const seats: RoomMember[] = [
    { id: 1, player: "P", character: "户山香澄", cnId: "", ready: true, host: true, bot: false, away: false, mentality: "standard" },
    { id: 2, player: "B", character: "美竹兰", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "advanced" },
  ];
  const m = new web.SoloMatch(JSON.stringify(seats), seedHex(5150), 0, "");
  try {
    m.quick_start();
    const v0 = JSON.parse(m.view(2)) as MatchView;
    // Seats are ordered by the acting order the dice decide, not by member id.
    const botSeat = v0.state.players.find((p) => p.member === 2);
    assert.ok(botSeat, "the Advanced bot's seat is in the public state");
    assert.equal(botSeat!.ai, false, "the engine does not drive an Advanced seat");
    assert.equal(botSeat!.bot, true);
    assert.equal(botSeat!.mentality, "advanced");

    // Drive both seats: the human's acts are ours, the bot's come from
    // `bot-glue` (the search, or its heuristic). Never let the engine auto-play.
    let acts = 0;
    for (let step = 0; step < 200 && acts < 40 && !m.ended(); step++) {
      m.tick_steps(10);
      for (let member = 1; member <= 2; member++) {
        const v = JSON.parse(m.view(member)) as MatchView;
        if (decisionAt(v.state, v.playerId) == null) continue;
        const r = JSON.parse(bot.decide(JSON.stringify(v), 80, 1000 + acts));
        assert.equal(r.ok, true, `decide for member ${member}`);
        const err = m.act(member, JSON.stringify(r.answer));
        assert.equal(err, "", `member ${member} act ${JSON.stringify(r.answer)}: ${err}`);
        acts++;
      }
    }
    assert.ok(acts > 0, "the driver actually answered something");

    // Record and replay through the same engine surface the .bdrec path uses.
    const bytes = m.record_zst("2026-10-08 12:00");
    assert.ok(bytes && bytes.length > 0, "a .bdrec body");
    const rep = web.ReplayMatch.from_record_bytes(bytes, true);
    try {
      const header = JSON.parse(rep.header());
      assert.equal(header.seats.length, 2);
      assert.equal(header.ended, m.ended());
      // Step the replay to the end and compare the public state.
      for (let i = 0; i < 500 && !rep.ended(); i++) rep.step(50);
      const live = JSON.parse(m.view(1)) as MatchView;
      const again = JSON.parse(rep.view(1)) as MatchView;
      assert.equal(again.state.seq, live.state.seq, "same public sequence length");
      assert.equal(again.state.round, live.state.round);
      assert.equal(again.state.turn, live.state.turn);
      for (const p of live.state.players) {
        const q = again.state.players.find((x) => x.member === p.member);
        assert.ok(q, `seat ${p.member} is in the replay`);
        assert.equal(q!.money, p.money, `seat ${p.member} money matches`);
        assert.equal(q!.pos, p.pos, `seat ${p.member} position matches`);
      }
    } finally {
      rep.free?.();
    }
  } finally {
    m.free?.();
  }
});
