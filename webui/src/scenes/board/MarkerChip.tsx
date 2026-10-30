import { memo } from "react";
import { chipParts, type MarkerDef } from "./markerKinds";
import s from "./MarkerChip.module.css";

/** Symbols a chip draws instead of printing -- see `MarkerChip.module.css`. */
const DRAWN: Record<string, string> = { "◆": s.diamond };

/**
 * One marker chip: its leading symbol and count. Most symbols are text; the
 * crystal's `◆` is drawn. U+25C6 has no glyph in the rounded UI face, so the
 * browser falls back to a system font and the diamond lands at a different
 * size on every platform -- tiny on desktop fonts, full-size on phones.
 */
export const MarkerChip = memo(function MarkerChip({ def, n, name, className, title }: {
  def: MarkerDef;
  n: number;
  name?: string;
  className?: string;
  title?: string;
}) {
  const { glyph, rest } = chipParts(def, n, name);
  const drawn = DRAWN[glyph];
  return (
    <span className={className} title={title}>
      {drawn ? <i className={drawn} /> : glyph}
      {rest}
    </span>
  );
});
