// Popup windows (the original's PopupWindow): pink title bar, round close
// button. Opened imperatively from anywhere; rendered by <ModalHost/> inside
// the stage so they scale with it and see the app's state.

import { cloneElement, type ReactElement, type ReactNode, useSyncExternalStore } from "react";
import { cx } from "../core/cx";
import { Btn } from "./Button";
import { Icon } from "./Icon";
import s from "./Modal.module.css";
import { t as tr } from "../i18n/t";

export type ModalSize = "small" | "mid" | "normal" | "wide" | "xl";

export interface ModalOpts {
  closable?: boolean;
  size?: ModalSize;
  /** Extra class on the window. */
  className?: string;
  onClose?: () => void;
  /** A key: opening another modal with the same key replaces it. */
  key?: string;
}

interface Entry extends ModalOpts {
  id: number;
  minimized: boolean;
  title: string;
  body: ReactNode | ((close: () => void) => ReactNode);
}

let entries: Entry[] = [];
let seq = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((cb) => cb());

export function openModal(title: string, body: Entry["body"], opts: ModalOpts = {}): () => void {
  const id = ++seq;
  const close = () => {
    const e = entries.find((x) => x.id === id);
    if (!e) return;
    entries = entries.filter((x) => x.id !== id);
    emit();
    e.onClose?.();
  };
  if (opts.key) entries.filter((e) => e.key === opts.key).forEach((e) => closeById(e.id));
  entries = [...entries, { id, title, body, minimized: false, ...opts }];
  emit();
  return close;
}

function closeById(id: number): void {
  const e = entries.find((x) => x.id === id);
  entries = entries.filter((x) => x.id !== id);
  e?.onClose?.();
}

function minimize(id: number): void {
  entries = entries.map((e) => e.id === id ? { ...e, minimized: true } : e);
  emit();
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
  return key === undefined ? entries.length > 0 : entries.some((e) => e.key === key);
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

export function ModalHost() {
  const list = useSyncExternalStore(subscribe, () => entries);
  return (
    <>
      {list.map((e) => {
        const close = () => {
          if (!entries.some((x) => x.id === e.id)) return;
          entries = entries.filter((x) => x.id !== e.id);
          emit();
          e.onClose?.();
        };
        const closable = e.closable ?? true;
        return (
          // Keep drafts, checked cards, scroll position and live subscriptions
          // mounted; minimizing only hides the window and its backdrop.
          <div key={e.id} className={s.back} hidden={e.minimized} onClick={(ev) => closable && ev.target === ev.currentTarget && close()}>
            <div className={cx(s.window, s[e.size ?? "normal"], e.className)}>
              <div className={s.head}>
                <h2>{e.title}</h2>
                <button type="button" className={cx(s.x, closable && s.minimize)} onClick={() => minimize(e.id)} title={tr("common.minimize")} aria-label={tr("common.minimize")}>
                  <span className={s.minus} aria-hidden />
                </button>
                {closable && (
                  <button type="button" className={s.x} onClick={close} title={tr("common.close")}>
                    <Icon name="close" />
                  </button>
                )}
              </div>
              <div className={s.body}>{typeof e.body === "function" ? e.body(close) : cloneElement(e.body as ReactElement)}</div>
            </div>
          </div>
        );
      })}
      {list.some((e) => e.minimized) && (
        <div className={s.restoreTray} aria-label={tr("common.minimizedPopups")}>
          {list.filter((e) => e.minimized).map((e) => (
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
