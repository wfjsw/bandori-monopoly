// Deploy gate for the shipped card ruleset + the precompiled guard conditions
// (docs/GUARDS.md §8.2). Run with:
//   node --test webui/src/game/ruleset.test.ts
//
// Loads the REAL built ruleset into the REAL browser glue -- the same
// `loadRulesetInto` sequence `webui/src/core/data.ts` runs on the page -- and
// asserts it comes up with zero load errors and that a card with a `pre`
// actually evaluates. This is what broke when G4 first shipped conditions:
// the glue is `runtime-only` (no CEL parser) and nothing was shipping the
// compiled form to it, so every solo match silently lost its card rules.
//
// Needs `node tools/build-ruleset.mjs` and `node tools/build-glue.mjs` first
// (the deploy gate runs both before this).

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { loadRulesetInto, type RulesetGlue, type RulesetIndex } from "../core/rulesetLoad.ts";

/** The real `index.json`: module entries embed the card manifests, which is
 *  where `pre` sources live (`ManifestOn.pre`). */
type BuiltIndex = RulesetIndex & {
  ruleset_sha256: string;
  modules: {
    file: string;
    cards: { id: string; on: ({ pre?: string | null } | null)[] }[];
  }[];
};

const WEBUI = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ROOT = resolve(WEBUI, "..");
const WASM_DIR = join(WEBUI, "src", "wasm");
const DATA_DIR = join(ROOT, "data");

/** The built ruleset the page fetches (`/assets/rules/`), falling back to the
 *  indexer output. `tools/build-ruleset.mjs` publishes both. */
function rulesDir(): string {
  for (const d of [join(WEBUI, "public", "assets", "rules"), join(ROOT, "dist", "cards")]) {
    if (existsSync(join(d, "index.json"))) return d;
  }
  throw new Error("no built ruleset -- run tools/build-ruleset.mjs");
}

/** One glue instance: import under a unique query (node's ESM cache keys on
 *  the URL, so `?tag` gives a fresh wasm instance), init it, feed the game
 *  tables. Mirrors `webui/public/assets/engine/replay-worker.js` and
 *  `tools/test-replay-archive.mjs`. */
async function loadGlue(tag: string): Promise<{ glue: Record<string, any>; dir: string }> {
  const glueJs = join(WASM_DIR, "glue.js");
  const glueWasm = join(WASM_DIR, "glue_bg.wasm");
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
  return { glue, dir: rulesDir() };
}

/** `loadRulesetInto`'s reader over a real directory. */
function reader(dir: string) {
  return async (rel: string) => new Uint8Array(readFileSync(join(dir, rel)));
}

/** Every `(card, entry)` the built index says declares a `pre`, plus its
 *  source text -- from the same `index.json` the loader reads. */
function preSources(index: BuiltIndex): { card: string; entry: number; pre: string }[] {
  const out: { card: string; entry: number; pre: string }[] = [];
  for (const m of index.modules) {
    for (const c of m.cards ?? []) {
      (c.on ?? []).forEach((o, entry) => {
        if (o?.pre) out.push({ card: c.id, entry, pre: o.pre });
      });
    }
  }
  return out;
}

function readIndex(dir: string): BuiltIndex {
  return JSON.parse(readFileSync(join(dir, "index.json"), "utf8"));
}

/** Window/candidate snapshots for `ruleset_pre_eval`: `actor`/`owner`/roll
 *  knobs, everything else default. */
function probe(over: { actor?: number; owner?: number; roll?: number | null; moveKind?: number | null }) {
  const actor = over.actor ?? 0;
  const owner = over.owner ?? 0;
  const win = {
    kind: 0,
    actor,
    target: 0,
    value: 0,
    mv: { roll: over.roll === undefined ? 5 : over.roll, kind: over.moveKind === undefined ? 0 : over.moveKind },
    players: [{}, {}],
  };
  const cand = { owner, owner_money: 10_000 };
  return [JSON.stringify(win), JSON.stringify(cand)] as const;
}

// The card whose missing blob first broke the browser load (the report this
// gate exists for). `entry` is the `on[]` index the built index reports.
const REPORTED = { card: "AG:回家的路上绕个道", entry: 1 };

test("the real built ruleset loads into the browser glue exactly like the page", async () => {
  const { glue, dir } = await loadGlue("full");
  const index = readIndex(dir);
  const n = await loadRulesetInto(glue as unknown as RulesetGlue, reader(dir));
  assert.equal(n, index.modules.length, "every listed module loads");
  assert.ok(n > 0, "the ruleset is not empty");
  // Zero load errors: `ruleset_build` returned, so no `BadPre`, and the glue
  // has a live ruleset (not StubRules) -- `engine_stamp` reports its hash.
  const stamp = JSON.parse(glue.engine_stamp());
  assert.match(stamp.ruleset_sha256, /^[0-9a-f]{64}$/, "a real ruleset, not \"stub\"");
  assert.equal(stamp.ruleset_sha256, index.ruleset_sha256, "same identity the index published");
});

test("the shipped ruleset has precompiled conditions and they evaluate", async () => {
  const { glue, dir } = await loadGlue("eval");
  const index = readIndex(dir);
  const pres = preSources(index);
  assert.ok(pres.length > 0, "the gate is meaningless until a shipped card has a `pre`");
  assert.ok(index.conds?.file, "index.json must name the precompiled conds blob");
  assert.ok(existsSync(join(dir, index.conds!.file)), `and it must be shipped: ${index.conds!.file}`);

  await loadRulesetInto(glue as unknown as RulesetGlue, reader(dir));

  // The reported card: `actor == owner && move.roll != null && move.kind != Teleport`.
  // Walk == 0, Teleport == 1 (rules-cond kinds::mv).
  const found = pres.find((p) => p.card === REPORTED.card && p.entry === REPORTED.entry);
  if (found) {
    assert.equal(found.pre, "actor == owner && move.roll != null && move.kind != Teleport");
    const ev = (w: readonly [string, string]) => glue.ruleset_pre_eval(REPORTED.card, REPORTED.entry, w[0], w[1]);
    assert.equal(ev(probe({ actor: 0, owner: 0, roll: 5, moveKind: 0 })), true, "own move with a roll: yes");
    assert.equal(ev(probe({ actor: 1, owner: 0, roll: 5, moveKind: 0 })), false, "someone else's move: no");
    assert.equal(ev(probe({ actor: 0, owner: 0, roll: null, moveKind: 0 })), false, "no roll: no");
    assert.equal(ev(probe({ actor: 0, owner: 0, roll: 5, moveKind: 1 })), false, "teleport: no");
  } else {
    // Card renamed/retargeted: still exercise a live `pre` -- any `actor ==
    // owner` clause must distinguish seats.
    const mine = pres.find((p) => /\bactor == owner\b/.test(p.pre));
    assert.ok(mine, `no probeable pre among ${pres.length}; update this gate`);
    const ev = (w: readonly [string, string]) => glue.ruleset_pre_eval(mine.card, mine.entry, w[0], w[1]);
    assert.equal(ev(probe({ actor: 0, owner: 0 })), true, "own window: yes");
    assert.equal(ev(probe({ actor: 1, owner: 0 })), false, "other seat: no");
  }

  // Every shipped `pre` decodes and evaluates through the runtime-only path
  // (a missing blob entry would have failed the load above; a corrupt one
  // fails here).
  for (const p of pres) {
    const [w, c] = probe({});
    assert.equal(typeof glue.ruleset_pre_eval(p.card, p.entry, w, c), "boolean", `${p.card}/${p.entry} evaluates`);
  }
});

test("a card with a `pre` and no precompiled entry fails loudly", async () => {
  const { glue, dir } = await loadGlue("noblob");
  const index = readIndex(dir);
  assert.ok(index.conds?.file, "this test needs a ruleset that ships conditions");
  // Feed the modules and deliberately skip the blob -- the page must never
  // treat a missing condition as `true`.
  for (const m of index.modules) glue.ruleset_add(new Uint8Array(readFileSync(join(dir, m.file))));
  await assert.rejects(
    async () => void glue.ruleset_build(),
    (e: Error) => {
      assert.match(String(e), /BadPre|precompiled/i, `loud, not "treat as true": ${e}`);
      return true;
    },
  );
});

test("a listed conds blob that cannot be fetched fails loudly", async () => {
  const { glue, dir } = await loadGlue("404");
  const index = readIndex(dir);
  if (!index.conds?.file) return; // cond-less set: nothing to fetch
  const read = async (rel: string) => {
    if (rel === index.conds!.file) throw new Error(`${rel}: HTTP 404`);
    return reader(dir)(rel);
  };
  await assert.rejects(
    () => loadRulesetInto(glue as unknown as RulesetGlue, read),
    /404/,
  );
});