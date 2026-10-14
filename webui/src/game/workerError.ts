/**
 * Describe a worker `error` event. A worker that fails to load or evaluate
 * (bad URL, wrong MIME type, CSP, failed static import) fires a bare
 * `ErrorEvent` with an empty `message`; `String(e)` of that is
 * `[object Event]`. Keep every field the browser does give us.
 */
export function describeWorkerError(e: unknown, url?: string | URL): string {
  const ev = (e ?? {}) as { message?: string; filename?: string; lineno?: number; colno?: number; type?: string; error?: unknown };
  const parts: string[] = [];
  if (ev.message) parts.push(ev.message);
  else if (ev.error instanceof Error) parts.push(ev.error.message);
  else parts.push("worker failed to load or start (no message from the browser)");
  const at = ev.filename || (url ? String(url) : "");
  if (at) parts.push(`at ${at}${ev.lineno ? `:${ev.lineno}${ev.colno ? `:${ev.colno}` : ""}` : ""}`);
  if (ev.type) parts.push(`[${ev.type}]`);
  return parts.join(" ");
}
