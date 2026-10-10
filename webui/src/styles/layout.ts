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
/** Hand fan peek above the bottom edge (Side `.fan`). */
export const HAND_PEEK = 30;
/** Skill-aside header peek (Side `.skillAside`). */
export const SKILL_PEEK = 28;
/** Map window's top margin (Board `.mapSlot`). */
export const MAP_MARGIN_TOP = 18;