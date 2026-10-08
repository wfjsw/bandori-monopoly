// The 60-tile ring, tokens, and the center: cards in play, the event deck, banner.

import { cardArt, charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { useRef, useState } from "react";
import { D, cardTitle } from "../../core/data";
import { plain } from "../../core/format";
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

/** The cached TTS board uses a 12 x 10 grid of square cells. These 60
 *  coordinates match its board object 53c41e / Lua pathXY, starting at
 *  CiRCLE in the bottom-right and travelling counter-clockwise. */
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

const cellX = (col: number) => (col + 0.5) * (RING / COLS);
const cellY = (row: number) => (row + 0.5) * (HEIGHT / ROWS);

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
      <div className={s.ring} ref={ringRef} style={{ backgroundImage: 'url("/assets/tts/board.png")' }}>
        <Center m={m} />
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
    <div className={s.inner}>
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
