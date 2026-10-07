// Game data: fetched once from /data, handed to the wasm rules and kept typed here
// for display.

import { settings } from "./store";
import init, * as glue from "../wasm/glue";
import { t as tr } from "../i18n/t";
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

/** The card / skill / tile rule modules `tools/build-ruleset.mjs` copies to
 *  `/assets/rules/`. Without them every solo match runs on the engine's
 *  `StubRules` (plain-Monopoly tiles, no card or skill effects), so this has to
 *  run after `load_data` (the ruleset binds to the game data) and before any
 *  `SoloMatch` is built. Online play runs the rules on the server, so a failure
 *  here is logged rather than fatal. */
async function loadRuleset(): Promise<void> {
  try {
    const r = await fetch("/assets/rules/index.json");
    if (!r.ok) throw new Error(`index.json: HTTP ${r.status}`);
    const index: { modules: { file: string }[] } = await r.json();
    for (const m of index.modules) {
      const w = await fetch("/assets/rules/" + m.file);
      if (!w.ok) throw new Error(`${m.file}: HTTP ${w.status}`);
      glue.ruleset_add(new Uint8Array(await w.arrayBuffer()));
    }
    const n = glue.ruleset_build();
    console.info(`[rules] ${n} card module(s) loaded`);
  } catch (e) {
    console.error("[rules] card modules failed to load; solo matches will run without card, skill or tile rules", e);
  }
}

export async function loadGameData(progress: (p: number) => void): Promise<void> {
  await init();
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
  return D.card(id)?.name || tr("common.unnamed");
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
