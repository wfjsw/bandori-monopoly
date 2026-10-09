import { strict as assert } from "node:assert";
import { test } from "node:test";
import { acceptsInputKey, completeCommand, isConsoleShortcut, recallCommand, rememberCommand } from "./input.ts";

function key(overrides: Partial<KeyboardEvent> = {}) {
  return { key: "~", code: "", repeat: false, isComposing: false,
    ctrlKey: false, metaKey: false, altKey: false, ...overrides };
}

test("backquote, tilde and localized key variants open and close the console", () => {
  for (const event of [key({ code: "Backquote", key: "Unidentified" }),
    ...["`", "~", "～", "·"].map((value) => key({ key: value }))]) {
    assert.equal(isConsoleShortcut(event, false, null), true);
    assert.equal(isConsoleShortcut(event, true, null), true);
  }
  assert.equal(isConsoleShortcut(key({ key: "a" }), false, null), false);
});

test("the shortcut respects editable fields while allowing an open console to close", () => {
  for (const target of [
    ...["INPUT", "TEXTAREA", "SELECT"].map((tagName) => ({ tagName, isContentEditable: false })),
    { tagName: "DIV", isContentEditable: true },
  ]) {
    assert.equal(isConsoleShortcut(key(), false, target), false);
    assert.equal(isConsoleShortcut(key(), true, target), true);
  }
  assert.equal(isConsoleShortcut(key(), false, { tagName: "BUTTON", isContentEditable: false }), true);
});

test("Escape closes the console without opening a closed one", () => {
  const escape = key({ key: "Escape" });
  assert.equal(isConsoleShortcut(escape, true, null), true);
  assert.equal(isConsoleShortcut(escape, false, null), false);
});

test("IME, held shortcut keys and modifier combinations do not toggle the console", () => {
  for (const field of ["isComposing", "repeat", "ctrlKey", "metaKey", "altKey"] as const) {
    for (const open of [false, true]) {
      assert.equal(isConsoleShortcut(key({ [field]: true }), open, null), false, field);
      assert.equal(isConsoleShortcut(key({ key: "Escape", [field]: true }), open, null), false, field);
    }
  }
  assert.equal(isConsoleShortcut(key({ shiftKey: true }), false, null), true);
});

test("history and completion leave IME and modified input keys alone", () => {
  assert.equal(acceptsInputKey(key()), true);
  for (const field of ["isComposing", "ctrlKey", "metaKey", "altKey"] as const) {
    assert.equal(acceptsInputKey(key({ [field]: true })), false, field);
  }
});

test("completion accepts a unique prefix and preserves ambiguous or argument input", () => {
  assert.equal(completeCommand(" MO "), "money ");
  assert.equal(completeCommand("help"), "help ");
  for (const input of ["", "  ", "h", "s", "does-not-exist", "money 5", "give\tc1"]) {
    assert.equal(completeCommand(input), null, input);
  }
});

test("history is bounded and only suppresses consecutive duplicates", () => {
  const history: string[] = [];
  for (const line of ["help", "help", "status", "help"]) rememberCommand(history, line);
  assert.deepEqual(history, ["help", "status", "help"]);
  for (let i = 0; i <= 100; i++) rememberCommand(history, `money ${i}`);
  assert.equal(history.length, 100);
  assert.equal(history[0], "money 1");
  assert.equal(history.at(-1), "money 100");
});

test("history navigation restores the unsent draft and clamps at both ends", () => {
  const history = ["help", "status"];
  let cursor = { index: history.length, draft: "", input: "money 50" };
  const move = (direction: "up" | "down") => {
    cursor = recallCommand(history, cursor.index, cursor.draft, cursor.input, direction);
    return cursor.input;
  };
  assert.equal(move("up"), "status");
  assert.equal(move("up"), "help");
  assert.equal(move("up"), "help");
  assert.equal(move("down"), "status");
  assert.equal(move("down"), "money 50");
  assert.equal(move("down"), "money 50");
  assert.equal(cursor.index, history.length);
});

test("empty history preserves input and editing starts a new recall draft", () => {
  for (const direction of ["up", "down"] as const) {
    assert.deepEqual(recallCommand([], 0, "", "draw 2", direction), { index: 0, draft: "draw 2", input: "draw 2" });
  }
  const history = ["help", "status"];
  const recalled = recallCommand(history, history.length, "old draft", "tp 10", "up");
  assert.equal(recalled.input, "status");
  const restored = recallCommand(history, recalled.index, recalled.draft, recalled.input, "down");
  assert.equal(restored.input, "tp 10");
});
