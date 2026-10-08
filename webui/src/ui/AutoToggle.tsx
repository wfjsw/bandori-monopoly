// 托管 / 混沌 / 进阶 mode selector: hands this seat to a browser-side
// autopilot. Stays clickable in every state, so the player can always take the
// controls back. On the board it sits in the turn card (`compact`); on
// character select it is in the TopBar `right` slot. The banner strip is
// rendered by the match screens. 进阶 is the worker-pool search
// (`docs/BOT.md` B6), lazy-loaded; it falls back to 托管 on any failure.

import { useEffect, useState } from "react";
import { useAutoMode } from "../core/hooks";
import { cx } from "../core/cx";
import type { AutoMode } from "../game/autopilot";
import type { GameSession } from "../game/session";
import { Icon } from "./Icon";
import s from "./AutoToggle.module.css";
import { t as tr } from "../i18n/t";

const NEXT: Record<AutoMode, AutoMode> = { off: "bot", bot: "chaos", chaos: "advanced", advanced: "off" };
const LABEL = (m: AutoMode) =>
  m === "bot" ? tr("board.afk") : m === "chaos" ? tr("board.chaos") : m === "advanced" ? tr("board.advanced") : tr("board.autoOff");

/** Class for a wrapper that lifts the pill above modals (see the CSS). */
export const autoFloat = s.float;

/**
 * The mode pill itself. Click cycles
 * off → 托管 → 混沌 → off; the label and colour name the current mode.
 */
export function AutoToggle({ sess, className, compact }: { sess: GameSession; className?: string; compact?: boolean }) {
  const mode = useAutoMode(sess);
  // A replay is read-only: there is no seat to hand to the autopilot.
  if (sess.readOnly) return null;
  return (
    <button
      type="button"
      className={cx(s.toggle, compact && s.compact, mode !== "off" && s.on, mode === "chaos" && s.chaos, mode === "advanced" && s.advanced, className)}
      onClick={() => sess.setAutoMode(NEXT[mode])}
      title={tr("board.autoplayHint")}
      aria-label={tr("board.autoplayHint")}
    >
      <Icon name={mode === "chaos" ? "cyclone" : mode === "advanced" ? "psychology" : "smart_toy"} />
      <span>{LABEL(mode)}</span>
    </button>
  );
}

/**
 * The "thinking…" pill: shown while a seat's 进阶 search is running
 * (`docs/BOT.md` B6). Small, transient, never blocks input.
 */
export function ThinkingPill({ sess, member }: { sess: GameSession; member?: number }) {
  const [, tick] = useState(0);
  useEffect(() => sess.subscribeThinking(() => tick((n) => n + 1)), [sess]);
  const who = member ?? sess.you;
  if (!sess.isThinking(who)) return null;
  return (
    <div className={s.thinking} role="status" aria-live="polite">
      <Icon name="psychology" />
      <span>{tr("board.thinking")}</span>
    </div>
  );
}

/** How long the strip stays after the cooldown ends (it is transient: the
 *  pill in the turn card is the standing indicator). */
const BANNER_HOLD_MS = 2000;
const BANNER_FADE_MS = 400;

/** The "托管中 / 混沌中" strip: shown when an autopilot mode is switched on (or
 *  changed), through the start-up cooldown, then fades away. */
export function AutoBanner({ sess }: { sess: GameSession }) {
  const mode = useAutoMode(sess);
  const [, tick] = useState(0);
  // When to drop the strip: re-armed on every mode change.
  const [hideAt, setHideAt] = useState(0);
  useEffect(() => {
    setHideAt(mode === "off" ? 0 : performance.now() + sess.autoCooldownLeft() + BANNER_HOLD_MS);
  }, [mode, sess]);
  const now = performance.now();
  const left = Math.ceil(sess.autoCooldownLeft() / 1000);
  const visible = mode !== "off" && now < hideAt;
  // Re-render to count the cooldown down and to fade / drop the strip on time.
  useEffect(() => {
    if (!visible) return;
    const id = window.setTimeout(() => tick((n) => n + 1), 200);
    return () => clearTimeout(id);
  });
  if (sess.readOnly || !visible) return null;
  return (
    <div className={cx(s.banner, mode === "chaos" && s.chaos, hideAt - now < BANNER_FADE_MS && s.fading)} role="status">
      <Icon name={mode === "chaos" ? "cyclone" : "smart_toy"} />
      {tr("board.autoplayBanner", { mode: LABEL(mode) })}
      {left > 0 && <span className={s.cooldown}>{tr("board.autoplayStarting", { n: left })}</span>}
    </div>
  );
}
