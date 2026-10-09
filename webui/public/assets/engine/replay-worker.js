// The replay driver worker (`docs/REPLAY.md` §9). One instance per archived
// engine bundle: it loads that bundle's glue (its own wasm instance -- never
// the page's), feeds it the bundle's frozen game tables and card modules, and
// drives `ReplayMatch` through the **stable replay API v1**. The current UI
// talks to any bundle through exactly this surface, so an old record plays
// with the engine that wrote it.
//
// Kept as a plain ES module in webui/public/ (not bundled) on purpose: it
// dynamically imports `<bundle>/glue.js` at runtime, which no bundler should
// rewrite. The message contract is mirrored by `webui/src/game/replayEngine.ts`.
//
// Messages in  ({id, op, ...}):  init | init-embedded | open | call | free | close
// Messages out ({id, ok, value} | {id, ok:false, error})
//
// `init-embedded` boots a **portable** record's own engine (`docs/REPLAY.md`
// §10). SECURITY: the record's `glue.js` is never imported -- the page hands
// this worker a URL of a loader *we* shipped (already hash-verified against
// the file's copy and the deployment's allow-list) plus the file's wasm and
// tables, all hash-checked before they got here. Once the engine is in, the
// globals an untrusted wasm could otherwise reach (fetch and friends) are
// replaced with throwing stubs. The wasm's own import surface is just
// `Error` / `throw` / the externref table -- see the audit in §10.4.

/** The frozen API: every method a driver may call on an engine bundle. */
const CALLS = new Set([
  "header",
  "compat",
  "step",
  "next_input",
  "index",
  "seek",
  "turns",
  "total_ticks",
  "view",
  "events_since",
  "take_changed",
  "ended",
  "status",
]);

let glue = null;
let match = null;

async function init(base) {
  const manifest = await (await fetch(new URL("bundle.json", base))).json();
  const L = manifest.layout;
  // The glue resolves `glue_bg.wasm` against its own URL, so importing it
  // from the bundle is enough -- a separate wasm instance per bundle.
  glue = await import(new URL(L.glueJs, base).href);
  await glue.default();
  const files = {};
  for (const name of L.dataFiles ?? JSON.parse(glue.data_files())) {
    const r = await fetch(new URL(`${L.dataDir}/${name}`, base));
    if (r.ok) files[name] = await r.text();
  }
  glue.load_data(JSON.stringify(files));
  const indexUrl = new URL(L.rulesIndex, base);
  const index = await (await fetch(indexUrl)).json();
  for (const m of index.modules ?? []) {
    const w = await fetch(new URL(`${L.modulesDir}/${m.file}`, base));
    if (!w.ok) throw new Error(`${m.file}: HTTP ${w.status}`);
    glue.ruleset_add(new Uint8Array(await w.arrayBuffer()));
  }
  // Precompiled guard conditions (docs/GUARDS.md §8.2): the bundle keeps
  // `conds-<sha>.bin` beside its rules index. Bundles from before conditions
  // existed carry neither the blob nor a `pre` -- nothing to load, unchanged.
  if (index.conds?.file) {
    if (typeof glue.ruleset_precompiled !== "function") {
      throw new Error("bundle ships precompiled conditions but its glue cannot load them");
    }
    const c = await fetch(new URL(index.conds.file, indexUrl));
    if (!c.ok) throw new Error(`${index.conds.file}: HTTP ${c.status}`);
    glue.ruleset_precompiled(new Uint8Array(await c.arrayBuffer()));
  }
  glue.ruleset_build();
  return {
    api: typeof glue.replay_api_version === "function" ? glue.replay_api_version() : 1,
    stamp: JSON.parse(glue.engine_stamp()),
    id: manifest.id,
  };
}

function open(bytes, force) {
  if (!glue) throw new Error("init first");
  match?.free?.();
  match = glue.ReplayMatch.from_record_bytes(bytes, force);
  return true;
}

/** Replace the globals an untrusted engine could otherwise reach with stubs
 *  that throw. The wasm cannot call them today (its import object is Error /
 *  throw / the externref table), and the trusted glue only touches them when
 *  it is given a URL to fetch -- which `init-embedded` never does. This is the
 *  belt to that braces: a future glue that asks for `fetch` fails loudly here
 *  instead of reaching the network from a record's own engine. */
function lockdown() {
  const trap = (name) =>
    function lockedDown() {
      throw new Error(`${name} is disabled inside the embedded-engine replay worker`);
    };
  for (const name of ["fetch", "XMLHttpRequest", "WebSocket", "EventSource", "importScripts", "caches"]) {
    try {
      Object.defineProperty(self, name, { value: trap(name), writable: false, configurable: false });
    } catch {
      /* not every worker global exists or is configurable -- best effort */
    }
  }
}

/**
 * Boot the engine a portable record carries. Every byte here was hash-checked
 * by the page (`webui/src/game/portableEngine.ts`) before it was posted; the
 * loader is our own copy, never the record's `glue.js`.
 */
async function initEmbedded({ loader, wasm, data, modules, conds }) {
  if (typeof loader !== "string" || !loader.startsWith("/assets/engine/")) {
    throw new Error(`refusing loader URL ${JSON.stringify(loader)} (must be a shipped /assets/engine/ asset)`);
  }
  if (!(wasm instanceof Uint8Array) || wasm.length === 0) throw new Error("embedded engine has no wasm");
  glue = await import(loader);
  // Bytes in, no fetch: `wasm-bindgen`'s init instantiates what it is given.
  await glue.default({ module_or_path: wasm });
  glue.load_data(JSON.stringify(data ?? {}));
  for (const m of modules ?? []) {
    if (!(m instanceof Uint8Array)) throw new Error("embedded rule module is not bytes");
    glue.ruleset_add(m);
  }
  if (conds) {
    if (typeof glue.ruleset_precompiled !== "function") {
      throw new Error("embedded bundle ships precompiled conditions but its glue cannot load them");
    }
    glue.ruleset_precompiled(conds);
  }
  glue.ruleset_build();
  lockdown();
  return {
    api: typeof glue.replay_api_version === "function" ? glue.replay_api_version() : 1,
    stamp: JSON.parse(glue.engine_stamp()),
    id: JSON.parse(glue.engine_stamp()).bundle || "",
  };
}

function call(method, args) {
  if (!match) throw new Error("open first");
  switch (method) {
    case "total_ticks":
      return match.total_ticks();
    case "take_changed":
    case "ended":
      return match[method]();
    case "step":
    case "next_input":
    case "index":
    case "seek":
    case "view":
    case "events_since":
    case "header":
    case "compat":
    case "turns":
    case "status":
      return JSON.parse(match[method](...args));
    default:
      throw new Error(`unknown call ${method}`);
  }
}

self.onmessage = async (e) => {
  const { id, op, ...rest } = e.data ?? {};
  try {
    let value;
    if (op === "init") value = await init(rest.base);
    else if (op === "init-embedded") {
      value = await initEmbedded(rest);
    } else if (op === "open") value = open(rest.bytes, !!rest.force);
    else if (op === "call") {
      if (!CALLS.has(rest.method)) throw new Error(`unknown call ${rest.method}`);
      value = call(rest.method, rest.args ?? []);
    } else if (op === "free") {
      match?.free?.();
      match = null;
      value = true;
    } else if (op === "close") {
      match?.free?.();
      match = null;
      glue = null;
      value = true;
      self.postMessage({ id, ok: true, value });
      self.close();
      return;
    } else throw new Error(`unknown op ${op}`);
    self.postMessage({ id, ok: true, value });
  } catch (err) {
    self.postMessage({ id, ok: false, error: String(err?.message ?? err) });
  }
};