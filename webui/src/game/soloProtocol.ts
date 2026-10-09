// The solo engine worker protocol (`soloEngine.ts` is the page side,
// `soloWorker.ts` the worker side). Shared types and the tick accumulator live
// here so neither end bundles the other and the tests can pin both.
//
// The message contract mirrors `webui/public/assets/engine/replay-worker.js`:
// requests are `{id, op, ...}` and answer `{id, ok, value} | {id, ok:false,
// error}`; the worker also pushes unsolicited `{push, ...}` frames (a dirty
// sync, a save snapshot).

import type { MatchEvent, MatchView, RoomMember, ScoreWeights } from "../core/types.ts";
import type { EngineStamp } from "./record.ts";

/** One recorded tick quantum (`docs/REPLAY.md` §1) -- the worker's step. */
export const TICK_STEP = 0.05;

/**
 * The fixed-step accumulator the worker runs every 50 ms, kept here so the
 * semantics are pinned by tests: whole `TICK_STEP` quanta (`tick_steps(k)`,
 * k in 1..=10), wall time capped at 0.5 s per wake, the remainder carried.
 * Identical to the pre-worker `SoloSession::tick`.
 */
export function tickQuanta(elapsedMs: number, acc: number): { k: number; acc: number } {
  const dt = Math.min(0.5, elapsedMs / 1000);
  const a = acc + dt;
  let k = Math.floor(a / TICK_STEP);
  if (k > 10) k = 10;
  if (k > 0) return { k, acc: a - k * TICK_STEP };
  return { k: 0, acc: a };
}

/** Engine strings for one `SoloSave` (the envelope around them is the session's). */
export interface SaveSnap {
  match: string;
  rec?: string;
  /** The event cursor `events_since` has consumed up to. */
  last: number;
}

/** What the worker pushes without being asked. */
export type SoloPush =
  | {
      /** A pump where something moved. `view` / `bots` arrive only when that
       *  frame's JSON changed -- the cheaper diff on top of `take_changed()`. */
      push: "sync";
      events: MatchEvent[];
      view?: MatchView;
      /** Per-seat frames for the 进阶 seats the page drives (only changed ones). */
      bots?: { member: number; view: MatchView }[];
      ended?: boolean;
    }
  | { push: "save"; match: string; rec?: string; last: number };

/** The slice of `Worker` the client uses (so tests can inject a fake). */
export interface SoloWorkerLike {
  postMessage(msg: unknown, transfer?: Transferable[]): void;
  terminate(): void;
  onmessage: ((e: { data: unknown }) => void) | null;
  onerror: ((e: unknown) => void) | null;
}

export interface WorkerReply {
  id: number;
  ok: boolean;
  value?: unknown;
  error?: string;
}

export interface SoloOpen {
  members: RoomMember[];
  seed: number;
  /** `MatchMode` as the engine's i32 (0 solo). */
  mode: number;
  weights: ScoreWeights;
  /** The human's seat id (the frame the board renders). */
  you: number;
}

export interface SoloRestore {
  save: string;
  rec?: string;
  last: number;
  you: number;
}

export interface EngineBoot {
  stamp: EngineStamp | null;
  cheats: boolean;
}

/** Every request op the worker answers. */
export type SoloOp = "init" | "start" | "restore" | "act" | "quick_start" | "view" | "save" | "export" | "visibility" | "close";