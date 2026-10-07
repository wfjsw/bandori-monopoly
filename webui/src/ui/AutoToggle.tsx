// 托管 / 混沌 mode selector: hands this seat to a browser-side autopilot.
// Stays clickable in every state, so the player can always take the controls
// back. On the board it sits in the turn card (`compact`); on character select
// it is in the TopBar `right` slot. The banner strip is rendered by the match
// screens.

import { useAutoMode } from "../core/hooks";
import { cx } from "../core/cx";
import type { AutoMode } from "../game/autopilot";
import type { GameSession } from "../game/session";
import { Icon } from "./Icon";
import s from "./AutoToggle.module.css";
import { t as tr } from "../i18n/t";

const NEXT: Record<AutoMode, AutoMode> = { off: "bot", bot: "chaos", chaos: "off" };
const LABEL = (m: AutoMode) => (m === "bot" ? tr("board.afk") : m === "chaos" ? tr("board.chaos") : tr("board.autoOff"));

/**
 * The mode pill itself. Click cycles
 * off → 托管 → 混沌 → off; the label and colour name the current mode.
 */
export function AutoToggle({ sess, className, compact }: { sess: GameSession; className?: string; compact?: boolean }) {
  const mode = useAutoMode(sess);
  return (
    <button
      type="button"
      className={cx(s.toggle, compact && s.compact, mode !== "off" && s.on, mode === "chaos" && s.chaos, className)}
      onClick={() => sess.setAutoMode(NEXT[mode])}
      title={tr("board.autoplayHint")}
      aria-label={tr("board.autoplayHint")}
    >
      <Icon name={mode === "chaos" ? "cyclone" : "smart_toy"} />
      <span>{LABEL(mode)}</span>
    </button>
  );
}

/** The "托管中 / 混沌中" strip -- shown while an autopilot owns the seat. */
export function AutoBanner({ sess }: { sess: GameSession }) {
  const mode = useAutoMode(sess);
  if (mode === "off") return null;
  return (
    <div className={cx(s.banner, mode === "chaos" && s.chaos)} role="status">
      <Icon name={mode === "chaos" ? "cyclone" : "smart_toy"} />
      {tr("board.autoplayBanner", { mode: LABEL(mode) })}
    </div>
  );
}