// The advanced-bot search worker (`docs/BOT.md` B6). One instance per pool
// slot: it loads its own `bot-glue` wasm bundle (game-core + game-rules +
// bot-core -- never the page's glue), feeds it the same game tables the page
// loads, and answers `decide` / `ponder` / `extras` for ONE seat's view.
//
// Default bundle is the **monolith** (docs/BOT.md §3.6): the card ruleset is
// linked into `glue_bg.wasm`, so this worker does **not** fetch
// `/assets/rules/*.wasm`. It only fetches `index.json` (small) and refuses to
// answer when its `ruleset_sha256` disagrees with the linked ruleset -- the
// monolith is built from one commit and can silently drift from whatever
// `dist/cards` the page deployed. A fallback wasmi build
// (`--no-default-features`) still loads the modules the page loads.
//
// Kept as a plain ES module in webui/public/ (not bundled) on purpose: it
// dynamically imports `./bot-glue/glue.js` at runtime, which no bundler should
// rewrite. The message contract is mirrored by `webui/src/game/botPool.ts`.
//
// Information boundary (`docs/BOT.md` §1): `view` is the exact per-member frame
// the page renders for that seat. Never a World, a match seed, or another
// seat's hidden information. The `seed` is the **search** RNG's.
//
// Messages in  ({id, op, ...}):  init | decide | act | ponder | extras | invalidate | close
// Messages out ({id, ok, value} | {id, ok:false, error})

let glue = null;

/**
 * The card-ruleset load sequence. **Same contract as
 * `webui/src/core/rulesetLoad.ts`** (index.json -> modules -> the
 * precompiled-condition blob `conds-<sha>.bin` -> `ruleset_build`); the node
 * gate `webui/src/game/botPool.test.ts` runs both against the real built
 * ruleset and asserts they agree, so the two cannot drift. Inlined here
 * because a plain public/ worker cannot import from src/.
 */
async function loadRulesetInto(g, read) {
  const indexText = new TextDecoder().decode(await read("index.json"));
  if (indexText.trimStart().startsWith("<")) {
    throw new Error("index.json: got HTML (the ruleset was not built -- run tools/build-ruleset.mjs)");
  }
  const index = JSON.parse(indexText);
  for (const m of index.modules) {
    g.ruleset_add(await read(m.file));
  }
  if (index.conds?.file) {
    g.ruleset_precompiled(await read(index.conds.file));
  }
  return g.ruleset_build();
}

/**
 * Monolith path: the rules are compiled in. Fetch `index.json` only and
 * refuse to answer when the page's deployed ruleset is not the one this
 * bundle was built from.
 */
async function verifyLinkedRuleset(g, read) {
  const indexText = new TextDecoder().decode(await read("index.json"));
  if (indexText.trimStart().startsWith("<")) {
    throw new Error("index.json: got HTML (the ruleset was not built -- run tools/build-ruleset.mjs)");
  }
  const index = JSON.parse(indexText);
  const want = index.ruleset_sha256;
  const have = g.ruleset_sha256();
  if (!want) {
    throw new Error(
      "index.json has no ruleset_sha256 -- cannot verify the linked bot ruleset " +
        "(run tools/build-ruleset.mjs)",
    );
  }
  if (!have) {
    throw new Error(
      "bot-glue was built without a ruleset identity (BOT_GLUE_RULESET_SHA256 / " +
        "dist/cards/index.json) -- rebuild with node tools/build-bot-glue.mjs",
    );
  }
  if (have !== want) {
    throw new Error(
      `linked ruleset mismatch: bot-glue ${have} != page index.json ${want}. ` +
        "The monolith is built from one commit; rebuild it from the same tree " +
        "as dist/cards (node tools/build-bot-glue.mjs) and reload the workers.",
    );
  }
  return g.ruleset_build();
}

async function init(base) {
  const baseHref = new URL(base, self.location.href).href;
  glue = await import(new URL("glue.js", baseHref).href);
  await glue.default();
  // Game tables from /data (the same files `webui/src/core/data.ts` fetches).
  const names = JSON.parse(glue.data_files());
  const files = {};
  await Promise.all(
    names.map(async (n) => {
      const r = await fetch(new URL(`../../../data/${n}`, baseHref));
      if (!r.ok) throw new Error(`${n}: HTTP ${r.status}`);
      files[n] = await r.text();
    }),
  );
  // Optional deck book (docs/BOT.md §3.7) -- absent = empty book.
  try {
    const book = await fetch(new URL("../../../data/deck_book.json", baseHref));
    if (book.ok) files["deck_book.json"] = await book.text();
  } catch {
    /* absent book is fine */
  }
  // Optional strategy book (docs/BOT.md §3.8) -- absent = default params.
  try {
    const book = await fetch(new URL("../../../data/strategy_book.json", baseHref));
    if (book.ok) files["strategy_book.json"] = await book.text();
  } catch {
    /* absent book is fine */
  }
  glue.load_data(JSON.stringify(files));
  // Ruleset: linked (monolith) or fetched (wasmi fallback). Data-then-ruleset
  // is safe: the glue rebuilds GameData from the kept tables plus the set's
  // roster at `ruleset_build` (was `cards.json` etc.).
  const rulesBase = new URL("../../../assets/rules/", baseHref);
  const read = async (rel) => {
    const r = await fetch(new URL(rel, rulesBase));
    if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
    return new Uint8Array(await r.arrayBuffer());
  };
  const linked = typeof glue.rules_linked === "function" ? glue.rules_linked() : false;
  const n = linked ? await verifyLinkedRuleset(glue, read) : await loadRulesetInto(glue, read);
  return {
    modules: n,
    linked,
    info: JSON.parse(glue.info()),
  };
}

function decide(view, budgetMs, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.decide(JSON.stringify(view), budgetMs | 0, seed >>> 0));
}

/**
 * The policy's command for one seat, with **no search** -- the standard /
 * chaos path (`docs/BOT-DRIVE.md`). `seed` is the determinizer's sampling
 * seed; the answer is `null` when the seat has nothing to decide.
 */
function act(view, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.act(JSON.stringify(view), seed >>> 0));
}

function ponder(view, budgetMs, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.ponder(JSON.stringify(view), budgetMs | 0, seed >>> 0));
}

/**
 * Per-viewer extras from ONE seat's frame (`{playable, estCost, skills,
 * aiAnswer}`). The online path's client-side twin of the engine's
 * `view_extra` -- the server no longer ships those fields to human seats.
 * `seed` is the determinizer's sampling seed (never the match RNG).
 */
function extras(view, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.extras(JSON.stringify(view), seed >>> 0));
}

/** Drop a cached answer after the engine refused it (docs/BOT.md §5 B6). */
function invalidate(decisionKey) {
  if (!glue) throw new Error("init first");
  return glue.invalidate(String(decisionKey ?? ""));
}

self.onmessage = async (e) => {
  const { id, op, ...rest } = e.data ?? {};
  try {
    let value;
    if (op === "init") value = await init(rest.base);
    else if (op === "decide") value = decide(rest.view, rest.budgetMs, rest.seed);
    else if (op === "ponder") value = ponder(rest.view, rest.budgetMs, rest.seed);
    else if (op === "act") value = act(rest.view, rest.seed);
    else if (op === "extras") value = extras(rest.view, rest.seed);
    else if (op === "invalidate") value = invalidate(rest.decisionKey);
    else if (op === "close") {
      glue = null;
      self.postMessage({ id, ok: true, value: true });
      self.close();
      return;
    } else throw new Error(`unknown op ${op}`);
    self.postMessage({ id, ok: true, value });
  } catch (err) {
    self.postMessage({ id, ok: false, error: String(err?.message ?? err) });
  }
};