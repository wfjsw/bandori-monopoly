// Game data: the identity roster from the wasm ruleset, merged with the
// display fields the webui owns (generated/display.json + data/*.json) and
// kept typed here for display.

import init, * as glue from "../wasm/glue";
import engineId from "../wasm/engine_id.json";
import displayJson from "../generated/display.json";
import { t as tr } from "../i18n/t";
import { toast } from "../ui/Toast";
import { applyRuleset, fetchRuleset, type RulesetBundle } from "./rulesetLoad";
import { buildId, buildNameOf, dataBase, isRemote, rulesBase, serverBase, setBuild, setServer } from "../net/server";
import { bandLogo, charArt, cardArt } from "./assets";
import { cardFaceTitle } from "./cardFace";
import type { GroupData, CardData, PersonaData, EventData, TileData, VoiceLine, DisplayFile, Block } from "./types";

/** Build-exported presentation fields (empty entries omitted). */
// JSON modules widen `t: "label"` to `t: string`, so the discriminated
// `Block` union does not line up with the inferred shape; the export is
// generated from `CardText` and matches `DisplayFile` at runtime.
const display = displayJson as unknown as DisplayFile;

/**
 * Current rules-book version, client-owned. Drives the rules NEW badge
 * (`Seen::Rules` used to read `game_core::data::RULES_VERSION`; the webui
 * supplies it now). Bump when the rules book changes.
 */
export const RULES_VERSION = 1;

/** Persona a new profile starts with (was `match_rules.defaultHomePersona`). */
export const DEFAULT_HOME_PERSONA = "高松灯";

/** UI-only tables the engine no longer carries in `glue.data_files()` after
 *  the presentation split. Fetched here, outside `glue.load_data`. */
const UI_DATA_FILES = ["home_lines.json", "voice_lines.json", "rules.txt"] as const;

export interface GameData {
  tiles: TileData[];
  cards: CardData[];
  personas: PersonaData[];
  groups: GroupData[];
  events: EventData[];
  voiceLines: { id: string; name: string; lines: VoiceLine[] }[];
  homeLines: { name: string; tag: string }[];
  rulesText: string;
  card(id: string): CardData | undefined;
  persona(name: string): PersonaData | undefined;
  group(name: string): GroupData | undefined;
  event(id: string): EventData | undefined;
  artId(c: PersonaData): string;
  homeTag(c: PersonaData): string;
}

export let D: GameData;
export const rules = glue;

/** True once `loadRuleset` has produced a live `WasmRules`; `false` means every
 *  solo match (and every local replay) is running on the engine's `StubRules`
 *  -- no card, skill or tile effects. A record sealed under one and replayed
 *  under the other diverges at the first checkpoint (`docs/REPLAY.md`), so the
 *  UI reads this to explain a divergence instead of showing it as engine drift. */
export let rulesetLoaded = false;

/** The rules / data directories the live `D` came from. Normally the selected
 *  server's build; older servers (no `/builds/...` routes) fall back to the
 *  bundled paths. */
let sources = { rules: rulesBase(), data: dataBase() };

/** The resolved sources, for the solo worker -- it must load exactly what the
 *  page booted (`session.ts`). */
export function currentSources(): { rules: string; data: string } {
  return sources;
}

interface Target {
  rules: string;
  data: string;
  /** The probed `index.json` body, so the loader does not fetch it again. */
  indexText?: string;
}

/** A request that never settles would leave the blocking overlay up forever. */
const FETCH_TIMEOUT_MS = 30_000;

function timeoutSignal(): AbortSignal | undefined {
  const timeout = (AbortSignal as { timeout?: (ms: number) => AbortSignal }).timeout;
  return timeout ? timeout(FETCH_TIMEOUT_MS) : undefined;
}

/**
 * Resolve the selected build's rules / data directories without touching the
 * live set. `rulesBase()` / `dataBase()` name the selected build, but a server
 * from before rule builds answers `/builds/...` with 404 (or the SPA's
 * index.html). Probe `index.json` once and fall back to the bundled paths --
 * the page ships them anyway, and an older server's single ruleset is that
 * same set.
 */
async function resolveTargets(): Promise<Target> {
  const target = { rules: rulesBase(), data: dataBase() };
  if (target.rules === "/assets/rules/") return target;
  try {
    const r = await fetch(target.rules + "index.json", { cache: "no-cache", signal: timeoutSignal() });
    if (r.ok) {
      const text = await r.text();
      if (!text.trimStart().startsWith("<")) return { ...target, indexText: text };
    }
  } catch {
    /* fall through to the bundled set */
  }
  console.warn("[rules] the selected server does not serve rule builds; using the bundled ruleset");
  toast(tr("boot.rulesFallback"));
  return { rules: "/assets/rules/", data: "/data/" };
}

// ------------------------------------------------------------------ versions

/** Bumped after `D` has been replaced by a live source switch. Scenes key
 *  their subtree on it so every screen re-reads the new roster / tables. */
let dataVersion = 0;
const dataListeners = new Set<() => void>();

export const subscribeData = (cb: () => void): (() => void) => {
  dataListeners.add(cb);
  return () => dataListeners.delete(cb);
};

export const dataVersionOf = (): number => dataVersion;

// Dev-only seam for browser checks: lets a script read the live source and
// roster without reaching into React.
if (import.meta.env?.DEV) {
  (globalThis as { __bmDebug?: unknown }).__bmDebug = {
    data: () => D,
    sources: currentSources,
    version: dataVersionOf,
    busy: () => loadState().busy,
    switch: (change: SourceChange) => switchSource(change),
  };
}

function bumpData(): void {
  dataVersion++;
  dataListeners.forEach((cb) => cb());
}

export type LoadStage = "rules" | "data";

export interface LoadState {
  /** True while a source switch is in flight (the overlay blocks the page). */
  busy: boolean;
  stage: LoadStage | "idle";
  done: number;
  total: number;
}

let load: LoadState = { busy: false, stage: "idle", done: 0, total: 0 };
const loadListeners = new Set<() => void>();

export const subscribeLoad = (cb: () => void): (() => void) => {
  loadListeners.add(cb);
  return () => loadListeners.delete(cb);
};

export const loadState = (): LoadState => load;

function setLoad(next: LoadState): void {
  load = next;
  loadListeners.forEach((cb) => cb());
}

/** The load run whose progress the overlay may show. A failed `loadTables`
 *  rejects `Promise.all` while its sibling fetches keep going; without this,
 *  their late `report` calls re-open the overlay after the switch closed it. */
let activeRun = 0;

function report(stage: LoadStage, done: number, total: number, run: number): void {
  if (run !== activeRun) return;
  setLoad({ busy: true, stage, done, total });
}

/** Fetch one rules directory's whole bundle (`index.json`, its modules, the
 *  precompiled conditions) without touching the live glue. Modules and the
 *  conds blob are content-addressed by file name, so only `index.json` is
 *  revalidated (`no-cache`); the rest may come from the HTTP cache. A target
 *  resolved by `resolveTargets` already carries the probed index body. */
function preloadRules(
  base: string,
  onFile?: (done: number, total: number) => void,
  indexText?: string,
): Promise<RulesetBundle> {
  const custom = customRulesActive() ? customRulesBytes() : null;
  return fetchRuleset(
    async (rel) => {
      const r = await fetch(base + rel, {
        cache: rel === "index.json" ? "no-cache" : "default",
        signal: timeoutSignal(),
      });
      if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
      return new Uint8Array(await r.arrayBuffer());
    },
    custom ? [custom] : [],
    onFile,
    indexText,
  );
}

// ------------------------------------------------------------- custom ruleset

export type { CustomRules } from "./customRules.ts";
export { customRulesHash, currentCustomRules as customRules } from "./customRules.ts";
import {
  currentCustomRules as customRules,
  customRulesHash,
  forgetCustomRules,
  hydrateCustomRules,
  storeCustomRules,
  type CustomRules,
} from "./customRules.ts";

/** A solo ruleset is one wasm module; 8 MiB is far above the shipped size. */
export const CUSTOM_RULES_MAX = 8 * 1024 * 1024;

/**
 * True when the custom module participates in the ruleset: only on the local
 * bundled path, with no server build selected. With a server or a build
 * chosen, that build wins.
 */
export function customRulesActive(): boolean {
  return !isRemote() && !buildId();
}

/** The custom module's bytes; null when absent. */
export function customRulesBytes(): Uint8Array | null {
  return customRules()?.bytes ?? null;
}

/**
 * Store a user's custom solo ruleset (one wasm module). Returns false when it
 * is too large or the browser's storage refuses it (toasted here). The caller
 * then switches the source (`switchSource`) so the roster / rules include it.
 */
export async function setCustomRules(bytes: Uint8Array, name: string): Promise<boolean> {
  if (!bytes.length) {
    toast(tr("solo.buildEmpty"), "error");
    return false;
  }
  if (bytes.length > CUSTOM_RULES_MAX) {
    toast(tr("solo.buildTooLarge", { mb: Math.round(CUSTOM_RULES_MAX / 1024 / 1024) }), "error");
    return false;
  }
  if (!(await storeCustomRules(bytes, name))) {
    toast(tr("solo.buildStoreError"), "error");
    return false;
  }
  return true;
}

/** Forget the custom ruleset; the next boot uses the bundled / server build. */
export function clearCustomRules(): Promise<void> {
  return forgetCustomRules();
}

/** The identity rows the built ruleset exports (`roster_json()`). */
interface Roster {
  cards: { id: string; group: string; name: string; owner: string; derived: boolean; tags: string[]; text: string }[];
  personas: { name: string; group: string; skill: string; exclusiveCards: string[]; preset: string[]; variantOf?: string; text: string }[];
  groups: { name: string; skill: string; sharesExclusives?: boolean; text: string }[];
  events: { id: string; name: string; derived: boolean; text: string }[];
}

const detail = (e: unknown): string => (e instanceof Error ? e.message : String(e));

function rulesFailed(e: unknown): void {
  console.error("[rules] card modules failed to load; solo matches will run without card, skill or tile rules", e);
  toast(tr("boot.rulesError", { detail: detail(e) }), "error");
}

/**
 * Fetch `target`'s tables after its ruleset has been built (the build names
 * the engine tables), hand the engine its own tables, and only then replace
 * `D`. A throw here leaves the old `D` in place; the engine may already carry
 * the new set, which the caller either reverts or surfaces.
 */
async function loadTables(target: Target, run: number, onProgress?: (p: number) => void): Promise<void> {
  // Game roster after the presentation split: identity plus the rulebook
  // prose (`text`), which is game content and rides `ruleset_metadata`. Pure
  // display columns are merged below from `generated/display.json`.
  const roster = JSON.parse(glue.roster_json()) as Roster;
  // Engine tables only (post-split: board / match_rules).
  const engineNames: string[] = JSON.parse(glue.data_files());
  const allNames = [...new Set([...engineNames, ...UI_DATA_FILES])];
  const files: Record<string, string> = {};
  let done = 0;
  setLoad({ busy: true, stage: "data", done: 0, total: allNames.length });
  await Promise.all(
    allNames.map(async (n) => {
      const r = await fetch(target.data + n, { cache: "no-cache", signal: timeoutSignal() });
      if (!r.ok) throw new Error(tr("boot.dataError", { name: n }));
      files[n] = await r.text();
      done++;
      report("data", done, allNames.length, run);
      onProgress?.(0.45 + (done / allNames.length) * 0.5);
    }),
  );
  // Optional: the bot deck book (`docs/BOT.md` §3.7). Missing = empty book =
  // 托管 / bots keep the designer's preset decks. Outside `data_files()` on
  // purpose -- it tunes bots, not the match, so it is not part of the data
  // hash either.
  try {
    const book = await fetch(target.data + "deck_book.json", { signal: timeoutSignal() });
    if (book.ok) files["deck_book.json"] = await book.text();
  } catch {
    /* absent book is fine */
  }
  // Optional: the bot strategy book (`docs/BOT.md` §3.8). Missing = empty
  // book = the default parameters (today's heuristic constants).
  try {
    const book = await fetch(target.data + "strategy_book.json", { signal: timeoutSignal() });
    if (book.ok) files["strategy_book.json"] = await book.text();
  } catch {
    /* absent book is fine */
  }
  // Only the engine tables (+ the optional bot books) cross into wasm. The
  // UI-only tables above stay on this side -- they are presentation.
  const engineFiles: Record<string, string> = {};
  for (const n of [...engineNames, "deck_book.json", "strategy_book.json"]) {
    if (files[n] !== undefined) engineFiles[n] = files[n];
  }
  glue.load_data(JSON.stringify(engineFiles));

  const j = (n: string) => JSON.parse(files[n].replace(/^﻿/, ""));
  // Cards / personas / groups: identity from the ruleset roster, display from
  // `generated/display.json` (authored in Rust rule code, exported at build).
  // The [反击] prompt's `counterReason` rides on the card row there too.
  const cards: CardData[] = roster.cards.map((c) => {
    const d = display.cards[c.id] ?? {};
    return {
      ...c,
      // Game prose is on the roster too (`CardData.text`); the export is the
      // build-time copy. Prefer it, but never blank a body the roster has.
      text: d.text ?? c.text ?? "",
      rawName: d.rawName ?? "",
      ...(d.body ? { body: d.body } : {}),
      ...(d.counterReason ? { counterReason: d.counterReason } : {}),
    };
  });
  const personas: PersonaData[] = roster.personas.map((p) => {
    const d = display.personas[p.name] ?? {};
    return {
      ...p,
      display: d.display ?? "",
      text: d.text ?? p.text ?? "",
      color: d.color ?? "",
      cnId: d.cnId ?? "",
      art: d.art ?? "",
      costume: d.costume ?? "",
      ...(d.variantTag ? { variantTag: d.variantTag } : {}),
    };
  });
  const groups: GroupData[] = roster.groups.map((g) => {
    const d = display.groups[g.name] ?? {};
    return {
      ...g,
      text: d.text ?? g.text ?? "",
      shortName: d.shortName ?? "",
      color: d.color ?? "",
      logo: d.logo ?? "",
    };
  });
  // Events: identity + game prose from the roster, display from `display.json`.
  const events: EventData[] = roster.events.map((e) => {
    const d = display.events?.[e.id] ?? {};
    return {
      ...e,
      text: d.text ?? e.text ?? "",
      name: d.name ?? e.name ?? "",
      ...(d.body ? { body: d.body } : {}),
    };
  });
  const homeLines = j("home_lines.json").personas ?? [];
  const byId = new Map(cards.map((c) => [c.id, c]));
  // Assign only after every piece is in, so a throw above leaves the old `D`.
  D = {
    tiles: j("board.json").tiles,
    cards,
    personas,
    groups,
    events,
    voiceLines: j("voice_lines.json").personas ?? [],
    homeLines,
    rulesText: files["rules.txt"].replace(/^﻿/, ""),
    card: (id) => byId.get(id),
    persona: (name) => personas.find((c) => c.name === name),
    group: (name) => groups.find((b) => b.name === name),
    event: (id) => events.find((e) => e.id === id),
    artId: (c) => c.art || c.cnId,
    homeTag: (c) => homeLines.find((x: { name: string; tag: string }) => x.name === c.name && x.tag)?.tag ?? c.display,
  };
  bumpData();
}

/**
 * Load one target: fetch its whole rules bundle first (no live mutation),
 * stage + build it, then fetch and apply its tables. `fatal` makes a rule
 * failure throw instead of degrading to `StubRules` -- the boot gate degrades
 * (online play runs the rules server-side), a source switch must not.
 */
async function loadInto(target: Target, fatal: boolean, onProgress?: (p: number) => void): Promise<void> {
  const run = ++activeRun;
  let bundle: RulesetBundle | null = null;
  try {
    setLoad({ busy: true, stage: "rules", done: 0, total: 0 });
    bundle = await preloadRules(
      target.rules,
      (done, total) => {
        report("rules", done, total, run);
        onProgress?.(total ? (done / total) * 0.45 : 0);
      },
      target.indexText,
    );
  } catch (e) {
    if (fatal) throw e;
    rulesFailed(e);
  }
  if (bundle) {
    try {
      // Stages the new modules beside the built set; `ruleset_build` swaps it.
      // A failed build leaves those modules staged; the next `applyRuleset`
      // (the revert) resets them before staging its own.
      const n = applyRuleset(glue, bundle);
      rulesetLoaded = true;
      console.info(`[rules] ${n} card module(s) loaded`);
    } catch (e) {
      rulesetLoaded = false;
      if (fatal) throw e;
      rulesFailed(e);
    }
  } else {
    rulesetLoaded = false;
  }
  // The public source identity is just the two directories (`indexText` is a
  // fetch detail of this run).
  sources = { rules: target.rules, data: target.data };
  await loadTables(target, run, onProgress);
  onProgress?.(1);
}

/**
 * The one boot gate for the typed `D`. Order matters:
 *
 * 1. wasm init + glue identity;
 * 2. resolve the target directories (older servers fall back to the bundled
 *    set, consistently for rules and tables);
 * 3. **ruleset first** -- `roster_json()` is empty until `ruleset_build`, and
 *    deck builder / gallery / home read `D` as soon as this resolves;
 * 4. `roster_json()` for the identity rows (cards / personas / groups);
 * 5. the remaining `data_files()` tables + the UI-only tables + optional bot
 *    books, `load_data` (engine tables only);
 * 6. merge `generated/display.json` onto the roster and assign `D` here, only
 *    at the end, so a throw leaves `D` untouched and Boot shows the error
 *    stage instead of a half-initialized table.
 */
export async function loadGameData(onProgress: (p: number) => void): Promise<void> {
  await init();
  // The custom module rides the ruleset (below) and every later boot path reads
  // it synchronously; load it out of the store first.
  await hydrateCustomRules();
  // Resolve the selected build's rules / data directories before anything
  // reads them (an older server without `/builds/...` falls back to the
  // bundled set there, consistently for both).
  const target = await resolveTargets();
  // Glue identity for the engine-bundle id (`docs/REPLAY.md` §9): every
  // record this build seals names the bundle that can replay it. Comes from
  // `tools/build-glue.mjs`; empty only if a glue was built without it.
  if (typeof glue.set_glue_sha === "function" && engineId?.glueSha256) {
    glue.set_glue_sha(engineId.glueSha256);
  }
  // Ruleset before the tables: the roster rides on the built set. Failure is
  // degraded (empty roster + StubRules), not fatal -- `loadInto` toasts. The
  // `finally` still clears the overlay when it is fatal (bad tables), or the
  // error screen would sit under a busy scrim that never closes.
  try {
    await loadInto(target, false, onProgress);
  } finally {
    // Invalidate the run: rejected siblings may still settle and report.
    activeRun++;
    setLoad({ busy: false, stage: "idle", done: 0, total: 0 });
  }
}

// ------------------------------------------------------------ live switching

/** What a live switch changes. Everything is persisted here, so `resolveTargets`
 *  sees exactly what a page reload would. */
export interface SourceChange {
  /** New game-server base; `null` = this page's origin. Clears the build. */
  server?: string | null;
  /** New build id; `null`/`""` = the server's default build. */
  build?: string | null;
  buildName?: string;
  /** Store this custom module before loading (local bundled path only). */
  custom?: { bytes: Uint8Array; name: string };
  clearCustom?: boolean;
}

interface SettingsSnapshot {
  server: string;
  build: string;
  buildName: string;
  custom: CustomRules | null;
}

function snapshotSettings(): SettingsSnapshot {
  return {
    server: serverBase(),
    build: buildId(),
    buildName: buildNameOf(),
    custom: customRules(),
  };
}

async function restoreSettings(s: SettingsSnapshot): Promise<void> {
  setServer(s.server || null);
  setBuild(s.build || null, s.buildName);
  if (s.custom) await setCustomRules(s.custom.bytes, s.custom.name);
  else await clearCustomRules();
}

const sameSettings = (s: SettingsSnapshot): boolean => {
  const custom = customRules();
  return (
    serverBase() === s.server &&
    buildId() === s.build &&
    buildNameOf() === s.buildName &&
    customRulesHash(custom) === customRulesHash(s.custom)
  );
};

async function applyChange(change: SourceChange): Promise<void> {
  if ("server" in change) {
    setServer(change.server ?? null);
    // `setServer` deliberately keeps the build when the base does not change,
    // but a server change always means that server's default build (which,
    // on the same origin, is how a selected build is cleared).
    if (!("build" in change)) setBuild(null);
  }
  if ("build" in change) setBuild(change.build ?? null, change.buildName ?? "");
  if (change.custom && !(await setCustomRules(change.custom.bytes, change.custom.name))) {
    throw new Error(tr("solo.buildStoreError"));
  }
  if (change.clearCustom) await clearCustomRules();
}

/** Switches run one at a time: a second click while one is in flight waits
 *  for it instead of interleaving two loads on one glue. */
let queue: Promise<void> = Promise.resolve();

/**
 * Point the page at another server / build / custom ruleset **without a page
 * reload**: persist the change, load the whole new source (rules and tables)
 * before anything live is replaced, and only then remount the scenes on the
 * new `D` (see `dataVersionOf`).
 *
 * A failure reverts the settings and reloads the previous source in place.
 * Only if that revert also fails does the page fall back to `location.reload`.
 * Progress is on `loadState`; the caller shows it and swallows the rejection
 * (the toast here already says what happened).
 */
export function switchSource(change: SourceChange): Promise<void> {
  const run = queue.then(() => doSwitch(change));
  queue = run.catch(() => {});
  return run;
}

async function doSwitch(change: SourceChange): Promise<void> {
  const before = snapshotSettings();
  try {
    // Inside the try: a storage refusal can throw after part of the change was
    // already persisted, and the page must not be left half-switched.
    await applyChange(change);
    setLoad({ busy: true, stage: "rules", done: 0, total: 0 });
    const target = await resolveTargets();
    await loadInto(target, true);
  } catch (e) {
    // The change itself may already have been toasted (e.g. a full storage);
    // only reload when something actually moved.
    if (!sameSettings(before)) {
      console.error("[rules] source switch failed; reverting", e);
      await restoreSettings(before);
      try {
        const target = await resolveTargets();
        await loadInto(target, true);
        toast(tr("boot.switchReverted", { detail: detail(e) }), "error");
      } catch (e2) {
        console.error("[rules] revert failed; reloading the page", e2);
        toast(tr("boot.switchReload", { detail: detail(e2) }), "error");
        location.reload();
      }
    }
    throw e;
  } finally {
    // Invalidate the last run so its rejected siblings cannot re-open the
    // overlay after this switch has closed it.
    activeRun++;
    setLoad({ busy: false, stage: "idle", done: 0, total: 0 });
  }
}

/** `band` value of the cards every character may take. A data key from the
 *  roster (the data set is Chinese), not display text. */
export const GENERAL_GROUP = "通用";

export function cardTitle(id: string): string {
  // A prompt can render before `load_data` has filled `D` (or during a module
  // hot-reload): never read through it unguarded, or the whole board blanks.
  if (!id || !D) return tr("common.unnamed");
  const card = D.card(id);
  if (card?.name) return card.name;
  // Skills use card-typed message arguments too, but their names live in the
  // roster's bands / characters. Match the engine's skill:<owner>:<skill> id.
  const skill = skillCard(id);
  if (skill) return skill.title;
  // An event's activation names it as `event:<id>` (the rule id) or as the
  // bare roster id the draw event carries -- both resolve here.
  const ev = D.event(id.startsWith("event:") ? id.slice(6) : id)?.name;
  if (ev) return ev;
  // A tile rule instance (`tile:circle`) is named after the board tile of that
  // kind; a mark owner (`mark:cp`) is named after its id.
  if (id.startsWith("tile:")) {
    const t = D.tiles.find((x) => x.kind === id.slice(5));
    if (t) return t.name.replace(/\n/g, "");
  }
  // A card row with no trimmed `name` is still titled -- `rawName` (the
  // character-prefix cards); a blank sheet name line carries its cell handle
  // in `name` already (`#J11`). Either way the 「效果适用」 header names the
  // same card the card's own log lines do.
  return cardFaceTitle(card, tr("common.unnamed"));
}

/** A `skill:<owner>:<skill>` id (the engine's band / character skill stand-in,
 *  e.g. the 团卡 `skill:Poppin' Party:星之鼓动`) resolved for display: title and
 *  body come from the roster's bands / characters, art from the band logo or
 *  the character's art. Undefined for anything that is not a skill id. */
export function skillCard(id: string): { title: string; text: string; group?: string; color: string; art: string } | undefined {
  if (!id || !D || !id.startsWith("skill:")) return undefined;
  const band = D.groups.find((b) => b.skill && id === `skill:${b.name}:${b.skill}`);
  if (band) return { title: band.skill, text: skillText(band), group: band.name, color: band.color, art: bandLogo(band.name) };
  const character = D.personas.find((c) => c.skill && id === `skill:${c.name}:${c.skill}`);
  if (character) return { title: character.skill, text: skillText(character), group: character.group, color: character.color, art: charArt(D.artId(character), "thumb") };
  return undefined;
}

/** Art url for any card id: a skill stand-in's own art (band logo / character
 *  thumb), else the manifest's card art (or the card back). `||`, not `??` --
 *  `skillCard` returns `""` when the roster entry has no art, and that must
 *  still fall through to the card back rather than a broken image. */
export function cardArtUrl(id: string): string {
  return skillCard(id)?.art || cardArt(id);
}

/** The effect text of a card id: a data card's text, or a skill stand-in's. */
export function cardText(id: string): string {
  if (!id || !D) return "";
  const c = D.card(id);
  if (c) return skillText(c);
  const sk = skillCard(id);
  if (sk) return sk.text;
  return D.event(id.startsWith("event:") ? id.slice(6) : id)?.text ?? "";
}

/** The effect text of a card id as structured sections / bulletpoints, when
 *  the display export carried them (cards and events do; skill stand-ins and
 *  persona / group bodies do not and fall back to `parse(cardText(id))`). */
export function cardBody(id: string): Block[] | undefined {
  if (!id || !D) return undefined;
  const c = D.card(id);
  if (c?.body) return c.body;
  const ev = D.event(id.startsWith("event:") ? id.slice(6) : id);
  return ev?.body;
}

/** Why a [反击] card may be declared: its own trigger-condition clause
 *  (「…时」), as one or more **locale keys** joined with `\n` (the client
 *  resolves them at render -- `counter.*` / `cards:<crate>.…`). Empty when the
 *  card carries no such clause. See `cardCounterReasonText` for the display
 *  string. */
export function cardCounterReason(id: string): string {
  if (!id || !D) return "";
  return D.card(id)?.counterReason ?? "";
}

/** The band colour a card id belongs to (skill stand-ins included). */
export function cardColor(id: string): string {
  if (!id || !D) return "#ED4E76";
  const c = D.card(id);
  return c ? bandColorOf(c) : skillCard(id)?.color ?? "#ED4E76";
}

function bandColorOf(c: CardData): string {
  return D.group(c.group)?.color ?? "#ED4E76";
}

/** The entry's own rulebook skill body. */
export function skillText(entry: { text: string } | undefined): string {
  return entry?.text ?? "";
}
