// The 60-tile ring, tokens, and the center: cards in play, the event deck, banner.

import { charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { useRef, useState } from "react";
import { useLayoutSize } from "../../hooks/measure";
import { useBoardViewport, type ViewportApi } from "./viewport";
import { D, cardTitle } from "../../core/data";
import { plain } from "../../core/format";
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

/** A tile's face, drawn from the same data the deeds and engine use -- no board
 *  texture. The `#index` chip wears the tile's colour (the old colour bar);
 *  below the name sits its caption (price / dealer / draw / corner event), and
 *  CiRCLE carries its pass / stop notes. */
function TileFace({ tile }: { tile: TileData }) {
  const name = plain(tile.name);
  const corner = ["circle", "cafe", "edogawa", "ryuseido"].includes(tile.kind);
  const caption = tile.price > 0 ? String(tile.price)
    : tile.kind === "agent" ? tr("deed.dealer")
    : tile.kind === "edogawa" ? tr("deed.draw")
    : tile.kind === "cafe" || tile.kind === "ryuseido" ? tr("board.cornerEvent")
    : "";
  return (
    <>
      <span className={s.tileNumber} style={{ background: tile.color }} aria-hidden="true">#{tile.index}</span>
      <span className={s.tileBody} aria-hidden="true">
        <span className={cx(s.tileName, (corner || tile.kind === "agent") && s.specialName, name.length > 18 && s.longName)}>{name}</span>
        {tile.kind === "circle" ? (
          <span className={s.tileNote}>{tr("board.circlePassing")}<br />{tr("board.circleStopping")}</span>
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
        <div className={s.ring} data-vp-bg>
          <Center m={m} roll={roll} />
          {D.tiles.map((t, i) => {
            const [col, row] = cell(i);
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
                <TileFace tile={t} />
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
              const top = cellY(row, vp.board.height) + 16;
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
