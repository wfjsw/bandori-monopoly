// First-crash registry and the debug blob the crash panel copies out.
//
// React 19 has no built-in crash UI: a render error unmounts the tree and the
// user gets a white screen. This store is the safety net's memory -- framework
// free on purpose so it sits beside the other `core/` stores and runs under
// `node --test`. `app/CrashBoundary` and the global handlers feed it; the panel
// reads it.

export type CrashKind = "render" | "error" | "rejection" | "worker";

export interface CrashInfo {
  message: string;
  stack?: string;
  componentStack?: string;
  kind: CrashKind;
  time: number;
  url: string;
  ua: string;
}

/** Log lines the debug blob tails. `console/log.ts`'s `LogEntry` is assignable. */
export interface CrashLogLine {
  time: number;
  level: string;
  text: string;
}

/** How many recent log lines `crashDebugText` tails. */
export const CRASH_LOG_TAIL = 30;

/** Same message inside this window collapses to one report. */
export const CRASH_DEDUPE_MS = 2_000;

// Page-fatal kinds blank the board; the other two often are not (the codebase
// deliberately fires and ignores many promises). Rank lets a real failure take
// over from noise recorded first -- never the other way round.
function rank(kind: CrashKind): number {
  return kind === "render" || kind === "worker" ? 1 : 0;
}

let current: CrashInfo | null = null;
const listeners = new Set<(c: CrashInfo) => void>();

let lastMessage: string | null = null;
let lastMessageAt = 0;

/**
 * True when `message` already arrived inside the dedupe window. Records the
 * message either way, so a burst of the same rejection reports once.
 */
export function isRepeatCrash(message: string, now = Date.now()): boolean {
  if (lastMessage === message && now - lastMessageAt < CRASH_DEDUPE_MS) return true;
  lastMessage = message;
  lastMessageAt = now;
  return false;
}

/** Environment fields of a crash report, filled at the call site that saw it. */
export function crashEnv(): Pick<CrashInfo, "time" | "url" | "ua"> {
  return {
    time: Date.now(),
    // Node (unit tests) has neither; a blank field beats a throw in a crash path.
    url: typeof location !== "undefined" ? location.href : "",
    ua: typeof navigator !== "undefined" ? navigator.userAgent : "",
  };
}

/**
 * Record a crash and notify subscribers. The **first** crash of the page load
 * wins -- it is the root cause and the rest of the reports are cascade -- except
 * that a page-fatal crash may still replace a recorded non-fatal one so a stray
 * rejection cannot bury the real failure. Later crashes never replace the first
 * among equals.
 */
export function reportCrash(
  info: Omit<CrashInfo, "time" | "url" | "ua"> & Partial<Pick<CrashInfo, "time" | "url" | "ua">>,
): void {
  const full: CrashInfo = { ...crashEnv(), ...info };
  if (isRepeatCrash(full.message, full.time)) return;
  if (current && rank(full.kind) <= rank(current.kind)) return;
  current = full;
  for (const cb of listeners) cb(full);
}

/** The first crash of this page load, if any. */
export function getCrash(): CrashInfo | null {
  return current;
}

/** Notified whenever a crash becomes the recorded one. Returns an unsubscriber. */
export function subscribeCrash(cb: (c: CrashInfo) => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

/** Tests only: forget the recorded crash and the dedupe window. */
export function clearCrash(): void {
  current = null;
  lastMessage = null;
  lastMessageAt = 0;
}

function iso(time: number): string {
  return new Date(time).toISOString();
}

/**
 * Plain-text debug blob for a bug report: header, message, stacks, then a tail
 * of recent log lines. Field names stay English -- a paste into a ticket wants
 * stable labels, not UI copy.
 */
export function crashDebugText(info: CrashInfo, logs: readonly CrashLogLine[] = []): string {
  const out: string[] = [];
  out.push("kind: " + info.kind);
  out.push("time: " + iso(info.time));
  out.push("url: " + info.url);
  out.push("ua: " + info.ua);
  out.push("");
  out.push("message: " + info.message);
  if (info.stack) {
    out.push("");
    out.push("stack:");
    out.push(info.stack);
  }
  if (info.componentStack) {
    out.push("");
    out.push("componentStack:");
    out.push(info.componentStack);
  }
  const tail = logs.slice(-CRASH_LOG_TAIL);
  if (tail.length) {
    out.push("");
    out.push("recent logs:");
    for (const line of tail) out.push(`${iso(line.time)} [${line.level}] ${line.text}`);
  }
  return out.join("\n");
}