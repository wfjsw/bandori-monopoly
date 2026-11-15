// In-surface find for windowed lists (the console output, the match log).
// The browser's Ctrl-F only sees the rows currently in the DOM, so the
// surfaces own a find over the whole line list and scroll the window to the
// hit instead of letting the native one run.

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { t as tr } from "../i18n/t";
import { findHits, highlightSegments, type FindHit, type FindSegment } from "./find";
import s from "./FindBar.module.css";

export interface FindApi {
  open: boolean;
  query: string;
  hits: readonly FindHit[];
  /** The hit the cursor sits on, if any. */
  current: FindHit | undefined;
  position: number;
  truncated: boolean;
  setQuery(query: string): void;
  show(): void;
  close(): void;
  /** Move the cursor by `delta` hits, wrapping. */
  move(delta: number): void;
  /** Segments for `text` when the line has hits, else null. */
  segments(text: string, line: number): FindSegment[] | null;
}

/**
 * Find state over `texts`; `scrollTo` brings a line into view. The cursor
 * starts at the first hit of a new query and stays put while the list grows.
 */
export function useFind(texts: readonly string[], scrollTo: (index: number) => void): FindApi {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [position, setPosition] = useState(-1);
  const { hits, truncated } = useMemo(() => findHits(texts, query), [texts, query]);

  // A new query jumps to its first hit. Appends keep the cursor: the hook is
  // deliberately not keyed on `hits`, which a live log replaces constantly.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { setPosition(query && hits.length ? 0 : -1); }, [query]);
  // A shrinking list (clear, a narrower filter) clamps the cursor.
  useEffect(() => {
    setPosition((p) => (p < hits.length ? p : hits.length ? 0 : -1));
  }, [hits.length]);

  const byLine = useMemo(() => {
    const map = new Map<number, FindHit[]>();
    for (const hit of hits) {
      const line = map.get(hit.line);
      if (line) line.push(hit);
      else map.set(hit.line, [hit]);
    }
    return map;
  }, [hits]);
  const hitsRef = useRef(hits);
  hitsRef.current = hits;
  const current = position >= 0 ? hits[position] : undefined;

  const show = useCallback(() => setOpen(true), []);
  const close = useCallback(() => setOpen(false), []);
  const move = useCallback((delta: number) => {
    setPosition((p) => {
      const count = hitsRef.current.length;
      if (!count) return -1;
      return ((p < 0 ? 0 : p) + delta + count) % count;
    });
  }, []);
  const segments = useCallback((text: string, line: number) => {
    const spans = byLine.get(line);
    return spans ? highlightSegments(text, spans, current) : null;
  }, [byLine, current]);

  // Keep the cursor's hit in view, and keep it there as the window re-measures.
  useEffect(() => {
    if (open && current) scrollTo(current.line);
  }, [open, current, scrollTo]);

  return { open, query, hits, current, position, truncated, setQuery, show, close, move, segments };
}

/** The bar itself; the caller decides where it sits and when Ctrl-F shows it. */
export function FindBar({ find }: { find: FindApi }) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!find.open) return;
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [find.open]);
  if (!find.open) return null;
  const count = !find.query ? "" : !find.hits.length ? tr("find.none") : `${Math.max(1, find.position + 1)}/${find.hits.length}${find.truncated ? "+" : ""}`;
  return (
    <div className={s.bar} onKeyDown={(e) => e.stopPropagation()}>
      <input
        ref={inputRef} type="search" value={find.query} spellCheck={false} autoComplete="off"
        placeholder={tr("find.placeholder")} aria-label={tr("find.placeholder")}
        onChange={(e) => find.setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") { e.preventDefault(); find.close(); }
          else if (e.key === "Enter" || e.key === "F3") { e.preventDefault(); find.move(e.shiftKey ? -1 : 1); }
        }}
      />
      <span className={s.count} aria-live="polite">{count}</span>
      <button type="button" onClick={() => find.move(-1)} disabled={!find.hits.length} aria-label={tr("find.prev")} title={tr("find.prev")}>↑</button>
      <button type="button" onClick={() => find.move(1)} disabled={!find.hits.length} aria-label={tr("find.next")} title={tr("find.next")}>↓</button>
      <button type="button" onClick={find.close} aria-label={tr("find.close")} title={tr("find.close")}>×</button>
    </div>
  );
}

/** Render `text` with its hits marked; plain text when none. */
export function findSegments(find: FindApi, text: string, line: number): ReactNode {
  if (!find.open) return text;
  const segments = find.segments(text, line);
  if (!segments) return text;
  return segments.map((segment, index) => (
    segment.hit
      ? <mark key={index} className={segment.current ? s.hitCurrent : s.hit}>{segment.text}</mark>
      : segment.text
  ));
}
