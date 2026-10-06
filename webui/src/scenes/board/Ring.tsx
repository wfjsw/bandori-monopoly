// The 60-tile ring, tokens, and the center: field panel, card piles, banner.

import { cardArt, charArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { plain } from "../../core/format";
import type { TileData } from "../../core/types";
import { CardFace, showCard } from "../../ui/Card";
import { Avatar } from "../../ui/Character";
import { toast } from "../../ui/Toast";
import type { Animator } from "./anim";
import type { Model } from "./model";
import { showDiscards, showEvent, showEventPile } from "./Popups";
import s from "./Ring.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";

const SIDE = 15; // tiles per side (60 = 4 x 15)
const CORNER = 78;
const CELL = 50;
const RING = CORNER * 2 + CELL * 14; // 856
const cornerRibbon = (kind: string) => ({ edogawa: tr("deed.draw"), cafe: tr("board.cornerEvent"), ryuseido: tr("board.cornerEvent") } as Record<string, string>)[kind];

/** Grid cell (1-based row, col) of tile `i` on the 16 x 16 ring. 0 = bottom right, clockwise. */
function cell(i: number): [number, number] {
  const side = Math.floor(i / SIDE);
  const k = i % SIDE;
  switch (side) {
    case 0: return [16, 16 - k];
    case 1: return [16 - k, 1];
    case 2: return [1, 1 + k];
    default: return [1 + k, 16];
  }
}

/** Pixel center of grid row/col `n` (1..16). */
const center = (n: number) => (n === 1 ? CORNER / 2 : n === 16 ? RING - CORNER / 2 : CORNER + (n - 2) * CELL + CELL / 2);

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
        <Center m={m} anim={anim} />
        {D.tiles.map((t, i) => {
          const [r, c] = cell(i);
          const owner = S.owners[i] ?? -1;
          const cls = cx(s.tile, owner >= 0 && s.owned, S.mortgaged[i] && s.mortgaged, S.phase === "play" && S.landed === i && s.landed, pick.has(i) && s.pickable);
          const style = { gridRow: r, gridColumn: c, ["--owner" as string]: owner >= 0 ? m.colorOf(owner) : "transparent" };
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
          return i % SIDE === 0
            ? <CornerTile key={i} t={t} cls={cls} style={style} onClick={() => onTile(i)}>{extras}</CornerTile>
            : <SideTile key={i} t={t} side={Math.floor(i / SIDE)} cls={cls} style={style} onClick={() => onTile(i)}>{extras}</SideTile>;
        })}
        <div className={s.tokens}>
          {S.players.map((x, i) => {
            if (x.bankrupt || x.left) return null;
            const p = pos[i] ?? x.pos;
            const [r, c] = cell(p);
            const same = S.players.map((_, k) => k).filter((k) => !S.players[k].bankrupt && !S.players[k].left && (pos[k] ?? S.players[k].pos) === p);
            const k = Math.max(0, same.indexOf(i));
            const n = same.length;
            const dx = n > 1 ? (k - (n - 1) / 2) * Math.min(14, 40 / (n - 1)) : 0;
            const ch = m.charOf(i);
            const hop = anim.hop?.playerId === i ? anim.hop.id : 0;
            const top = center(r) + 16;
            return (
              <div key={i} className={cx(s.token, i === S.turn && s.current)} style={{ left: center(c) + dx, top, zIndex: Math.round(top) }} title={x.player}>
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
            <span>{tr(anim.phase.key)}</span>
          </div>
        )}
        {anim.reveal && <div className={cx(s.reveal, anim.reveal.out && s.revealOut)}><CardFace id={anim.reveal.card} size="big" /></div>}
      </div>
    </div>
  );
}

interface TileProps { t: TileData; cls: string; style: React.CSSProperties; onClick: () => void; children: React.ReactNode }

function CornerTile({ t, cls, style, onClick, children }: TileProps) {
  const art = sceneImg(`area_${t.area}`);
  const ribbon = cornerRibbon(t.kind);
  return (
    <div className={cx(cls, s.corner, t.kind === "circle" && s.circle)} style={style} onClick={onClick}>
      {ribbon && <div className={s.ribbon}>{ribbon}</div>}
      {art && <img className={s.cornerArt} src={art} alt="" />}
      <div className={s.cornerName}>{plain(t.name)}</div>
      {children}
    </div>
  );
}

function SideTile({ t, side, cls, style, onClick, children }: TileProps & { side: number }) {
  const ring = t.kind === "ring";
  return (
    <div className={cx(cls, s[`s${side}`])} style={style} onClick={onClick}>
      <div className={s.bar} style={{ background: t.color }} />
      <div className={s.body}>
        <div className={cx(s.name, ring && s.ringName)}>{ring ? "RiNG" : t.shortName || t.name}</div>
        {t.kind === "agent" ? <div className={s.agent}>{tr("deed.dealer")}</div> : t.price > 0 && <div className={s.price}>{t.price}</div>}
      </div>
      {children}
    </div>
  );
}

function Center({ m, anim }: { m: Model; anim: Animator }) {
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
  const discards = S.players.reduce((a, x) => a + x.discard.length, 0);
  const lastPlayed = [...S.events].reverse().find((e) => (e.type === "play" || e.type === "discard") && e.card)?.card ?? "";
  const top = anim.lastDiscard || lastPlayed;
  return (
    <div className={s.inner} style={{ backgroundImage: `url("${sceneImg("world_map")}")` }}>
      <div className={s.field}>
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
        {S.eventActive?.length > 0 && (
          <div className={s.eventChips}>
            {S.eventActive.slice(0, 4).map((e) => <button key={e.id} type="button" className={s.eventChip} onClick={() => showEvent(e.id, fmtMsg(e.note, namesOf(S)))}>{tr("events.label", { id: e.id })}{e.counter ? ` ×${e.counter}` : ""}</button>)}
          </div>
        )}
      </div>
      <div className={s.piles}>
        <button type="button" className={s.pile} onClick={() => showEventPile(S)}>
          <div className={s.stack}><img src={sceneImg("card_back")} alt="" /></div>
          <div className={s.pileLabel}>{tr("board.eventDeck")}<b>×{S.eventDeck}</b></div>
        </button>
        <button type="button" className={s.pile} onClick={() => toast(tr("board.drawLeft", { n: m.me.draw }))}>
          <div className={s.stack}><img src={sceneImg("card_back")} alt="" /></div>
          <div className={s.pileLabel}>{tr("board.drawPile")}<b>×{m.me.draw}</b></div>
        </button>
        <button type="button" className={s.pile} onClick={() => showDiscards(m)}>
          <div className={s.stack}>{top && discards ? <img className={s.face} src={cardArt(top)} alt="" /> : <div className={s.pileEmpty}>{tr("common.empty")}</div>}</div>
          <div className={s.pileLabel}>{tr("board.discardPile")}<b>×{discards}</b></div>
        </button>
      </div>
      <div className={s.hint}>{tr("board.ringHint")}</div>
    </div>
  );
}
