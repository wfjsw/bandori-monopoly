// The board's one input lock: who may send a match command.
//
// While 托管 / 混沌 is on the seat belongs to the autopilot, so ordinary play
// is locked (the buttons that would send one are disabled too). `overAuto` is
// the exception for the leave dialog's settle / forfeit exits: the player must
// always be able to walk away from a running auto seat. A read-only seat
// (replay, spectator) never sends.
export function canSend(sess: { readOnly: boolean; autoMode: string }, overAuto = false): boolean {
  if (sess.readOnly) return false;
  return overAuto || sess.autoMode === "off";
}