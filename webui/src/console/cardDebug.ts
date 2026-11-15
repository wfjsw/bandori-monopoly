// Card-activation debug lines for the console. The game event stream already
// logs every event's prose at info level; these add the machine-readable
// fields a card bug hunt needs (which card, which trigger, whose seat, which
// field instance, what it targeted, and whether a counteraction negated it).
//
// The formatter is passed in so this module stays free of the i18n runtime
// (`node --test` can load it with a plain stub).

import type { MatchEvent } from "../core/types.ts";
import type { Msg } from "../i18n/msg.ts";

/** Whether `e` carries card detail worth a debug line. */
export function hasCardDebug(e: MatchEvent): boolean {
  return e.type === "card" || !!e.card;
}

/** One debug line for a card-bearing event; `format` renders a message. */
export function cardDebugLine(e: MatchEvent, format: (m: Msg) => string): string {
  const fields = [
    `event=#${e.id}`,
    `type=${e.type}`,
    `card=${e.card || "(none)"}`,
    e.msg ? `msg=${format(e.msg)}` : "",
    e.kind ? `kind=${e.kind}` : "",
    e.playerId >= 0 ? `seat=#${e.playerId}` : "",
    e.uid != null && e.uid >= 0 ? `uid=${e.uid}` : "",
    e.parent != null && e.parent >= 0 ? `parent=#${e.parent}` : "",
    e.other >= 0 ? `target=#${e.other}` : "",
    e.value ? `value=${e.value}` : "",
    e.negated ? "negated" : "",
    ...(e.results ?? []).map((r) => `result=${format(r)}`),
  ];
  return `[card] ${fields.filter(Boolean).join(" ")}`;
}
