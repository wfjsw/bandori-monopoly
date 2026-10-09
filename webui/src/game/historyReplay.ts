// Linking a match-history row to a stored `.bdrec`. A row written while the
// local replay store was in play carries the store id (`MatchRecord.replayId`,
// filled by `profile_apply_match`); older rows only carry what the match itself
// knows -- finish time, mode, character, rank -- so the link for those is a
// best-effort scan of the store. Nothing here talks to IndexedDB: callers pass
// the `listReplays()` headers in, which keeps the rules unit-testable.

import type { ReplayEntry } from "./record";

/** The history-row fields the link needs (`MatchRecord` minus the rewards). */
export interface HistoryReplayRow {
  /** `yyyy-MM-dd HH:mm`, local wall clock (the profile's `now()` stamp). */
  time: string;
  mode: number;
  rank: number;
  character: string;
  /** Local replay-store id, empty/absent on rows written before ids existed. */
  replayId?: string;
}

/** How a history row resolves against the local replay store. */
export type HistoryReplayLink =
  /** The row's own id, and the store still holds that record. */
  | { kind: "linked"; id: string }
  /** The row's own id, but the record was evicted (the store keeps 10). */
  | { kind: "gone"; id: string }
  /** No id on the row; a stored record matches it by time + character. */
  | { kind: "matched"; id: string }
  /** Nothing to play: no id, and no stored record looks like this match. */
  | { kind: "none" };

/**
 * Minutes-since-epoch of a local wall-clock stamp, or null when it does not
 * parse. Used only for differences, so the absolute value is arbitrary --
 * parts are read as UTC so the number is independent of the machine's zone.
 */
export function wallMinutes(stamp: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(stamp);
  if (!m) return null;
  return Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]) / 60_000;
}

/**
 * Wall-clock minutes of a record's `created`. The engine stamps ISO UTC
 * (`new Date().toISOString()`); older rows and test fixtures use the profile's
 * local `yyyy-MM-dd HH:mm` form. A local form is taken as-is; an ISO instant is
 * converted to the machine's wall clock first, which is what the history row
 * was stamped with.
 */
export function recordMinutes(created: string): number | null {
  if (/(Z|[+-]\d{2}:?\d{2})$/.test(created)) {
    const d = new Date(created);
    if (Number.isNaN(d.getTime())) return null;
    const p = (n: number) => String(n).padStart(2, "0");
    return wallMinutes(
      `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`,
    );
  }
  return wallMinutes(created);
}

/**
 * Resolve one history row against the store's headers (newest first).
 *
 * The row's own id is authoritative. Without one (rows from before ids, or an
 * online match whose record was never kept locally), a record matches when the
 * player's seat agrees on character and final rank, the mode is the same, and
 * the finish stamps sit within a minute of each other -- the export and the
 * history write can straddle a minute boundary. Closest finish wins.
 */
export function resolveHistoryReplay(
  row: HistoryReplayRow,
  entries: readonly ReplayEntry[],
): HistoryReplayLink {
  const id = row.replayId ?? "";
  if (id) {
    return entries.some((e) => e.id === id) ? { kind: "linked", id } : { kind: "gone", id };
  }
  const want = wallMinutes(row.time);
  let best: { id: string; dt: number; savedAt: number } | null = null;
  for (const e of entries) {
    const h = e.header;
    if (h.mode !== row.mode) continue;
    const mine = h.seats.find((x) => !x.bot);
    if (!mine || mine.character !== row.character || mine.rank !== row.rank) continue;
    const at = recordMinutes(h.created);
    if (want == null || at == null) continue;
    const dt = Math.abs(at - want);
    if (dt > 1) continue;
    if (!best || dt < best.dt || (dt === best.dt && e.savedAt > best.savedAt)) {
      best = { id: e.id, dt, savedAt: e.savedAt };
    }
  }
  return best ? { kind: "matched", id: best.id } : { kind: "none" };
}