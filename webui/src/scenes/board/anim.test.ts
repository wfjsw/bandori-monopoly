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