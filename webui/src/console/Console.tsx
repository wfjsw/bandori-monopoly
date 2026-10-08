import { type FormEvent, type KeyboardEvent as ReactKeyboardEvent, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useLangVersion } from "../core/hooks";
import { t as tr } from "../i18n/t";
import { COMMANDS } from "./commands";
import { getLogs, subscribeLogs, clearLogs, type LogSource } from "./log";
import { runConsoleCommand } from "./runtime";
import s from "./Console.module.css";

const HISTORY_LIMIT = 100;
// Session-only history survives scene and language changes; no save mutations.
const history: string[] = [];

export function Console() {
  useLangVersion();
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [source, setSource] = useState<LogSource | "all">("all");
  const [errors, setErrors] = useState(false);
  const [search, setSearch] = useState("");
  const [follow, setFollow] = useState(true);
  const lines = useSyncExternalStore(subscribeLogs, getLogs);
  const inputRef = useRef<HTMLInputElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const historyIndex = useRef(history.length);
  const draft = useRef("");

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.isComposing || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      const editable = target?.isContentEditable || (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
      const toggle = e.code === "Backquote" || ["`", "~", "～", "·"].includes(e.key);
      if ((toggle && (open || !editable)) || (open && e.key === "Escape")) {
        e.preventDefault();
        e.stopImmediatePropagation();
        setOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    previousFocus.current = document.activeElement as HTMLElement | null;
    inputRef.current?.focus();
    historyIndex.current = history.length;
    return () => { previousFocus.current?.isConnected && previousFocus.current.focus(); };
  }, [open]);

  useEffect(() => {
    const el = outputRef.current;
    if (open && follow && el) el.scrollTop = el.scrollHeight;
  }, [open, lines, follow, source, errors, search]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const line = input.trim();
    if (!line || busy) return;
    if (history.at(-1) !== line) {
      history.push(line);
      if (history.length > HISTORY_LIMIT) history.shift();
    }
    historyIndex.current = history.length;
    draft.current = "";
    setInput("");
    setBusy(true);
    try { await runConsoleCommand(line); } finally { setBusy(false); }
    inputRef.current?.focus();
  };

  const inputKey = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      if (historyIndex.current === history.length) draft.current = input;
      const index = Math.max(0, Math.min(history.length, historyIndex.current + (e.key === "ArrowUp" ? -1 : 1)));
      historyIndex.current = index;
      setInput(index === history.length ? draft.current : history[index] ?? "");
    }
    if (e.key === "Tab" && input.trim() && !/\s/.test(input.trim())) {
      const matches = COMMANDS.filter((cmd) => cmd.startsWith(input.trim().toLowerCase()));
      if (matches.length === 1) { e.preventDefault(); setInput(matches[0] + " "); }
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
