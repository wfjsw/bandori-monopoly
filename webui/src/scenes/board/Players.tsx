// Player panels on the left; the match log is placed in the right column.

import { useCompactValue } from "../../app/Stage";
import { COMPACT_PLAYERS_H, COMPACT_SEAT_H } from "../../styles/layout";
import { fireMaxOf, fireOf, stateOf } from "../../core/names";
import { NO_HAND_LIMIT } from "../../core/hand";
import { memo, useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { sceneImg } from "../../core/assets";
import { cardTitle, cardArtUrl } from "../../core/data";
import { cx } from "../../core/cx";
import { n0 } from "../../core/format";
import { useTick } from "../../core/hooks";
import { useScrollFollow } from "../../hooks/measure";
import { Avatar } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { showCard } from "../../ui/Card";
import { stickCard } from "../../ui/CardPreview";
import type { LogLine } from "./anim";
import type { Model } from "./model";
import type { MatchPlayer } from "../../core/types";
import s from "./Players.module.css";
import { borrowedBand, byTitle, cardKeys, showGraveyard, showPlayerInfo } from "./Popups";
import { t as tr } from "../../i18n/t";
import { counterLabel, stateLabel } from "../../i18n/stateLabel";
import { setHoverSeat } from "./hoverSeat";
import { holdPeek } from "../../hooks/hold";
import { clockSync, displayRemainingMs } from "../../game/clockSync";

/** A player's status chips: [停留] / [眩晕] / [除外] / 托管 (the engine's takeover). */
export function statusChips(x: MatchPlayer): string[] {
  const out = x.bankrupt || x.left;
  return [
    stateOf(x, "stay") ? tr("board.stayN", { n: stateOf(x, "stay") }) : "",
    stateOf(x, "stun") + stateOf(x, "stunStart") ? tr("board.stunned") : "",
    stateOf(x, "exile") ? tr("board.exiled") : "",
    x.ai && !x.bot && !out ? tr("board.afk") : "",
  ].filter(Boolean);
}

/** The turn clock of the player whose turn it is: **bonus first, then base**
 *  (`docs/SERVER.md` §Turn clock). Solo has no deadlines (the engine never
 *  expires one), and a machine-driven turn has none either, so both show no
 *  clock. Online, the server publishes absolute deadlines (`m.v.clock`) on its
 *  monotonic clock; remaining is `deadline - (performance.now() + offset)` --
 *  never counted from the view's arrival -- so animation lag cannot eat the
 *  display. While the clock is held (`clockPaused` / no deadline) it sits at
 *  full. `bonusDeadlineMs` is when the bonus pool empties (the sooner of the
 *  two); `baseDeadlineMs` is when the whole clock is gone. */
function timerOf(m: Model, solo: boolean, elapsed: number): { value: string; caption: string; frac: number; cls: string } | null {
  const S = m.S;
  const cur = S.players[S.turn];
  if (solo || S.phase !== "play" || S.turn < 0 || !cur || cur.ai) return null;
  const fullBase = S.baseTime > 0 ? S.baseTime : 15;
  const fullBonus = S.bonusTime > 0 ? S.bonusTime : 30;
  const show = (secs: number, caption: string, cls: string, fracOf: number) =>
    ({ value: String(Math.max(0, Math.ceil(secs - 1e-9))), caption, frac: Math.min(1, Math.max(0, secs) / fracOf), cls });

  const clock = m.v.clock;
  if (clock) {
    clockSync.step();
    const now = performance.now();
    // Expiry epsilon: the display never races ahead of the server (which is
    // the sole authority for actually expiring a turn).
    const baseMs = displayRemainingMs(clock.baseDeadlineMs, now, clockSync.offset);
    const bonusMs = displayRemainingMs(clock.bonusDeadlineMs, now, clockSync.offset);
    const rawBase = clock.baseDeadlineMs - clockSync.toServer(now);
    const rawBonus = clock.bonusDeadlineMs - clockSync.toServer(now);
    // Bonus drains first: show 免费时间 while the bonus pool has anything left,
    // then 恢复时间 for the base pool underneath it.
    if (rawBonus > 0 && bonusMs > 0) return show(bonusMs / 1000, tr("board.bonusTime"), bonusMs < 10_000 ? s.low : "", fullBonus);
    if (rawBase > 0 && baseMs > 0) return show(baseMs / 1000, tr("board.baseTime"), s.base, fullBase);
    // At / past the local estimate of the deadline: hold at 0 and say so --
    // the server may not have expired the turn yet.
    return show(0, tr("board.waitingServer"), s.low, fullBonus);
  }

  // Held (paused) or a path with no deadline (replay / older server): show
  // the snapshot's remaining, frozen while busy/asking. When held at full,
  // `clockPaused` pins the display to the un-consumed bonus/base (bonus first).
  if (S.clockPaused) {
    if (S.bonusTime > 0) return show(S.bonusTime, tr("board.bonusTime"), s.low, fullBonus);
    return show(S.baseTime || fullBase, tr("board.baseTime"), s.base, fullBase);
  }
  const e = !S.busy && !m.asking ? elapsed : 0;
  const bonus = Math.max(0, S.bonusTime - e);
  const base = Math.max(0, S.baseTime - Math.max(0, e - S.bonusTime));
  if (bonus > 0) return show(bonus, tr("board.bonusTime"), bonus < 10 ? s.low : "", fullBonus);
  return show(base, tr("board.baseTime"), s.base, fullBase);
}

/** Seats in the order they act from now: the current turn first, then the
 *  next and so on (seat order wraps around). Seats that are out sink to the
 *  bottom. Before the first turn it is plain seat order. The badge keeps the
 *  seat number, so a panel moving does not renumber it. */
function turnOrder(S: Model["S"]): number[] {
  const n = S.players.length;
  const first = S.turn >= 0 ? S.turn : 0;
  const seats = Array.from({ length: n }, (_, k) => (first + k) % n);
  const out = (i: number) => S.players[i].bankrupt || S.players[i].left;
  return [...seats.filter((i) => !out(i)), ...seats.filter(out)];
}

/** Every marker a player holds: counter units, keyed state values, plus each field card's crystals, CP and named counters.
 *  `n` is `∞` for a lifted hand limit (`NO_HAND_LIMIT`), absent for a phrase that carries no count.
 *  A row whose name has no display phrase is dropped -- never printed as its wire key. */
function markersOf(x: MatchPlayer | undefined): { label: string; n?: number | "∞" }[] {
  if (!x) return [];
  const out: { label: string; n?: number | "∞" }[] = [];
  for (const tok of x.tokens ?? []) {
    if (tok.value <= 0) continue;
    const label = stateLabel(tok.name);
    if (label) out.push({ label, n: tok.value });
  }
  for (const [key, v] of Object.entries(x.state ?? {})) {
    const n = typeof v === "number" ? v : (v?.value ?? 0);
    if (n <= 0) continue;
    const label = stateLabel(key);
    if (!label) continue;
    // `handLimit`'s i32-max sentinel is 「手牌数没有上限」, not a count.
    out.push({ label, n: key === "handLimit" && n === NO_HAND_LIMIT ? "∞" : n });
  }
  for (const fc of x.field ?? []) {
    const card = cardTitle(fc.card);
    // Crystals and CP have their own labels; the walk below is for the
    // instance's *other* named counters, so the two are not listed twice.
    const crystals = fc.counters["crystals"] ?? 0;
    const cp = fc.counters["cp"] ?? 0;
    if (crystals > 0) out.push({ label: `${card} · ${tr("board.crystalsLabel")}`, n: crystals });
    if (cp > 0) out.push({ label: `${card} · CP`, n: cp });
    for (const [name, n] of Object.entries(fc.counters ?? {})) {
      if (n <= 0 || name === "crystals" || name === "cp") continue;
      const label = counterLabel(name);
      if (label) out.push({ label: `${card} · ${label}`, n });
    }
  }
  return out;
}

/**
 * Player panels on the left. The turn clock repaints on its own 500ms tick
 * (`at` is the view's arrival time and is stable per snapshot), so the parent
 * board's animation / timer re-renders do not drag the seat list along.
 */
export const Players = memo(function Players({ m, solo, at }: { m: Model; solo: boolean; at: number }) {
  useTick(500);
  const elapsed = (performance.now() - at) / 1000;
  const S = m.S;
  // The seat list's height: 548 on the 900px stage, what the phone canvas leaves otherwise.
  const compact = useCompactValue();
  const avail = compact ? COMPACT_PLAYERS_H : 548;
  const n = S.players.length;
  const gap = n > 6 ? 5 : 8;
  // Seats shrink to fit only down to a floor; past that the list scrolls
  // instead of squeezing every row until its content is unreadable.
  const maxH = compact ? COMPACT_SEAT_H : 70;
  const minH = compact ? COMPACT_SEAT_H : 58;
  const height = Math.max(minH, Math.min(maxH, Math.floor((avail - gap * (n - 1)) / n)));
  const t = timerOf(m, solo, elapsed);
  const order = turnOrder(S);
  // Graveyard hover peek (like the draw pile's in Side): which seat, and its
  // panel's row so the peek lines up beside it.
  const [peek, setPeek] = useState<number | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const peekRow = peek == null ? -1 : order.indexOf(peek);
  const [firePeek, setFirePeek] = useState<number | null>(null);
  const fireRow = firePeek == null ? -1 : order.indexOf(firePeek);
  const fireMarks = firePeek == null ? [] : markersOf(S.players[firePeek]);
  const peekCards = peek == null ? [] : byTitle(S.players[peek]?.discard ?? []);
  const peekKeys = cardKeys(peekCards);
  // The panels unmount-proof the highlight: clear it when this column goes away.
  useEffect(() => () => setHoverSeat(-1), []);
  return (
    <div className={s.playersWrap}>
    <div className={s.players} style={{ gap }} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
      {order.map((i) => {
        const x = S.players[i];
        const c = m.charOf(i);
        const out = x.bankrupt || x.left;
        const status = statusChips(x);
        // The acting seat's turn clock rides its avatar, for every viewer; the
        // seat number keeps its own badge.
        const clock = i === S.turn ? t : null;
        return (
          <button key={i} type="button" style={{ height }} className={cx(s.panel, i === m.playerId && s.me, i === S.turn && s.turn, out && s.out, height < 58 && s.compact)} onClick={() => showPlayerInfo(m, i)} {...holdPeek(() => setHoverSeat(i), () => setHoverSeat(-1))}>
            <span className={s.n}>{i + 1}</span>
            <div className={s.av}>
              <Avatar c={c} size={height < 58 ? 40 : 54} />
              {clock && (
                <span className={cx(s.clock, clock.cls)} style={{ ["--frac" as string]: clock.frac }} title={clock.caption}>{clock.value}</span>
              )}
              {x.bot && (
                <span
                  className={cx(s.bot, x.mentality === "chaos" && s.chaos)}
                  title={x.mentality === "chaos" ? tr("solo.mentalityChaos") : x.mentality === "advanced" ? tr("solo.mentalityAdvanced") : tr("solo.mentalityStandard")}
                >
                  <Icon name={x.mentality === "chaos" ? "cyclone" : "smart_toy"} />
                </span>
              )}
            </div>
            <div className={s.who}>
              <b>{c ? (c.variantTag ? `${c.variantOf}(${c.variantTag})` : c.display) : "—"}</b>
            </div>
            <div className={s.money}><img src={sceneImg("icon_coin")} alt="" /><b>{n0(x.money)}</b></div>
            <div className={s.sub}>
              {x.player && <span className={s.nick} title={x.player}>{x.player}</span>}
              {status.map((t) => <span key={t} className={s.status}>{t}</span>)}
              {(() => {
                const bb = borrowedBand(x);
                const who = bb ? m.charOf(bb.from)?.display ?? "" : "";
                return bb && <span className={s.status} title={tr("board.borrowedTip", { skill: cardTitle(bb.skill), who })}>{tr("board.borrowed", { who })}</span>;
              })()}
              <span className={s.hand}><Icon name="playing_cards" />{x.hand}</span>
              <span className={s.fire} {...holdPeek(() => setFirePeek(i), () => setFirePeek(null))}><img src={sceneImg("icon_fire")} alt="" />{fireOf(x)}/{fireMaxOf(x)}</span>
              {/* Per-player graveyard, at the right of the module (it used to be
                  one shared pile in the board centre). */}
              <span className={s.grave} title={tr("board.graveyardPile")} {...holdPeek(() => setPeek(i), () => setPeek(null))} onClick={(e) => { e.stopPropagation(); setPeek(null); showGraveyard(m, i); }}>
                <img src={sceneImg("card_back")} alt="" /><b>{x.discard.length}</b>
              </span>
            </div>
            {out && <div className={s.outMark}>{x.bankrupt ? tr("board.bankrupt") : tr("board.forfeit")}</div>}
          </button>
        );
      })}
    </div>
      {/* The hovered player's graveyard, alphabetical, beside their panel. Outside
          the scrolling list, which would clip it. */}
      <div className={cx(s.peek, firePeek != null && s.peekOn)} style={{ top: 5 + Math.max(0, fireRow) * (height + gap) - scrollTop }}>
        {firePeek != null && (
          <>
            <div className={s.peekHead}>{tr("board.markers")}<b>×{fireMarks.length}</b></div>
            {fireMarks.length
              ? fireMarks.map((mk) => <div key={mk.label} className={s.peekRow}><span>{mk.label}</span>{mk.n != null && <b>{mk.n}</b>}</div>)
              : <div className={s.peekEmpty}>{tr("board.markersEmpty")}</div>}
          </>
        )}
      </div>
      <div className={cx(s.peek, peek != null && s.peekOn)} style={{ top: 5 + Math.max(0, peekRow) * (height + gap) - scrollTop }}>
        {peek != null && (
          <>
            <div className={s.peekHead}>{tr("board.graveyardPile")}<b>×{peekCards.length}</b></div>
            {peekCards.length
              ? peekCards.map((id, k) => <div key={peekKeys[k]} className={s.peekRow}><img src={cardArtUrl(id)} alt="" /><span>{cardTitle(id)}</span></div>)
              : <div className={s.peekEmpty}>{tr("graveyard.empty")}</div>}
          </>
        )}
      </div>
    </div>
  );
});

/** Group the log per turn: each 「第 N 轮 · … 的回合」 heading opens a block
 *  whose entries follow it, barred in that player's colour. Setup lines before
 *  the first turn stay unheaded. */
function turnGroups(lines: LogLine[]): { head?: LogLine; body: LogLine[] }[] {
  const groups: { head?: LogLine; body: LogLine[] }[] = [];
  for (const l of lines) {
    if (l.turn) groups.push({ head: l, body: [] });
    else if (groups.length) groups[groups.length - 1].body.push(l);
    else (groups[0] ??= { body: [] }).body.push(l);
  }
  return groups;
}

/** A card name inside a log line: hover shows the card's details, click opens
 *  the full card view. */
const CardRef = memo(function CardRef({ card, text, onHover, onLeave }: { card: string; text: string; onHover: (id: string, el: HTMLElement) => void; onLeave: () => void }) {
  return (
    <span
      className={s.cardRef}
      onMouseEnter={(e) => onHover(card, e.currentTarget)}
      onMouseLeave={onLeave}
      onClick={(e) => {
        e.stopPropagation();
        showCard(card);
      }}
    >
      {text}
    </span>
  );
});

export const Log = memo(function Log({ lines, colorOf, children, titled = true }: { lines: LogLine[]; colorOf?: (playerId: number) => string; children?: ReactNode; titled?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  // The log follows its own tail: re-pin only when new lines arrive, so a
  // reader who scrolled up is not thrown back down by a hover re-render.
  useScrollFollow(ref, [lines]);
  // Hovered card reference: the standing card panel (right sidebar) shows it.
  // Stable identities, so the memoized card refs are not re-rendered by a
  // fresh closure on every log line the board appends.
  const onHover = useCallback((id: string) => { stickCard(id); }, []);
  const onLeave = useCallback(() => undefined, []);
  const groups = turnGroups(lines);
  return (
    <div className={s.log}>
      {titled && <PanelTab><Icon name="history" />{tr("menu.history")}</PanelTab>}
      {children}
      <div className={s.lines} ref={ref}>
        {groups.map((g) => {
          const bar = g.head && g.head.who != null && g.head.who >= 0 ? colorOf?.(g.head.who) : undefined;
          return (
            <div
              key={g.head ? `h${g.head.id}` : "pre"}
              className={cx(g.head && s.turnGroup)}
              style={bar ? { ["--bar" as string]: bar } : undefined}
            >
              {g.head && <div className={s.groupHead}>{g.head.text}</div>}
              {g.body.map((l, n) => (
                <div key={`${l.id}:${n}`} className={cx(l.stage && s.stageLine)}>
                  {l.parts?.length
                    ? l.parts.map((p, k) =>
                        typeof p === "string" ? (
                          <span key={k}>{p}</span>
                        ) : (
                          <CardRef
                            key={k}
                            card={p.card}
                            text={p.text}
                            onHover={onHover}
                            onLeave={onLeave}
                          />
                        ))
                    : l.text}
                </div>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
});