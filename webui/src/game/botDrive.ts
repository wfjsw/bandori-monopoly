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
import { plan, richPassRewardCard, type AutopilotCtx } from "./autopilot.ts";
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
  /** Search (`advanced`) or answer with the policy alone (standard / chaos,
   *  `docs/BOT-DRIVE.md`). Default: search. */
  search?: boolean;
  /** Mentality to decide under. The policy reads it off the frame, so a
   *  human 混沌 seat's frame is rewritten before it is sent; a bot seat
   *  already carries its own. */
  mentality?: "standard" | "chaos" | "advanced";
  /** Pool override for tests. */
  pool?: BotPool;
  /** Notified when a search starts / stops (the "thinking…" indicator). */
  onThinking?: (member: number, on: boolean) => void;
  /** Decision counter, for tests / logs. */
  onDone?: (member: number, outcome: DriveOutcome, iterations: number) => void;
  /** A command was applied for `member` against `decision`; `ok` is false when
   *  the engine refused it, so the caller can re-arm a revisitable surface. */
  onApplied?: (member: number, decision: Decision, ok: boolean) => void;
}

/** `server/src/botsvc.rs::decision_at`, already ported in `botBudget`. */
export { decisionAt };

/** A decision a seat owed: its surface and the state version it was read at. */
export interface Decision {
  /** [`decisionKey`] of the frame it was read from. */
  key: number;
  /** `state.seq` of that frame. */
  seq: number;
  /** The 运营 window owes several decisions on one surface (a card, a skill,
   *  the roll, then `end`): a new state there is a new decision. */
  revisitable: boolean;
}

/** One key per (turn, stage, prompt): a prompt id stays below 100 000. */
function keyOf(turn: number, step: number, promptId: number): number {
  return ((turn + 1) * 8 + step) * 100_000 + Math.max(0, promptId);
}

/** The decision surface a frame shows. Coarse on purpose: `seq` also moves for
 *  cosmetic reasons (an end-phase log, a status wearing off) which must not
 *  re-arm a seat that already answered. */
export function decisionKey(v: MatchView): number {
  return keyOf(v.state.turn, v.state.step, v.state.prompt?.id ?? 0);
}

/** The 运营 surface, the only one a seat answers more than once. */
function revisitable(v: MatchView): boolean {
  return !(v.state.prompt?.id > 0) && v.state.step === 2;
}

/** The decision a seat's frame shows right now. */
export function decisionOf(v: MatchView): Decision {
  return { key: decisionKey(v), seq: v.state.seq, revisitable: revisitable(v) };
}

/** Where an applied `end` leaves the match: the turn's 结束 surface. An `end`
 *  at 运营 moves the stage to 结束 on the same turn first, so the frame shows
 *  a *new* surface between the answer and the next turn -- the seat owes
 *  nothing more on this turn, so the answer is recorded against that one. */
function settled(v: MatchView): Decision {
  return { key: keyOf(v.state.turn, 4, 0), seq: v.state.seq, revisitable: false };
}

/** What an applied command is recorded against: an `end` settles the turn's
 *  结束 surface, anything else the decision it answered. */
export function answerFor(v: MatchView, cmd: Command, decision: Decision): Decision {
  return cmd.act === "end" ? settled(v) : decision;
}

/**
 * Should the driver ask this seat again, given the decision each seat has
 * already answered?
 *
 * A turn surface outlives its answer: 结束 holds for a beat after `end` is
 * applied (and the walk animation before that), so the seat's frame still
 * shows the same decision while the turn is already on its way out. Asking
 * again lands after the turn moved on and is refused (`err.not_your_turn`) --
 * once or twice per turn for every bot. A prompt is the same: the answer is
 * already in the prompt's answer list, so a re-ask is `err.answered`. Only the
 * 运营 window (no prompt) re-arms on a new state.
 */
export function needsAnswer(
  answered: ReadonlyMap<number, Decision>,
  member: number,
  v: MatchView,
): boolean {
  const prev = answered.get(member);
  if (!prev || prev.key !== decisionKey(v)) return true;
  return prev.revisitable && prev.seq !== v.state.seq;
}

/** Record an applied answer. A refusal only re-arms a revisitable (运营)
 *  surface, where the next candidate is still legal on the same surface; a
 *  refused turn / prompt answer means the surface is already moving on. */
export function recordAnswer(
  answered: Map<number, Decision>,
  member: number,
  decision: Decision,
  ok: boolean,
): void {
  if (!ok && decision.revisitable) answered.delete(member);
  else answered.set(member, decision);
}

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

  // Every applied command is reported against the decision it was read from,
  // so the caller can keep the seat from being asked the same decision twice.
  const decision = decisionOf(v);
  const apply = async (cmd: Command): Promise<Msg | null> => {
    const err = await act(member, cmd);
    hooks.onApplied?.(member, answerFor(v, cmd, decision), !err);
    return err;
  };

  const search = hooks.search ?? true;
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

  // A rich seat always takes the card [pass-reward], whatever the search
  // would value -- answer it without paying for a search.
  const rich = richPassRewardCard(v, v.state.prompt);
  if (rich) {
    const err = await apply(rich);
    if (!err) {
      hooks.onDone?.(member, "heuristic", 0);
      return "heuristic";
    }
    // Refused on a stale view: let the search try with the frame it has.
  }

  const pool = hooks.pool ?? safePool();
  if (!pool) {
    if (fallback) await apply(fallback);
    hooks.onDone?.(member, "fallback", 0);
    return "fallback";
  }

  hooks.onThinking?.(member, true);
  try {
    const sent = withMentality(v, hooks.mentality);
    if (!search) {
      // Standard / chaos: the policy alone, no search budget, no tree.
      const cmd = await withTimeout(pool.act(sent, seed), 5_250);
      if (!cmd) {
        if (fallback) await apply(fallback);
        hooks.onDone?.(member, "heuristic", 0);
        return "heuristic";
      }
      const err = await apply(cmd);
      if (err) {
        console.warn(`bot ${member}: policy answer refused`, cmd, err);
        if (fallback) await apply(fallback);
        hooks.onDone?.(member, "fallback", 0);
        return "fallback";
      }
      hooks.onDone?.(member, "heuristic", 0);
      return "heuristic";
    }
    // The pool's own outer deadline is `outerDeadlineMs(budgetMs)`; wrap with
    // a slightly wider race so a wedged worker cannot park the turn either.
    const result = await withTimeout(
      pool.decide(sent, budgetMs, seed),
      outerDeadlineMs(budgetMs) + 250,
    );
    if (result.heuristic && !result.answer) {
      if (fallback) await apply(fallback);
      hooks.onDone?.(member, "heuristic", 0);
      return "heuristic";
    }
    const err = await apply(result.answer);
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
      if (fallback) await apply(fallback);
      hooks.onDone?.(member, "fallback", result.iterations);
      return "fallback";
    }
    hooks.onDone?.(member, result.reused ? "reused" : "searched", result.iterations);
    return result.reused ? "reused" : "searched";
  } catch (e) {
    console.warn(`bot ${member}: search failed -- falling back`, e);
    if (fallback) await apply(fallback);
    hooks.onDone?.(member, "fallback", 0);
    return "fallback";
  } finally {
    hooks.onThinking?.(member, false);
  }
}

/** `v` with the viewer's mentality forced to `m` (`chaos` only -- `standard`
 *  and `advanced` already decide alike in the policy). */
function withMentality(v: MatchView, m?: DriveHooks["mentality"]): MatchView {
  if (m !== "chaos") return v;
  const me = v.playerId;
  if (me < 0 || v.state.players[me]?.mentality === "chaos") return v;
  const players = v.state.players.map((p, i) => (i === me ? { ...p, mentality: "chaos" as const } : p));
  return { ...v, state: { ...v.state, players } };
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
 * Which seats the page drives: **every** bot seat (`docs/BOT-DRIVE.md` -- the
 * engine decides for nobody). Every mentality is here: standard / chaos
 * answer through the policy alone, `advanced` searches. A bankrupt / left
 * seat is out and needs nothing.
 */
export function botSeats(v: MatchView): number[] {
  const out: number[] = [];
  for (const p of v.state.players) {
    if (p.bot && !p.bankrupt && !p.left) {
      out.push(p.member);
    }
  }
  return out;
}