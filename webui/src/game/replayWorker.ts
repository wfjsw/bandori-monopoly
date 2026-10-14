// The replay driver worker for the **running** build's engine: the same
// `webui/src/wasm/glue` the page imports, in its own wasm instance, so
// stepping, keyframe indexing, seeking and view building never run on the UI
// thread. (A record written by another build is driven by the archived-bundle
// worker, `public/assets/engine/replay-worker.js`; this file is its sibling
// for "this very build", which has no archive directory to load from.)
//
// Same message contract as the archived worker, mirrored by
// `WorkerReplayHandle` in `replayEngine.ts`:
//   in  ({id, op, ...}):  init | open | verify | call | close
//   out ({id, ok, value} | {id, ok:false, error})
//
// Boot is the page's `loadGameData` + `loadRuleset` minus the display data --
// the same tables, bot books and ruleset (`soloWorker.ts` does likewise), so
// the stamp is the one the page's instance would report.

import init, * as glue from "../wasm/glue";
import engineId from "../wasm/engine_id.json";
import { loadRulesetInto } from "../core/rulesetLoad.ts";

const ctx = self as unknown as {
  postMessage(msg: unknown): void;
  onmessage: ((e: MessageEvent) => void) | null;
  close(): void;
};

type Replay = InstanceType<typeof glue.ReplayMatch>;

/** The frozen API surface: every method a driver may call. */
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

let match: Replay | null = null;
let booted: Promise<unknown> | null = null;

async function boot(): Promise<{ api: number; stamp: unknown; id: string }> {
  await init();
  if (typeof glue.set_glue_sha === "function" && engineId?.glueSha256) {
    glue.set_glue_sha(engineId.glueSha256);
  }
  const names: string[] = JSON.parse(glue.data_files());
  const files: Record<string, string> = {};
  await Promise.all(
    names.map(async (n) => {
      const r = await fetch("/data/" + n);
      if (!r.ok) throw new Error(`/data/${n}: HTTP ${r.status}`);
      files[n] = await r.text();
    }),
  );
  // Optional bot books: outside `data_files()` (unhashed) but they shape the
  // answers a record stores. Mirror the page.
  for (const book of ["deck_book.json", "strategy_book.json"]) {
    try {
      const r = await fetch("/data/" + book);
      if (r.ok) files[book] = await r.text();
    } catch {
      /* absent book is fine */
    }
  }
  glue.load_data(JSON.stringify(files));
  // Unlike the page this is fatal: a replay without the rules the record was
  // played under only diverges.
  await loadRulesetInto(glue, async (rel) => {
    const r = await fetch("/assets/rules/" + rel);
    if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
    return new Uint8Array(await r.arrayBuffer());
  });
  const stamp = JSON.parse(glue.engine_stamp()) as { bundle?: string };
  return {
    api: typeof glue.replay_api_version === "function" ? glue.replay_api_version() : 1,
    stamp,
    id: stamp.bundle ?? "",
  };
}

function call(method: string, args: unknown[]): unknown {
  if (!match) throw new Error("open first");
  const m = match as unknown as Record<string, (...a: unknown[]) => unknown>;
  switch (method) {
    case "total_ticks":
    case "take_changed":
    case "ended":
      return m[method]();
    default:
      return JSON.parse(m[method](...args) as string);
  }
}

ctx.onmessage = async (e: MessageEvent) => {
  const { id, op, ...rest } = (e.data ?? {}) as { id: number; op: string; [k: string]: unknown };
  try {
    let value: unknown;
    if (op === "init") {
      booted ??= boot();
      value = await booted;
    } else if (op === "open") {
      match?.free();
      match = glue.ReplayMatch.from_record_bytes(rest.bytes as Uint8Array, !!rest.force);
      value = true;
    } else if (op === "verify") {
      value = glue.verify_fair(rest.bytes as Uint8Array);
    } else if (op === "call") {
      if (!CALLS.has(rest.method as string)) throw new Error(`unknown call ${rest.method}`);
      value = call(rest.method as string, (rest.args as unknown[]) ?? []);
    } else if (op === "close") {
      match?.free();
      match = null;
      ctx.postMessage({ id, ok: true, value: true });
      ctx.close();
      return;
    } else {
      throw new Error(`unknown op ${op}`);
    }
    ctx.postMessage({ id, ok: true, value });
  } catch (err) {
    ctx.postMessage({ id, ok: false, error: String((err as Error)?.message ?? err) });
  }
};
