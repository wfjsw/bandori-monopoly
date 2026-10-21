// Game data: the identity roster from the wasm ruleset, merged with the
// display fields the webui owns (generated/display.json + data/*.json) and
// kept typed here for display.

import init, * as glue from "../wasm/glue";
import engineId from "../wasm/engine_id.json";
import displayJson from "../generated/display.json";
import { t as tr } from "../i18n/t";
import { toast } from "../ui/Toast";
import { loadRulesetInto } from "./rulesetLoad";
import { bandLogo, charArt } from "./assets";
import { cardFaceTitle } from "./cardFace";
import type { GroupData, CardData, PersonaData, EventData, TileData, VoiceLine, DisplayFile } from "./types";

/** Build-exported presentation fields (empty entries omitted). */
const display = displayJson as DisplayFile;

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

/** The card / skill / tile rule modules `tools/build-ruleset.mjs` copies to
 *  `/assets/rules/`. Without them every solo match runs on the engine's
 *  `StubRules` (plain-Monopoly tiles, no card or skill effects) and the roster
 *  is empty, so this runs **before** `roster_json()` / `load_data` (the
 *  ruleset is the roster source) and before any `SoloMatch` is built. Online
 *  play runs the rules on the server, so a failure here is not fatal -- it is
 *  toasted and the page continues with an empty roster / StubRules -- but a
 *  match recorded without card rules and replayed with them (or the other way
 *  round) diverges at the first checkpoint and looks like engine drift. */
async function loadRuleset(): Promise<void> {
  try {
    // The sequence lives in `rulesetLoad.ts` and is shared with the node gate
    // (`webui/src/game/ruleset.test.ts`): index.json -> modules -> the
    // precompiled-condition blob (`conds-*.bin`, docs/GUARDS.md §8.2) ->
    // `ruleset_build`.
    const n = await loadRulesetInto(glue, async (rel) => {
      const r = await fetch("/assets/rules/" + rel);
      if (!r.ok) throw new Error(`${rel}: HTTP ${r.status}`);
      return new Uint8Array(await r.arrayBuffer());
    });
    rulesetLoaded = true;
    console.info(`[rules] ${n} card module(s) loaded`);
  } catch (e) {
    console.error("[rules] card modules failed to load; solo matches will run without card, skill or tile rules", e);
    toast(tr("boot.rulesError", { detail: e instanceof Error ? e.message : String(e) }), "error");
  }
}

/**
 * The one boot gate for the typed `D`. Order matters:
 *
 * 1. wasm init + glue identity;
 * 2. **ruleset first** -- `roster_json()` is empty until `ruleset_build`, and
 *    deck builder / gallery / home read `D` as soon as this resolves;
 * 3. `roster_json()` for the identity rows (cards / personas / groups);
 * 4. the remaining `data_files()` tables + the UI-only tables + optional bot
 *    books, `load_data` (engine tables only);
 * 5. merge `generated/display.json` onto the roster and assign `D` here, only
 *    at the end, so a throw leaves `D` untouched and Boot shows the error
 *    stage instead of a half-initialized table.
 */
export async function loadGameData(progress: (p: number) => void): Promise<void> {
  await init();
  // Glue identity for the engine-bundle id (`docs/REPLAY.md` §9): every
  // record this build seals names the bundle that can replay it. Comes from
  // `tools/build-glue.mjs`; empty only if a glue was built without it.
  if (typeof glue.set_glue_sha === "function" && engineId?.glueSha256) {
    glue.set_glue_sha(engineId.glueSha256);
  }
  // Ruleset before the tables: the roster rides on the built set. Failure is
  // degraded (empty roster + StubRules), not fatal -- `loadRuleset` toasts.
  await loadRuleset();

  // Game roster after the presentation split: identity plus the rulebook
  // prose (`text`), which is game content and rides `ruleset_metadata`. Pure
  // display columns are merged below from `generated/display.json`.
  const roster = JSON.parse(glue.roster_json()) as {
    cards: { id: string; group: string; name: string; owner: string; derived: boolean; tags: string[]; text: string }[];
    personas: { name: string; group: string; skill: string; exclusiveCards: string[]; preset: string[]; variantOf?: string; text: string }[];
    groups: { name: string; skill: string; sharesExclusives?: boolean; text: string }[];
    events: { id: string; name: string; derived: boolean; text: string }[];
  };
  // Engine tables only (post-split: board / match_rules).
  const engineNames: string[] = JSON.parse(glue.data_files());
  const allNames = [...new Set([...engineNames, ...UI_DATA_FILES])];
  const files: Record<string, string> = {};
  let done = 0;
  await Promise.all(
    allNames.map(async (n) => {
      const r = await fetch("/data/" + n);
      if (!r.ok) throw new Error(tr("boot.dataError", { name: n }));
      files[n] = await r.text();
      progress(++done / allNames.length);
    }),
  );
  // Optional: the bot deck book (`docs/BOT.md` §3.7). Missing = empty book =
  // 托管 / bots keep the designer's preset decks. Outside `data_files()` on
  // purpose -- it tunes bots, not the match, so it is not part of the data
  // hash either.
  try {
    const book = await fetch("/data/deck_book.json");
    if (book.ok) files["deck_book.json"] = await book.text();
  } catch {
    /* absent book is fine */
  }
  // Optional: the bot strategy book (`docs/BOT.md` §3.8). Missing = empty
  // book = the default parameters (today's heuristic constants).
  try {
    const book = await fetch("/data/strategy_book.json");
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
    };
  });
  const homeLines = j("home_lines.json").personas ?? [];
  const byId = new Map(cards.map((c) => [c.id, c]));
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

/** The effect text of a card id: a data card's text, or a skill stand-in's. */
export function cardText(id: string): string {
  if (!id || !D) return "";
  const c = D.card(id);
  if (c) return skillText(c);
  const sk = skillCard(id);
  if (sk) return sk.text;
  return D.event(id.startsWith("event:") ? id.slice(6) : id)?.text ?? "";
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
