// Budget policy for the advanced bot in the browser (`docs/BOT.md` §3.6,
// user ruling 2026-10-07).
//
//   * **Timed contexts** (online 托管, the room's timer): use the prompt's /
//     turn's remaining time minus a safety margin, spread across the turn's
//     expected decisions -- answer before the deadline, never let the timeout
//     fallback fire.
//   * **Solo** (no deadline): a generous configurable cap (seconds, not the
//     server's 200 ms) so play stays responsive. Default [`SOLO_CAP_MS`].
//
// Trivial prompts (roll / end / discard) never reach here: the worker answers
// them from the heuristic in ~1 ms (`heuristic: true`).

/** Default per-decision search cap when nothing is running the clock (solo). */
export const SOLO_CAP_MS = 3_000;
/** Floor, so "answer now" still gets one ISMCTS iteration. */
export const MIN_BUDGET_MS = 100;
/** Never spend more than this on one browser decision, whatever the clock says.
 *  The server caps at 1 000 ms; the browser ruling allows much more, but a
 *  single decision still has to leave room for the rest of the turn. */
export const MAX_BUDGET_MS = 12_000;
/** Keep this much of the live clock out of the search budget (apply + render +
 *  scheduling). Wider than the server's 200 ms: the worker pool also has to
 *  wake up, post the view, and merge. */
export const SAFETY_MARGIN_MS = 500;
/** How many searched decisions a turn is expected to still hold. A turn is
 *  roughly: card plays in 运营 + buy/build in 结束 + the occasional prompt.
 *  3 is the spread divisor once one decision has already been spent. */
export const EXPECTED_DECISIONS_PER_TURN = 3;
/** Extra wall-clock on top of the search budget for the pool's outer deadline
 *  (`docs/BOT.md` §5 B7): the ISMCTS loop is anytime *between* iterations and
 *  finishes the one it is in; on wasmi one real-ruleset iteration is 0.5–2 s. */
export const ITERATION_OVERRUN_MS = 2_000;

export interface BudgetInput {
  /** True when the match clock is running (online). Solo is untimed. */
  timed: boolean;
  /** The open prompt's clock, in seconds. `null` / absent on the turn surface. */
  promptTimeLeft?: number | null;
  /** The turn clock, in seconds (online). */
  turnTimeLeft?: number | null;
  /** How many decisions this seat has already spent this turn (for spreading). */
  decisionsThisTurn?: number;
  /** Solo cap in ms; ignored when `timed`. Defaults to [`SOLO_CAP_MS`]. */
  soloCapMs?: number;
}

/**
 * Search budget for one decision, in milliseconds.
 *
 * Timed: `min(prompt clock, turn clock) − SAFETY_MARGIN`, then spread over the
 * turn's remaining expected decisions so the first one does not eat the whole
 * clock. Solo: the configured cap. Always clamped to
 * [`MIN_BUDGET_MS`]..[`MAX_BUDGET_MS`].
 */
export function decideBudgetMs(input: BudgetInput): number {
  const cap = input.soloCapMs ?? SOLO_CAP_MS;
  if (!input.timed) {
    return clamp(Math.round(cap), MIN_BUDGET_MS, MAX_BUDGET_MS);
  }
  const promptMs = msOr(input.promptTimeLeft);
  const turnMs = msOr(input.turnTimeLeft);
  // A prompt clock is the real deadline while one is running; otherwise the
  // turn clock. Whichever is tighter binds.
  let live: number;
  if (promptMs != null) {
    live = turnMs != null ? Math.min(promptMs, turnMs) : promptMs;
  } else if (turnMs != null) {
    live = turnMs;
  } else {
    // No clock visible (defensive): fall back to the solo cap rather than 0.
    return clamp(Math.round(cap), MIN_BUDGET_MS, MAX_BUDGET_MS);
  }
  const spent = Math.max(0, Math.floor(input.decisionsThisTurn ?? 0));
  // Spread over the decisions still expected this turn (user ruling: never
  // spend it all on the first one). The clock is live, so a fast search hands
  // the next decision more room.
  const remaining = Math.max(1, EXPECTED_DECISIONS_PER_TURN - Math.min(spent, EXPECTED_DECISIONS_PER_TURN - 1));
  const share = (live - SAFETY_MARGIN_MS) / remaining;
  return clamp(Math.round(share), MIN_BUDGET_MS, MAX_BUDGET_MS);
}

/**
 * Outer deadline for one pool ask: the search budget plus room for one
 * iteration overrun and the merge / apply, capped so a stuck worker cannot
 * park the turn. The **timeout fallback** fires only at this bound
 * (`docs/BOT.md` §1) -- inside it the search is anytime and returns whatever
 * it has.
 */
export function outerDeadlineMs(budgetMs: number): number {
  return Math.min(budgetMs + ITERATION_OVERRUN_MS, MAX_BUDGET_MS + ITERATION_OVERRUN_MS);
}

/**
 * What one seat is being asked right now, from the view frame alone
 * (`server/src/botsvc.rs::decision_at` -- the browser port).
 *
 * `null` = nothing to ask (setup phases are engine-side even for Advanced
 * bots; the seat is mid-routine; or it is not its turn).
 * `Some(promptId)` = an open prompt waiting on the seat; `0` = the turn
 * surface (运营 card plays / 结束 buy-build-end).
 */
export function decisionAt(
  state: { phase: string; busy: boolean; turn: number; step: number; prompt: { id: number; players: number[]; answers: number[] } },
  playerId: number,
): number | null {
  if (state.phase !== "play") return null;
  if (state.prompt.id > 0) {
    const k = state.prompt.players.indexOf(playerId);
    return k >= 0 && (state.prompt.answers[k] ?? -1) < 0 ? state.prompt.id : null;
  }
  if (state.busy || state.turn !== playerId) return null;
  // stage::OPS = 2, stage::END = 4 (game-core/src/state.rs `stage`).
  return state.step === 2 || state.step === 4 ? 0 : null;
}

/**
 * Search-side seed for one decision. Derived from the **public** decision
 * identity (never the match RNG, `docs/BOT.md` §1). Mirrors
 * `server/src/lib.rs::decision_seed`'s shape; the room id is folded in so two
 * solo matches at the same seq do not share a stream.
 */
export function decisionSeed(room: string, member: number, seq: number, promptId: number): number {
  // FNV-1a over the identity, then a splitmix avalanche so nearby seqs do not
  // produce nearby streams. u32: the worker API's seed field is a JS number.
  let h = 0x811c9dc5;
  const eat = (s: string) => {
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 0x01000193);
    }
  };
  eat(room);
  eat(`|${member}|${seq}|${promptId}`);
  return Number(splitmix64(BigInt(h >>> 0)) & 0xffffffffn);
}

/**
 * The per-worker seed, bit-identical to `bot_core::seed_for_thread` (which
 * `bot-glue` re-exports): worker 0 keeps `seed` exactly (N=1 == single search),
 * the others are split-mixed away from it. `botBudget.test.ts` pins the two
 * together.
 */
export function seedForThread(seed: number, thread: number): number {
  if (thread === 0) return seed >>> 0;
  const M = 0xffffffffffffffffn;
  const mix = (0x9e3779b97f4a7c15n * BigInt(thread + 1)) & M;
  const s = splitmix64((BigInt(seed >>> 0) ^ mix) & M);
  return Number(s & 0xffffffffn);
}

function splitmix64(x: bigint): bigint {
  const M = 0xffffffffffffffffn;
  const z0 = (x + 0x9e3779b97f4a7c15n) & M;
  const z1 = ((z0 ^ (z0 >> 30n)) * 0xbf58476d1ce4e5b9n) & M;
  const z2 = ((z1 ^ (z1 >> 27n)) * 0x94d049bb133111ebn) & M;
  return (z2 ^ (z2 >> 31n)) & M;
}

function msOr(sec: number | null | undefined): number | null {
  if (sec == null || !Number.isFinite(sec) || sec < 0) return null;
  return sec * 1000;
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}