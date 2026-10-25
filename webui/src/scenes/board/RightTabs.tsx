// The phone canvas' right column: the standing card and the match log share one
// panel behind a two-tab toggle (the desktop stacks them, half each). The tab
// changes when the user taps it, and when they *pick* a card on the board (the
// field row's coarse first tap -- that is asking to see the card). A card
// merely pointed at or flashed updates the (mounted) stand but never pulls the
// user away from the log they are reading.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { cx } from "../../core/cx";
import { t as tr } from "../../i18n/t";
import { useCardPick } from "../../ui/CardPreview";
import { Icon } from "../../ui/Icon";
import { CardStand } from "./CardStand";
import s from "./RightTabs.module.css";

export function RightTabs({ flash, log }: { flash?: string; log: ReactNode }) {
  const [tab, setTab] = useState<"card" | "log">("log");
  // A user pick opens the card stand; a hover / flash never does.
  const pick = useCardPick();
  const seen = useRef(pick);
  useEffect(() => {
    if (pick === seen.current) return;
    seen.current = pick;
    setTab("card");
  }, [pick]);
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
