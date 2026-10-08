// Keep landing decisions open, then finish a quiet, settled human turn.
// Engine capability flags also cover card-driven movement and [Stay].
import type { MatchState } from "../../core/types";

interface TurnView {
  S: Pick<MatchState, "phase" | "step" | "busy" | "roller" | "canRollHere" | "canEndHere">;
  playerId: number;
  myTurn: boolean;
  asking: boolean;
  out: boolean;
  overHand: boolean;
}

interface Controls {
  auto: boolean;
  animating: boolean;
  readOnly: boolean;
  connected: boolean;
}

function quiet(m: TurnView, c: Controls): boolean {
  return m.S.phase === "play" && !m.out && !m.S.busy && !m.asking
    && !c.auto && !c.animating && !c.readOnly && c.connected;
}

/** One small button: roll, or skip movement after playing OPS cards on [Stay]. */
export function movementControl(m: TurnView, c: Controls): "roll" | "end" | null {
  if (!quiet(m, c) || m.S.step !== 2) return null;
  if (m.myTurn && m.S.canEndHere && !m.overHand) return "end";
  // canRollHere belongs to the current turn's seat. A delegated roller uses
  // `roller` instead, since it is rolling on behalf of another player.
  if (m.S.roller === m.playerId && (!m.myTurn || m.S.canRollHere)) return "roll";
  return null;
}

export function shouldFinishTurn(m: TurnView, c: Controls & { modalOpen: boolean }): boolean {
  return quiet(m, c) && m.myTurn && m.S.step === 4 && m.S.canEndHere
    && !m.overHand && !c.modalOpen;
}
