// The original's toast: a dark translucent bar in the middle of the stage.

import { useSyncExternalStore } from "react";
import { cx } from "../core/cx";
import s from "./Toast.module.css";

interface T { id: number; text: string; error: boolean; out: boolean }
let current: T | null = null;
let seq = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((cb) => cb());

export function toast(text: string, kind: "info" | "error" = "info"): void {
  const id = ++seq;
  current = { id, text, error: kind === "error", out: false };
  emit();
  setTimeout(() => {
    if (current?.id === id) {
      current = { ...current, out: true };
      emit();
    }
  }, 2200);
  setTimeout(() => {
    if (current?.id === id) {
      current = null;
      emit();
    }
  }, 2600);
}

const subscribe = (cb: () => void) => {
  listeners.add(cb);
  return () => listeners.delete(cb);
};

export function ToastHost() {
  const t = useSyncExternalStore(subscribe, () => current);
  return <div className={s.host}>{t && <div key={t.id} className={cx(s.toast, t.error && s.error, t.out && s.out)}>{t.text}</div>}</div>;
}
