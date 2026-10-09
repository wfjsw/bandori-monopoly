// The solo match engine worker: one `SoloMatch` (wasm) per page match, off the
// UI thread. Owns the 50 ms fixed-step tick loop, the engine's own standard
// bots (they answer inside `tick_steps` / `act`), and the save / `.bdrec`
// export. The page (`soloEngine.ts` / `SoloSession`) sends commands and gets
// back only what the UI needs: events, and a view frame when it changed.
//
// Bundled by rsbuild from `new URL("./soloWorker.ts", import.meta.url)` -- the
// same `webui/src/wasm/glue` build the page imports (`engine_id.json` /
// `glueSha256` ride along), the same `/data` tables and the same
// `/assets/rules` modules + precompiled-condition blob as `core/data.ts`, so a
// record sealed here names the same engine bundle one sealed on the page would
// (`docs/REPLAY.md` §9).
//
// Messages in  ({id, op, ...}):  init | start | restore | act | quick_start |
//                                view | save | export | visibility | close
// Messages out ({id, ok, value} | {id, ok:false, error}) and unsolicited
//              {push: "sync" | "save", ...}.

import init, * as glue from "../wasm/glue";
import engineId from "../wasm/engine_id.json";
import { loadRulesetInto } from "../core/rulesetLoad.ts";
import type { MatchEvent, MatchView, Command } from "../core/types.ts";
import { tickQuanta, TICK_STEP, type EngineBoot, type SaveSnap, type SoloOpen, type SoloPush, type SoloRestore } from "./soloProtocol.ts";

/** The worker global (tsconfig has no WebWorker lib; only these are used). */
const ctx = self as unknown as {
  postMessage(msg: unknown): void;
  onmessage: ((e: MessageEvent) => void) | null;
  close(): void;
};

type Match = InstanceType<typeof glue.SoloMatch>;

/** The running match, or null before `start` / after `close`. */
let match: Match | null = null;
/** The human's member id -- the frame the board renders. */
let you = 1;
/** `events_since` cursor, shared with the save snapshot. */
let last = 0;
// ---------------------------------------------------------------- tick loop
const TICK_MS = 50;
/** While the tab is hidden the loop matches a hidden page's coalesced timers
 *  (the pre-worker loop lived on the page and was throttled with it). Solo
 *  never expires a player's choice (`docs/BOT.md` §3.6), so this only paces
 *  the bot seats' human beats. */
const HIDDEN_TICK_MS = 1000;
/** Push a save at most this often while dirty (the page writes it). */
const SAVE_MS = 1000;
let timer = 0;
let lastTick = 0;
let acc = 0;
let hidden = false;
let dirty = false;
let lastSavePush = 0;
// ---------------------------------------------------------------- diff cache
/** Last pushed human frame (JSON) -- a frame is re-posted only when it moves. */
let viewJson = "";
/** Last pushed per-seat frame (JSON) for the 进阶 seats the page drives. */
const botJson = new Map<number, string>();
let haveView = false;

function post(msg: unknown): void {
  ctx.postMessage(msg);
}

// ---------------------------------------------------------------- boot

async function ensureInit(): Promise<EngineBoot> {
  initOnce ??= boot();
  return initOnce;
}
let initOnce: Promise<EngineBoot> | null = null;

/** The page's `loadGameData` + `loadRuleset` sequence, minus the display data:
 *  same tables, same optional bot books, same ruleset (and conds blob). */
async function boot(): Promise<EngineBoot> {
  await init();
  // Glue identity for the engine-bundle id (`docs/REPLAY.md` §9) -- the same
  // value `core/data.ts` hands the page's instance, so both ends seal records
  // for the same bundle.
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
  // Optional bot books -- outside `data_files()` (not hashed), but they tune
  // bot answers and those answers are what a record stores. Mirror the page.
  for (const book of ["deck_book.json", "strategy_book.json"]) {
    try {
      const r = await fetch("/data/" + book);
      if (r.ok) files[book] = await r.text();
    } catch {
      /* absent book is fine */
    }
  }
  glue.load_data(JSON.stringify(files));
  try {
    await loadRulesetInto(glue, async (rel) => {
      const r = await fetch("/assets/rules/" + rel);
      if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
      return new Uint8Array(await r.arrayBuffer());
    });
  } catch (e) {
    // Same policy as the page (`core/data.ts`): a solo match may run on the
    // engine's StubRules, but it is loud about it -- a record written without
    // card rules and replayed with them diverges at the first checkpoint.
    console.error("[solo-worker] card modules failed to load; the match runs without card, skill or tile rules", e);
  }
  let stamp: EngineStampJson | null = null;
  try {
    stamp = JSON.parse(glue.engine_stamp()) as EngineStampJson;
  } catch {
    stamp = null;
  }
  return { stamp, cheats: !!glue.cheats_enabled?.() };
}

interface EngineStampJson {
  format: number;
  save_version: number;
  abi: number;
  ruleset_sha256: string;
  data_sha256: string;
  engine: string;
  build: string;
  bundle?: string;
}

// ---------------------------------------------------------------- match

/** `session.ts`'s pre-worker `restoreSoloMatch` (the Rust side needs no self). */
function restoreMatch(save: string, rec: string | undefined): Match {
  if (!rec) return glue.SoloMatch.restore(save);
  const tmp = glue.SoloMatch.restore(save);
  try {
    return tmp.restore_with_record(save, rec);
  } finally {
    tmp.free();
  }
}

function openMatch(m: Match, eventCursor: number): void {
  freeMatch();
  match = m;
  last = eventCursor;
  viewJson = "";
  botJson.clear();
  haveView = false;
  dirty = false;
  lastSavePush = 0;
  lastTick = performance.now();
  acc = 0;
  setTickPeriod();
  pump(true);
}

function freeMatch(): void {
  if (timer) clearInterval(timer);
  timer = 0;
  try {
    match?.free();
  } catch {
    /* already freed */
  }
  match = null;
}

function setTickPeriod(): void {
  if (timer) clearInterval(timer);
  timer = setInterval(tick, hidden ? HIDDEN_TICK_MS : TICK_MS) as unknown as number;
}

// ---------------------------------------------------------------- pump

function tick(): void {
  if (!match) return;
  const t = performance.now();
  const step = tickQuanta(t - lastTick, acc);
  lastTick = t;
  acc = step.acc;
  if (step.k > 0) match.tick_steps(step.k);
  pump();
}

/**
 * Drain events and push the frames that moved. `take_changed()` is the cheap
 * dirty signal; a frame is then re-serialised only when the match moved and
 * posted only when its JSON differs from the last push. The pre-worker pump
 * re-serialised every 进阶 seat's frame every 50 ms -- this is the same set of
 * frames, but only when they change.
 */
function pump(forceSave = false): void {
  if (!match) return;
  const evs: MatchEvent[] = JSON.parse(match.events_since(last));
  for (const e of evs) last = e.id;
  const taken = match.take_changed();
  if (taken || evs.length > 0 || !haveView) {
    dirty = true;
    haveView = true;
    const push: SoloPush = { push: "sync", events: evs };
    const v = JSON.parse(match.view(you)) as MatchView;
    const j = JSON.stringify(v);
    if (j !== viewJson) {
      viewJson = j;
      push.view = v;
    }
    const bots = botFrames(v);
    if (bots.length) push.bots = bots;
    if (match.ended()) push.ended = true;
    post(push);
  }
  const now = performance.now();
  if (forceSave || (dirty && now - lastSavePush >= SAVE_MS)) pushSave();
}

/** The 进阶 seats' frames, only where the JSON moved since the last push. */
function botFrames(main: MatchView): { member: number; view: MatchView }[] {
  const out: { member: number; view: MatchView }[] = [];
  for (const p of main.state.players) {
    if (!(p.bot && p.mentality === "advanced" && !p.ai && !p.bankrupt && !p.left)) continue;
    const v = JSON.parse(match!.view(p.member)) as MatchView;
    const j = JSON.stringify(v);
    if (j !== botJson.get(p.member)) {
      botJson.set(p.member, j);
      out.push({ member: p.member, view: v });
    }
  }
  return out;
}

function pushSave(): void {
  if (!match) return;
  lastSavePush = performance.now();
  dirty = false;
  const snap: SaveSnap = { match: match.save(), rec: match.record_state(), last };
  post({ push: "save", ...snap });
}

// ---------------------------------------------------------------- requests

function requireMatch(): Match {
  if (!match) throw new Error("no match open (start / restore first)");
  return match;
}

/** Draw the solo openings and derive the match seed (`docs/FAIRNESS.md`).
 *  Same recipe the server runs online; the openings ride in the exported
 *  record (`set_fair`) so a viewer can verify. */
function soloFair(
  members: SoloOpen["members"],
  mode: number,
  weights: SoloOpen["weights"],
  human: number,
): { derived: string; open: Record<string, unknown> } {
  const rand32 = () => crypto.getRandomValues(new Uint8Array(32));
  const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  const seed = hex(rand32());
  const salt = hex(rand32());
  const nonce = hex(rand32());
  const stamp = JSON.parse(glue.engine_stamp()) as { bundle?: string; ruleset_sha256?: string };
  // The commitment covers the openings and the engine identity only (the
  // settings are not fixed yet when it is drawn); the settings still ride the
  // header so the verifier can pin them (`docs/FAIRNESS.md` §1.1).
  const settings = glue.fair_canon_settings(mode, TICK_STEP, JSON.stringify(weights), JSON.stringify(members));
  const commit = glue.fair_commit(seed, salt, stamp.bundle ?? "", stamp.ruleset_sha256 ?? "");
  // One human seat; bots contribute none.
  const derived = glue.fair_derive_seed(seed, JSON.stringify([{ member: human, nonce }]));
  return {
    derived,
    // `v` is the scheme version (`game_core::fair::FAIR_VERSION`).
    open: { v: 2, commit, seed, salt, nonces: [{ member: human, nonce }], settings },
  };
}

function call(op: string, rest: Record<string, unknown>): unknown {
  switch (op) {
    case "start": {
      const s = rest as unknown as SoloOpen;
      you = s.you;
      // Commit-reveal, locally (`docs/FAIRNESS.md` "solo"): the same recipe an
      // online match runs, so the exported record carries the openings and
      // verifies. Solo proves little -- the player is both committer and
      // contributor -- so the UI does not show the commitment. `seed256` is a
      // test seam that pins the derived stream and skips the openings.
      const fair = s.seed256
        ? { derived: s.seed256, open: null }
        : soloFair(s.members, s.mode, s.weights, s.you);
      const m = new glue.SoloMatch(JSON.stringify(s.members), fair.derived, s.mode, JSON.stringify(s.weights));
      if (fair.open) m.set_fair(JSON.stringify(fair.open));
      openMatch(m, 0);
      return true;
    }
    case "restore": {
      const s = rest as unknown as SoloRestore;
      you = s.you;
      openMatch(restoreMatch(s.save, s.rec), s.last);
      return true;
    }
    case "act": {
      const m = requireMatch();
      const raw = m.act(rest.member as number, JSON.stringify(rest.cmd as Command));
      // Pump (and the save push) first: they post before this reply, so the
      // page's `view` is already the post-command frame when `act` resolves.
      pump(true);
      return raw ? JSON.parse(raw) : null;
    }
    case "quick_start": {
      requireMatch().quick_start();
      pump(true);
      return true;
    }
    case "view":
      return JSON.parse(requireMatch().view(rest.member as number)) as MatchView;
    case "save": {
      const m = requireMatch();
      return { match: m.save(), rec: m.record_state(), last } satisfies SaveSnap;
    }
    case "export":
      return requireMatch().record_zst(rest.created as string);
    case "visibility": {
      hidden = !!rest.hidden;
      setTickPeriod();
      return true;
    }
    case "close": {
      freeMatch();
      return true;
    }
    default:
      throw new Error(`unknown op ${op}`);
  }
}

const OPS = new Set(["start", "restore", "act", "quick_start", "view", "save", "export", "visibility", "close"]);

ctx.onmessage = async (e: MessageEvent) => {
  const { id, op, ...rest } = (e.data ?? {}) as { id: number; op: string } & Record<string, unknown>;
  try {
    let value: unknown;
    if (op === "init") value = await ensureInit();
    else {
      if (!OPS.has(op)) throw new Error(`unknown op ${op}`);
      await ensureInit();
      value = call(op, rest);
    }
    if (op === "close") {
      post({ id, ok: true, value });
      ctx.close();
      return;
    }
    post({ id, ok: true, value });
  } catch (err) {
    post({ id, ok: false, error: String((err as Error)?.message ?? err) });
  }
};