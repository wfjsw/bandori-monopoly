// Render a marker plan (`markerKinds.ts`) as display strings.
//
// The rows, their order and their glyphs are rules data (`markerDefs.ts`); this
// file only resolves a text slot to its sentence -- a `tr` key here, a literal
// template as-is. Every surface (face, live note, popups, marker list, skill
// chips) renders the same `Marker[]`.

import { counterLabel } from "../../i18n/stateLabel";
import { t as tr } from "../../i18n/t";
import type { ActiveEvent, FieldCard } from "../../core/types";
import { MARKER_DEFS } from "./markerDefs";
import { glyphOf, planMarkers, type PlannedMarker, type TextSlot } from "./markerKinds";

export { showTurnsChip } from "./markerKinds";

/** Named counters resolve through the i18n phrase table (`null` = no phrase). */
const nameOf = (name: string): string | undefined => counterLabel(name) ?? undefined;

/** One text slot: its `tr` key with the count, or its template filled in. */
function text(slot: TextSlot, n: number, name?: string): string {
  if (slot.key) return tr(slot.key, { n, count: n, name });
  return (slot.text ?? "").replaceAll("{n}", `${n}`).replaceAll("{name}", name ?? "");
}

/** A planned marker plus the strings the UI prints for it. */
export interface Marker extends PlannedMarker {
  chip: string;
  note: string;
  label: string;
}

function build(planned: PlannedMarker[]): Marker[] {
  return planned.map((m) => ({
    ...m,
    chip: glyphOf(m.def, m.n, m.name),
    note: text(m.def.note, m.n, m.name),
    label: text(m.def.label, m.n, m.name),
  }));
}

/** The prop name the exported countdown row reads (an event row has no props
 *  map; its `turnsLeft` is a top-level field). */
const COUNTDOWN_KEY = MARKER_DEFS.find((d) => d.countdown)?.key;

/** The markers of one placed card. */
export function fieldMarkers(fc: FieldCard): Marker[] {
  return build(planMarkers(MARKER_DEFS, { props: fc.props, counters: fc.counters ?? {} }, nameOf));
}

/** The markers of one active event (its `counter` / `counter2` are unnamed
 *  engine pools, which the counter rows print). */
export function eventMarkers(e: ActiveEvent): Marker[] {
  const props = COUNTDOWN_KEY && e.turnsLeft !== undefined ? { [COUNTDOWN_KEY]: e.turnsLeft } : undefined;
  return build(planMarkers(MARKER_DEFS, { props, counters: { counter: e.counter, counter2: e.counter2 } }, nameOf));
}

/** The live-note text for a card: its own note, then every marker sentence. */
export function markerNote(note: string, markers: Marker[]): string {
  return [note, ...markers.map((m) => m.note)].filter(Boolean).join(" · ");
}
