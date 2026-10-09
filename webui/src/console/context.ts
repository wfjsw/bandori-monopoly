// Keep session binding separate from command/UI modules so editing commands
// during development does not lose the live match.
import type { GameSession } from "../game/session";
import { namesOf } from "../core/names";
import { fmtMsg } from "../i18n/msg";
import { appendLog } from "./log";

let active: GameSession | null = null;
export const consoleSession = () => active;

/** Scope commands to the match actually on screen; unmount removes access. */
export function bindConsoleSession(sess: GameSession): () => void {
  active = sess;
  const off = sess.subscribe(() => {}, (e) => {
    appendLog("info", "game", `#${e.id} ${e.type}`, fmtMsg(e.msg, namesOf(sess.view?.state)));
  });
  return () => {
    off();
    if (active === sess) active = null;
  };
}
