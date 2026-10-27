// What a card face prints in its marker zone.
//
// The *choice* of markers (kinds, order, the countdown-suppression rule) is
// `markerKinds.ts`; this file only dresses each one for the UI: the glyph on
// the face, the sentence its note / tooltip shows and the label the player
// marker list pairs with the card name. Every surface (face, live note,
// popups, marker list, skill chips) renders the same `Marker[]`, so adding a
// kind is a row in `ORDER` plus its three strings here.

import { counterLabel } from "../../i18n/stateLabel";
import { t as tr } from "../../i18n/t";
import type { ActiveEvent, FieldCard } from "../../core/types";
import { planMarkers, type MarkerKind, type PlannedMarker } from "./markerKinds";

/** Named counters resolve through the i18n phrase table (`null` = no phrase). */
const nameOf = (name: string): string | undefined => counterLabel(name) ?? undefined;

export { showTurnsChip } from "./markerKinds";

/** How one kind dresses itself; `name` is the named counter's display phrase. */
interface Dress {
  chip: (n: number, name?: string) => string;
  note: (n: number, name?: string) => string;
  label: (n: number, name?: string) => string;
}

const DRESS: Record<MarkerKind, Dress> = {
  turns: {
    chip: (n) => `⌛${n}`,
    note: (n) => tr("board.turnsLeft", { n, count: n }),
    label: () => tr("board.turnsLabel"),
  },
  counter: { chip: (n) => `×${n}`, note: (n) => `×${n}`, label: (n) => `×${n}` },
  counter2: { chip: (n) => `×${n}`, note: (n) => `×${n}`, label: (n) => `×${n}` },
  crystals: {
    chip: (n) => `◆${n}`,
    note: (n) => tr("board.crystals", { n }),
    label: () => tr("board.crystalsLabel"),
  },
  cp: { chip: (n) => `CP${n}`, note: (n) => tr("board.cp", { n }), label: () => "CP" },
  named: {
    chip: (n, name) => `${name}×${n}`,
    note: (n, name) => `${name} ×${n}`,
    label: (_n, name) => name ?? "",
  },
};

/** A planned marker plus the three strings the UI prints for it. */
export interface Marker extends PlannedMarker {
  chip: string;
  note: string;
  label: string;
}

function build(planned: PlannedMarker[]): Marker[] {
  return planned.map((m) => {
    const d = DRESS[m.kind];
    return { ...m, chip: d.chip(m.n, m.name), note: d.note(m.n, m.name), label: d.label(m.n, m.name) };
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
