// The stage's screen mapping: the design-px -> screen-px scale and whether the
// stage is turned a quarter (a portrait phone forced to landscape), both
// recorded by `Stage` each render. Design px are the design canvas's own units
// (the CSS `px` values in the modules); screen px are what the window shows.
//
// `getBoundingClientRect()` always reports screen px, in every browser's
// `zoom` semantics, so dividing by the scale is the one reliable way back to
// design px -- but its width/height are the *axis-aligned* box, so a rotated
// stage reports them swapped. `designBox` undoes both.

let scale = 1;
let rotated = false;

export function setStageScale(k: number): void {
  scale = Number.isFinite(k) && k > 0 ? k : 1;
}

export function stageScale(): number {
  return scale;
}

/** Set by `Stage` each render: the stage is `rotate(90deg)`ed on screen. */
export function setStageRotated(on: boolean): void {
  rotated = !!on;
}

export function isStageRotated(): boolean {
  return rotated;
}

/**
 * `el`'s box in design px (the stage's own pixel space). Under the stage's
 * rotation the screen box is the element's box with width/height traded, so
 * trade them back before dividing out the scale -- the same undoing `localRect`
 * in `rotation.ts` does for a child's box.
 */
export function designBox(el: HTMLElement): { width: number; height: number } {
  const r = el.getBoundingClientRect();
  const k = scale || 1;
  return rotated
    ? { width: r.height / k, height: r.width / k }
    : { width: r.width / k, height: r.height / k };
}

/**
 * `el`'s box in screen CSS px, un-swapped by the stage's rotation. Use this
 * (not `getBoundingClientRect`) whenever the result sizes a surface that must
 * match the element's own axes -- a canvas drawing buffer, say.
 */
export function screenCssBox(el: HTMLElement): { width: number; height: number } {
  const b = designBox(el);
  const k = scale || 1;
  return { width: b.width * k, height: b.height * k };
}