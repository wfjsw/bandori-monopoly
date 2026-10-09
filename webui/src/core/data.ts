// Game data: fetched once from /data, handed to the wasm rules and kept typed here
// for display.

import { settings } from "./store";
import init, * as glue from "../wasm/glue";
import engineId from "../wasm/engine_id.json";
import { t as tr } from "../i18n/t";
import { toast } from "../ui/Toast";
import { loadRulesetInto } from "./rulesetLoad";
import { bandLogo, charArt } from "./assets";
import type { BandData, CardData, CharacterData, EventData, TileData, VoiceLine } from "./types";

export interface GameData {
  tiles: TileData[];
  cards: CardData[];
  characters: CharacterData[];
  bands: BandData[];
  events: EventData[];
  voiceLines: { id: string; name: string; lines: VoiceLine[] }[];
  homeLines: { name: string; tag: string }[];
  rulesText: string;
  card(id: string): CardData | undefined;
  character(name: string): CharacterData | undefined;
  band(name: string): BandData | undefined;
  event(id: string): EventData | undefined;
  artId(c: CharacterData): string;
  homeTag(c: CharacterData): string;
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
 *  `StubRules` (plain-Monopoly tiles, no card or skill effects), so this has to
 *  run after `load_data` (the ruleset binds to the game data) and before any
 *  `SoloMatch` is built. Online play runs the rules on the server, so a failure
 *  here is not fatal -- but it **is** shown, because a match recorded without
 *  card rules and replayed with them (or the other way round) diverges at the
 *  first checkpoint and looks like engine drift. */
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

export async function loadGameData(progress: (p: number) => void): Promise<void> {
  await init();
  // Glue identity for the engine-bundle id (`docs/REPLAY.md` §9): every
  // record this build seals names the bundle that can replay it. Comes from
  // `tools/build-glue.mjs`; empty only if a glue was built without it.
  if (typeof glue.set_glue_sha === "function" && engineId?.glueSha256) {
    glue.set_glue_sha(engineId.glueSha256);
  }
  const names: string[] = JSON.parse(glue.data_files());
  const files: Record<string, string> = {};
  let done = 0;
  await Promise.all(
    names.map(async (n) => {
      const r = await fetch("/data/" + n);
      if (!r.ok) throw new Error(tr("boot.dataError", { name: n }));
      files[n] = await r.text();
      progress(++done / names.length);
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
  glue.load_data(JSON.stringify(files));
  await loadRuleset();

  const j = (n: string) => JSON.parse(files[n].replace(/^﻿/, ""));
  const cards: CardData[] = j("cards.json").cards;
  const characters: CharacterData[] = j("characters.json").characters;
  const bands: BandData[] = j("bands.json").bands;
  const events: EventData[] = j("events.json").events;
  const homeLines = j("home_lines.json").characters ?? [];
  // `skill_simple.json` is the hand-written newcomer version of each skill body
  // (`· ` bullets). It rides on the character/band record as `simple`, which is
  // what `skillText` picks when the setting is on -- the originals in
  // characters.json / bands.json stay the `text` fallback.
  const simple = j("skill_simple.json") as {
    characters: { name: string; text: string }[];
    bands: { name: string; text: string }[];
  };
  for (const c of characters) c.simple = simple.characters.find((x) => x.name === c.name)?.text;
  for (const b of bands) b.simple = simple.bands.find((x) => x.name === b.name)?.text;
  const byId = new Map(cards.map((c) => [c.id, c]));
  D = {
    tiles: j("board.json").tiles,
    cards,
    characters,
    bands,
    events,
    voiceLines: j("voice_lines.json").characters ?? [],
    homeLines,
    rulesText: files["rules.txt"].replace(/^﻿/, ""),
    card: (id) => byId.get(id),
    character: (name) => characters.find((c) => c.name === name),
    band: (name) => bands.find((b) => b.name === name),
    event: (id) => events.find((e) => e.id === id),
    artId: (c) => c.art || c.cnId,
    homeTag: (c) => homeLines.find((x: { name: string; tag: string }) => x.name === c.name && x.tag)?.tag ?? c.display,
  };
}

/** `band` value of the cards every character may take. A data key from
 *  `cards.json` (the data set is Chinese), not display text. */
export const GENERAL_BAND = "通用";

export function cardTitle(id: string): string {
  // A prompt can render before `load_data` has filled `D` (or during a module
  // hot-reload): never read through it unguarded, or the whole board blanks.
  if (!id || !D) return tr("common.unnamed");
  const name = D.card(id)?.name;
  if (name) return name;
  // Skills use card-typed message arguments too, but their names live in
  // bands.json / characters.json. Match the engine's skill:<owner>:<skill> id.
  const skill = skillCard(id);
  if (skill) return skill.title;
  return tr("common.unnamed");
}

/** A `skill:<owner>:<skill>` id (the engine's band / character skill stand-in,
 *  e.g. the 团卡 `skill:Poppin' Party:星之鼓动`) resolved for display: title and
 *  body come from bands.json / characters.json, art from the band logo or the
 *  character's art. Undefined for anything that is not a skill id. */
export function skillCard(id: string): { title: string; text: string; band?: string; color: string; art: string } | undefined {
  if (!id || !D || !id.startsWith("skill:")) return undefined;
  const band = D.bands.find((b) => b.skill && id === `skill:${b.name}:${b.skill}`);
  if (band) return { title: band.skill, text: skillText(band), band: band.name, color: band.color, art: bandLogo(band.name) };
  const character = D.characters.find((c) => c.skill && id === `skill:${c.name}:${c.skill}`);
  if (character) return { title: character.skill, text: skillText(character), band: character.band, color: character.color, art: charArt(character.art, "thumb") };
  return undefined;
}

/** The effect text of a card id: a data card's text, or a skill stand-in's. */
export function cardText(id: string): string {
  if (!id || !D) return "";
  const c = D.card(id);
  if (c) return skillText(c);
  return skillCard(id)?.text ?? "";
}

/** The band colour a card id belongs to (skill stand-ins included). */
export function cardColor(id: string): string {
  if (!id || !D) return "#ED4E76";
  const c = D.card(id);
  return c ? bandColorOf(c) : skillCard(id)?.color ?? "#ED4E76";
}

function bandColorOf(c: CardData): string {
  return D.band(c.band)?.color ?? "#ED4E76";
}

/** C# `SkillText.Of(c)`: the simplified skill text when the setting is on. */
export function skillText(entry: { text: string; simple?: string } | undefined): string {
  if (!entry) return "";
  return settings().skillTextSimple && entry.simple ? entry.simple : entry.text;
}

/** C# `SkillText.SwitchLabel`: the action the toggle performs. */
export function skillTextSwitch(): string {
  return settings().skillTextSimple ? "settings.skillFull" : "settings.skillSimple";
}
