// Geometry for the vector name banner: the sticker-style plate — a rounded pill
// with a dotted end zone, a star at each end and the name in the rounded face on
// top. Measured off the extracted `name_0xx.webp` plates (tools/name-banner.py
// builds the same shape for offline assets); drawing it live is what lets the
// plate follow the roster row instead of the art set, which only covers the base
// characters.
//
// The parts are returned as numbers and path strings so the component can build
// the SVG as elements — nothing here is markup.

/** Banner height; the whole layout is measured against it. */
export const BANNER_HEIGHT = 115;

/** White rim between the pill and the edge. */
export const RIM = 2.8;

/** Title metrics: size, baseline and outline width. */
export const TITLE = { size: 57, base: 61.5, spacing: 0.3, outline: 14 };

/** Subtitle metrics. */
export const SUBTITLE = { size: 19, spacing: 3.2 };

export interface Pill {
  /** Pill top edge and height (the rim sits outside them). */
  y: number;
  height: number;
  /** Vertical centre — where the stars sit. */
  mid: number;
  /** Inner, dot-bearing box. */
  innerX: number;
  innerY: number;
  innerWidth: number;
  innerHeight: number;
  innerRadius: number;
  radius: number;
}

/** Pill box and dot zone for a banner `width` px long. */
export function pill(width: number): Pill {
  const y = 61;
  const height = 54;
  return {
    y,
    height,
    mid: y + height / 2,
    innerX: RIM,
    innerY: y + RIM,
    innerWidth: width - 2 * RIM,
    innerHeight: height - 2 * RIM,
    innerRadius: height / 2 - RIM,
    radius: height / 2,
  };
}

/** The dotted end zone: a 45-degree cut on its left, anchored to the right end. */
export function dotZone(width: number, p: Pill) {
  const length = 150;
  const cellX = 17.5;
  const cellY = 17;
  const cutBottom = width - length;
  return {
    cellX,
    cellY,
    radius: 2.5,
    cutBottom,
    cutTop: cutBottom + p.innerHeight,
    offsetX: (width - 149.5) % cellX,
    offsetY: 109 % cellY,
  };
}

/** Star centres: measured off the plates, which sit a touch in from the ends. */
export function starCentres(width: number, p: Pill) {
  return { left: 31.5, right: width - 29.5, y: p.mid - 0.5, radius: 10.6 };
}

/** CJK punctuation: fullwidth in the font's em box, but not a full em of ink. */
function isPunctuation(cp: number) {
  return (cp >= 0x3000 && cp <= 0x303f) || (cp >= 0xff01 && cp <= 0xff0f) ||
    (cp >= 0xff1a && cp <= 0xff20) || (cp >= 0xff3b && cp <= 0xff40) || (cp >= 0xff5b && cp <= 0xff65);
}

/** Rough advance width of `text`, good enough to size the pill. */
function textWidth(text: string, size: number, spacing: number) {
  let w = 0;
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    w += isPunctuation(cp) ? size * 0.68 : cp >= 0x2e80 ? size : size * 0.585;
  }
  return w + spacing * Math.max(0, [...text].length - 1);
}

/** A length that frames `title` and `subtitle` with the end zones clear. */
export function bannerWidth(title: string, subtitle = "") {
  const need = Math.max(
    textWidth(title, TITLE.size, TITLE.spacing),
    textWidth(subtitle, SUBTITLE.size, SUBTITLE.spacing),
  );
  return Math.round(Math.max(430, need + 2 * MARGIN));
}

/** Side room the plate keeps clear of the title, per side. */
export const MARGIN = 95;

/** Title size that keeps `title` inside a banner `width`, never above the plate size. */
export function titleSize(title: string, width: number) {
  const natural = textWidth(title, TITLE.size, TITLE.spacing);
  const room = width - 2 * MARGIN;
  return natural <= room ? TITLE.size : Math.max(20, (TITLE.size * room) / natural);
}

/** Subtitle size that keeps `subtitle` inside a banner `width`, never above the plate size. */
export function subtitleSize(subtitle: string, width: number) {
  const natural = textWidth(subtitle, SUBTITLE.size, SUBTITLE.spacing);
  const room = width - 2 * MARGIN;
  return natural <= room ? SUBTITLE.size : Math.max(9, (SUBTITLE.size * room) / natural);
}

/** Width of the strip that hides the pill rim under the title. */
export function titleBand(title: string, size: number) {
  const est = textWidth(title, size, TITLE.spacing);
  return Math.max(0, est - 6);
}

/** Five-point star with rounded tips and inner corners. */
export function starPath(cx: number, cy: number, rOut: number) {
  const fatness = 0.55;
  const tip = 3;
  const inner = 1;
  const pts: [number, number][] = [];
  for (let i = 0; i < 10; i++) {
    const r = i % 2 === 0 ? rOut : rOut * fatness;
    const a = ((-90 + i * 36) * Math.PI) / 180;
    pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
  }
  const toward = (a: [number, number], b: [number, number], d: number): [number, number] => {
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const t = l ? Math.min(d / l, 0.49) : 0;
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  };
  const segs = pts.map((v, i) => {
    const d = i % 2 === 0 ? tip : inner;
    return [toward(v, pts[(i + 9) % 10], d), v, toward(v, pts[(i + 1) % 10], d)] as const;
  });
  const f = (p: readonly [number, number]) => `${p[0].toFixed(2)},${p[1].toFixed(2)}`;
  let d = `M${f(segs[0][2])}`;
  for (const [a, v, b] of [...segs.slice(1), segs[0]]) d += ` L${f(a)} Q${f(v)} ${f(b)}`;
  return d + "Z";
}
