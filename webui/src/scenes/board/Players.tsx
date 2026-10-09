// Player panels on the left; the match log is placed in the right column.

import { stateOf, stateMax } from "../../core/names";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cardTitle } from "../../core/data";
import { cx } from "../../core/cx";
import { n0 } from "../../core/format";
import { Avatar } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import type { LogLine } from "./anim";
import type { Model } from "./model";
import type { MatchPlayer } from "../../core/types";
import s from "./Players.module.css";
import { byTitle, showGraveyard, showPlayerInfo } from "./Popups";
import { t as tr } from "../../i18n/t";

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

/** The turn clock of the player whose turn it is: the 20s shield, then the
 *  60s bank. Solo has no deadlines (the engine never expires one), and a
 *  machine-driven turn has none either, so both show no clock. */
function timerOf(m: Model, solo: boolean, elapsed: number): { value: string; caption: string; frac: number; cls: string } | null {
  const S = m.S;
  const cur = S.players[S.turn];
  if (solo || S.phase !== "play" || S.turn < 0 || !cur || cur.ai) return null;
  const e = !S.busy && !m.asking ? elapsed : 0;
  const shield = Math.max(0, S.shield - e);
  const bank = Math.max(0, S.bank - Math.max(0, e - S.shield));
  if (shield > 0) return { value: String(Math.ceil(shield)), caption: tr("board.shield"), frac: Math.min(1, shield / 20), cls: s.shield };
  return { value: String(Math.ceil(bank)), caption: tr("board.remain"), frac: Math.min(1, bank / 60), cls: bank < 10 ? s.low : "" };
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

export function Players({ m, solo, elapsed }: { m: Model; solo: boolean; elapsed: number }) {
  const S = m.S;
  const n = S.players.length;
  const gap = n > 6 ? 5 : 8;
  const height = Math.min(70, Math.floor((548 - gap * (n - 1)) / n));
  const t = timerOf(m, solo, elapsed);
  const order = turnOrder(S);
  // Graveyard hover peek (like the draw pile's in Side): which seat, and its
  // panel's row so the peek lines up beside it.
  const [peek, setPeek] = useState<number | null>(null);
  const peekRow = peek == null ? -1 : order.indexOf(peek);
  const peekCards = peek == null ? [] : byTitle(S.players[peek]?.discard ?? []);
  return (
    <div className={s.players} style={{ gap }}>
      {order.map((i) => {
        const x = S.players[i];
        const c = m.charOf(i);
        const out = x.bankrupt || x.left;
        const status = statusChips(x);
        // The seat number doubles as the turn clock while it is this seat's turn.
        const clock = i === S.turn ? t : null;
        return (
          <button key={i} type="button" style={{ height }} className={cx(s.panel, i === m.playerId && s.me, i === S.turn && s.turn, out && s.out, height < 58 && s.compact)} onClick={() => showPlayerInfo(m, i)}>
            {clock ? (
              <span className={cx(s.n, s.clock, clock.cls)} style={{ ["--frac" as string]: clock.frac }} title={clock.caption}>{clock.value}</span>
            ) : (
              <span className={s.n}>{i + 1}</span>
            )}
            <div className={s.av}>
              <Avatar c={c} size={height < 58 ? 40 : 54} />
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
              <b>{c?.display.replace(/[（(]CRYCHIC[）)]$/i, "(c)") ?? "—"}</b>
            </div>
            <div className={s.money}><img src={sceneImg("icon_coin")} alt="" /><b>{n0(x.money)}</b></div>
            <div className={s.sub}>
              {status.map((t) => <span key={t} className={s.status}>{t}</span>)}
              <span className={s.hand}><Icon name="playing_cards" />{x.hand}</span>
              <span className={s.fire}><img src={sceneImg("icon_fire")} alt="" />{stateOf(x, "fire")}/{stateMax(x, "fire")}</span>
              {/* Per-player graveyard, at the right of the module (it used to be
                  one shared pile in the board centre). */}
              <span className={s.grave} title={tr("board.graveyardPile")} onMouseEnter={() => setPeek(i)} onMouseLeave={() => setPeek(null)} onClick={(e) => { e.stopPropagation(); setPeek(null); showGraveyard(m, i); }}>
                <img src={sceneImg("card_back")} alt="" /><b>{x.discard.length}</b>
              </span>
            </div>
            {out && <div className={s.outMark}>{x.bankrupt ? tr("board.bankrupt") : tr("board.forfeit")}</div>}
          </button>
        );
      })}
      {/* The hovered player's graveyard, alphabetical, beside their panel. */}
      <div className={cx(s.peek, peek != null && s.peekOn)} style={{ top: Math.max(0, peekRow) * (height + gap) }}>
        {peek != null && (
          <>
            <div className={s.peekHead}>{tr("board.graveyardPile")}<b>×{peekCards.length}</b></div>
            {peekCards.length
              ? peekCards.map((id, k) => <div key={k} className={s.peekRow}><img src={cardArt(id)} alt="" /><span>{cardTitle(id)}</span></div>)
              : <div className={s.peekEmpty}>{tr("graveyard.empty")}</div>}
          </>
        )}
      </div>
    </div>
  );
}

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

export function Log({ lines, colorOf, children }: { lines: LogLine[]; colorOf?: (playerId: number) => string; children?: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);
  const groups = turnGroups(lines);
  return (
    <div className={s.log}>
      <PanelTab><Icon name="history" />{tr("menu.history")}</PanelTab>
      {children}
      <div className={s.lines} ref={ref}>
        {groups.map((g) => {
          const bar = g.head && g.head.who != null && g.head.who >= 0 ? colorOf?.(g.head.who) : undefined;
          return (
            <div
              key={g.head?.id ?? "pre"}
              className={cx(g.head && s.turnGroup)}
              style={bar ? { ["--bar" as string]: bar } : undefined}
            >
              {g.head && <div className={s.groupHead}>{g.head.text}</div>}
              {g.body.map((l) => <div key={l.id} className={cx(l.stage && s.stageLine)}>{l.text}</div>)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
