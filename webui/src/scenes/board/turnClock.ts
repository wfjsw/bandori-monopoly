// The acting seat's turn clock, as the board draws it: the ring on the seat's
// avatar (`Players.module.css` `.clock`).
//
// Online, the server publishes absolute deadlines on its monotonic clock
// (`docs/SERVER.md` §Turn clock); remaining is
// `deadline - (performance.now() + offset)` -- never counted from the view's
// arrival -- so animation lag cannot eat the display. While the clock is held
// (`clockPaused` / no deadline) the display sits on the engine's frozen pool:
// `bonusDeadlineMs` is when the bonus pool empties (the sooner of the two),
// `baseDeadlineMs` is when the whole clock is gone.
//
// The board also passes `held` while it is still presenting a commit: the
// shown state lags the paused commit, so its `clock` deadlines are stale and
// must not count down.
//
// **Within a turn the countdown only ever moves down.** Every published frame
// re-stamps the deadline from `now + pool`, and the board draws the newest
// frame while the ring still sits on the presented seat, so two things would
// otherwise restart the count: the next seat's refill landing on the old
// seat's ring, and a pool that is re-read as the ring's 100% on every frame.
// The hold below fixes both: the turn's biggest pool is the ring's full mark,
// and a value only exceeds the shown one when that pool actually grew --
// which is what an effect granting time does. `busy` / a viewer prompt on the
// no-deadline path freeze the shown value instead of restarting from the
// pool.
//
// Pure (no i18n runtime, no CSS module), so it runs under `node --test`.

import type { MatchState, MatchView } from "../../core/types";
import { clockSync, displayRemainingMs } from "../../game/clockSync.ts";

export interface TurnClock {
  /** Seconds left, unrounded. */
  secs: number;
  /** i18n key of the pool the ring shows (`board.bonusTime` / `board.baseTime`). */
  captionKey: string;
  /** Ring fill, 0..1, of this turn's biggest pool of that kind. */
  frac: number;
  /** CSS tone (`Players.module.css`): `low` a draining / spent clock, `base`
   *  the 恢复时间 pool, `` the ordinary 免费时间 ring. */
  tone: "low" | "base" | "";
}

/** What the display has to remember across renders (one per mounted board). */
export interface TurnClockHold {
  /** Turn identity the maxima and `shown` belong to (round + seat). */
  key: number;
  /** Biggest bonus / base pool seen this turn: the ring's 100%. */
  maxBonus: number;
  maxBase: number;
  /** The value on screen. */
  shown: TurnClock | null;
}

export function freshHold(): TurnClockHold {
  return { key: -1, maxBonus: 0, maxBase: 0, shown: null };
}

/** The printed whole second (`ceil`, never negative). */
export function clockText(t: TurnClock): string {
  return String(Math.max(0, Math.ceil(t.secs - 1e-9)));
}

export interface TurnClockInput {
  /** The state the board draws (its `turn` is the presented seat). */
  state: MatchState;
  /** The newest frame's published deadlines, while the clock runs. */
  clock: MatchView["clock"];
  /** The viewer's own prompt is open. */
  asking: boolean;
  solo: boolean;
  /** Seconds since the drawn view arrived (used only by the no-deadline path). */
  elapsed: number;
  /** The board is still presenting a commit. */
  held: boolean;
  /** The newest frame; may be several commits ahead of `state`. */
  frozen: MatchState | null;
  /** Cross-render memory, mutated. */
  hold: TurnClockHold;
}

/**
 * The visible clock, or `null` for a seat with none (solo, machine seat,
 * outside the play phase).
 */
export function turnClock(o: TurnClockInput): TurnClock | null {
  const S = o.state;
  const cur = S.players[S.turn];
  if (o.solo || S.phase !== "play" || S.turn < 0 || !cur || cur.ai) return null;
  const h = o.hold;
  // `round` only moves on the seat wrap, so key + seat names the turn.
  const key = S.round * 16 + S.turn;
  if (h.key !== key) {
    h.key = key;
    h.maxBonus = 0;
    h.maxBase = 0;
    h.shown = null;
  }
  // A frame ahead of the drawn seat already carries the next seat's pools
  // (the refill happens before the hold is armed). The ring only moves on the
  // new-turn mark, so keep the shown value until the drawn seat catches up.
  if (o.frozen && o.frozen.turn !== S.turn && h.shown) return h.shown;

  /** Track the turn's biggest pool of a kind and hand back the ring's 100%. */
  const gauge = (bonus: boolean, pool: number) => {
    const max = bonus ? h.maxBonus : h.maxBase;
    const grew = pool > max;
    if (grew) {
      if (bonus) h.maxBonus = pool;
      else h.maxBase = pool;
    }
    return { max: Math.max(1, grew ? pool : max || (bonus ? 30 : 15)), grew };
  };
  const put = (secs: number, captionKey: string, tone: TurnClock["tone"], g: { max: number; grew: boolean }) => {
    // A refill (effect / hook) is the one thing allowed to raise the count.
    // Only against the *same* pool, too: handing the ring from an emptied
    // bonus to the base (or back off 0) is not a reset, so it is not capped.
    const keep = h.shown && h.shown.captionKey === captionKey && !g.grew;
    const s = Math.max(0, keep ? Math.min(secs, h.shown!.secs) : secs);
    const t: TurnClock = { secs: s, captionKey, tone, frac: Math.min(1, s / g.max) };
    h.shown = t;
    return t;
  };

  const clock = o.clock;
  if (clock && !o.held) {
    clockSync.step();
    const now = performance.now();
    // Expiry epsilon: the display never races ahead of the server (which is
    // the sole authority for actually expiring a turn).
    const baseMs = displayRemainingMs(clock.baseDeadlineMs, now, clockSync.offset);
    const bonusMs = displayRemainingMs(clock.bonusDeadlineMs, now, clockSync.offset);
    const rawBase = clock.baseDeadlineMs - clockSync.toServer(now);
    const rawBonus = clock.bonusDeadlineMs - clockSync.toServer(now);
    // Bonus drains first: show 免费时间 while the bonus pool has anything left,
    // then 恢复时间 for the base pool underneath it.
    const bonusG = gauge(true, S.bonusTime);
    if (rawBonus > 0 && bonusMs > 0) return put(bonusMs / 1000, "board.bonusTime", bonusMs < 10_000 ? "low" : "", bonusG);
    if (rawBase > 0 && baseMs > 0) return put(baseMs / 1000, "board.baseTime", "base", gauge(false, S.baseTime));
    // At / past the local estimate of the deadline: hold at 0 and say so --
    // the server may not have expired the turn yet.
    return put(0, "board.waitingServer", "low", bonusG);
  }

  // Held (paused / still presenting) or a path with no deadline (replay /
  // older server): show the remaining, frozen while busy/asking. The held
  // branch reads the **newest** state (`frozen`) -- the shown state lags the
  // paused commit, so its pools can predate a refill / drain. With no newer
  // frame the pools are the values at the commit either way, and no drain
  // happens until the gate opens, so this never walks the clock backwards.
  if (o.held || S.clockPaused) {
    const F = (o.held && o.frozen) || S;
    if (F.bonusTime > 0) return put(F.bonusTime, "board.bonusTime", "low", gauge(true, F.bonusTime));
    const g = gauge(false, F.baseTime);
    return put(F.baseTime || g.max, "board.baseTime", "base", g);
  }
  // No deadline on this path either: a settle (`busy`) or an open prompt used
  // to restart the interpolation at the pool. Freeze instead.
  if (o.asking || S.busy) {
    if (h.shown) return h.shown;
    const g = S.bonusTime > 0 ? gauge(true, S.bonusTime) : gauge(false, S.baseTime);
    return put(S.bonusTime > 0 ? S.bonusTime : S.baseTime, S.bonusTime > 0 ? "board.bonusTime" : "board.baseTime", S.bonusTime > 0 ? "low" : "base", g);
  }
  const e = o.elapsed;
  const bonus = Math.max(0, S.bonusTime - e);
  const base = Math.max(0, S.baseTime - Math.max(0, e - S.bonusTime));
  if (bonus > 0) return put(bonus, "board.bonusTime", bonus < 10 ? "low" : "", gauge(true, S.bonusTime));
  return put(base, "board.baseTime", "base", gauge(false, S.baseTime));
}
