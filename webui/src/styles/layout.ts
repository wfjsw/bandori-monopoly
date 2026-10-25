// The shared stage geometry, for TypeScript. Mirrors the --stage-* / --board-*
// tokens in styles/base.css (docs/CSS.md); keep the two in sync. CSS reads the
// tokens; TS reads these numbers. Nothing else hard-codes them.

/** The design canvas (the original's reference resolution). */
export const STAGE_W = 1600;
export const STAGE_H = 900;

/** TopBar height. */
export const TOPBAR_H = 110;

/** Board body's side inset (Board `.body`). */
export const BOARD_INSET_X = 38;
/** Players / actions column width. */
export const BOARD_COL_LEFT = 350;
/** Stand + log column width. */
export const BOARD_COL_RIGHT = 300;
/** Gap between the board's three columns. */
export const BOARD_GAP = 9;
/**
 * Both insets + both columns + both gaps: the side chrome the centre column
 * does not get. `1600 - 744 = 856` is the centre column on the design stage.
 */
export const BOARD_CHROME_X = BOARD_INSET_X * 2 + BOARD_COL_LEFT + BOARD_COL_RIGHT + BOARD_GAP * 2; // 744
/** Centre column width on the design stage (`1600 - 744`). */
export const BOARD_MID = STAGE_W - BOARD_CHROME_X; // 856
/**
 * Centre column's centre minus the stage's, in px. Width-invariant: the side
 * chrome is symmetric, so the centre column's centre sits at `stage/2 + 25`
 * on any stage width. Modal / sheet anchor here to centre on the tile ring.
 */
export const BOARD_MID_OFFSET = 25;

/** Prompt sheet's retracted title strip (Modal `.sheetWin`). */
export const SHEET_STRIP = 60;
/** Field row's retracted peek: the face's bottom zone (title + marks + pad). */
export const FIELD_PEEK = 60;
/** Hand fan peek above the bottom edge (Side `.fan`). The skill stack peeks the
 *  same strip, so its cards sit at the hand's height (Side `.cards` / `.skillStack`). */
export const HAND_PEEK = 30;
/** Skill-stack card face width: the same face as a hand card (Card `mini`), so the two always match. */
export const SKILL_CARD_W = 120;
/** Skill-stack dense horizontal step: px between neighbouring card centres. */
export const SKILL_STEP = 34;
/** Gap the skill stack leaves between itself and the hand fan. */
export const SKILL_FAN_GAP = 24;
/** Map window's top margin (Board `.mapSlot`). */
export const MAP_MARGIN_TOP = 18;

// ---- compact (phone) stage ----
// A phone-sized landscape window keeps the desktop composition but is laid out
// on a shorter design canvas, so the effective scale (and with it every font
// and touch target) is ~1.7x what 900px of design height would give. Nothing is
// hidden: the side columns just narrow (tokens below, mirrored in base.css
// `:root[data-compact]`) and the centre column keeps what is left.

/** The compact design canvas' height (the board fills it; width follows the window). */
export const COMPACT_H = 540;
/** The narrowest design width the compact stage allows (scale is capped by it). */
export const COMPACT_MIN_W = 980;
export const COMPACT_INSET_X = 10;
export const COMPACT_GAP = 6;
export const COMPACT_COL_LEFT = 240;
export const COMPACT_COL_RIGHT = 216;
/** Seat list height on the compact stage (540 - top bar - actions). */
export const COMPACT_PLAYERS_H = 372;
/** A seat panel's tallest height there (70 on the desktop stage). */
export const COMPACT_SEAT_H = 50;
/** Centre column width on the narrowest compact stage. */
export const COMPACT_MID = COMPACT_MIN_W - COMPACT_INSET_X * 2 - COMPACT_COL_LEFT - COMPACT_COL_RIGHT - COMPACT_GAP * 2;

/** Is this window a phone held in landscape (or `?compact=1`, for desktop testing)? */
export function isCompactWindow(w: number, h: number, coarse: boolean, forced: boolean | null): boolean {
  if (forced !== null) return forced;
  return coarse && Math.min(w, h) <= 560 && w > h;
}
