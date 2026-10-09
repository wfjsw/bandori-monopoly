// Driving one seat with the advanced (ISMCTS) bot (`docs/BOT.md` B6).
//
// Shared by the solo advanced-bot seats (`SoloSession`) and the 进阶 托管
// policy (`Autopilot`): ask the worker pool for ONE seat's view, apply the
// answer through the ordinary `act` path, fall back to the heuristic on
// error / timeout so a match never stalls (§1). Recordings stay exact because
// the answers are what get recorded.
//
// Information boundary (§1): the view handed to the pool is the same frame the
// page renders for that seat -- `SoloMatch::view(member)` / the server's
// `match` frame. Never the World.

import type { Command, MatchView } from "../core/types.ts";
import type { Msg } from "../i18n/msg.ts";
import { plan, type AutopilotCtx } from "./autopilot.ts";
import {
  decideBudgetMs,
  decisionAt,
  decisionSeed,
  nextTurnNear,
  outerDeadlineMs,
  ponderBudgetMs,
  type BudgetInput,
} from "./botBudget.ts";
import { ensureBotPool, type BotPool } from "./botPool.ts";

/** What one ask came back as. `"fallback"` = the pool failed and the existing
 *  bot policy answered instead -- logged, never a stall. */
export type DriveOutcome = "searched" | "reused" | "heuristic" | "fallback";

export interface DriveHooks {
  /** In-memory inputs the fallback policy needs (tiles / characters / ...). */
  ctx: AutopilotCtx;
  /** Room id folded into the search seed (`"solo"` for a solo match). */
  room: string;
  /** Solo = no deadline: use the configured cap. Online = the room clock. */
  timed: boolean;
  /** Solo per-decision search cap, ms. */
  soloCapMs?: number;
  /** How many decisions this seat has already spent this turn. */
  decisionsThisTurn?: number;
  /** Pool override for tests. */
  pool?: BotPool;
  /** Notified when a search starts / stops (the "thinking…" indicator). */
  onThinking?: (member: number, on: boolean) => void;
  /** Decision counter, for tests / logs. */
  onDone?: (member: number, outcome: DriveOutcome, iterations: number) => void;
}

/** `server/src/botsvc.rs::decision_at`, already ported in `botBudget`. */
export { decisionAt };

/**
 * Ask the pool for `member`'s answer at the current view and apply it.
 * Resolves to the outcome; never throws -- a failed search falls back to the
 * existing bot policy (`plan(v, ctx, "bot")` / the view's `aiAnswer`).
 *
 * Returns `"heuristic"` without touching the pool when the surface is trivial
 * (roll / end / discard) -- the worker would answer it in ~1 ms anyway, but
 * skipping the round-trip keeps the UI snappy.
 */
export async function driveSeat(
  act: (member: number, cmd: Command) => Promise<Msg | null>,
  getMemberView: (member: number) => MatchView | null,
  member: number,
  hooks: DriveHooks,
): Promise<DriveOutcome> {
  const v = getMemberView(member);
  if (!v) return "fallback";
  const promptId = decisionAt(v.state, v.playerId);
  if (promptId == null) return "fallback";

  const budgetInput: BudgetInput = {
    timed: hooks.timed,
    promptTimeLeft: v.state.prompt?.id > 0 ? v.state.prompt.timeLeft : null,
    turnTimeLeft: v.state.timeLeft,
    decisionsThisTurn: hooks.decisionsThisTurn ?? 0,
    soloCapMs: hooks.soloCapMs,
  };
  const budgetMs = decideBudgetMs(budgetInput);
  const seed = decisionSeed(hooks.room, member, v.state.seq, promptId);

  // Fallback first, so a failed search still has a legal answer in hand.
  const fallback = firstPlan(v, hooks.ctx);

  const pool = hooks.pool ?? safePool();
  if (!pool) {
    if (fallback) await act(member, fallback);
    hooks.onDone?.(member, "fallback", 0);
    return "fallback";
  }

  hooks.onThinking?.(member, true);
  try {
    // The pool's own outer deadline is `outerDeadlineMs(budgetMs)`; wrap with
    // a slightly wider race so a wedged worker cannot park the turn either.
    const result = await withTimeout(
      pool.decide(v, budgetMs, seed),
      outerDeadlineMs(budgetMs) + 250,
    );
    if (result.heuristic && !result.answer) {
      if (fallback) await act(member, fallback);
      hooks.onDone?.(member, "heuristic", 0);
      return "heuristic";
    }
    const err = await act(member, result.answer);
    if (err) {
      // The engine refused the search's answer (a stale view, an already-taken
      // prompt). Never re-ask: drop the cached answer so it cannot be replayed,
      // and fall back to the policy's candidate for this decision right now
      // (`docs/BOT.md` §5 B6).
      console.warn(`bot ${member}: search answer refused`, result.answer, err);
      try {
        await pool.invalidate(result.decisionKey ?? "");
      } catch {
        /* best-effort */
      }
      if (fallback) await act(member, fallback);
      hooks.onDone?.(member, "fallback", result.iterations);
      return "fallback";
    }
    hooks.onDone?.(member, result.reused ? "reused" : "searched", result.iterations);
    return result.reused ? "reused" : "searched";
  } catch (e) {
    console.warn(`bot ${member}: search failed -- falling back`, e);
    if (fallback) await act(member, fallback);
    hooks.onDone?.(member, "fallback", 0);
    return "fallback";
  } finally {
    hooks.onThinking?.(member, false);
  }
}

/** The policy's first candidate (the existing bot policy's answer). */
function firstPlan(v: MatchView, ctx: AutopilotCtx): Command | null {
  try {
    return plan(v, ctx, "bot")[0] ?? null;
  } catch {
    return null;
  }
}

/**
 * Speculative search on the seat's **next own decision** while another seat
 * acts (`docs/BOT.md` §3.5 / §3.6, BOT-RESEARCH #5). Fire-and-forget: the
 * caller never waits, and the worker's ponder cache answers a later `decide`
 * whose decision key matches.
 *
 * Cheap no-op when the next decision is not near (the pre-C1 idle polls
 * searched 0 of ~14 k ponders -- an idle view has no searchable surface). The
 * worker predicts the turn-start 运营 view from the idle frame
 * (`bot_core::predict_upcoming_view`); here we only gate the call so the page
 * does not pay a worker round-trip for a dead surface.
 */
export function ponderUpcoming(
  getMemberView: (member: number) => MatchView | null,
  member: number,
  hooks: Pick<DriveHooks, "room" | "timed" | "soloCapMs" | "pool">,
): void {
  const v = getMemberView(member);
  if (!v) return;
  if (decisionAt(v.state, v.playerId) != null) return; // a live decision -- not a ponder
  if (!nextTurnNear(v.state, v.playerId)) return;
  const pool = hooks.pool ?? safePool();
  if (!pool) return;
  const budget = ponderBudgetMs({
    timed: hooks.timed,
    soloCapMs: hooks.soloCapMs,
  });
  const seed = decisionSeed(hooks.room, member, v.state.seq, -1);
  void pool.ponder(v, budget, seed).catch(() => undefined);
}

function safePool(): BotPool | null {
  try {
    return ensureBotPool();
  } catch (e) {
    console.warn("bot pool unavailable:", e);
    return null;
  }
}

function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`bot ask timed out after ${ms} ms`)), ms);
    p.then(
      (v) => {
        clearTimeout(t);
        resolve(v);
      },
      (e) => {
        clearTimeout(t);
        reject(e);
      },
    );
  });
}

// ---------------------------------------------------------------- seat set

/**
 * Which seats an advanced driver must move: every `bot` seat whose
 * `mentality` is `"advanced"` and that the engine does **not** drive (`ai` is
 * off -- `Match::new` holds Advanced seats; `docs/BOT.md` B5/B6). A seat the
 * engine took over (`ai` flipped on after a leave) is dropped.
 */
export function advancedSeats(v: MatchView): number[] {
  const out: number[] = [];
  for (const p of v.state.players) {
    if (p.bot && p.mentality === "advanced" && !p.ai && !p.bankrupt && !p.left) {
      out.push(p.member);
    }
  }
  return out;
}