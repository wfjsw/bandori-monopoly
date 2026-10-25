import { memo } from "react";
import { cx } from "../core/cx";
import s from "./Chips.module.css";

/** A row of pink (active) / white chips. */
function ChipsInner<T extends string>({ items, on, onPick, count, className }: {
  items: readonly T[]; on: T; onPick: (v: T) => void; count?: (v: T) => number | string | null; className?: string;
}) {
  return (
    <div className={cx(s.chips, className)}>
      {items.map((it) => {
        const c = count?.(it);
        return (
          <button key={it} type="button" className={cx(s.chip, it === on && s.on)} onClick={() => onPick(it)}>
            {it}
            {c !== undefined && c !== null && <small> {c}</small>}
          </button>
        );
      })}
    </div>
  );
}
export const Chips = memo(ChipsInner) as typeof ChipsInner;

/** Pink rounded label on top of panels ("match log", "hand 2/5"). */
export const PanelTab = memo(function PanelTab({ children, className }: { children: React.ReactNode; className?: string }) {
  return <span className={cx(s.tab, className)}>{children}</span>;
});
