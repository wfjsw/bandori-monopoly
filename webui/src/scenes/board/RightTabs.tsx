// The phone canvas' right column: the standing card and the match log share one
// panel behind a two-tab toggle (the desktop stacks them, half each). A card the
// user points at, clicks or sees flashed brings the card tab forward -- that is
// what they are asking to read -- and the toggle flips back to the log.

import { useEffect, useState, type ReactNode } from "react";
import { cx } from "../../core/cx";
import { t as tr } from "../../i18n/t";
import { useStickyCard } from "../../ui/CardPreview";
import { Icon } from "../../ui/Icon";
import { CardStand } from "./CardStand";
import s from "./RightTabs.module.css";

export function RightTabs({ flash, log }: { flash?: string; log: ReactNode }) {
  const [tab, setTab] = useState<"card" | "log">("log");
  const card = useStickyCard();
  // Follows the sticky card (hover, click or flash) -- including re-picking the
  // same card after the user flipped to the log, via the object identity.
  useEffect(() => {
    if (card) setTab("card");
  }, [card]);
  return (
    <div className={s.slot}>
      <div className={s.tabs} role="tablist">
        <button type="button" role="tab" aria-selected={tab === "card"} className={cx(s.tab, tab === "card" && s.on)} onClick={() => setTab("card")}>
          {tr("board.tabCard")}
        </button>
        <button type="button" role="tab" aria-selected={tab === "log"} className={cx(s.tab, tab === "log" && s.on)} onClick={() => setTab("log")}>
          <Icon name="history" />
          {tr("menu.history")}
        </button>
      </div>
      {/* Both stay mounted: the log keeps its scroll position, the stand keeps
          following flashes while hidden. */}
      <div className={cx(s.pane, tab !== "card" && s.hidden)}>
        <CardStand flash={flash} />
      </div>
      <div className={cx(s.pane, tab !== "log" && s.hidden)}>{log}</div>
    </div>
  );
}
