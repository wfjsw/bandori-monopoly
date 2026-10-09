import { type FormEvent, type KeyboardEvent as ReactKeyboardEvent, useRef, useState, useSyncExternalStore } from "react";
import { useLangVersion } from "../core/hooks";
import { useAutoFocus, useEventListener } from "../hooks/dom";
import { useScrollFollow } from "../hooks/measure";
import { t as tr } from "../i18n/t";
import { acceptsInputKey, completeCommand, isConsoleShortcut, recallCommand, rememberCommand } from "./input";
import { getLogs, subscribeLogs, clearLogs, type LogEntry, type LogSource } from "./log";
import { runConsoleCommand } from "./runtime";
import s from "./Console.module.css";

// Session-only history survives scene and language changes; no save mutations.
const history: string[] = [];
// Closed panel must not re-render on log traffic: the snapshot is a stable
// empty list and `useSyncExternalStore` bails out while it is unchanged.
const NO_LINES: readonly LogEntry[] = [];

export function Console() {
  useLangVersion();
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [source, setSource] = useState<LogSource | "all">("all");
  const [errors, setErrors] = useState(false);
  const [search, setSearch] = useState("");
  const [follow, setFollow] = useState(true);
  const lines = useSyncExternalStore(subscribeLogs, () => (open ? getLogs() : NO_LINES));
  const inputRef = useRef<HTMLInputElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);
  const historyIndex = useRef(history.length);
  const draft = useRef("");

  // The console's own shortcut, on the capture phase so it outranks the scene
  // hotkeys behind it.
  useEventListener(window, "keydown", (e) => {
    const ev = e as KeyboardEvent;
    if (isConsoleShortcut(ev, open, ev.target as HTMLElement | null)) {
      ev.preventDefault();
      ev.stopImmediatePropagation();
      setOpen((v) => !v);
    }
  }, { capture: true });

  // The input takes over while the panel is open; focus returns on close.
  useAutoFocus(open, inputRef, () => {
    historyIndex.current = history.length;
  });
  // The output stays pinned to the newest line while following.
  useScrollFollow(outputRef, [open, lines, follow, source, errors, search], open && follow);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const line = input.trim();
    if (!line || busy) return;
    rememberCommand(history, line);
    historyIndex.current = history.length;
    draft.current = "";
    setInput("");
    setBusy(true);
    try { await runConsoleCommand(line); } finally { setBusy(false); }
    inputRef.current?.focus();
  };

  const inputKey = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (!acceptsInputKey(e.nativeEvent)) return;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const recalled = recallCommand(history, historyIndex.current, draft.current, input, e.key === "ArrowUp" ? "up" : "down");
      historyIndex.current = recalled.index;
      draft.current = recalled.draft;
      setInput(recalled.input);
    }
    if (e.key === "Tab") {
      const completed = completeCommand(input);
      if (completed !== null) { e.preventDefault(); setInput(completed); }
    }
  };

  const download = () => {
    const text = lines.map((e) => `${new Date(e.time).toISOString()} [${e.source}/${e.level}] ${e.text}`).join("\n");
    const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `bandori-console-${Date.now()}.log`;
    a.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  const filtered = lines.filter((e) => (source === "all" || e.source === source)
    && (!errors || e.level === "warn" || e.level === "error")
    && (!search || e.text.toLowerCase().includes(search.toLowerCase())));

  if (!open) return null;
  return (
    <section className={s.panel} aria-label={tr("console.title")} role="dialog" onKeyDown={(e) => e.stopPropagation()}>
      <header className={s.header}>
        <strong><span className={s.symbol}>&gt;_</span> {tr("console.title")}</strong>
        <span className={s.hint}>{tr("console.shortcut")}</span>
        <button type="button" onClick={() => setOpen(false)} aria-label={tr("common.close")}>×</button>
      </header>
      <div className={s.toolbar}>
        <select value={source} aria-label={tr("console.source")} onChange={(e) => setSource(e.target.value as typeof source)}>
          <option value="all">{tr("console.all")}</option>
          <option value="console">{tr("console.commands")}</option>
          <option value="browser">{tr("console.browser")}</option>
          <option value="game">{tr("console.game")}</option>
        </select>
        <input type="search" value={search} onChange={(e) => setSearch(e.target.value)} placeholder={tr("console.search")} aria-label={tr("console.search")} />
        <label><input type="checkbox" checked={errors} onChange={(e) => setErrors(e.target.checked)} />{tr("console.errors")}</label>
        <label><input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />{tr("console.follow")}</label>
        <button type="button" onClick={clearLogs}>{tr("console.clear")}</button>
        <button type="button" onClick={download}>{tr("console.export")}</button>
      </div>
      <div className={s.output} ref={outputRef} tabIndex={0} aria-label={tr("console.output")}>
        {filtered.length === 0 && <p className={s.empty}>{tr("console.empty")}</p>}
        {filtered.map((entry) => (
          <div key={entry.id} className={`${s.line} ${s[entry.level]}`}>
            <time>{new Date(entry.time).toLocaleTimeString([], { hour12: false })}</time>
            <span className={s.source}>{entry.source}</span>
            <pre>{entry.text}</pre>
          </div>
        ))}
      </div>
      <form className={s.commandBar} onSubmit={(e) => void submit(e)}>
        <span className={s.symbol}>&gt;</span>
        <input ref={inputRef} value={input} onChange={(e) => { setInput(e.target.value); historyIndex.current = history.length; }} onKeyDown={inputKey}
          spellCheck={false} autoComplete="off" autoCapitalize="off" aria-label={tr("console.input")} placeholder={tr("console.placeholder")} />
        <button type="submit" disabled={busy || !input.trim()}>{busy ? tr("console.running") : tr("console.run")}</button>
      </form>
      <footer className={s.footer}>
        <span>{tr("console.footer")}</span>
        <button type="button" onClick={() => { setInput("help"); inputRef.current?.focus(); }}>help</button>
      </footer>
    </section>
  );
}