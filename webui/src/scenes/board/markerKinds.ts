// What a card face prints in its marker zone.
//
// The kinds are **rules data**: one row per marker in
// `bandori_std::marker::MARKERS` (`rules/bandori-std/src/marker.rs`), exported
// through `generated/display.json` (`docs/DISPLAY.md`). This module hardcodes
// no kind -- it turns rows + an instance's numbers into a print plan. Adding a
// marker is a rules edit; only a new style tone also needs a CSS class.
//
// The face, the live note, the popups, the player marker list and the skill
// chips all render the plan built here and none of them knows a kind.
//
// The module holds no i18n runtime (so it is testable under `node --test`):
// a text slot is either a `tr` key (resolved by `markers.ts`) or a literal
// template with `{n}` / `{name}` placeholders.

/** CSS class (in `FieldSheet.module.css`) a chip tone wears. */
export type ChipClass = "turns" | "count" | "crystal" | "cp";

/** Where a marker's count is read from on the instance. */
export type MarkerSource = "prop" | "counter";

/** One text slot: a `tr` key, or a literal template. */
export interface TextSlot {
  key?: string;
  text?: string;
}

/** One exported marker row (`bandori_std::marker::MarkerDef`). */
export interface MarkerDef {
  /** The counter / prop name this row describes; absent on the fallback row. */
  key?: string;
  source: MarkerSource;
  /** Style tone. */
  tone: ChipClass;
  /** Glyph on the face. `{n}` = the count, `{name}` = a named counter. */
  glyph: string;
  /** Sentence for the chip's tooltip and the live note. */
  note: TextSlot;
  /** Label the player marker list pairs with the card name. */
  label: TextSlot;
  /** A countdown is dropped when another marker already carries its number. */
  countdown?: boolean;
  /** Takes every counter no other row covers. */
  fallback?: boolean;
}

export interface PlannedMarker {
  /** Stable render key: the row's key, or `c:<name>` for a named counter. */
  key: string;
  /** The row this marker was planned from (glyph / tone / sentences). */
  def: MarkerDef;
  /** The number the marker counts. */
  n: number;
  /** A countdown is dropped when another marker already carries its number. */
  countdown: boolean;
  /** Display name, for a named counter. */
  name?: string;
}

/** Fill a glyph template (`{n}` / `{name}`). */
export function glyphOf(def: MarkerDef, n: number, name?: string): string {
  return def.glyph.replaceAll("{n}", `${n}`).replaceAll("{name}", name ?? "");
}

/** Whether a `⌛N` countdown should print: a number is scheduled to end and no
 *  other marker on the same face already carries it. The countdown stays in the
 *  data either way; this only decides whether the face prints it again. */
export function showTurnsChip(n: number, ...others: number[]): boolean {
  return n > 0 && !others.some((o) => o === n);
}

/** Numbers a card instance can carry, whatever surface it lives on. */
export interface Counts {
  /** The instance's numeric props (the countdown lives here). */
  props?: Record<string, number>;
  /** The instance's counters, keyed by counter name. */
  counters?: Record<string, number>;
}

/**
 * The face's markers, in row order. Each row reads its own `source` + `key`;
 * the fallback row takes every positive counter no row covers, labelled by
 * `nameOf` (an unresolved counter is dropped, never printed as its wire key).
 * A countdown another marker already carries (the crystal count a decay
 * removes, a phase counter) is not printed a second time.
 */
export function planMarkers(
  defs: MarkerDef[],
  c: Counts,
  nameOf: (name: string) => string | undefined,
): PlannedMarker[] {
  const out: PlannedMarker[] = [];
  const push = (def: MarkerDef, key: string, n: number, name?: string) => {
    out.push({ key, def, n, countdown: def.countdown ?? false, name });
  };
  const covered = new Set(defs.flatMap((d) => (d.key ? [d.key] : [])));

  for (const def of defs) {
    if (def.fallback) {
      for (const [name, n] of Object.entries(c.counters ?? {})) {
        if (n <= 0 || covered.has(name)) continue;
        const label = nameOf(name);
        if (label) push(def, `c:${name}`, n, label);
      }
      continue;
    }
    const n = (def.source === "prop" ? c.props : c.counters)?.[def.key ?? ""];
    if (n !== undefined && n > 0) push(def, def.key ?? "", n);
  }

  return out.filter((m) => !m.countdown || showTurnsChip(m.n, ...out.filter((o) => o !== m).map((o) => o.n)));
}
