// Which markers a card face prints, in what order -- the part that needs no
// i18n runtime (so it is testable under `node --test`).
//
// `markers.ts` turns a plan into display strings; the face, the notes, the
// popups and the marker list all consume that, and none of them knows a kind.

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

interface KindSpec {
  kind: MarkerKind;
  cls: ChipClass;
  countdown?: boolean;
}

/** Face order: the countdown, then the unnamed counters, then the named ones. */
const ORDER: KindSpec[] = [
  { kind: "turns", cls: "turns", countdown: true },
  { kind: "counter", cls: "count" },
  { kind: "crystals", cls: "crystal" },
  { kind: "cp", cls: "cp" },
];

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
  const add = (spec: KindSpec, kind: MarkerKind, n: number, key: string = spec.kind, name?: string) => {
    out.push({ key, kind, n, cls: spec.cls, countdown: spec.countdown ?? false, name });
  };
  const addIfAny = (spec: KindSpec, n: number | undefined, key?: string) => {
    if (n !== undefined && n > 0) add(spec, spec.kind, n, key);
  };

  const [turns, counter, crystals, cp] = ORDER;
  addIfAny(turns, c.turnsLeft);
  // `counter2` shares the generic-counter row; it is a second unnamed pool.
  addIfAny(counter, c.counter);
  addIfAny(counter, c.counter2, "counter2");
  addIfAny(crystals, c.crystals);
  addIfAny(cp, c.cp);

  for (const [name, n] of c.named ?? []) {
    if (n <= 0 || name === "crystals" || name === "cp") continue;
    const label = nameOf(name);
    if (label) add({ kind: "named", cls: "count" }, "named", n, `c:${name}`, label);
  }

  return out.filter((m) => !m.countdown || showTurnsChip(m.n, ...out.filter((o) => o !== m).map((o) => o.n)));
}
