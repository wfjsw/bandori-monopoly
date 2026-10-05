// Game data: fetched once from /data, handed to the wasm rules and kept typed here
// for display.

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

  const j = (n: string) => JSON.parse(files[n].replace(/^﻿/, ""));
  const cards: CardData[] = j("cards.json").cards;
  const characters: CharacterData[] = j("characters.json").characters;
  const bands: BandData[] = j("bands.json").bands;
  const events: EventData[] = j("events.json").events;
  const homeLines = j("home_lines.json").characters ?? [];
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
