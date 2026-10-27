// Render a marker plan (`markerKinds.ts`) as display strings.
//
// The kinds, their order and their glyphs live in the marker table; this file
// only resolves a text slot to its sentence -- a `tr` key here, a literal
// template as-is. Every surface (face, live note, popups, marker list, skill
// chips) renders the same `Marker[]`.

import { counterLabel } from "../../i18n/stateLabel";
import { t as tr } from "../../i18n/t";
import type { ActiveEvent, FieldCard } from "../../core/types";
import { glyphOf, MARKER_KIND, planMarkers, type PlannedMarker, type TextSlot } from "./markerKinds";

export { showTurnsChip } from "./markerKinds";

/** Named counters resolve through the i18n phrase table (`null` = no phrase). */
const nameOf = (name: string): string | undefined => counterLabel(name) ?? undefined;

/** One text slot: its `tr` key with the count, or its template filled in. */
function text(slot: TextSlot, n: number, name?: string): string {
  if (slot.key) return tr(slot.key, { n, count: n, name });
  return (slot.text ?? "").replaceAll("{n}", `${n}`).replaceAll("{name}", name ?? "");
}

/** A planned marker plus the three strings the UI prints for it. */
export interface Marker extends PlannedMarker {
  chip: string;
  note: string;
  label: string;
}

function build(planned: PlannedMarker[]): Marker[] {
  return planned.map((m) => {
    const def = MARKER_KIND[m.kind];
    return {
      ...m,
      chip: glyphOf(m.kind, m.n, m.name),
      note: text(def.note, m.n, m.name),
      label: text(def.label, m.n, m.name),
    };
  });
}

/** The markers of one placed card. */
export function fieldMarkers(fc: FieldCard): Marker[] {
  const counters = fc.counters ?? {};
  return build(
    planMarkers(
      {
        turnsLeft: fc.props?.turnsLeft,
        crystals: counters["crystals"],
        cp: counters["cp"],
        named: Object.entries(counters),
      },
      nameOf,
    ),
  );
}

/** The markers of one active event (its `counter` is the mirrored crystals). */
export function eventMarkers(e: ActiveEvent): Marker[] {
  return build(planMarkers({ turnsLeft: e.turnsLeft, counter: e.counter, counter2: e.counter2 }, nameOf));
}

/** The live-note text for a card: its own note, then every marker sentence. */
export function markerNote(note: string, markers: Marker[]): string {
  return [note, ...markers.map((m) => m.note)].filter(Boolean).join(" · ");
}
