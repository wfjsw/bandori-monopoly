// NTP-style clock sync onto the server's monotonic clock (docs/SERVER.md
// §Turn clock). All local measurements use `performance.now()` only -- never
// `Date.now()` -- so a user changing the OS clock cannot disturb the offset.
// The server stamps T2 at receive and T3 just before send on the same
// monotonic clock the turn deadlines are expressed in.
//
// Pure (no React): the live UI's second-boundary repaint hook lives in
// `../hooks/timers.ts` (`useSecondBoundary`).

/** One raw ping sample (all times in ms). */
export interface ClockSample {
  /** Client send (`performance.now()`). */
  t1: number;
  /** Server receive (monotonic server ms). */
  t2: number;
  /** Server send (monotonic server ms). */
  t3: number;
  /** Client receive (`performance.now()`). */
  t4: number;
}

/** Cristian/NTP offset (server minus client) and the round-trip delay. */
export function estimateOffset(s: ClockSample): { offset: number; rtt: number } {
  const rtt = s.t4 - s.t1 - (s.t3 - s.t2);
  const offset = (s.t2 - s.t1 + (s.t3 - s.t4)) / 2;
  return { offset, rtt };
}

/**
 * Offset estimator: keeps the lowest-RTT sample, replaces it only when a
 * better (or sufficiently stale-refresh) sample arrives, and slews the
 * displayed offset toward the target so the countdown never visibly runs
 * backwards.
 */
export class ClockOffset {
  private target = 0;
  private shown = 0;
  private bestRtt = Infinity;
  private sampled = false;

  /** Feed one ping. Returns true when this sample became the estimate. */
  push(s: ClockSample): boolean {
    const { offset, rtt } = estimateOffset(s);
    // A sane sample: finite, non-negative delay, and the server cannot send
    // before it received (T3 >= T2).
    if (rtt < 0 || !Number.isFinite(rtt) || !Number.isFinite(offset) || s.t3 < s.t2) return false;
    // Keep the lowest-RTT sample. A later sample only wins when its RTT is
    // better, or when we have no estimate yet.
    if (!this.sampled || rtt < this.bestRtt) {
      this.target = offset;
      this.bestRtt = rtt;
      this.sampled = true;
      return true;
    }
    return false;
  }

  /** Force-accept a sample (reconnect / stale refresh), still preferring RTT. */
  refresh(s: ClockSample): void {
    const { offset, rtt } = estimateOffset(s);
    if (rtt < 0 || !Number.isFinite(rtt) || !Number.isFinite(offset) || s.t3 < s.t2) return;
    if (!this.sampled || rtt <= this.bestRtt * 1.25) {
      this.target = offset;
      this.bestRtt = Math.min(this.bestRtt, rtt);
      this.sampled = true;
    }
  }

  /** Slew `shown` toward `target` (call once per frame / tick). */
  step(maxDeltaMs = 2): number {
    const d = this.target - this.shown;
    if (Math.abs(d) <= maxDeltaMs) this.shown = this.target;
    else this.shown += Math.sign(d) * maxDeltaMs;
    return this.shown;
  }

  /** Current displayed offset (server minus local `performance.now()`). */
  get offset(): number {
    return this.shown;
  }

  /** The raw target estimate (tests). */
  get targetOffset(): number {
    return this.target;
  }

  get hasSample(): boolean {
    return this.sampled;
  }

  get bestRoundTrip(): number {
    return this.bestRtt;
  }

  /** Map a local `performance.now()` stamp onto the server clock. */
  toServer(localMs: number): number {
    return localMs + this.shown;
  }

  /** Remaining ms until a server-clock deadline. */
  remainingMs(deadlineMs: number, localNow: number): number {
    return deadlineMs - this.toServer(localNow);
  }
}

/** Shared estimator for the live UI. */
export const clockSync = new ClockOffset();

/** How often the background refresh re-pings (ms). */
export const REFRESH_MS = 45_000;
/** Samples in the connect burst. */
export const BURST = 8;
/** Gap between burst samples (ms). */
export const BURST_GAP_MS = 30;

/**
 * Run one ping through the shared estimator. `ping` is the transport
 * (`POST /api/clock`); `now` is `performance.now()`.
 */
export async function pingClock(
  ping: (t1: number) => Promise<{ t1: number; t2: number; t3: number } | null>,
  now: () => number = () => performance.now(),
): Promise<boolean> {
  const t1 = now();
  const pong = await ping(t1);
  const t4 = now();
  if (!pong) return false;
  return clockSync.push({ t1, t2: pong.t2, t3: pong.t3, t4 });
}

/**
 * Connect burst + periodic refresh. Returns a stop function. Re-sync is the
 * caller's job on `visibilitychange` / reconnect (call `resync()`).
 */
export function startClockSync(
  ping: (t1: number) => Promise<{ t1: number; t2: number; t3: number } | null>,
  now: () => number = () => performance.now(),
): () => void {
  let stopped = false;
  let timer = 0;

  const burst = async () => {
    for (let i = 0; i < BURST && !stopped; i++) {
      await pingClock(ping, now);
      if (i + 1 < BURST) await sleep(BURST_GAP_MS);
    }
  };
  const background = async () => {
    // A few samples each refresh; `push` keeps the lowest RTT.
    for (let i = 0; i < 3 && !stopped; i++) {
      await pingClock(ping, now);
      if (i + 1 < 3) await sleep(BURST_GAP_MS);
    }
    if (!stopped) timer = window.setTimeout(background, REFRESH_MS);
  };

  void burst().then(() => {
    if (!stopped) timer = window.setTimeout(background, REFRESH_MS);
  });

  return () => {
    stopped = true;
    clearTimeout(timer);
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
