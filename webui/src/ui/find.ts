// In-console find over the whole log. The browser's own Ctrl-F only sees the
// windowed rows in the DOM, so the console runs its own search over the
// snapshot and scrolls the window to a hit.

export interface FindHit {
  /** Index into the searched line list. */
  line: number;
  /** Half-open match range inside that line's text. */
  start: number;
  end: number;
}

export interface FindResult {
  hits: FindHit[];
  /** More matches existed than `limit`; `hits` holds the first ones. */
  truncated: boolean;
}

/**
 * Case-insensitive, non-overlapping matches per line, in reading order.
 * `limit` bounds the work on an unbounded log.
 */
export function findHits(texts: readonly string[], query: string, limit = 5_000): FindResult {
  const needle = query.toLowerCase();
  if (!needle) return { hits: [], truncated: false };
  const hits: FindHit[] = [];
  for (let line = 0; line < texts.length; line++) {
    const hay = texts[line].toLowerCase();
    let at = hay.indexOf(needle);
    while (at >= 0) {
      if (hits.length >= limit) return { hits, truncated: true };
      hits.push({ line, start: at, end: at + needle.length });
      at = hay.indexOf(needle, at + needle.length);
    }
  }
  return { hits, truncated: false };
}

export interface FindSegment {
  text: string;
  hit: boolean;
  /** The hit the find cursor sits on. */
  current: boolean;
}

/** Split `text` at `spans` (sorted ones for that line) into marked segments. */
export function highlightSegments(text: string, spans: readonly FindHit[], current?: FindHit): FindSegment[] {
  const out: FindSegment[] = [];
  let at = 0;
  for (const span of spans) {
    if (span.start < at || span.end > text.length) continue;
    if (span.start > at) out.push({ text: text.slice(at, span.start), hit: false, current: false });
    out.push({
      text: text.slice(span.start, span.end), hit: true,
      current: !!current && current.line === span.line && current.start === span.start,
    });
    at = span.end;
  }
  if (at < text.length) out.push({ text: text.slice(at), hit: false, current: false });
  return out;
}
