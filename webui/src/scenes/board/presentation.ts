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
 * whenever it is posted. The activation's final decision is `e.flash` itself
 * (set at commit); the client does not re-derive it.
 *
 * A proactive `"flash"` cue (`ctx::flash_card` / `flash_tile`, v55) always
 * shows -- the named card's face, or a tile banner when there is no card --
 * unless this view has already played that event id (see {@link FlashOnce}).
 */
export type ActivationPresentation = "none" | "pulse" | "flash";

export interface PresentationEvent {
  /** Event id: the once-per-flash identity (see {@link FlashOnce}). */
  id?: number;
  /** Event kind: `"card"` activation, proactive `"flash"` cue, … */
  type?: string;
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
  // Proactive `"flash"` cues always show (centre face, or a tile banner when
  // there is no card). Never a field-slot pulse; never owner-suppressed.
  if (e.type === "flash") return "flash";
  if (e.flash === false && !e.negated) return "none";
  if (e.msg?.k === "log.effect_applied" && !FRESH_KINDS.has(e.kind ?? "") && (e.uid ?? -1) >= 0) {
    return "pulse";
  }
  return "flash";
}

/**
 * Once-per-flash identity, client side (ABI v55).
 *
 * A flash is identified by its event id: card activations that flashed, and
 * proactive `type: "flash"` cues. Replaying the same event frames twice --
 * SSE reconnect, replay seek, `addLog` of an already-seen id -- plays each
 * flash **once** in this view. {@link seal} records a history row on mount
 * without animating; {@link claim} is the gate in front of every play.
 */
export class FlashOnce {
  private played = new Set<number>();

  /** History ingest (mount): mark the id as already shown, play nothing. */
  seal(id: number): void {
    if (id >= 0) this.played.add(id);
  }

  /** Claim an id for play. `false` when this view already played it. A
   *  negative id carries no identity and always plays. */
  claim(id: number): boolean {
    if (id < 0) return true;
    if (this.played.has(id)) return false;
    this.played.add(id);
    return true;
  }

  has(id: number): boolean {
    return this.played.has(id);
  }
}

/**
 * [`activationPresentation`] after once-per-flash accounting: a re-delivered
 * frame of an id this view already played (or sealed) presents as `"none"`.
 */
export function flashPlay(once: FlashOnce, e: PresentationEvent): ActivationPresentation {
  const p = activationPresentation(e);
  if (p === "none") return "none";
  return once.claim(e.id ?? -1) ? p : "none";
}