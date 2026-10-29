// Client-side extras for the **online** path (`docs/BOT.md` §1.1, the
// seat-view engine). The server no longer ships `aiAnswer` / `playable` /
// `estCost` / `skills` on a human seat's frame; the page recomputes them from
// the viewer's own `MatchView` through the bot worker's `extras` op
// (`bot-glue`'s `view_extras`: one determinize + the same engine gates the
// server's `Match::view_extra` runs).
//
// Solo keeps the extras the local engine already ships in the frame
// (`web-glue`'s `view_extra`) and does **not** go through here: in solo the
// local engine *is* the authoritative engine, so `view_extra` is exact and
// free (no determinization, no sampling). Online uses the seat-view engine
// because the client must not ask for hidden state -- the extras are sampled
// from one seat's frame and are therefore not identical to solo's, so the two
// paths stay separate.
//
// Information boundary: the only input is the viewer's own frame. `seed` is
// `decisionSeed(room, member, seq, promptId)` -- the determinizer's sampling
// seed, never the match RNG.

import type { MatchView } from "../core/types.ts";
import { decisionSeed } from "./botBudget.ts";
import { ensureBotPool, type BotPool, type ViewExtras } from "./botPool.ts";

export type { ViewExtras };

/** Cache key: `state.seq` + `prompt.id` (a new prompt on the same state is a
 *  new decision and gets its own extras). */
export function extrasKey(v: MatchView): string {
  return `${v.state.seq}:${Math.max(0, v.state.prompt?.id ?? 0)}`;
}

/** Copy `v` with the extras fields overwritten (`{...v}` so React re-renders). */
export function mergeExtras(v: MatchView, e: ViewExtras): MatchView {
  return {
    ...v,
    playable: e.playable,
    estCost: e.estCost,
    skills: e.skills,
    aiAnswer: e.aiAnswer,
  };
}

/**
 * Online extras cache. Keyed by [`extrasKey`]; computed in the bot worker so
 * the main thread never blocks on the determinize pass.
 *
 * Each key caches its own exact extras. On a miss, the most recent set's
 * `skills` carries forward -- a stale-but-present stand-in, so a new state seq
 * does not empty the skill row and unmount its faces for a paint. The decision
 * fields (`playable` / `estCost` / `aiAnswer`) never carry: they describe one
 * prompt, and borrowing another key's would grey a legal card or answer 托管
 * with the wrong choice.
 *
 * Failure is not fatal: a refused / timed-out `extras` leaves the frame
 * carrying whatever stand-in is already there (consumers already tolerate a
 * missing field -- `view.playable && !view.playable[i]`, `view.skills ?? []`,
 * `view.aiAnswer ?? null`) and the key is marked failed so the same decision
 * is not retried.
 */
export class OnlineExtras {
  private cache = new Map<string, ViewExtras>();
  private inflight = new Map<string, Promise<void>>();
  private failed = new Set<string>();
  /** Most recent successful extras: the stand-in for a key still in flight. */
  private latest: ViewExtras | null = null;
  private pool: BotPool | null;
  /** Room id folded into the seed (`"solo"` never reaches here). */
  private room: string;
  /** The viewer's seat, folded into the seed. */
  private member: number;

  /**
   * @param room  Room id folded into the seed (`"solo"` never reaches here).
   * @param member The viewer's seat, folded into the seed.
   * @param pool  Pool override for tests; defaults to [`ensureBotPool`].
   */
  constructor(room: string, member: number, pool?: BotPool | null) {
    this.room = room;
    this.member = member;
    this.pool = pool ?? null;
  }

  /** Merge already-cached extras into `v`. An exact-key hit wins. On a miss,
   *  **only `skills`** carries the most recent set forward: the roster's
   *  pressable skills are stable across a decision, and an absent list empties
   *  the skill row (its faces remount, and their art flashes). The decision
   *  fields (`playable` / `estCost` / `aiAnswer`) are decision-specific and
   *  must never be borrowed from another key -- a stale `playable` greys a
   *  legal card and a stale `aiAnswer` answers 托管 with the previous
   *  decision.
   *  Returns `v` itself when nothing can be merged (or the frame already
   *  carries those fields). */
  mergeCached(v: MatchView): MatchView {
    const exact = this.cache.get(extrasKey(v));
    if (exact) {
      if (
        v.playable === exact.playable &&
        v.estCost === exact.estCost &&
        v.skills === exact.skills &&
        v.aiAnswer === exact.aiAnswer
      ) {
        return v;
      }
      return mergeExtras(v, exact);
    }
    if (!this.latest || v.skills === this.latest.skills) return v;
    return { ...v, skills: this.latest.skills };
  }

  /**
   * Kick off extras for `v` unless the key is cached or previously failed.
   * `onDone` runs with the extras when they land (the caller decides whether
   * the frame is still current). Resolves when that request settles -- a
   * concurrent call for the same key joins the in-flight promise. Never
   * rejects (a failed extras is not fatal).
   */
  request(v: MatchView, onDone: (extras: ViewExtras) => void): Promise<void> {
    const key = extrasKey(v);
    const running = this.inflight.get(key);
    if (running) return running;
    if (this.cache.has(key) || this.failed.has(key)) return Promise.resolve();
    const promptId = Math.max(0, v.state.prompt?.id ?? 0);
    const seed = decisionSeed(this.room, this.member, v.state.seq, promptId);
    const p = this.getPool()
      .then((pool) => pool.extras(v, seed))
      .then((e) => {
        this.cache.set(key, e);
        this.latest = e;
        onDone(e);
      })
      .catch((err) => {
        // Leave the view usable without extras; never retry this key.
        this.failed.add(key);
        console.warn("view extras failed:", err);
      })
      .finally(() => {
        this.inflight.delete(key);
      });
    this.inflight.set(key, p);
    return p;
  }

  /** Test / reset hook: how many keys are cached (successes only). */
  get cachedCount(): number {
    return this.cache.size;
  }

  private async getPool(): Promise<BotPool> {
    if (this.pool) return this.pool;
    this.pool = ensureBotPool();
    return this.pool;
  }
}

/**
 * The online emit cycle (`OnlineSession.emitView`'s body): merge cached
 * extras (exact, else the most recent `skills` as a stale-but-present
 * stand-in), emit, and always kick off the seat-view computation for this key
 * when it is not already cached. `emit` notifies the session's subscribers;
 * `current` returns the live frame so a late extras reply only re-emits for
 * the same decision.
 *
 * Until *any* extras have landed the fields stay absent (consumers already
 * tolerate that); after that they never drop out -- the stand-in is replaced
 * by the exact set as soon as it arrives. A failed / slow `extras` never
 * blocks the frame -- greying may be coarse and 托管 falls back to its policy
 * table.
 */
export function emitOnlineView(
  extras: OnlineExtras,
  v: MatchView,
  emit: (merged: MatchView) => void,
  current: () => MatchView | null,
): Promise<void> {
  // Emit the stand-in (or the exact extras) first. The request below must run
  // even when that stand-in made `merged !== v` -- otherwise a cache miss for
  // this key would never compute the real ones. `request` is a no-op when the
  // key is already cached / failed / in flight.
  emit(extras.mergeCached(v));
  return extras.request(v, (e) => {
    // Only re-emit when this is still the live decision (same seq +
    // prompt); a newer frame starts its own request.
    const cur = current();
    if (!cur || extrasKey(cur) !== extrasKey(v)) return;
    emit(mergeExtras(cur, e));
  });
}