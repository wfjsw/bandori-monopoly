// The 60-tile ring, tokens, and the center: cards in play, the event deck, banner.

import { charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { useRef, useState } from "react";
import { useLayoutSize } from "../../hooks/measure";
import { useBoardViewport, type ViewportApi } from "./viewport";
import { D, cardTitle } from "../../core/data";
import { isLight, plain } from "../../core/format";
import type { TileData } from "../../core/types";
import type { Animator } from "./anim";
import type { Model } from "./model";
import { showEventPile } from "./Popups";
import s from "./Ring.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";

/** The TTS board (mod 3506424344, object 53c41e / Lua `pathXY`): 60 tiles on a
 *  12 x 10 grid of cells, `boardIndex 0` = CiRCLE at the bottom right,
 *  counter-clockwise. Each side's middle steps inward one cell and runs back
 *  out -- that extra rung is what lets one side carry 15 tiles instead of 12 on
 *  the same box. The board fills its own rect (the slot held to 1..1.6,
 *  `viewport.ts` `boardRect`): cells stretch to `board / 12` by `board / 10`,
 *  so the ring uses the whole rectangle while the path -- and the pockets it
 *  encloses -- keep their grid shape. The window around it is the whole map
 *  slot, so the visible range is 100% of the slot even though the map is not. */
const COLS = 12;
const ROWS = 10;
const FOLD: readonly (readonly [number, number])[] = [
  [11, 9], [10, 9], [9, 9], [8, 9], [7, 9], [7, 8], [7, 7], [6, 7], [5, 7], [4, 7],
  [4, 8], [4, 9], [3, 9], [2, 9], [1, 9], [0, 9], [0, 8], [0, 7], [0, 6], [1, 6],
  [2, 6], [3, 6], [3, 5], [3, 4], [3, 3], [2, 3], [1, 3], [0, 3], [0, 2], [0, 1],
  [0, 0], [1, 0], [2, 0], [3, 0], [4, 0], [4, 1], [4, 2], [5, 2], [6, 2], [7, 2],
  [7, 1], [7, 0], [8, 0], [9, 0], [10, 0], [11, 0], [11, 1], [11, 2], [11, 3], [10, 3],
  [9, 3], [8, 3], [8, 4], [8, 5], [8, 6], [9, 6], [10, 6], [11, 6], [11, 7], [11, 8],
];

/** Grid cell of tile `i` as 0-based [col, row]. */
function cell(i: number): readonly [number, number] {
  return FOLD[i % FOLD.length];
}

/** Pixel centre of a 0-based column / row on a `w` x `h` board rect (the ring
 *  inside the window -- not the window itself). */
const cellX = (col: number, w: number) => (col + 0.5) * (w / COLS);
const cellY = (row: number, h: number) => (row + 0.5) * (h / ROWS);

export interface RingProps {
  m: Model;
  anim: Animator;
  /** Tiles a "tile" prompt lets you pick by clicking. */
  pickable: number[];
  onTile: (i: number) => void;
  /** The die in the board's roll zone (hidden in replays). */
  roll?: RollControl | null;
}

/** The roll-zone die button: same action as the old side-column one. */
export interface RollControl {
  enabled: boolean;
  rolling: boolean;
  dice: number;
  hint: string;
  onClick: () => void;
}

/** What a hovered tile marker says, and where (the wrap's own pixel space). */
interface MarkTip { x: number; y: number; title: string; lines: string[] }

/** Rough advance width in half-CJK units: fullwidth glyphs count 2, others 1.
 *  Used to pick a name size tier and to balance the two name lines -- Latin
 *  `DUB MUSIC EXPERIMENT` and CJK 「瑟罗希亚国际学校」 both need the small end
 *  even though `.length` differs. */
function textUnits(s: string): number {
  let n = 0;
  for (const ch of s) {
    const c = ch.codePointAt(0) ?? 0;
    const wide =
      (c >= 0x1100 && c <= 0x115f) || (c >= 0x2e80 && c <= 0xa4cf) ||
      (c >= 0xac00 && c <= 0xd7a3) || (c >= 0xf900 && c <= 0xfaff) ||
      (c >= 0xfe30 && c <= 0xfe6f) || (c >= 0xff00 && c <= 0xff60) ||
      (c >= 0xffe0 && c <= 0xffe6);
    n += wide ? 2 : 1;
  }
  return n;
}

/** Trailing noun kept whole on line 2 (full match: 「…/大学」, 「…/女子学院」). */
const NAME_SUFFIX = /^(?:女子)?(?:学院|大学|中学|学园|高中|学校|公园|公司|咖啡厅|餐厅|车站|乐器店|事务所|面包房|拉面馆|精肉店|澡堂|画室|大厦|豪宅|住宅区|小巷|山丘|商店|中心|公寓)$/;

/** Advance width of `s` at `font` px. CJK ≈ 1em; Latin in this UI face runs
 *  about 0.64em (measured on 「EXPERIMENT」 / 「CHUCHU的公寓」) -- 0.55em
 *  under-estimates and the line then clips. */
function estWidth(s: string, font: number): number {
  let w = 0;
  for (const ch of s) {
    const c = ch.codePointAt(0) ?? 0;
    const wide =
      (c >= 0x1100 && c <= 0x115f) || (c >= 0x2e80 && c <= 0xa4cf) ||
      (c >= 0xac00 && c <= 0xd7a3) || (c >= 0xf900 && c <= 0xfaff) ||
      (c >= 0xfe30 && c <= 0xfe6f) || (c >= 0xff00 && c <= 0xff60) ||
      (c >= 0xffe0 && c <= 0xffe6);
    w += wide ? font : ch === " " ? font * 0.3 : font * 0.64;
  }
  return w;
}

/**
 * Split a tile name into at most two lines. CSS `text-wrap: balance` alone
 * breaks CJK per character (「庆鹏女/子大学」), so the split is chosen here:
 * never inside a Latin word, only when one line cannot fit, balanced by
 * {@link textUnits}, and biased toward a short trailing noun on line 2.
 */
function splitNameLines(name: string, font: number, avail: number): string[] {
  if (estWidth(name, font) <= avail) return [name];
  const chars = [...name];
  if (chars.length < 2) return [name];
  let best = -1;
  let bestScore = Infinity;
  for (let i = 1; i < chars.length; i++) {
    const prev = chars[i - 1]!;
    const next = chars[i]!;
    // Never split inside a Latin/number run.
    if (/[A-Za-z0-9]/.test(prev) && /[A-Za-z0-9]/.test(next)) continue;
    const a = chars.slice(0, i).join("").trim();
    const b = chars.slice(i).join("").trim();
    if (!a || !b) continue;
    const ua = textUnits(a);
    const ub = textUnits(b);
    let score = Math.abs(ua - ub);
    // Prefer a 2+ char trailing noun on line 2 (「…/大学」, 「…/女子学院」).
    if (NAME_SUFFIX.test(b) && ub >= 4) score -= 5;
    // Prefer a script boundary (「CHUCHU/的公寓」, 「Bandori/车站」).
    if (/[A-Za-z]/.test(prev) !== /[A-Za-z]/.test(next)) score -= 2;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  }
  if (best < 0) return [name];
  return [chars.slice(0, best).join("").trim(), chars.slice(best).join("").trim()];
}

/** Name size tier from the width metric (see {@link textUnits}). */
function nameTier(text: string): string {
  const u = textUnits(text);
  return u <= 8 ? s.nameL : u <= 14 ? s.nameM : u <= 20 ? s.nameS : s.nameXS;
}

/** Name-size multiplier for a tier class (mirrors `.nameL`..`.nameXS`). */
const NAME_TIERS: readonly (readonly [string, number])[] = [
  [s.nameL, 1.1],
  [s.nameM, 1],
  [s.nameS, 0.92],
  [s.nameXS, 0.8],
];

/**
 * Pick the largest name tier whose (up to two) lines fit `avail`. Starts from
 * the width-metric tier so short names stay big, then steps down if a line
 * like 「EXPERIMENT」 would clip. Corners / agents only get the size boost
 * when the name is short (「CiRCLE」); a long one (「Live House」) stays on the
 * shared scale so it can fit one line on the wide board.
 */
function fitName(name: string, u: number, avail: number, special: boolean): { tier: string; lines: string[]; boost: boolean } {
  const start = NAME_TIERS.findIndex(([cls]) => cls === nameTier(name));
  const boost = special && textUnits(name) <= 8;
  const kindK = boost ? 1.12 : 1;
  for (let i = Math.max(0, start); i < NAME_TIERS.length; i++) {
    const [cls, k] = NAME_TIERS[i]!;
    const font = u * k * kindK;
    const lines = splitNameLines(name, font, avail);
    const ok = lines.every((l) => estWidth(l, font) <= avail * 0.98);
    if (ok || i === NAME_TIERS.length - 1) return { tier: cls, lines, boost };
  }
  return { tier: s.nameXS, lines: [name], boost };
}

/** A tile's face, drawn from the same data the deeds and engine use -- no board
 *  texture. The `#index` chip is the colour band (top), the name sits in the
 *  middle (≤ 2 pre-split lines), and the caption (price / dealer / draw /
 *  corner event) is centred on a common bottom baseline. CiRCLE carries its
 *  pass / stop notes in place of the caption. Sizes are fractions of the
 *  measured cell (`--cell-h` / `--cell-w` on `.ring`), so the scale holds from
 *  the narrow 1366 cells up to the wide 2560 ones. */
function TileFace({ tile, cellW, cellH }: { tile: TileData; cellW: number; cellH: number }) {
  const name = plain(tile.name);
  const corner = ["circle", "cafe", "edogawa", "ryuseido"].includes(tile.kind);
  const special = corner || tile.kind === "agent";
  const caption = tile.price > 0 ? String(tile.price)
    : tile.kind === "agent" ? tr("deed.dealer")
    : tile.kind === "edogawa" ? tr("deed.draw")
    : tile.kind === "cafe" || tile.kind === "ryuseido" ? tr("board.cornerEvent")
    : "";
  const u = Math.min(cellH * 0.18, cellW * 0.17);
  // Matches `.tileName` / `.tileBody` horizontal padding.
  const avail = cellW - u * 0.12 * 2 - u * 0.08 * 2 - 2;
  const { tier, lines, boost } = fitName(name, u, avail, special);
  return (
    <>
      <span
        className={s.tileNumber}
        style={{ background: tile.color, color: isLight(tile.color) ? "#17221e" : "#fff" }}
        aria-hidden="true"
      >#{tile.index}</span>
      <span className={s.tileBody} aria-hidden="true">
        <span className={cx(s.tileName, special && s.specialName, boost && s.nameBoost, tier)}>
          <span className={s.nameText}>
            {lines.map((l, i) => <span key={i} className={s.nameLine}>{l}</span>)}
          </span>
        </span>
        {tile.kind === "circle" ? (
          <span className={s.tileNote}>
            <span>{tr("board.circlePassing")}</span>
            <span>{tr("board.circleStopping")}</span>
          </span>
        ) : <span className={s.tileCaption}>{caption}</span>}
      </span>
    </>
  );
}

export function Ring({ m, anim, pickable, onTile, roll }: RingProps) {
  const S = m.S;
  const pos = anim.pos ?? S.players.map((x) => x.pos);
  const pick = new Set(pickable);
  const vp = useBoardViewport();
  const wrapRef = vp.wrapRef;
  const [tip, setTip] = useState<MarkTip | null>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  // Markers sit inside a tile (which clips), so their popup is drawn on the
  // untransformed overlay above the zoom layer. The wrap is not zoomed: its
  // bounding box carries only the stage scale, and converting the marker's
  // screen box against it lands in the wrap's own pixel space (the window) at
  // any zoom.
  const showTip = (e: React.MouseEvent, title: string, lines: string[]) => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const r = wrap.getBoundingClientRect();
    const k = r.width / (wrap.offsetWidth || 1) || 1;
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setTip({ x: (b.left + b.width / 2 - r.left) / k, y: (b.top - r.top) / k, title, lines: lines.filter(Boolean) });
  };
  const hideTip = () => setTip(null);
  // The tip is content-sized (`width: max-content`, max 220px) and the wrap
  // clips the panned board, so keep it inside the *window*: clamp x by its
  // measured width against the window's width and flip it below the marker
  // near the top edge.
  const tipBox = useLayoutSize(tipRef, tip);
  const half = Math.min(tipBox.width || 220, 220) / 2;
  const tipX = tip ? Math.min(Math.max(tip.x, half + 8), vp.box.width - half - 8) : 0;
  const tipUp = tip ? tip.y - (tipBox.height || 90) - 12 >= 0 : true;
  return (
    <div
      className={s.wrap}
      data-vp-bg
      ref={wrapRef}
      tabIndex={0}
      onKeyDown={vp.onKeyDown}
      onPointerDown={vp.onPointerDown}
      onPointerMove={vp.onPointerMove}
      onPointerUp={vp.onPointerUp}
      onPointerCancel={vp.onPointerUp}
      onClickCapture={vp.onClickCapture}
      onDoubleClick={vp.onDoubleClick}
      aria-label={tr("board.zoomLabel")}
    >
      {/* Window pixels are the wrap's own pixels; the zoom layer is the board
          rect (ring + interior) placed by the user's pan/zoom. It does not
          fill the window -- on a wide slot the board keeps its aspect and sits
          centred at fit, and the wrap shows the rest of the slot. */}
      <div className={s.viewport} style={{ ...vp.style, width: vp.board.width, height: vp.board.height }}>
        <div
          className={s.ring}
          data-vp-bg
          style={{
            // Measured cell, so tile type scales with the stretched grid.
            ["--cell-w" as string]: `${vp.board.width / COLS}px`,
            ["--cell-h" as string]: `${vp.board.height / ROWS}px`,
          }}
        >
          <Center m={m} roll={roll} />
          {D.tiles.map((t, i) => {
            const [col, row] = cell(i);
            const cellW = vp.board.width / COLS;
            const cellH = vp.board.height / ROWS;
            const owner = S.owners[i] ?? -1;
            const cls = cx(s.tile, owner >= 0 && s.owned, S.mortgaged[i] && s.mortgaged, S.phase === "play" && S.landed === i && s.landed, pick.has(i) && s.pickable);
            const style = { gridRow: row + 1, gridColumn: col + 1, ["--owner" as string]: owner >= 0 ? m.colorOf(owner) : "transparent" };
            const marks = (S.marks ?? []).filter((x) => x.tile === i).slice(0, 3);
            const embers = S.embers?.[i] ?? 0;
            const extras = (
              <>
                <div className={s.houses}>{Array.from({ length: Math.min(S.houses[i] ?? 0, 4) }, (_, k) => <img key={k} src={sceneImg("house")} alt="" />)}</div>
                <div className={s.marks}>
                  {marks.map((x) => {
                    // [CP点] is its own tile-mark category: neutral (no seat owns
                    // one), labelled 「CP点」, and its popup has no 「放置者」.
                    const cp = x.category === "cp";
                    const title = cp
                      ? (x.count > 1 ? `${tr("board.markCp")} ×${x.count}` : tr("board.markCp"))
                      : (x.count > 1 ? `${x.kind} ×${x.count}` : x.kind);
                    return (
                      <span
                        key={x.uid}
                        className={cx(s.mark, cp && s.cp)}
                        style={cp ? undefined : { background: x.owner >= 0 ? m.colorOf(x.owner) : "var(--note)" }}
                        onMouseEnter={(e) => showTip(e, title, [
                          // 「放置者」 is a player mark's only; a [CP点] has no owner.
                          !cp && x.owner >= 0 ? tr("board.markOwner", { who: m.nameOf(x.owner) }) : "",
                          x.card ? tr("board.markFrom", { card: cardTitle(x.card) }) : "",
                          x.note ? fmtMsg(x.note, namesOf(S)) : "",
                        ])}
                        onMouseLeave={hideTip}
                      >{cp ? "CP" : x.kind.slice(0, 1)}</span>
                    );
                  })}
                  {embers > 0 && (
                    <span className={cx(s.mark, s.ember)} onMouseEnter={(e) => showTip(e, tr("board.embers", { n: embers }), [])} onMouseLeave={hideTip}>{embers}</span>
                  )}
                </div>
              </>
            );
            return (
              <button
                key={i}
                type="button"
                className={cls}
                style={style}
                aria-label={`${t.index}. ${plain(t.name)}`}
                title={`${t.index}. ${plain(t.name)}${t.price > 0 ? ` · ${t.price}` : ""}${owner >= 0 ? ` · ${m.nameOf(owner)}` : ""}`}
                onClick={() => onTile(i)}
              >
                <TileFace tile={t} cellW={cellW} cellH={cellH} />
                {owner >= 0 && <span className={s.ownerStrip} />}
                {extras}
              </button>
            );
          })}
          <div className={s.tokens}>
            {S.players.map((x, i) => {
              if (x.bankrupt || x.left) return null;
              const p = pos[i] ?? x.pos;
              const [col, row] = cell(p);
              const same = S.players.map((_, k) => k).filter((k) => !S.players[k].bankrupt && !S.players[k].left && (pos[k] ?? S.players[k].pos) === p);
              const k = Math.max(0, same.indexOf(i));
              const n = same.length;
              const dx = n > 1 ? (k - (n - 1) / 2) * Math.min(14, 40 / (n - 1)) : 0;
              const ch = m.charOf(i);
              const hop = anim.hop?.playerId === i ? anim.hop.id : 0;
              // Anchor near the cell's lower third so the SD art sits over the
              // caption / gutter, not over the name.
              const top = cellY(row, vp.board.height) + (vp.board.height / ROWS) * 0.22;
              return (
                <div key={i} className={cx(s.token, i === S.turn && s.current)} style={{ left: cellX(col, vp.board.width) + dx, top, zIndex: Math.round(top) }} title={namesOf(S).playerId(i)}>
                  <img className={s.shadow} src={sceneImg("piece_shadow")} alt="" />
                  {ch
                    ? <img key={hop} className={cx(s.sd, hop > 0 && s.hop)} src={charArt(D.artId(ch), "sdThumb")} alt="" />
                    : <div className={s.dot} style={{ background: m.colorOf(i) }}>{namesOf(S).playerId(i).slice(0, 1)}</div>}
                </div>
              );
            })}
          </div>
        </div>
      </div>
      {/* Fixed overlays: the marker tip stays out of the zoom layer so it
          keeps its size and screen position at any zoom. The turn / action
          banners and the card flash are match-screen chrome (Board's FX
          layer) -- not map-sized, not map-clipped. Field cards now read from
          the board's top sheet (FieldSheet), with the shared hover preview. */}
      <div className={s.overlay}>
        {tip && (
          <div ref={tipRef} className={s.markTip} style={{ left: tipX, top: tip.y, transform: tipUp ? undefined : "translate(-50%, 6px)" }}>
            <b>{tip.title}</b>
            {tip.lines.map((l, k) => <span key={k}>{l}</span>)}
          </div>
        )}
      </div>
      <ZoomControls vp={vp} />
    </div>
  );
}

/** + / − / fit controls, on the board's top-right corner. Outside the zoom
 *  layer: they stay put and stay the same size at any zoom. */
function ZoomControls({ vp }: { vp: ViewportApi }) {
  return (
    <div className={s.zoomCtl} data-vp-ctl role="group">
      <button type="button" onClick={vp.zoomOut} title={tr("board.zoomOut")} aria-label={tr("board.zoomOut")}>−</button>
      <button type="button" onClick={vp.reset} title={tr("board.zoomReset")} aria-label={tr("board.zoomReset")}>1:1</button>
      <button type="button" onClick={vp.zoomIn} title={tr("board.zoomIn")} aria-label={tr("board.zoomIn")}>+</button>
    </div>
  );
}

function Center({ m, roll }: { m: Model; roll?: RollControl | null }) {
  const S = m.S;
  return (
    <div className={s.inner} data-vp-bg>
      {/* Board colour and zone labels on the path's 12 x 10 grid (1-based grid
          lines). Teal fills only the ring's five enclosed pockets -- top-left,
          top-right, centre, bottom-left, bottom-right. The four side notches
          of the TTS board (its marker / dice-storage pockets) are outside the
          ring, so they carry no background and their labels move inside: the
          markers to the top-left pocket, the dice storage to the roll zone's
          flanks. */}
      <div className={s.zones} aria-hidden="true">
        <span className={s.pocket} style={{ gridColumn: "2 / 5", gridRow: "2 / 4" }} />
        <span className={s.pocket} style={{ gridColumn: "9 / 12", gridRow: "2 / 4" }} />
        <span className={s.pocket} style={{ gridColumn: "5 / 9", gridRow: "4 / 8" }} />
        <span className={s.pocket} style={{ gridColumn: "2 / 5", gridRow: "8 / 10" }} />
        <span className={s.pocket} style={{ gridColumn: "9 / 12", gridRow: "8 / 10" }} />
        <span style={{ gridColumn: "2 / 5", gridRow: "2 / 4" }}>{tr("board.markerZone")}</span>
        <span style={{ gridColumn: "9 / 12", gridRow: "2 / 4" }}>{tr("board.specialMarkerZone")}</span>
        <span className={s.zoneNarrow} style={{ gridColumn: "5 / 6", gridRow: "4 / 8" }}>{tr("board.diceZone")}</span>
        <span style={{ gridColumn: "6 / 8", gridRow: "4 / 8" }}>{tr("board.rollZone")}</span>
        <span className={s.zoneNarrow} style={{ gridColumn: "8 / 9", gridRow: "4 / 8" }}>{tr("board.diceZone")}</span>
      </div>
      {/* The die in the board's roll zone (掷骰区): the one roll control, on
          the spot the board names for it. Hidden in replays (`roll` null). */}
      {roll && (
        <div className={s.rollDock}>
          <button
            type="button"
            className={cx(s.roll, roll.enabled && s.can, roll.rolling && s.rolling)}
            title={roll.hint}
            disabled={!roll.enabled}
            onClick={roll.onClick}
          >
            <img className={s.diceImg} src={sceneImg("dice_d20")} alt="" />
            <div className={s.diceNum}>{roll.dice || ""}</div>
            <div className={s.diceCap}>{tr("board.dice")}</div>
          </button>
        </div>
      )}
      {/* The shared event deck sits in the bottom-left pocket of the fold; your
          own draw pile is at the head of your hand (Side.tsx). Cards in play
          sit on the board's top sheet (FieldSheet), not in the interior. */}
      <div className={s.piles}>
        <button type="button" className={s.pile} onClick={() => showEventPile(S)}>
          <div className={s.stack}><img src={sceneImg("card_back")} alt="" /></div>
          <div className={s.pileLabel}>{tr("board.eventDeck")}<b>×{S.eventDeck}</b></div>
        </button>
      </div>
      <div className={s.hint}>{tr("board.ringHint")}</div>
    </div>
  );
}
