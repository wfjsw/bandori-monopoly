// The fixed 1600 x 900 stage (the original's reference resolution), scaled to
// fit the window, over a blurred per-screen backdrop that fills the letterbox.

import { type ReactNode, useEffect, useState, useSyncExternalStore } from "react";
import { sceneImg } from "../core/assets";
import s from "./Stage.module.css";

export const STAGE_W = 1600;
export const STAGE_H = 900;

let backdrop: string | null = null;
const listeners = new Set<() => void>();

/** Scene backgrounds: bg_common (menu, board), bg_live (lobby), bg_band (prep, gallery, deck). */
export function setBackdrop(name: string | null): void {
  if (backdrop === name) return;
  backdrop = name;
  listeners.forEach((cb) => cb());
}

/** Set the backdrop while a component is mounted. */
export function useBackdrop(name: string | null): void {
  useEffect(() => setBackdrop(name), [name]);
}

function useScale(): number {
  const calc = () => Math.min(window.innerWidth / STAGE_W, window.innerHeight / STAGE_H);
  const [scale, set] = useState(calc);
  useEffect(() => {
    const on = () => set(calc());
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return scale;
}

export function Stage({ children, fading }: { children: ReactNode; fading: boolean }) {
  const scale = useScale();
  const bg = useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => backdrop,
  );
  const src = bg ? sceneImg(bg) : "";
  return (
    <>
      <div className={s.backdrop} style={{ backgroundImage: src ? `url("${src}")` : undefined }} />
      <div className={s.stage} style={{ transform: `translate(-50%, -50%) scale(${scale})` }}>
        {children}
        <div className={fading ? `${s.fader} ${s.on}` : s.fader} />
      </div>
    </>
  );
}
