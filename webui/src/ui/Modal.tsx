// Popup windows (the original's PopupWindow): pink title bar, round close
// button. Opened imperatively from anywhere; rendered by <ModalHost/> inside
// the stage so they scale with it and see the app's state.
//
// Prompt chrome is not a centred window: it is a bottom sheet over the hand
// (Master Duel's), sliding up from the bottom edge and sitting above the hand
// bar / fan in z-order. Its title-bar control is a retract toggle -- the sheet
// drops until just the strip (title, countdown, raise chevron) rides above the
// hand bar -- and Esc does the same. The board and the hand stay usable while
// it is retracted; a new question always raises it again.

import { cloneElement, type ReactElement, type ReactNode, useEffect, useRef, useSyncExternalStore } from "react";
import { cx } from "../core/cx";
import { useAutoFocus, useHotkeys } from "../hooks/dom";
import { Btn } from "./Button";
import { Icon } from "./Icon";
import s from "./Modal.module.css";
import { t as tr } from "../i18n/t";

/** `fit`: as wide as the content needs (between 380px and the board width) --
 *  the inline board prompts. */
export type ModalSize = "small" | "mid" | "normal" | "wide" | "xl" | "fit";

export interface ModalOpts {
  closable?: boolean;
  size?: ModalSize;
  /** Extra class on the window. */
  className?: string;
  onClose?: () => void;
  /** A key: opening another modal with the same key replaces it. */
  key?: string;
  /**
   * "overlay" (default): dimmed, centred window that blocks the board.
   * "inline": a floating card over the board centre -- no backdrop, so the
   * ring stays visible and pickable around it (the prompt's tile picks).
   */
  placement?: "overlay" | "inline";
  /**
   * "window" (default): the pink title bar of a popup.
   * "prompt": white panel with a pink border and pink title text, presented as
   * the bottom sheet over the hand -- the match prompt's look.
   */
  chrome?: "window" | "prompt";
  /** Extra content in the title bar -- the prompt sheet's countdown. */
  headExtra?: ReactNode;
}

interface Entry extends ModalOpts {
  id: number;
  minimized: boolean;
  /** Prompt sheets only: dropped until just the title strip is up. */
  retracted: boolean;
  /** Exit slide running; the entry is already gone for every caller. */
  closing: boolean;
  title: string;
  body: ReactNode | ((close: () => void) => ReactNode);
}

let entries: Entry[] = [];
let seq = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((cb) => cb());

export function openModal(title: string, body: Entry["body"], opts: ModalOpts = {}): () => void {
  const id = ++seq;
  const close = () => requestClose(id);
  if (opts.key) entries.filter((e) => e.key === opts.key).forEach((e) => closeById(e.id));
  entries = [...entries, { id, title, body, minimized: false, retracted: false, closing: false, ...opts }];
  emit();
  return close;
}

/** How long a prompt sheet takes to slide away (matches `.sheetOut`). */
const SHEET_OUT_MS = 180;

/**
 * Close one entry. A prompt sheet first slides down (its content lingers inside
 * the panel for the ride); every other popup still disappears at once.
 */
function requestClose(id: number): void {
  const e = entries.find((x) => x.id === id);
  if (!e || e.closing) return;
  if (e.chrome !== "prompt") {
    entries = entries.filter((x) => x.id !== id);
    emit();
    e.onClose?.();
    return;
  }
  entries = entries.map((x) => x.id === id ? { ...x, closing: true } : x);
  emit();
  window.setTimeout(() => {
    const gone = entries.find((x) => x.id === id);
    if (!gone || !gone.closing) return;
    entries = entries.filter((x) => x.id !== id);
    emit();
    gone.onClose?.();
  }, SHEET_OUT_MS);
}

/** Replace-style close (same `key` opening over it, scene change): no exit slide. */
function closeById(id: number): void {
  const e = entries.find((x) => x.id === id);
  if (!e) return;
  entries = entries.filter((x) => x.id !== id);
  e.onClose?.();
}

function minimize(id: number): void {
  entries = entries.map((e) => e.id === id ? { ...e, minimized: true } : e);
  emit();
}

function setRetracted(id: number, retracted: boolean): void {
  entries = entries.map((e) => e.id === id ? { ...e, retracted } : e);
  emit();
}

function toggleRetract(id: number): void {
  const e = entries.find((x) => x.id === id && !x.closing);
  if (!e) return;
  setRetracted(id, !e.retracted);
}

const livePrompt = () => entries.find((x) => x.chrome === "prompt" && !x.closing);

/** The one prompt sheet, if any: Esc and the title strip drive this. */
export function togglePromptSheet(): boolean {
  const e = livePrompt();
  if (!e) return false;
  toggleRetract(e.id);
  return true;
}

/** Raise the prompt sheet (a new question takes over). */
export function raisePromptSheet(): void {
  const e = livePrompt();
  if (e?.retracted) setRetracted(e.id, false);
}

function restore(id: number): void {
  const e = entries.find((x) => x.id === id);
  if (!e) return;
  entries = [...entries.filter((x) => x.id !== id), { ...e, minimized: false }];
  emit();
}

/** Close every popup (scene changes). */
export function closeAllModals(): void {
  const old = entries;
  entries = [];
  emit();
  old.forEach((e) => e.onClose?.());
}

export function isModalOpen(key?: string): boolean {
  // A sheet mid-exit has already been answered; it must not hold the board.
  const live = entries.filter((e) => !e.closing);
  return key === undefined ? live.length > 0 : live.some((e) => e.key === key);
}

/** A yes / no question. */
export function ask(title: string, text: string, label = tr("common.confirm")): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    const finish = (v: boolean) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const close = openModal(title, (
      <div className={s.ask}>
        <p>{text}</p>
        <div className={s.row}>
          <Btn onClick={() => finish(false)}>{tr("common.cancel")}</Btn>
          <Btn kind="pink" onClick={() => finish(true)}>{label}</Btn>
        </div>
      </div>
    ), { size: "small", onClose: () => finish(false) });
  });
}

const subscribe = (cb: () => void) => {
  listeners.add(cb);
  return () => listeners.delete(cb);
};

/**
 * The prompt sheet's live state for the hand: `open` while a question is up
 * (raised or retracted -- the hand is inspect-only then), `raised` while it
 * owns the bottom edge (the fan stays down behind it).
 */
export function usePromptSheet(): { open: boolean; raised: boolean } {
  const list = useSyncExternalStore(subscribe, () => entries);
  const e = list.find((x) => x.chrome === "prompt" && !x.closing);
  return { open: !!e, raised: !!e && !e.retracted };
}

/** Raise the prompt sheet whenever `key` changes -- a new question takes over. */
export function useRaiseSheetOn(key: unknown): void {
  useEffect(() => {
    raisePromptSheet();
  }, [key]);
}

function ModalEntry({ e }: { e: Entry }) {
  const prompt = e.chrome === "prompt";
  const closable = e.closable ?? true;
  const winRef = useRef<HTMLDivElement>(null);
  // The sheet takes focus when it opens and when it raises again (and hands
  // focus back on retract / close).
  useAutoFocus(prompt && !e.closing && !e.retracted, winRef);
  const close = () => requestClose(e.id);
  // Keep drafts, checked cards, scroll position and live subscriptions
  // mounted; minimizing only hides the window and its backdrop.
  return (
    <div
      className={prompt ? s.sheet : e.placement === "inline" ? s.inline : s.back}
      hidden={e.minimized}
      onClick={(ev) => closable && ev.target === ev.currentTarget && close()}
    >
      <div
        ref={winRef}
        tabIndex={prompt ? -1 : undefined}
        role={prompt ? "dialog" : undefined}
        aria-label={prompt ? e.title : undefined}
        className={cx(
          s.window,
          s[e.size ?? "normal"],
          prompt && s.promptWin,
          prompt && s.sheetWin,
          prompt && e.retracted && s.retracted,
          prompt && e.closing && s.sheetOut,
          e.className,
        )}
      >
        <div
          className={cx(s.head, prompt && s.headToggle)}
          onClick={prompt ? () => toggleRetract(e.id) : undefined}
        >
          <h2>{e.title}</h2>
          {e.headExtra}
          {prompt ? (
            <button
              type="button"
              className={s.x}
              onClick={(ev) => { ev.stopPropagation(); toggleRetract(e.id); }}
              title={tr(e.retracted ? "prompt.raise" : "prompt.retract")}
              aria-label={tr(e.retracted ? "prompt.raise" : "prompt.retract")}
              aria-expanded={!e.retracted}
            >
              <span className={cx(s.chev, e.retracted && s.chevUp)} aria-hidden />
            </button>
          ) : (
            <>
              <button type="button" className={cx(s.x, closable && s.minimize)} onClick={() => minimize(e.id)} title={tr("common.minimize")} aria-label={tr("common.minimize")}>
                <span className={s.minus} aria-hidden />
              </button>
              {closable && (
                <button type="button" className={s.x} onClick={close} title={tr("common.close")}>
                  <Icon name="close" />
                </button>
              )}
            </>
          )}
        </div>
        <div className={s.body}>{typeof e.body === "function" ? e.body(close) : cloneElement(e.body as ReactElement)}</div>
      </div>
    </div>
  );
}

export function ModalHost() {
  const list = useSyncExternalStore(subscribe, () => entries);
  // Esc retracts / raises the prompt sheet -- prompts never dismiss on Esc.
  useHotkeys([{ key: "Escape", once: true, run: () => { togglePromptSheet(); } }]);
  return (
    <>
      {list.map((e) => <ModalEntry key={e.id} e={e} />)}
      {list.some((e) => e.minimized && !e.closing) && (
        <div className={s.restoreTray} aria-label={tr("common.minimizedPopups")}>
          {list.filter((e) => e.minimized && !e.closing).map((e) => (
            <button key={e.id} type="button" className={s.restore} onClick={() => restore(e.id)} title={tr("common.restorePopup", { title: e.title })} aria-label={tr("common.restorePopup", { title: e.title })}>
              <span className={s.restoreIcon} aria-hidden />
              <span className={s.restoreTitle}>{e.title}</span>
              <span className={s.restoreHint}>{tr("common.restore")}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}
