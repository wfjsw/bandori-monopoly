// The 60-tile ring, tokens, and the center: cards in play, the event deck, banner.

import { cardArt, charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { useRef, useState } from "react";
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

/** The 10-05 fold: 60 tiles on a 12 x 10 grid, `boardIndex 0` = bottom right,
 *  counter-clockwise. Each side's middle steps inward one cell and runs back
 *  out -- that extra rung is what lets one side carry 15 tiles instead of 12 on
 *  the same box, and it is why the cells can be 83 x 100 rather than 50 x 50.
 *  Extracted from the upstream scene (`level4`, `BoardTileView` layout). */
const COLS = 12;
const ROWS = 10;
const RING = 856;
const FOLD: readonly (readonly [number, number])[] = [
  [11, 9], [10, 9], [9, 9], [8, 9], [7, 9], [7, 8], [7, 7], [6, 7], [5, 7], [4, 7],
  [4, 8], [4, 9], [3, 9], [2, 9], [1, 9], [0, 9], [0, 8], [0, 7], [0, 6], [1, 6],
  [2, 6], [3, 6], [3, 5], [3, 4], [3, 3], [2, 3], [1, 3], [0, 3], [0, 2], [0, 1],
  [0, 0], [1, 0], [2, 0], [3, 0], [4, 0], [4, 1], [4, 2], [5, 2], [6, 2], [7, 2],
  [7, 1], [7, 0], [8, 0], [9, 0], [10, 0], [11, 0], [11, 1], [11, 2], [11, 3], [10, 3],
  [9, 3], [8, 3], [8, 4], [8, 5], [8, 6], [9, 6], [10, 6], [11, 6], [11, 7], [11, 8],
];
const cornerRibbon = (kind: string) => ({ edogawa: tr("deed.draw"), cafe: tr("board.cornerEvent"), ryuseido: tr("board.cornerEvent") } as Record<string, string>)[kind];

/** Grid cell of tile `i` as 0-based [col, row]. */
function cell(i: number): readonly [number, number] {
  return FOLD[i % FOLD.length];
}

/** Pixel centre of a 0-based column / row. (`cx` is the classnames helper.) */
const cellX = (col: number) => (col + 0.5) * (RING / COLS);
const cellY = (row: number) => (row + 0.5) * (RING / ROWS);

/** Which edge the colour bar belongs on: the ring's **interior**, which is the
 *  left of the direction of travel -- the loop runs counter-clockwise. Every
 *  step of the fold is axis-aligned, so the normal is exact.
 *
 *  "Facing the grid centre" is not the same thing and gets it wrong twice over:
 *  it mirrors the left/right edges, and the fold's notch walls sit beside a void
 *  that is *outside* the ring (a bite taken out of the board edge), so their
 *  interior is away from that void, not toward the middle. */
function sideOf(i: number): number {
  const [col, row] = FOLD[i];
  const [pc, pr] = FOLD[(i + FOLD.length - 1) % FOLD.length];
  const nc = -(row - pr); // interior normal = left of travel
  const nr = col - pc;
  if (nr < 0) return 0; // bar at top
  if (nc > 0) return 1; // bar at right
  if (nr > 0) return 2; // bar at bottom
  return 3; // bar at left
}

/** How the ring turns at a tile: `outer` is the tile's vertex on the ring's
 *  outside (0 = bottom-right, 1 = bottom-left, 2 = top-left, 3 = top-right).
 *
 *  `convex` is which way the fold bends. Bending toward the interior (a board
 *  corner, or an edge stepping down into a notch) leaves the tile's two free
 *  edges -- the ones with no neighbour on them -- facing out. Bending away from
 *  it (the floor of a notch) leaves them facing in. */
interface Turn { outer: number; convex: boolean }

/** Where the ring *turns*, or null on a straight run where the single-strip
 *  form is right.
 *
 *  Every turn gets the wrapped mark, not just the four board corners -- the
 *  fold turns back on itself at each notch, and those corners need it too. */
function turnAt(i: number): Turn | null {
  const n = FOLD.length;
  const [c, r] = FOLD[i];
  const [pc, pr] = FOLD[(i + n - 1) % n];
  const [nc, nr] = FOLD[(i + 1) % n];
  const din: readonly [number, number] = [c - pc, r - pr];
  const dout: readonly [number, number] = [nc - c, nr - r];
  if (din[0] === dout[0] && din[1] === dout[1]) return null;
  // The interior is the left of travel; map each segment's normal to an edge
  // (0 top, 1 right, 2 bottom, 3 left).
  const edge = (d: readonly [number, number]) => {
    const col = -d[1];
    const row = d[0];
    return row < 0 ? 0 : row > 0 ? 2 : col > 0 ? 1 : 3;
  };
  const e = new Set([edge(din), edge(dout)]);
  const outer = e.has(0) && e.has(3) ? 0 // inner top+left -> outer bottom-right
    : e.has(0) && e.has(1) ? 1 // inner top+right -> outer bottom-left
    : e.has(2) && e.has(1) ? 2 // inner bottom+right -> outer top-left
    : 3; // inner bottom+left -> outer top-right
  // Convex when the path turns toward the incoming step's interior normal
  // (`[-din[1], din[0]]`, as in `sideOf`).
  const convex = -din[1] * dout[0] + din[0] * dout[1] > 0;
  return { outer, convex };
}

export interface RingProps {
  m: Model;
  anim: Animator;
  /** Tiles a "tile" prompt lets you pick by clicking. */
  pickable: number[];
  onTile: (i: number) => void;
}

/** What a hovered tile marker says, and where (ring coordinates). */
interface MarkTip { x: number; y: number; title: string; lines: string[] }

export function Ring({ m, anim, pickable, onTile }: RingProps) {
  const S = m.S;
  const pos = anim.pos ?? S.players.map((x) => x.pos);
  const pick = new Set(pickable);
  const ringRef = useRef<HTMLDivElement>(null);
  const [tip, setTip] = useState<MarkTip | null>(null);
  // Markers sit inside a tile (which clips), so their popup is drawn on the
  // ring instead, at the marker's position. The stage is CSS-scaled: convert
  // screen pixels back to the ring's own 856 px space.
  const showTip = (e: React.MouseEvent, title: string, lines: string[]) => {
    const ring = ringRef.current;
    if (!ring) return;
    const r = ring.getBoundingClientRect();
    const k = r.width / RING || 1;
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setTip({ x: (b.left + b.width / 2 - r.left) / k, y: (b.top - r.top) / k, title, lines: lines.filter(Boolean) });
  };
  const hideTip = () => setTip(null);
  return (
    <div className={s.wrap}>
      <div className={s.ring} ref={ringRef}>
        <Center m={m} />
        {D.tiles.map((t, i) => {
          const [col, row] = cell(i);
          const corner = (col === 0 || col === COLS - 1) && (row === 0 || row === ROWS - 1);
          const turn = turnAt(i);
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
          return corner
            ? <CornerTile key={i} t={t} cls={cls} style={style} turn={turn} onClick={() => onTile(i)}>{extras}</CornerTile>
            : <SideTile key={i} t={t} side={sideOf(i)} turn={turn} cls={cls} style={style} onClick={() => onTile(i)}>{extras}</SideTile>;
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
        {tip && (
          <div className={s.markTip} style={{ left: tip.x, top: tip.y }}>
            <b>{tip.title}</b>
            {tip.lines.map((l, k) => <span key={k}>{l}</span>)}
          </div>
        )}
      </div>
    </div>
  );
}

interface TileProps { t: TileData; cls: string; style: React.CSSProperties; onClick: () => void; children: React.ReactNode }

/** The wrapped marks at a turn. The rule is the straight run's -- the tile's
 *  colour on the ring's inside, ownership on its outside -- but at a turn one
 *  strip has to wrap the corner as an L along the tile's free edges, and the
 *  other shrinks to a block in the opposite vertex. Which one wraps follows the
 *  fold: on a convex turn the free edges face out, so the L is ownership; on a
 *  concave one they face in, so the L is the tile's colour. `null` means the
 *  path is straight here and the single-strip form applies instead. */
function TurnMarks({ turn, color }: { turn: Turn | null; color: string }) {
  if (turn == null) return null;
  const at = turn.convex ? turn.outer : (turn.outer + 2) % 4;
  const tile = { background: color };
  const lOwn = turn.convex && s.own;
  const lStyle = turn.convex ? undefined : tile;
  return (
    <div className={cx(s.turnMarks, s[`l${at}`])}>
      <div className={cx(s.lA, lOwn)} style={lStyle} />
      <div className={cx(s.lB, lOwn)} style={lStyle} />
      <div className={cx(s.sq, !turn.convex && s.own)} style={turn.convex ? tile : undefined} />
    </div>
  );
}

function CornerTile({ t, cls, style, onClick, children, turn }: TileProps & { turn: Turn | null }) {
  const art = sceneImg(`area_${t.area}`);
  const ribbon = cornerRibbon(t.kind);
  return (
    <div className={cx(cls, s.corner, t.kind === "circle" && s.circle)} style={style} onClick={onClick}>
      {ribbon && <div className={s.ribbon}>{ribbon}</div>}
      <TurnMarks turn={turn} color={t.color} />
      {art && <img className={s.cornerArt} src={art} alt="" />}
      <div className={s.cornerName}>{plain(t.name)}</div>
      {children}
    </div>
  );
}

function SideTile({ t, side, turn, cls, style, onClick, children }: TileProps & { side: number; turn: Turn | null }) {
  const ring = t.kind === "ring";
  return (
    <div className={cx(cls, s[`s${side}`])} style={style} onClick={onClick}>
      {/* Straight run: one strip each side. At a turn the strip has to wrap, so
          the single strips give way to the L + block. */}
      {turn == null && <div className={s.bar} style={{ background: t.color }} />}
      <div className={s.body}>
        <div className={cx(s.name, ring && s.ringName)}>{ring ? "RiNG" : t.shortName || t.name}</div>
        {t.kind === "agent" ? <div className={s.agent}>{tr("deed.dealer")}</div> : t.price > 0 && <div className={s.price}>{t.price}</div>}
      </div>
      {turn == null && <div className={s.obar} />}
      <TurnMarks turn={turn} color={t.color} />
      {children}
    </div>
  );
}

function Center({ m }: { m: Model }) {
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
  // Hover target: a card in play, or an event in effect (both expand the same way).
  const [hover, setHover] = useState<{ kind: "card" | "event"; id: string; note: string; owner?: number } | null>(null);
  const hc = hover?.kind === "card" ? D.card(hover.id) : undefined;
  const he = hover?.kind === "event" ? D.event(hover.id) : undefined;
  return (
    <div className={s.inner} style={{ backgroundImage: `url("${sceneImg("world_map")}")` }}>
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
          <div className={s.fieldGrid}>
            {rows.flatMap(([i, f]) => f.map((fc) => {
              const note = fc.note ? fmtMsg(fc.note, names) : "";
              const color = m.colorOf(i);
              return fc.faceDown ? (
                <div key={fc.uid} className={s.fieldBack} style={{ ["--own" as string]: color }} title={`${m.nameOf(i)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <CardFace key={fc.uid} id={fc.card} size="hand" className={s.fieldCard} style={{ ["--own" as string]: color }} onClick={() => showCard(fc.card, [], note)} onMouseEnter={() => setHover({ kind: "card", id: fc.card, owner: i, note: note || [fc.crystals ? tr("board.crystals", { n: fc.crystals }) : "", fc.cp > 0 ? tr("board.cp", { n: fc.cp }) : ""].filter(Boolean).join(" · ") })} onMouseLeave={() => setHover(null)}>
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
                  <div key={e.id} className={s.eventFace} onClick={() => showEvent(e.id, note)} onMouseEnter={() => setHover({ kind: "event", id: e.id, note })} onMouseLeave={() => setHover(null)}>
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
      {/* Hovered field card, full size beside the interior (like the hand's preview). */}
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
