// The stage's design-px -> screen-px scale, recorded by `Stage` each render.
// Design px are the design canvas's own units (the CSS `px` values in the
// modules); screen px are what the window shows. `getBoundingClientRect()`
// always reports screen px, in every browser's `zoom` semantics, so dividing
// by this is the one reliable way back to design px.

let scale = 1;

export function setStageScale(k: number): void {
  scale = Number.isFinite(k) && k > 0 ? k : 1;
}

export function stageScale(): number {
  return scale;
}

/** `el`'s box in design px (the stage's own pixel space). */
export function designBox(el: HTMLElement): { width: number; height: number } {
  const r = el.getBoundingClientRect();
  const k = scale || 1;
  return { width: r.width / k, height: r.height / k };
}

/** `el`'s box in screen px. */
export function screenBox(el: HTMLElement): DOMRect {
  return el.getBoundingClientRect();
}