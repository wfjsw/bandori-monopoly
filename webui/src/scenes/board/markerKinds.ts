// What a card face prints in its marker zone: one row per marker kind, in
// print order, each with its own glyph, styling and sentence.
//
// Adding a marker kind is a row in `MARKER_KINDS` -- nothing else. The face,
// the live note, the popups, the player marker list and the skill chips all
// render the plan built here and none of them knows a kind.
//
// The module holds no i18n runtime (so it is testable under `node --test`):
// a text slot is either a `tr` key (resolved by `markers.ts`) or a literal
// template with `{n}` / `{name}` placeholders.

/** CSS class (in `FieldSheet.module.css`) a chip of this kind wears. */
export type ChipClass = "turns" | "count" | "crystal" | "cp";

export type MarkerKind = "turns" | "counter" | "counter2" | "crystals" | "cp" | "named";

export interface PlannedMarker {
  /** Stable render key: the kind, or `c:<name>` for a named counter. */
  key: string;
  kind: MarkerKind;
  /** The number the marker counts. */
  n: number;
  cls: ChipClass;
  /** A countdown is dropped when another marker already carries its number. */
  countdown: boolean;
  /** Display name, for a named counter. */
  name?: string;
}

/** One text slot: a `tr` key, or a literal template. */
export interface TextSlot {
  key?: string;
  text?: string;
}

/** One marker kind: how it is written, styled and described. */
export interface MarkerKindDef {
  kind: MarkerKind;
  cls: ChipClass;
  countdown?: boolean;
  /** Glyph on the face. `{n}` = the count, `{name}` = a named counter. */
  glyph: string;
  /** Sentence for the live note and the chip's tooltip. */
  note: TextSlot;
  /** Label the player marker list pairs with the card name. */
  label: TextSlot;
}

/** Face order: the countdown, then the unnamed counters, then the named ones. */
const MARKER_KINDS: MarkerKindDef[] = [
  {
    kind: "turns",
    cls: "turns",
    countdown: true,
    glyph: "⌛{n}",
    note: { key: "board.turnsLeft" },
    label: { key: "board.turnsLabel" },
  },
  { kind: "counter", cls: "count", glyph: "×{n}", note: { text: "×{n}" }, label: { text: "×{n}" } },
  { kind: "crystals", cls: "crystal", glyph: "◆{n}", note: { key: "board.crystals" }, label: { key: "board.crystalsLabel" } },
  { kind: "cp", cls: "cp", glyph: "CP{n}", note: { key: "board.cp" }, label: { text: "CP" } },
  {
    kind: "named",
    cls: "count",
    glyph: "{name}×{n}",
    note: { text: "{name} ×{n}" },
    label: { text: "{name}" },
  },
];

/** A kind's definition. */
export const MARKER_KIND: Record<MarkerKind, MarkerKindDef> = Object.fromEntries(
  MARKER_KINDS.map((k) => [k.kind, k]),
) as Record<MarkerKind, MarkerKindDef>;

/** Fill a glyph template (`{n}` / `{name}`). */
export function glyphOf(kind: MarkerKind, n: number, name?: string): string {
  return MARKER_KIND[kind].glyph.replaceAll("{n}", `${n}`).replaceAll("{name}", name ?? "");
}

/** Whether a `⌛N` countdown should print: a number is scheduled to end and no
 *  other marker on the same face already carries it. The countdown stays in the
 *  data either way; this only decides whether the face prints it again. */
export function showTurnsChip(n: number, ...others: number[]): boolean {
  return n > 0 && !others.some((o) => o === n);
}

/** Numbers a card instance can carry, whatever surface it lives on. */
export interface Counts {
  turnsLeft?: number;
  counter?: number;
  counter2?: number;
  crystals?: number;
  cp?: number;
  /** The instance's other named counters, in map order. */
  named?: [string, number][];
}

/**
 * The face's markers. `nameOf` resolves a named counter to its display phrase;
 * an unresolved one is dropped, never printed as its wire key. A countdown
 * another marker already carries (the crystal count a decay removes, a phase
 * counter) is not printed a second time.
 */
export function planMarkers(c: Counts, nameOf: (name: string) => string | undefined): PlannedMarker[] {
  const out: PlannedMarker[] = [];
  const add = (kind: MarkerKind, n: number, key: string = kind, name?: string) => {
    out.push({ key, kind, n, cls: MARKER_KIND[kind].cls, countdown: MARKER_KIND[kind].countdown ?? false, name });
  };
  const addIfAny = (kind: MarkerKind, n: number | undefined, key?: string) => {
    if (n !== undefined && n > 0) add(kind, n, key);
  };

  addIfAny("turns", c.turnsLeft);
  // `counter2` shares the generic-counter row; it is a second unnamed pool.
  addIfAny("counter", c.counter);
  addIfAny("counter", c.counter2, "counter2");
  addIfAny("crystals", c.crystals);
  addIfAny("cp", c.cp);

  for (const [name, n] of c.named ?? []) {
    if (n <= 0 || name === "crystals" || name === "cp") continue;
    const label = nameOf(name);
    if (label) add("named", n, `c:${name}`, label);
  }

  return out.filter((m) => !m.countdown || showTurnsChip(m.n, ...out.filter((o) => o !== m).map((o) => o.n)));
}
