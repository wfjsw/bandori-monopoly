// The 60-tile ring, tokens, and the center: cards in play, the event deck, banner.

import { cardArt, charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { useLayoutEffect, useRef, useState } from "react";
import { useBoardViewport, type ViewportApi } from "./viewport";
import { D, cardTitle } from "../../core/data";
import { plain } from "../../core/format";
import type { TileData } from "../../core/types";
import { CardFace, showCard } from "../../ui/Card";
import { Avatar, bandColor } from "../../ui/Character";
import { SkillBody } from "../../ui/SkillBody";
import type { Animator } from "./anim";
import type { Model } from "./model";
import { showEvent, showEventPile, showField } from "./Popups";
import s from "./Ring.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";

/** The TTS board (mod 3506424344, object 53c41e / Lua `pathXY`): 60 tiles on a
 *  12 x 10 grid of square cells, `boardIndex 0` = CiRCLE at the bottom right,
 *  counter-clockwise. Each side's middle steps inward one cell and runs back
 *  out -- that extra rung is what lets one side carry 15 tiles instead of 12 on
 *  the same box. Cells are square (12 x 10 = the board's 6:5 aspect). */
const COLS = 12;
const ROWS = 10;
const RING = 856;
const HEIGHT = RING * ROWS / COLS;
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

/** Pixel centre of a 0-based column / row (square cells). */
const cellX = (col: number) => (col + 0.5) * (RING / COLS);
const cellY = (row: number) => (row + 0.5) * (HEIGHT / ROWS);

export interface RingProps {
  m: Model;
  anim: Animator;
  /** Tiles a "tile" prompt lets you pick by clicking. */
  pickable: number[];
  onTile: (i: number) => void;
}

/** What a hovered tile marker says, and where (the wrap's 856 px space). */
interface MarkTip { x: number; y: number; title: string; lines: string[] }

/** Hover target: a card in play, or an event in effect (both expand the same way). */
interface FieldHover { kind: "card" | "event"; id: string; note: string; owner?: number }

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

export function Ring({ m, anim, pickable, onTile }: RingProps) {
  const S = m.S;
  const pos = anim.pos ?? S.players.map((x) => x.pos);
  const pick = new Set(pickable);
  const vp = useBoardViewport();
  const wrapRef = vp.wrapRef;
  const [tip, setTip] = useState<MarkTip | null>(null);
  const [tipBox, setTipBox] = useState({ w: 0, h: 0 });
  const tipRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<FieldHover | null>(null);
  // Markers sit inside a tile (which clips), so their popup is drawn on the
  // untransformed overlay above the zoom layer. The wrap is not zoomed: its
  // bounding box carries only the stage scale, and converting the marker's
  // screen box against it lands in the wrap's own 856 px space at any zoom.
  const showTip = (e: React.MouseEvent, title: string, lines: string[]) => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const r = wrap.getBoundingClientRect();
    const k = r.width / RING || 1;
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setTip({ x: (b.left + b.width / 2 - r.left) / k, y: (b.top - r.top) / k, title, lines: lines.filter(Boolean) });
  };
  const hideTip = () => setTip(null);
  // The tip is content-sized (`width: max-content`, max 220px) and the wrap
  // clips the panned board, so keep it inside the board box: clamp x by its
  // measured width and flip it below the marker near the top edge.
  useLayoutEffect(() => {
    const el = tipRef.current;
    if (!el || !tip) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    if (w !== tipBox.w || h !== tipBox.h) setTipBox({ w, h });
  }, [tip, tipBox.w, tipBox.h]);
  const half = Math.min(tipBox.w || 220, 220) / 2;
  const tipX = tip ? Math.min(Math.max(tip.x, half + 8), RING - half - 8) : 0;
  const tipUp = tip ? tip.y - (tipBox.h || 90) - 12 >= 0 : true;
  return (
    <div
      className={s.wrap}
      data-vp-bg
      ref={wrapRef}
      style={{ width: vp.box.width, height: vp.box.height }}
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
      {/* Board pixels -> the window: one fit scale, then the user's pan/zoom. */}
      <div className={s.fit} style={{ transform: `scale(${vp.fit})` }}>
        <div className={s.viewport} style={vp.style}>
        <div className={s.ring} data-vp-bg>
          <Center m={m} onHover={setHover} />
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
              const top = cellY(row) + 16;
              return (
                <div key={i} className={cx(s.token, i === S.turn && s.current)} style={{ left: cellX(col) + dx, top, zIndex: Math.round(top) }} title={namesOf(S).playerId(i)}>
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
      {/* Fixed overlays: turn banner, card flash and the hover details stay out
          of the zoom layer so they keep their size and screen position. */}
      <div className={s.overlay}>
        {anim.banner && (
          <div key={anim.banner.id} className={s.banner}>
            <b>{anim.banner.title}</b>
            {anim.banner.body && <span>{anim.banner.body}</span>}
          </div>
        )}
        {anim.turnAnnouncement && (
          <div key={anim.turnAnnouncement.id} className={s.phaseFlash}>
            <i className={s.link} /><i className={s.link} /><i className={s.link} /><i className={s.link} />
            <span>{anim.turnAnnouncement.label}</span>
          </div>
        )}
        {anim.reveal && <div className={cx(s.reveal, anim.reveal.out && s.revealOut)}><CardFace id={anim.reveal.card} size="big" /></div>}
        <FieldPreview m={m} hover={hover} />
        {tip && (
          <div ref={tipRef} className={s.markTip} style={{ left: tipX, top: tip.y, transform: tipUp ? undefined : "translate(-50%, 6px)" }}>
            <b>{tip.title}</b>
            {tip.lines.map((l, k) => <span key={k}>{l}</span>)}
          </div>
        )}
      </div>
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

/** The field-card / event hover detail, over the right of the board window.
 *  Rendered outside the zoom layer, so it never scales away from the viewer. */
function FieldPreview({ m, hover }: { m: Model; hover: FieldHover | null }) {
  const hc = hover?.kind === "card" ? D.card(hover.id) : undefined;
  const he = hover?.kind === "event" ? D.event(hover.id) : undefined;
  return (
    <div className={cx(s.fieldPreview, hover && s.fieldPreviewOn)}>
      {hover && (
        <>
          {hover.kind === "card" ? (
            <>
              <div className={s.fpArt} style={{ borderColor: hc ? bandColor(hc.band) : "#ED4E76" }}><img src={cardArt(hover.id)} alt="" /></div>
              <div className={s.fpTitle}>{cardTitle(hover.id)}</div>
              {hover.owner != null && (
                <div className={s.fpOwner} style={{ ["--own" as string]: m.colorOf(hover.owner) }}>
                  <Avatar c={m.charOf(hover.owner)} size={22} /><span>{tr("board.fieldOwner", { who: m.nameOf(hover.owner) })}</span>
                </div>
              )}
              <div className={s.fpText}><SkillBody text={hc?.text ?? ""} /></div>
            </>
          ) : (
            <>
              <div className={s.fpTitle}><span className={s.fpEvent}>{tr("events.label", { id: hover.id })}</span> {he?.name ?? ""}</div>
              <div className={s.fpText}>{he?.text ?? ""}</div>
            </>
          )}
          {hover.note && <div className={s.fpNote}>{hover.note}</div>}
        </>
      )}
    </div>
  );
}

function Center({ m, onHover }: { m: Model; onHover: (h: FieldHover | null) => void }) {
  const S = m.S;
  // Skill rules are placed on the field so `On::Hook` reaches them (engine
  // `bind_skills`); they are not cards in play and carry no displayable state,
  // and the skill button is where they are actually shown. `skill:` is the
  // rule-id prefix `skill_id` builds -- without this every player opens the
  // match with two （未命名） cards on their field.
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const fieldCount = rows.reduce((a, [, f]) => a + f.length, 0);
  const events = S.eventActive ?? [];
  const names = namesOf(S);
  return (
    <div className={s.inner} data-vp-bg>
      {/* Interior zones of the TTS board, on the same square-cell grid as the
          path, beneath the live cards. */}
      <div className={s.zones} aria-hidden="true">
        <span style={{ gridColumn: "6 / 8", gridRow: "1 / 3" }}>{tr("board.markerZone")}</span>
        <span style={{ gridColumn: "9 / 12", gridRow: "2 / 4" }}>{tr("board.specialMarkerZone")}</span>
        <span style={{ gridColumn: "5 / 9", gridRow: "4 / 8" }}>{tr("board.rollZone")}</span>
        <span style={{ gridColumn: "1 / 4", gridRow: "5 / 7" }}>{tr("board.diceZone")}</span>
        <span style={{ gridColumn: "10 / 13", gridRow: "5 / 7" }}>{tr("board.diceZone")}</span>
      </div>
      {/* 场上的卡 in the fold's big interior: compact card faces grouped by
          whose field they are on, each with its live state (crystals, note).
          Hover shows the full card; click opens it; the header opens the
          large list (Popups `showField`). */}
      {(fieldCount > 0 || events.length > 0) && (
        <div className={s.field}>
          <button type="button" className={s.fieldHead} onClick={() => showField(m)}>
            <b>{tr("board.field")}</b><small>×{fieldCount}</small>
          </button>
          {/* One compact grid for every card in play, grouped by owner (seat
              order); each card is framed in its owner's colour, and hovering
              names the owner in the preview. Events follow, unowned. */}
          <div className={s.fieldGrid} data-vp-scroll>
            {rows.flatMap(([i, f]) => f.map((fc) => {
              const note = fc.note ? fmtMsg(fc.note, names) : "";
              const color = m.colorOf(i);
              return fc.faceDown ? (
                <div key={fc.uid} className={s.fieldBack} style={{ ["--own" as string]: color }} title={`${m.nameOf(i)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <CardFace key={fc.uid} id={fc.card} size="hand" className={s.fieldCard} style={{ ["--own" as string]: color }} onClick={() => showCard(fc.card, [], note)} onMouseEnter={() => onHover({ kind: "card", id: fc.card, owner: i, note: note || [fc.crystals ? tr("board.crystals", { n: fc.crystals }) : "", fc.cp > 0 ? tr("board.cp", { n: fc.cp }) : ""].filter(Boolean).join(" · ") })} onMouseLeave={() => onHover(null)}>
                  {fc.crystals > 0 && <span className={s.crystal}>◆{fc.crystals}</span>}
                  {fc.cp > 0 && <span className={s.cpBadge} title={tr("board.cp", { n: fc.cp })}>CP{fc.cp}</span>}
                  {note && <span className={s.noteDot} />}
                </CardFace>
              );
            }))}
          </div>
          {/* Events in effect: the same compact face (event art is the deck's
              back, tinted), its counters as badges, hover to expand. */}
          {events.length > 0 && (
            <div className={s.eventRow} title={tr("board.activeEvents")}>
              {events.map((e) => {
                const ev = D.event(e.id);
                const note = e.note?.k ? fmtMsg(e.note, names) : "";
                return e.faceDown ? (
                  <div key={e.id} className={s.fieldBack} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
                ) : (
                  <div key={e.id} className={s.eventFace} onClick={() => showEvent(e.id, note)} onMouseEnter={() => onHover({ kind: "event", id: e.id, note })} onMouseLeave={() => onHover(null)}>
                    <div className={s.eventArt}><img src={sceneImg("card_back")} alt="" /></div>
                    <div className={s.eventTitle}>{ev?.name ?? e.id}</div>
                    {e.counter > 0 && <span className={s.crystal}>×{e.counter}</span>}
                    {note && <span className={s.noteDot} />}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
      {/* The shared event deck sits in the bottom-left pocket of the fold; your
          own draw pile is at the head of your hand (Side.tsx). */}
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
