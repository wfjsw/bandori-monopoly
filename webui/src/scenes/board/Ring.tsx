// The 60-tile ring, tokens, and the center: field panel, card piles, banner.

import { cardArt, charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { plain } from "../../core/format";
import type { TileData } from "../../core/types";
import { CardFace, showCard } from "../../ui/Card";
import { Avatar } from "../../ui/Character";
import type { Animator } from "./anim";
import type { Model } from "./model";
import { showEvent, showEventPile } from "./Popups";
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

export function Ring({ m, anim, pickable, onTile }: RingProps) {
  const S = m.S;
  const pos = anim.pos ?? S.players.map((x) => x.pos);
  const pick = new Set(pickable);
  return (
    <div className={s.wrap}>
      <div className={s.ring}>
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
                {marks.map((x) => <span key={x.uid} className={s.mark} style={{ background: x.owner >= 0 ? m.colorOf(x.owner) : "var(--note)" }} title={x.note ? fmtMsg(x.note, namesOf(S)) : x.kind}>{x.kind.slice(0, 1)}</span>)}
                {embers > 0 && <span className={cx(s.mark, s.ember)} title={tr("board.embers", { n: embers })}>{embers}</span>}
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
              <div key={i} className={cx(s.token, i === S.turn && s.current)} style={{ left: cellX(col) + dx, top, zIndex: Math.round(top) }} title={x.player}>
                <img className={s.shadow} src={sceneImg("piece_shadow")} alt="" />
                {ch
                  ? <img key={hop} className={cx(s.sd, hop > 0 && s.hop)} src={charArt(D.artId(ch), "sdThumb")} alt="" />
                  : <div className={s.dot} style={{ background: m.colorOf(i) }}>{x.player.slice(0, 1)}</div>}
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
        {anim.phase && (
          <div key={anim.phase.id} className={s.phaseFlash}>
            <i className={s.link} /><i className={s.link} /><i className={s.link} /><i className={s.link} />
            <span>{anim.phase.label ?? tr(anim.phase.key)}</span>
          </div>
        )}
        {anim.reveal && <div className={cx(s.reveal, anim.reveal.out && s.revealOut)}><CardFace id={anim.reveal.card} size="big" /></div>}
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
  return (
    <div className={s.inner} style={{ backgroundImage: `url("${sceneImg("world_map")}")` }}>
      {/* Nothing on the field, nothing in play: the panel is an empty box with a
          「0」 count, so it stays away until a card or a live event exists. The
          event chips live in here too, so the panel opens for them on their own. */}
      {(fieldCount > 0 || (S.eventActive?.length ?? 0) > 0) && (
        <div className={s.field}>
          {fieldCount > 0 && (
            <>
              <div className={s.fieldHead}><b>{tr("board.field")}</b><span>{tr("board.fieldHint")}</span><small>{tr("board.fieldCount", { n: fieldCount })}</small></div>
              <div className={s.fieldRows}>
                {rows.map(([i, f]) => (
                  <div key={i} className={s.fieldRow} style={{ borderLeftColor: m.colorOf(i) }}>
                    <div className={s.who}><Avatar c={m.charOf(i)} size={30} /><span>{m.nameOf(i)}</span></div>
                    <div className={s.fieldCards}>
                      {f.map((fc) => (
                        <button key={fc.uid} type="button" className={s.fieldCard} onClick={() => showCard(fc.card, [], fmtMsg(fc.note, namesOf(S)))}>
                          <img src={cardArt(fc.card)} alt="" />
                          <div><b>{fc.faceDown ? tr("board.faceDown") : cardTitle(fc.card)}</b><small>{fc.note ? fmtMsg(fc.note, namesOf(S)) : fc.crystals ? tr("board.crystals", { n: fc.crystals }) : ""}</small></div>
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
          {S.eventActive?.length > 0 && (
            <div className={s.eventChips}>
              {S.eventActive.slice(0, 4).map((e) => <button key={e.id} type="button" className={s.eventChip} onClick={() => showEvent(e.id, fmtMsg(e.note, namesOf(S)))}>{tr("events.label", { id: e.id })}{e.counter ? ` ×${e.counter}` : ""}</button>)}
            </div>
          )}
        </div>
      )}
      {/* The shared event deck stays on the board; your own draw pile sits at
          the head of your hand in the right column (Side.tsx). */}
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
