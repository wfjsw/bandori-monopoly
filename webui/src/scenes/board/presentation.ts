/**
 * Ruling 1 + the field-pulse reconciliation: what a `"card"` activation
 * animates. Pure -- no DOM, no parameter properties -- so `node --test` can
 * import it directly.
 *
 * - `"none"`: owner-self-only (`flash === false`) -- the log entry stays, but
 *   neither the centre flash nor the field-slot pulse shows.
 * - `"pulse"`: an in-play field card's 「效果适用」 -- light its slot, once.
 * - `"flash"`: everything else (hand play, [反击], skill, event).
 *
 * A negated activation (无效) always shows. This never reads the header
 * position, `results`, or `parent` -- the pulse keys on the activation event
 * whenever it is posted.
 */
export type ActivationPresentation = "none" | "pulse" | "flash";

export interface PresentationEvent {
  flash?: boolean;
  negated?: boolean;
  uid?: number;
  kind?: string;
  msg?: { k?: string };
}

/**
 * Fresh-activation kinds (play / [反击] / skill / event) flash the centre
 * face. A field hook's 「效果适用」 (`log.effect_applied`, kind `hook`) lights
 * the field slot instead.
 */
const FRESH_KINDS = new Set(["play", "counter", "skill", "event"]);

export function activationPresentation(e: PresentationEvent): ActivationPresentation {
  if (e.flash === false && !e.negated) return "none";
  if (e.msg?.k === "log.effect_applied" && !FRESH_KINDS.has(e.kind ?? "") && (e.uid ?? -1) >= 0) {
    return "pulse";
  }
  return "flash";
}
