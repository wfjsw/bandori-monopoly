// An unbounded in-memory log, independent of React and game initialization.
// The panel windows it at render (`useWindowedList`), so the whole session's
// log exists for search / export without a per-entry or per-text cap.
export type LogLevel = "log" | "info" | "warn" | "error" | "command" | "result";
export type LogSource = "browser" | "game" | "console";
export interface LogEntry { id: number; time: number; level: LogLevel; source: LogSource; text: string }
let entries: LogEntry[] = [];
// `useSyncExternalStore` compares snapshots by identity, so readers get a new
// frozen array per change. Copying on read (not on every append) coalesces a
// burst of logs into one copy.
let snapshot: readonly LogEntry[] = entries;
let stale = false;
let nextId = 0;
const listeners = new Set<() => void>();
let queued = false;
// Logs may originate inside another component's render (e.g. i18n warnings).
// Notify after that render, and coalesce a burst into one console update.
function emit(): void {
  if (queued) return;
  queued = true;
  queueMicrotask(() => {
    queued = false;
    listeners.forEach((cb) => cb());
  });
}

export function formatValue(value: unknown): string {
  try {
    if (typeof value === "string") return value;
    if (value instanceof Error) return value.stack || `${value.name}: ${value.message}`;
    const seen = new WeakSet<object>();
    return JSON.stringify(value, (_key, item: unknown) => {
      if (typeof item === "bigint") return `${item}n`;
      if (typeof item === "object" && item !== null) {
        if (seen.has(item)) return "[Circular]";
        seen.add(item);
      }
      return item;
    }, 2) ?? String(value);
  } catch {
    return "[Unserializable]";
  }
}

export function appendLog(level: LogLevel, source: LogSource, ...values: unknown[]): void {
  const text = values.map(formatValue).join(" ");
  entries.push({ id: ++nextId, time: Date.now(), level, source, text });
  stale = true;
  emit();
}

export function clearLogs(): void {
  entries = [];
  stale = true;
  emit();
}

/** The current log, as a snapshot that only changes when the log does. */
export function getLogs(): readonly LogEntry[] {
  if (stale) {
    snapshot = entries.slice();
    stale = false;
  }
  return snapshot;
}
export function subscribeLogs(cb: () => void): () => void {
  listeners.add(cb);
  return () => { listeners.delete(cb); };
}

let installed = false;
/** Keep the native DevTools output while capturing startup and runtime failures. */
export function installLogCapture(): void {
  if (installed) return;
  installed = true;
  for (const level of ["log", "info", "warn", "error", "debug"] as const) {
    const native = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      native(...args);
      appendLog(level === "debug" ? "log" : level, "browser", ...args);
    };
  }
  window.addEventListener("error", (e) => {
    appendLog("error", "browser", e.error ?? `${e.message} (${e.filename}:${e.lineno}:${e.colno})`);
  });
  window.addEventListener("unhandledrejection", (e) => appendLog("error", "browser", e.reason));
}
