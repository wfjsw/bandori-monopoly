// Blocks the page while a live server / build / custom-ruleset switch loads
// its rules and tables. The swap itself happens under here; the scenes remount
// on the new data when it clears.

import { useLoadState } from "../core/hooks";
import { t as tr } from "../i18n/t";
import s from "./SourceOverlay.module.css";

export function SourceOverlay() {
  const state = useLoadState();
  if (!state.busy) return null;
  const stage = state.stage === "data" ? tr("boot.switchData") : tr("boot.switchRules");
  const pct = state.total ? Math.round((state.done / state.total) * 100) : 0;
  return (
    <div className={s.scrim} role="status" aria-live="polite">
      <div className={s.panel}>
        <b className={s.title}>{tr("boot.switchTitle")}</b>
        <div className={s.bar}>
          <div className={s.fill} style={{ width: `${pct}%` }} />
        </div>
        <span className={s.stage}>
          {state.total ? `${stage} ${state.done}/${state.total}` : stage}
        </span>
      </div>
    </div>
  );
}
