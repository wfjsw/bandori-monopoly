import { D, rules } from "../core/data";
import { hasProfile, getProfile } from "../core/store";
import { namesOf } from "../core/names";
import { fmtMsg, type Msg } from "../i18n/msg";
import { t } from "../i18n/t";
import { executeCommand } from "./commands";
import { appendLog, clearLogs } from "./log";
import { consoleSession } from "./context";

export async function runConsoleCommand(line: string): Promise<void> {
  appendLog("command", "console", `> ${line}`);
  try {
    const result = await executeCommand(line, {
      session: consoleSession,
      cards: () => D?.cards ?? [],
      tiles: () => D?.tiles ?? [],
      profile: () => hasProfile() ? getProfile() : null,
      engine: () => JSON.parse(rules.engine_stamp()),
      clear: clearLogs, t,
      errorMessage: (error) => fmtMsg(error as Msg, namesOf(consoleSession()?.view?.state)),
    });
    if (result !== undefined) appendLog("result", "console", result);
  } catch (e) {
    appendLog("error", "console", e instanceof Error ? e.message : e);
  }
}
