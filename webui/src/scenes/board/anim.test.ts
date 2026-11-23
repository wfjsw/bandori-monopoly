// Run with: node --test webui/src/scenes/board/anim.test.ts
//
// Ruling 1 + the field-pulse reconciliation + v55 once-per-flash:
// - a field card with a self-only effect neither pulses nor flashes;
// - a cross-player effect pulses once;
// - a negated activation always shows;
// - replaying the same event frames twice plays each flash once.
// The decision never reads the header position, `results`, or `parent`.
//
// `Animator` itself is imported through a tiny ESM hook shim (below) so the
// real queue / `cardFlash` path runs under node --test; the shim only fills in
// DOM / React / CSS / wasm leaves the flash path never executes.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { existsSync, readFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import path from "node:path";
import { registerHooks } from "node:module";
import { activationPresentation, FlashOnce, flashPlay } from "./presentation.ts";
import type { PresentationEvent } from "./presentation.ts";

// -- node-side shim so `import { Animator } from "./anim.ts"` links -----------

const g = globalThis as Record<string, unknown>;
if (!g.localStorage) {
  g.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
}
if (!g.document) g.document = { documentElement: { lang: "zh-CN" } };
// `core/audio.ts` touches `window` at module scope; node has none.
if (!g.window) g.window = g;

const STUB = new Map<string, string>();
const stub = (spec: string, source: string): string => {
  const u = `stub:${spec}`;
  STUB.set(u, source);
  return u;
};

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "react" || specifier === "react-i18next" || specifier === "i18next") {
      return {
        url: stub(specifier, `
          export const useEffect = () => {};
          export const useReducer = () => [0, () => {}];
          export const useRef = (v) => ({ current: v });
          export const useState = (v) => [typeof v === "function" ? v() : v, () => {}];
          export const useSyncExternalStore = (_sub, snapshot) => snapshot();
          export const initReactI18next = { type: "3rdParty", init: () => {} };
          export default { t: (k) => k, use: () => ({ init: async () => {} }), init: async () => {}, changeLanguage: async () => {} };
        `),
        shortCircuit: true,
      };
    }
    if (specifier.endsWith(".css") || specifier.endsWith(".module.css")) {
      return { url: stub(`css:${specifier}`, "export default {};"), shortCircuit: true };
    }
    if ((specifier.startsWith("./") || specifier.startsWith("../")) && !/\.[cm]?[jt]sx?$/.test(specifier)) {
      const parent = context.parentURL ? fileURLToPath(context.parentURL) : process.cwd();
      const base = path.resolve(path.dirname(parent), specifier);
      for (const ext of [".ts", ".tsx", ".js", ".mjs", ".jsx", "/index.ts", "/index.tsx", "/index.js"]) {
        const cand = base + ext;
        if (existsSync(cand)) return nextResolve(pathToFileURL(cand).href, context);
      }
    }
    return nextResolve(specifier, context);
  },
  load(url, _context, nextLoad) {
    if (url.startsWith("stub:")) {
      return { format: "module", source: STUB.get(url) ?? "export default {};", shortCircuit: true };
    }
    if (url.endsWith(".css")) return { format: "module", source: "export default {};", shortCircuit: true };
    if (url.endsWith(".tsx")) {
      // Link-only stub: Animator never renders these.
      return {
        format: "module",
        source: `
          export default function Stub() { return null; }
          export const toast = () => {};
          export const showEffect = () => {};
          export const Btn = () => null;
          export const InspectCard = () => null;
          export const Avatar = () => null;
          export const openModal = () => {};
          export const SkillBody = () => null;
          export const cx = (...a) => a.filter(Boolean).join(" ");
        `,
        shortCircuit: true,
      };
    }
    if (url.endsWith(".json")) {
      const data = JSON.parse(readFileSync(fileURLToPath(url), "utf8"));
      return { format: "module", source: `export default ${JSON.stringify(data)};`, shortCircuit: true };
    }
    return nextLoad(url, _context);
  },
});

const { Animator } = await import("./anim.ts");

// -- helpers ----------------------------------------------------------------

function ev(over: Partial<PresentationEvent> & Record<string, unknown>): PresentationEvent {
  return {
    id: 1,
    type: "card",
    playerId: 0,
    other: -1,
    value: 0,
    from: 0,
    to: 0,
    dice: 0,
    card: "TEST:field",
    msg: { k: "log.effect_applied" } as PresentationEvent["msg"],
    kind: "hook",
    negated: false,
    parent: -1,
    results: [],
    uid: 7,
    flash: true,
    ...over,
  };
}

/** One wire event for `Animator.push`. */
function frame(over: Partial<Record<string, unknown>> & { id: number; type: string }) {
  return {
    playerId: 0,
    other: -1,
    value: 0,
    from: 0,
    to: 0,
    dice: 0,
    card: "",
    msg: { k: "log.x" },
    kind: "",
    negated: false,
    parent: -1,
    results: [],
    uid: -1,
    flash: true,
    ...over,
  } as never;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Construct an Animator and drive its queue. */
function makeAnim() {
  const bumps: number[] = [];
  const anim = new Animator(() => bumps.push(Date.now()), () => null, false);
  anim.speed = 2; // speed > 2 skips flashes (`noFlash`); 2 halves the holds
  return { anim, bumps };
}

// -- activationPresentation -------------------------------------------------

test("a field card with a self-only effect neither pulses nor flashes", () => {
  const e = ev({ flash: false });
  assert.equal(activationPresentation(e), "none");
});

test("a field card with a cross-player effect pulses once (field slot)", () => {
  const e = ev({ flash: true, uid: 7, msg: { k: "log.effect_applied" } as PresentationEvent["msg"] });
  assert.equal(activationPresentation(e), "pulse");
});

test("a hand play flashes the centre face (no field uid)", () => {
  const e = ev({ uid: -1, msg: { k: "" } as PresentationEvent["msg"], kind: "play" });
  assert.equal(activationPresentation(e), "flash");
});

test("a negated activation still shows (无效 is not suppressed by flash)", () => {
  const e = ev({ flash: false, negated: true });
  assert.notEqual(activationPresentation(e), "none");
});

test("the decision never reads header position / results / parent", () => {
  // Two events identical except for parent / results / id must decide alike.
  const a = ev({ flash: true });
  const b = ev({ flash: true, parent: 42, id: 99, results: [{ k: "log.effect_nothing" } as never] });
  assert.equal(activationPresentation(a), activationPresentation(b));
});

test("activationPresentation: none when flash === false and not negated", () => {
  assert.equal(activationPresentation(ev({ flash: false, negated: false, uid: -1, kind: "play" })), "none");
});

test("activationPresentation: flash for a normal activation", () => {
  assert.equal(activationPresentation(ev({ flash: true, uid: -1, kind: "play", msg: { k: "" } as never })), "flash");
});

test("activationPresentation: pulse for a field-card effect_applied", () => {
  assert.equal(
    activationPresentation(ev({ flash: true, uid: 7, kind: "hook", msg: { k: "log.effect_applied" } as never })),
    "pulse",
  );
});

test("activationPresentation: a proactive \"flash\" cue always shows", () => {
  assert.equal(activationPresentation(ev({ type: "flash", flash: false, uid: -1, kind: "" })), "flash");
});

// -- once-per-flash (pure model, wired into Animator.flashOnce) --------------

test("flashPlay: a re-delivered frame presents as none", () => {
  const once = new FlashOnce();
  const e = ev({ id: 3, type: "card", uid: -1, kind: "play", flash: true });
  assert.equal(flashPlay(once, e), "flash");
  assert.equal(flashPlay(once, e), "none");
});

test("flashPlay: history seal (addLog on mount) blocks a later play", () => {
  const once = new FlashOnce();
  const e = ev({ id: 4, type: "card", uid: -1, kind: "play", flash: true });
  once.seal(e.id!);
  assert.equal(flashPlay(once, e), "none");
});

test("flashPlay: different ids each play once", () => {
  const once = new FlashOnce();
  assert.equal(flashPlay(once, ev({ id: 10, uid: -1, kind: "play" })), "flash");
  assert.equal(flashPlay(once, ev({ id: 11, uid: -1, kind: "play" })), "flash");
  assert.equal(flashPlay(once, ev({ id: 10, uid: -1, kind: "play" })), "none");
  assert.equal(flashPlay(once, ev({ id: 11, uid: -1, kind: "play" })), "none");
});

// -- (g) required: Animator plays each flash once across re-delivered frames

test("replaying the same event frames twice plays each flash once", async () => {
  const { anim } = makeAnim();
  const card = frame({
    id: 21,
    type: "card",
    card: "TEST:play",
    kind: "play",
    uid: -1,
    msg: { k: "log.played" },
    flash: true,
  });
  const cue = frame({
    id: 22,
    type: "flash",
    card: "TEST:cue",
    kind: "",
    uid: -1,
    msg: { k: "log.look" },
    flash: true,
    parent: 21,
  });
  const frames = [card, cue];

  const plays: string[] = [];
  const note = () => {
    const f = anim.flash;
    if (f && !plays.includes(`${f.actId}:${f.card}`)) plays.push(`${f.actId}:${f.card}`);
  };

  /** Drain the queue and the current face; sample every flash that shows. */
  const drain = async () => {
    for (let i = 0; i < 400 && (anim.animating || anim.flash); i++) {
      note();
      await sleep(10);
    }
    note();
    assert.equal(anim.animating, false);
  };

  for (const e of frames) anim.push(e);
  await drain();
  // First delivery: both the activation and the proactive cue flash.
  assert.deepEqual(plays, ["21:TEST:play", "22:TEST:cue"]);
  assert.equal(anim.flash, null);

  for (const e of frames) anim.push(e);
  await drain();
  // Replaying the same frames must not grow the play list.
  assert.deepEqual(plays, ["21:TEST:play", "22:TEST:cue"]);
  assert.equal(anim.flash, null);

  // And the log kept each row once (addLog dedupes by id too).
  const ids = anim.log.map((l) => l.id);
  assert.equal(ids.filter((id) => id === 21).length, 1);
  assert.equal(ids.filter((id) => id === 22).length, 1);
});

test("Animator: addLog history ingest seals the flash id (mount replay)", async () => {
  const { anim } = makeAnim();
  const card = frame({
    id: 31,
    type: "card",
    card: "TEST:hist",
    kind: "play",
    uid: -1,
    msg: { k: "log.played" },
    flash: true,
  });
  // Mount-time ingest of events already in the state: log only, no animation.
  anim.addLog(card, { history: true });
  assert.equal(anim.flash, null);
  assert.equal(anim.log.length, 1);

  // A later re-delivery of the same frame must not flash it now.
  anim.push(card);
  for (let i = 0; i < 30 && anim.animating; i++) await sleep(10);
  assert.equal(anim.flash, null);
  // Still a single log row.
  assert.equal(anim.log.length, 1);
});

test("Animator: a negated activation shows even when flash === false", async () => {
  const { anim } = makeAnim();
  anim.push(frame({
    id: 41,
    type: "card",
    card: "TEST:neg",
    kind: "counter",
    uid: -1,
    msg: { k: "log.played" },
    flash: false,
    negated: true,
  }));
  for (let i = 0; i < 30 && !anim.flash; i++) await sleep(10);
  assert.ok(anim.flash, "negated activation must show");
  assert.equal(anim.flash?.negated, true);
  assert.equal(anim.flash?.actId, 41);
});

test("Animator: owner-self-only (flash === false) stays in the log, no flash", async () => {
  const { anim } = makeAnim();
  anim.push(frame({
    id: 51,
    type: "card",
    card: "TEST:self",
    kind: "play",
    uid: -1,
    msg: { k: "log.played" },
    flash: false,
    negated: false,
  }));
  for (let i = 0; i < 30; i++) await sleep(10);
  assert.equal(anim.flash, null);
  assert.equal(anim.log.length, 1);
});

test("Animator: a board-rule card event (status / tile) keeps the log, no empty flash", async () => {
  const { anim } = makeAnim();
  anim.push(frame({
    id: 91,
    type: "card",
    card: "status:no_hand",
    kind: "hook",
    uid: -1,
    msg: { k: "log.effect_applied" },
    flash: true,
  }));
  for (let i = 0; i < 40; i++) await sleep(10);
  assert.equal(anim.flash, null, "no empty face comes up");
  assert.equal(anim.log.filter((l) => l.id === 91).length, 1, "the line still logs");
});

test("Animator: a real card activation still flashes", async () => {
  const { anim } = makeAnim();
  anim.push(frame({
    id: 92,
    type: "card",
    card: "TEST:real",
    kind: "play",
    uid: -1,
    msg: { k: "log.played" },
    flash: true,
  }));
  let flashed = false;
  for (let i = 0; i < 60 && !flashed; i++) { flashed = !!anim.flash; await sleep(10); }
  assert.ok(flashed);
  assert.equal(anim.flash?.card, "TEST:real");
});

test("Animator: a \"flash\" tile cue (no card) banners once and dedupes", async () => {
  const { anim } = makeAnim();
  // Cue text only (no `tile` arg -- node has no game data to resolve one).
  const cue = frame({
    id: 61,
    type: "flash",
    card: "",
    kind: "",
    uid: -1,
    msg: { k: "log.lookTile" },
    flash: true,
    parent: -1,
  });
  anim.push(cue);
  for (let i = 0; i < 30 && !anim.banner; i++) await sleep(10);
  assert.ok(anim.banner, "tile cue shows a banner");
  const bannerId = anim.banner?.id;
  // Let the banner time out, then re-deliver.
  for (let i = 0; i < 80 && anim.banner; i++) await sleep(20);
  anim.push(cue);
  for (let i = 0; i < 30; i++) await sleep(10);
  // Either no new banner, or the same presentation id (not a second play).
  if (anim.banner) assert.equal(anim.banner.id, bannerId);
  assert.equal(anim.log.filter((l) => l.id === 61).length, 1);
});
test("Animator: two same-name field cards pulse one after the other", async () => {
  const bumps: number[] = [];
  const field = (uid: number) => ({ uid, card: "TEST:twin", faceDown: false });
  const view = { state: { players: [{ field: [field(70), field(71)] }], prompt: { id: 0 } } } as never;
  const anim = new Animator(() => bumps.push(Date.now()), () => view, false);
  const seen: number[] = [];
  const t0 = Date.now();
  const stamps: Array<[number, number]> = [];
  const poll = setInterval(() => {
    const u = anim.pulse?.uid ?? -1;
    if (seen[seen.length - 1] !== u) { seen.push(u); stamps.push([u, Date.now() - t0]); }
  }, 5);
  for (const [id, uid] of [[61, 70], [63, 71]]) {
    anim.push(frame({ id, type: "card", card: "TEST:twin", kind: "hook", uid, msg: { k: "log.effect_applied" }, flash: true }));
  }
  for (let i = 0; i < 600 && (anim.animating || anim.pulse); i++) await sleep(10);
  clearInterval(poll);
  console.log(JSON.stringify(stamps));
  assert.deepEqual(seen.filter((u) => u >= 0), [70, 71]);
});

test("Animator: a face-down card's slot pulses for its owner only", async () => {
  const view = { playerId: 0, state: { players: [{ field: [
    { uid: 90, card: "TEST:down", owner: 0, faceDown: true },
    { uid: 91, card: "TEST:down", owner: 1, faceDown: true },
  ] }], prompt: { id: 0 } } } as never;
  const anim = new Animator(() => {}, () => view, false);
  const seen: number[] = [];
  const poll = setInterval(() => { if (anim.pulse) seen.push(anim.pulse.uid); }, 5);
  anim.push(frame({ id: 71, type: "card", card: "TEST:down", kind: "hook", uid: 90, msg: { k: "log.effect_applied" }, flash: true }));
  for (let i = 0; i < 600 && (anim.animating || anim.pulse); i++) await sleep(10);
  clearInterval(poll);
  assert.deepEqual([...new Set(seen)], [90], "the owner's face-down card lights its slot");

  // Someone else's face-down card has no face to light: the centre flash plays.
  anim.push(frame({ id: 73, type: "card", card: "TEST:down", kind: "hook", uid: 91, msg: { k: "log.effect_applied" }, flash: true }));
  let flashed = false;
  for (let i = 0; i < 60 && !flashed; i++) { flashed = !!anim.flash; await sleep(10); }
  assert.ok(flashed, "centre flash shows");
  assert.equal(anim.pulse, null);
});

test("Animator: a skill rule's hook (not drawn in the field row) keeps the centre flash", async () => {
  const view = { state: { players: [{ field: [{ uid: 5, card: "skill:A:B", faceDown: false }] }], prompt: { id: 0 } } } as never;
  const anim = new Animator(() => {}, () => view, false);
  anim.push(frame({ id: 81, type: "card", card: "skill:A:B", kind: "hook", uid: 5, msg: { k: "log.effect_applied" }, flash: true }));
  let flashed = false;
  for (let i = 0; i < 60 && !flashed; i++) { flashed = !!anim.flash; await sleep(10); }
  assert.ok(flashed, "centre flash shows");
  assert.equal(anim.pulse, null);
});

// -- chain overlay: cards only, two before it shows -------------------------

/** Push frames and sample `chain` while the queue drains. */
async function chainShots(frames: never[]) {
  const { anim } = makeAnim();
  const shots: { shown: boolean; links: { seq: number; card: string }[] }[] = [];
  const poll = setInterval(() => {
    if (anim.chain) shots.push({ shown: anim.chain.shown, links: anim.chain.links.map((l) => ({ seq: l.seq, card: l.card })) });
  }, 5);
  for (const e of frames) anim.push(e);
  for (let i = 0; i < 600 && anim.animating; i++) await sleep(10);
  clearInterval(poll);
  return { anim, shots };
}

test("chain: a non-card link 1 joins no overlay; the first card stays a lone flash", async () => {
  const { anim, shots } = await chainShots([
    frame({ id: 101, type: "chain", card: "", value: 1, msg: { k: "log.chain_link_trigger" } }),
    frame({ id: 102, type: "chain", card: "TEST:counter", value: 2, msg: { k: "log.chain_link" } }),
  ]);
  assert.ok(anim.chain, "the chain itself exists");
  assert.deepEqual(anim.chain?.links.map((l) => l.seq), [2], "only the card is a link");
  assert.equal(anim.chain?.links.every((l) => l.card), true);
  assert.equal(shots.some((s) => s.shown), false, "one card never shows the overlay");
  assert.equal(anim.chain?.done.length, 0);
});

test("chain: a status / tile / mark link 1 has no face and is dropped", async () => {
  for (const [i, id] of ["status:no_hand", "tile:circle", "mark:cp"].entries()) {
    const { anim, shots } = await chainShots([
      frame({ id: 120 + i * 2, type: "chain", card: id, value: 1, msg: { k: "log.chain_link_trigger" } }),
      frame({ id: 121 + i * 2, type: "chain", card: "TEST:counter", value: 2, msg: { k: "log.chain_link" } }),
    ]);
    assert.deepEqual(anim.chain?.links.map((l) => l.card), ["TEST:counter"], id);
    assert.equal(shots.some((s) => s.shown), false, id);
  }
});

test("chain: an event still counts as a card link", async () => {
  const { anim, shots } = await chainShots([
    frame({ id: 130, type: "chain", card: "event:some_event", value: 1, msg: { k: "log.chain_link" } }),
    frame({ id: 131, type: "chain", card: "TEST:counter", value: 2, msg: { k: "log.chain_link" } }),
  ]);
  assert.deepEqual(anim.chain?.links.map((l) => l.card), ["event:some_event", "TEST:counter"]);
  assert.ok(shots.some((s) => s.shown), "two card faces raise the overlay");
});

test("chain: two cards show the overlay with both", async () => {
  const { anim, shots } = await chainShots([
    frame({ id: 111, type: "chain", card: "TEST:a", value: 1, msg: { k: "log.chain_link" } }),
    frame({ id: 112, type: "chain", card: "TEST:b", value: 2, msg: { k: "log.chain_link" } }),
  ]);
  assert.deepEqual(anim.chain?.links.map((l) => l.card), ["TEST:a", "TEST:b"]);
  const shown = shots.filter((s) => s.shown).at(-1);
  assert.ok(shown, "two cards raise the overlay");
  assert.deepEqual(shown!.links.map((l) => l.card), ["TEST:a", "TEST:b"]);
});

// -- full history -----------------------------------------------------------

test("Animator: the log keeps the whole match (no tail cap)", () => {
  const { anim } = makeAnim();
  for (let id = 1; id <= 450; id++) {
    anim.addLog(frame({ id, type: "pass", msg: { k: "log.x" } }), { history: true });
  }
  assert.equal(anim.log.length, 450);
  assert.equal(anim.log[0].id, 1);
  assert.equal(anim.log[449].id, 450);
});

test("Animator: the log keeps more than the old 200-row cap across stage lines", () => {
  const { anim } = makeAnim();
  for (let id = 1; id <= 260; id++) {
    anim.addLog(frame({ id, type: "pass", msg: { k: "log.x" } }), { history: true });
    if (id % 13 === 0) anim.stageTo(1, { step: 2, newTurn: false });
  }
  assert.equal(anim.log.filter((l) => !l.stage).length, 260);
  assert.equal(anim.log[0].id, 1);
});

test("Animator: mergeHistory slots older rows in id order, ahead of the tail", () => {
  const { anim } = makeAnim();
  for (const id of [100, 101]) anim.addLog(frame({ id, type: "pass", msg: { k: "log.x" } }), { history: true });
  // A second, newer event arrives before the fetch resolves.
  anim.addLog(frame({ id: 102, type: "pass", msg: { k: "log.x" } }), { history: true });

  anim.mergeHistory([10, 11, 100, 12].map((id) => frame({ id, type: "pass", msg: { k: "log.x" } })));

  assert.deepEqual(anim.log.map((l) => l.id), [10, 11, 12, 100, 101, 102]);
  // A re-delivered fetch (or stream replay) adds nothing.
  anim.mergeHistory([10, 11, 12].map((id) => frame({ id, type: "pass", msg: { k: "log.x" } })));
  assert.equal(anim.log.length, 6);
});

test("Animator: mergeHistory puts history before a live stage line, not after it", () => {
  const { anim } = makeAnim();
  for (const id of [10, 11]) anim.addLog(frame({ id, type: "pass", msg: { k: "log.x" } }), { history: true });
  // A stage change lands while the history fetch is in flight (negative id).
  anim.stageTo(1, { step: 2, newTurn: false });
  anim.addLog(frame({ id: 12, type: "pass", msg: { k: "log.x" } }), { history: true });

  anim.mergeHistory([5, 6].map((id) => frame({ id, type: "pass", msg: { k: "log.x" } })));

  assert.deepEqual(anim.log.slice(0, 2).map((l) => l.id), [5, 6]);
  assert.deepEqual(anim.log.map((l) => l.id), [5, 6, 10, 11, anim.log[4].id, 12]);
  assert.ok(anim.log[4].stage);
});

test("Animator: mergeHistory folds a walk's 继续前进 republish like the live ingest", () => {
  const { anim } = makeAnim();
  const mov = (id: number) => frame({ id, type: "pass", msg: { k: "log.move_on" } });
  anim.mergeHistory([mov(1), mov(2), mov(3)]);
  assert.equal(anim.log.length, 1);
  assert.equal(anim.log[0].repeat, 3);
  // Closing slot placement when the same event had already logged? No -- a
  // second event of another kind ends the run, and a later one starts afresh.
  anim.mergeHistory([frame({ id: 4, type: "pass", msg: { k: "log.x" } }), mov(5)]);
  assert.equal(anim.log.length, 3);
});

test("Animator: mergeHistory seals the flash ids it seeds", async () => {
  const { anim } = makeAnim();
  const card = frame({ id: 55, type: "card", card: "TEST:old", kind: "play", uid: -1, msg: { k: "log.played" }, flash: true });
  anim.mergeHistory([card]);
  assert.equal(anim.log.length, 1);
  anim.push(card);
  for (let i = 0; i < 30 && anim.animating; i++) await sleep(10);
  assert.equal(anim.flash, null);
  assert.equal(anim.log.length, 1);
});

// -- flash length is transport-independent ----------------------------------

/** Mount a centre-face flash with `backlog` trivial events behind it and
 *  sample its halo duration and total lifetime. */
async function flashHold(backlog: number): Promise<{ hit: number; rest: number; life: number }> {
  const anim = new Animator(() => {}, () => null, false);
  // A leading pass keeps the loop busy (250ms) while the card and its backlog
  // queue up, so the animator reaches the card with the queue already deep --
  // the same shape as an SSE batch, where `push` runs for every frame before
  // the loop takes the next item. Pushing the card alone would let it start
  // against an empty queue and never exercise the fast path.
  anim.push(frame({ id: 300 + backlog, type: "pass", msg: { k: "log.x" } }));
  const card = frame({ id: 90 + backlog, type: "card", card: "TEST:hold", kind: "play", uid: -1, msg: { k: "log.played" }, flash: true });
  anim.push(card);
  // Behind the card, like a batched online resolution: the queue stays deep
  // for the whole flash.
  for (let i = 0; i < backlog; i++) anim.push(frame({ id: 200 + backlog * 20 + i, type: "pass", msg: { k: "log.x" } }));
  for (let i = 0; i < 100 && !anim.flash; i++) await sleep(5);
  assert.ok(anim.flash, "flash must show");
  const shown = Date.now();
  let hitStart = 0;
  let hitEnd = 0;
  let outAt = 0;
  for (let i = 0; i < 700 && anim.flash; i++) {
    const f = anim.flash;
    if (f?.hit) {
      if (!hitStart) hitStart = Date.now();
    } else if (hitStart && f && !hitEnd) {
      hitEnd = Date.now();
    }
    if (f?.out) { outAt = Date.now(); break; }
    await sleep(5);
  }
  for (let i = 0; i < 200 && anim.flash; i++) await sleep(5);
  assert.equal(anim.flash, null);
  return { hit: hitEnd - hitStart, rest: hitEnd && outAt ? outAt - hitEnd : 0, life: Date.now() - shown };
}

test("Animator: the flash halo takes the field glow's share, and is short when deep", async () => {
  const shallow = await flashHold(0);
  const deep = await flashHold(12); // > FAST_QUEUE
  // A field slot lives PULSE_MS (1500) shallow / PULSE_FAST_MS (800) deep and
  // its glow runs from 25% to 65% of that; the face copies the same clock and
  // must not outlive it.
  assert.ok(shallow.hit >= 450 && shallow.hit <= 800, `shallow glow ${shallow.hit}ms`);
  assert.ok(shallow.life >= 1300 && shallow.life <= 1750, `shallow face ${shallow.life}ms`);
  assert.ok(deep.hit >= 210 && deep.hit <= 460, `deep glow ${deep.hit}ms`);
  assert.ok(deep.life >= 650 && deep.life <= 1000, `deep face ${deep.life}ms`);
  assert.ok(shallow.rest >= 20, `no rest before the shallow fade: ${shallow.rest}ms`);
  assert.ok(deep.rest >= 20, `no rest before the deep fade: ${deep.rest}ms`);
  assert.ok(deep.hit < shallow.hit, "a deep queue must take the field's fast blink");
});

test("Animator: the flash hits once, then restores before fading", async () => {
  const anim = new Animator(() => {}, () => null, false);
  anim.push(frame({ id: 401, type: "card", card: "TEST:hit", kind: "play", uid: -1, msg: { k: "log.played" }, flash: true }));
  let hitStart = 0;
  let hitEnd = 0;
  let out = false;
  for (let i = 0; i < 400; i++) {
    const f = anim.flash;
    if (f && !f.out) {
      if (f.hit && !hitStart) hitStart = Date.now();
      if (hitStart && !f.hit && !hitEnd) hitEnd = Date.now();
    }
    if (f?.out) { out = true; break; }
    await sleep(10);
  }
  assert.ok(hitStart, "the flash must hit");
  assert.ok(hitEnd, "the hit must restore before the fade");
  assert.ok(out, "the face must then fade");
  assert.ok(hitEnd - hitStart >= 450, `hit shorter than the field glow: ${hitEnd - hitStart}ms`);
  assert.equal(anim.flash?.hit ?? false, false);
  for (let i = 0; i < 200 && anim.flash; i++) await sleep(10);
  assert.equal(anim.flash, null);
});

test("flash styles: the halo is a composited triangle on a chain-sized face", () => {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const css = readFileSync(path.join(here, "Board.module.css"), "utf8");
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const face = bare.match(/\.flashFace \{[^}]*\}/)?.[0] ?? "";
  assert.match(face, /--card-w:\s*150px/, "the face falls back to the chain's two-link width");
  assert.doesNotMatch(face, /--card-w:\s*100%/, "a percentage breaks min-height: calc(100% * 1.5)");
  const board = readFileSync(path.join(here, "Board.tsx"), "utf8");
  assert.match(board, /flashW = Math\.round\(CHAIN_CARD_W \* chainK\)/, "a flash uses the chain's scaled width");
  assert.match(board, /\["--card-w" as string\]: `\$\{flashW\}px`/, "the flash face is given that width inline");
  const card = bare.match(/\.flashCard \{[^}]*\}/)?.[0] ?? "";
  assert.doesNotMatch(card, /animation:/, "the glow must not be permanently attached");
  // The halo runs on its own composited layer: animating `filter` on the card
  // re-rasterises the art and title every frame and stuttered at 150px.
  const glowLayer = bare.match(/\.flashCard::after \{[^}]*\}/)?.[0] ?? "";
  assert.match(glowLayer, /position:\s*absolute/, "the halo leaves the flex flow");
  assert.match(glowLayer, /inset:\s*0/, "the halo covers the face");
  assert.match(glowLayer, /border-radius:\s*inherit/, "the halo follows the face's radius");
  assert.match(glowLayer, /will-change:\s*opacity/, "the halo needs its own layer");
  assert.match(glowLayer, /opacity:\s*0/, "it starts dark");
  assert.match(glowLayer, /box-shadow:/, "the halo carries the ring and bloom");
  const hitRule = bare.match(/\.flashHit::after \{[^}]*\}/)?.[0] ?? "";
  assert.match(hitRule, /var\(--hit-ms/, "the hit runs on the shared blink clock");
  assert.match(hitRule, /ease-in-out/, "the hit keeps the field blink's easing");
  assert.doesNotMatch(hitRule, /filter:/, "no filter animation on the hit");
  const field = readFileSync(path.join(here, "FieldSheet.module.css"), "utf8");
  assert.match(field, /@keyframes pulse-glow/, "the field blink the hit is copied from still exists");
  // Same 0/50/100 triangle as the field slot's `pulse-glow`: no held peak.
  const keyframes = (css: string, name: string): string => {
    const at = css.indexOf(`@keyframes ${name} {`);
    if (at < 0) return "";
    let depth = 0;
    for (let i = css.indexOf("{", at); i < css.length; i++) {
      if (css[i] === "{") depth++;
      else if (css[i] === "}" && --depth === 0) return css.slice(at, i + 1);
    }
    return "";
  };
  const glow = keyframes(bare, "flashGlow").replace(/\s+/g, " ");
  assert.match(glow, /0%, 100% \{ opacity: 0; \}/);
  assert.match(glow, /50% \{ opacity: 1; \}/);
  assert.doesNotMatch(bare, /4%,\s*94%/, "no held peak");
});

test("big cards: the title is at least 1.5x the old scale", () => {
  const css = readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), "../../ui/Card.module.css"), "utf8");
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const title = bare.match(/\.big \.title \{[^}]*\}/)?.[0] ?? "";
  assert.ok(title, "the big title rule exists");
  const m = /font-size:\s*calc\(var\(--card-w, 240px\) \* ([\d.]+)\)/.exec(title);
  assert.ok(m, "the big title must scale with --card-w");
  assert.ok(+m[1] >= 0.18, `title scale ${m[1]} is under 1.5x the old 0.12`);
});

test("Animator: a body line re-arms one hit, never during the fade", async () => {
  const anim = new Animator(() => {}, () => null, false);
  anim.push(frame({ id: 500, type: "card", card: "TEST:rearm", kind: "play", uid: -1, msg: { k: "log.played" }, flash: true }));
  for (let i = 0; i < 30 && !anim.flash; i++) await sleep(10);
  await sleep(200);
  anim.push(frame({ id: 501, type: "pass", parent: 500, msg: { k: "log.effect_nothing" } }));
  const before = anim.flash?.id;
  await sleep(10);
  assert.equal(anim.flash?.id, before, "the same face stays");
  assert.equal(anim.flash?.out, false, "the re-arm must not land in the fade");
  assert.equal(anim.flash?.detail.length, 1, "the line rides the face");
  for (let i = 0; i < 300 && anim.flash; i++) await sleep(10);
  assert.equal(anim.flash, null);
});
