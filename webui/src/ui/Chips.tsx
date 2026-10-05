import { cx } from "../core/cx";
import s from "./Chips.module.css";

/** A row of pink (active) / white chips. */
export function Chips<T extends string>({ items, on, onPick, count, className }: {
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

/** Pink rounded label on top of panels ("match log", "hand 2/5"). */
export function PanelTab({ children, className }: { children: React.ReactNode; className?: string }) {
  return <span className={cx(s.tab, className)}>{children}</span>;
}
