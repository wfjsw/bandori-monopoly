import { COMMANDS } from "./commands.ts";

type InputKey = Pick<KeyboardEvent, "isComposing" | "ctrlKey" | "metaKey" | "altKey">;
type ShortcutKey = InputKey & Pick<KeyboardEvent, "key" | "code" | "repeat">;

export function acceptsInputKey(event: InputKey): boolean {
  return !event.isComposing && !event.ctrlKey && !event.metaKey && !event.altKey;
}

export function isConsoleShortcut(event: ShortcutKey, open: boolean,
  target: Pick<HTMLElement, "tagName" | "isContentEditable"> | null): boolean {
  if (!acceptsInputKey(event) || event.repeat) return false;
  const editable = target?.isContentEditable || (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
  const toggle = event.code === "Backquote" || ["`", "~", "～", "·"].includes(event.key);
  return Boolean((toggle && (open || !editable)) || (open && event.key === "Escape"));
}

export function completeCommand(input: string): string | null {
  const prefix = input.trim().toLowerCase();
  if (!prefix || /\s/.test(prefix)) return null;
  const matches = COMMANDS.filter((command) => command.startsWith(prefix));
  return matches.length === 1 ? matches[0] + " " : null;
}

export function rememberCommand(history: string[], line: string): void {
  if (history.at(-1) === line) return;
  history.push(line);
  if (history.length > 100) history.shift();
}

export function recallCommand(history: readonly string[], index: number, draft: string,
  input: string, direction: "up" | "down") {
  if (index === history.length) draft = input;
  index = Math.max(0, Math.min(history.length, index + (direction === "up" ? -1 : 1)));
  return { index, draft, input: index === history.length ? draft : history[index] ?? "" };
}
