/**
 * Text a copy made inside the match log puts on the clipboard.
 *
 * A non-collapsed selection copies as selected; `null` (a collapsed caret)
 * falls back to the whole log, because the windowed log keeps only the
 * visible rows in the DOM and a native collapsed copy would take nothing.
 */
export function copiedLogText(fullLog: string, selected: string | null): string {
  return selected ?? fullLog;
}
