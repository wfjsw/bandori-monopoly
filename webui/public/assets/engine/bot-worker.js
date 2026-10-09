// The advanced-bot search worker (`docs/BOT.md` B6). One instance per pool
// slot: it loads its own `bot-glue` wasm bundle (game-core + game-rules wasmi +
// bot-core -- never the page's glue), feeds it the same game tables and card
// modules the page loads (`webui/src/core/rulesetLoad.ts`'s sequence), and
// answers `decide` / `ponder` for ONE seat's view.
//
// Kept as a plain ES module in webui/public/ (not bundled) on purpose: it
// dynamically imports `./bot-glue/glue.js` at runtime, which no bundler should
// rewrite. The message contract is mirrored by `webui/src/game/botPool.ts`.
//
// Information boundary (`docs/BOT.md` §1): `view` is the exact per-member frame
// the page renders for that seat. Never a World, a match seed, or another
// seat's hidden information. The `seed` is the **search** RNG's.
//
// Messages in  ({id, op, ...}):  init | decide | ponder | invalidate | close
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
  // The ruleset the page loads (`/assets/rules/`) -- same bytes, same sequence.
  const rulesBase = new URL("../../../assets/rules/", baseHref);
  const n = await loadRulesetInto(glue, async (rel) => {
    const r = await fetch(new URL(rel, rulesBase));
    if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
    return new Uint8Array(await r.arrayBuffer());
  });
  return {
    modules: n,
    info: JSON.parse(glue.info()),
  };
}

function decide(view, budgetMs, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.decide(JSON.stringify(view), budgetMs | 0, seed >>> 0));
}

function ponder(view, budgetMs, seed) {
  if (!glue) throw new Error("init first");
  return JSON.parse(glue.ponder(JSON.stringify(view), budgetMs | 0, seed >>> 0));
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