// A prompt waiting on you (PromptView): choice, tile (also pickable on the
// board), mortgage, pick cards, auction. Reads the live prompt; closes itself
// when it is answered or replaced.

import { useEffect, useState } from "react";
import { cardArt } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { n0, plain } from "../../core/format";
import { useAutoplay, useMatchView, useTick } from "../../core/hooks";
import type { Command, MatchPrompt } from "../../core/types";
import type { Names } from "../../i18n/msg";
import type { GameSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { CardFace, showCard, TagChip } from "../../ui/Card";
import { bandColor } from "../../ui/Character";
import { SkillBody } from "../../ui/SkillBody";
import { TextInput } from "../../ui/Form";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { act } from "./model";
import s from "./Prompt.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg, type Msg } from "../../i18n/msg";
import { namesOf } from "../../core/names";

/** Is this prompt waiting on `playerId`? */
export function waitingOn(p: MatchPrompt, playerId: number): boolean {
  const k = p.players.indexOf(playerId);
  return p.id > 0 && k >= 0 && p.answers[k] < 0;
}

function Prompt({ sess, id, close }: { sess: GameSession; id: number; close: () => void }) {
  const { view, at } = useMatchView(sess);
  useTick(500);
  const auto = useAutoplay(sess); // 托管: answers are read-only
  const p = view?.state.prompt;
  const live = !!view && !!p && p.id === id && waitingOn(p, view.playerId);
  useEffect(() => {
    if (!live) close();
  }, [live, close]);
  if (!live || !view || !p) return null;
  const answer = (extra: Partial<Command>) => (auto ? Promise.resolve(false) : act(sess, { act: "answer", prompt: p.id, ...extra }));
  const left = Math.max(0, Math.ceil(p.timeLeft - (performance.now() - at) / 1000));

  let side = null;
  if (p.card) side = <div className={s.side}><div className={s.sideTitle}>{tr("prompt.related")}</div><CardFace id={p.card} size="big" className={s.sideCard} onClick={() => showCard(p.card)} /></div>;
  else if (/hand|mulligan/.test(p.title.k)) {
    side = (
      <div className={s.side}>
        <div className={s.sideTitle}>{tr("prompt.hand")}</div>
        <div className={s.sideCards}>{view.hand.map((h, k) => <CardFace key={k} id={h} size="mid" className={s.handCard} onClick={() => showCard(h)} />)}</div>
        <div className={s.sideHint}>{tr("prompt.handHint")}</div>
      </div>
    );
  }

  return (
    <div className={cx(s.prompt, side && s.withSide)}>
      {side}
      <div className={s.main}>
        {/* Solo has no answer deadline, so the clock is not shown at all. */}
        {sess.kind === "online" && (
          <div className={s.timer}><Icon name="timer" /><b>{left}</b>{tr("common.unitSec")}</div>
        )}
        <div className={s.text}>{fmtMsg(p.text, namesOf(view.state))}</div>
        <div className={s.options}>
          {p.kind === "tile" && <TileOptions p={p} answer={answer} names={namesOf(view.state)} auto={auto} />}
          {p.kind === "mortgage" && <MortgageOptions p={p} answer={answer} auto={auto} />}
          {p.kind === "pick" && <PickOptions p={p} answer={answer} auto={auto} />}
          {p.kind === "auction" && <Auction p={p} playerId={view.playerId} bidderName={p.bidder >= 0 ? view.state.players[p.bidder]?.player ?? "" : ""} answer={answer} auto={auto} />}
          {p.kind !== "pick" && isCardChoice(p) && <CardChoice p={p} answer={answer} auto={auto} names={namesOf(view.state)} />}
          {!["tile", "mortgage", "pick", "auction"].includes(p.kind) && !isCardChoice(p) && p.options.map((o, i) => (
            <Btn key={i} kind={i === 0 ? "pink" : "white"} className={s.opt} disabled={auto} onClick={() => void answer({ value: i })}>{fmtMsg(o, namesOf(view.state))}</Btn>
          ))}
        </div>
      </div>
    </div>
  );
}

type Answer = (extra: Partial<Command>) => Promise<boolean>;

function TileOptions({ p, answer, names, auto }: { p: MatchPrompt; answer: Answer; names: Names; auto: boolean }) {
  return (
    <>
      <p className={s.sub}>{tr("prompt.tapTiles")}</p>
      {p.options.map((o, i) => <Btn key={i} kind={i === 0 ? "pink" : "white"} className={s.opt} disabled={auto} onClick={() => void answer({ value: i })}>{fmtMsg(o, names)}</Btn>)}
      <Btn className={s.opt} disabled={auto} onClick={() => void answer({ value: p.items.length })}>{tr("prompt.none")}</Btn>
    </>
  );
}

function MortgageOptions({ p, answer, auto }: { p: MatchPrompt; answer: Answer; auto: boolean }) {
  const [picked, setPicked] = useState<string[]>([]);
  const sum = picked.reduce((a, t) => a + Math.floor(D.tiles[Number(t)].price / 2), 0);
  return (
    <>
      <div className={s.checks}>
        {p.items.map((t) => {
          const tile = D.tiles[Number(t)];
          return (
            <label key={t} className={s.check}>
              <input type="checkbox" disabled={auto} checked={picked.includes(t)} onChange={(e) => setPicked(e.target.checked ? [...picked, t] : picked.filter((x) => x !== t))} />
              <i style={{ background: tile.color }} />{plain(tile.name)}<b>+{n0(Math.floor(tile.price / 2))}</b>
            </label>
          );
        })}
      </div>
      <div className={s.sum}>{tr("prompt.selected")}<b className={sum >= p.bid ? s.ok : ""}>{n0(sum)}</b>{tr("prompt.need", { n: n0(p.bid) })}</div>
      <Btn kind="pink" className={s.opt} disabled={auto} onClick={() => void answer({ cards: picked })}>{tr("prompt.mortgage")}</Btn>
    </>
  );
}

function PickOptions({ p, answer, auto }: { p: MatchPrompt; answer: Answer; auto: boolean }) {
  const [picked, setPicked] = useState<number[]>([]);
  const toggle = (k: number) => setPicked(picked.includes(k) ? picked.filter((x) => x !== k) : picked.length < p.count ? [...picked, k] : picked);
  return (
    <>
      <CardGrid ids={p.items} picked={picked} onPick={(k) => !auto && toggle(k)} />
      <Btn kind="pink" className={s.opt} disabled={auto || picked.length !== p.count} onClick={() => void answer({ cards: picked.map((k) => p.items[k]) })}>{tr("prompt.pickCards", { n: picked.length, total: p.count })}</Btn>
    </>
  );
}

/** The card an option names (`ask.cardOption` and any other `{{card}}`-only label). */
function optionCard(o: Msg): string | null {
  const a = o.a ?? {};
  const keys = Object.keys(a);
  const v = keys.length === 1 ? a[keys[0]] : undefined;
  return v && "card" in v ? v.card : null;
}

/** Prompts whose options are cards: render them as cards, not as text buttons. */
export function isCardChoice(p: MatchPrompt): boolean {
  return p.kind === "pick" || (p.kind !== "tile" && p.kind !== "mortgage" && p.kind !== "auction" && p.options.some((o) => optionCard(o) !== null));
}

/** Single choice among cards: click selects (and pins the detail), confirm answers.
 *  Options that are not cards (e.g. 「不选」) stay buttons below. */
function CardChoice({ p, answer, auto, names }: { p: MatchPrompt; answer: Answer; auto: boolean; names: Names }) {
  const [sel, setSel] = useState<number | null>(null);
  const cards = p.options.map(optionCard);
  const ids = cards.filter((c): c is string => c !== null);
  const idx = cards.flatMap((c, i) => (c !== null ? [i] : []));
  const rest = cards.flatMap((c, i) => (c === null ? [i] : []));
  return (
    <>
      <CardGrid ids={ids} picked={sel === null ? [] : [sel]} onPick={(k) => !auto && setSel(k)} onConfirm={(k) => !auto && void answer({ value: idx[k] })} />
      <Btn kind="pink" className={s.opt} disabled={auto || sel === null} onClick={() => sel !== null && void answer({ value: idx[sel] })}>
        {sel === null ? tr("prompt.pickOne") : tr("prompt.pickThis", { card: cardTitle(ids[sel]) })}
      </Btn>
      {rest.map((i) => <Btn key={i} className={s.opt} disabled={auto} onClick={() => void answer({ value: i })}>{fmtMsg(p.options[i], names)}</Btn>)}
    </>
  );
}

/** Cards laid out like the hand, but larger, with a detail panel: hovering
 *  previews a card, clicking selects it and keeps it in the panel. */
function CardGrid({ ids, picked, onPick, onConfirm }: { ids: string[]; picked: number[]; onPick: (k: number) => void; onConfirm?: (k: number) => void }) {
  const [hover, setHover] = useState<number | null>(null);
  const focus = hover ?? picked[picked.length - 1] ?? null;
  const id = focus === null ? null : ids[focus];
  const c = id ? D.card(id) : undefined;
  return (
    <div className={s.cardChoice}>
      <div className={s.cardGrid}>
        {ids.map((cid, k) => (
          <CardFace
            key={`${cid}:${k}`}
            id={cid}
            size="hand"
            className={s.choiceCard}
            on={picked.includes(k)}
            onClick={() => onPick(k)}
            onMouseEnter={() => setHover(k)}
            onMouseLeave={() => setHover(null)}
          />
        ))}
      </div>
      <div className={s.detail} onDoubleClick={() => focus !== null && onConfirm?.(focus)}>
        {id && c ? (
          <>
            <div className={s.dArt} style={{ borderColor: bandColor(c.band) }}><img src={cardArt(id)} alt="" /></div>
            <div className={s.dTitle}>{cardTitle(id)}</div>
            {!!c.tags.length && <div className={s.dTags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
            <div className={s.dText}><SkillBody text={c.text} /></div>
            <button type="button" className={s.dMore} onClick={() => showCard(id)}>{tr("prompt.cardDetail")}</button>
          </>
        ) : <div className={s.dEmpty}>{tr("prompt.cardHover")}</div>}
      </div>
    </div>
  );
}

function Auction({ p, playerId, bidderName, answer, auto }: { p: MatchPrompt; playerId: number; bidderName: string; answer: Answer; auto: boolean }) {
  const min = p.bid <= 0 ? 100 : p.bid + 100;
  const [bid, setBid] = useState(min);
  useEffect(() => setBid((b) => Math.max(b, min)), [min]);
  return (
    <div className={s.auction}>
      <div className={s.auctionTop}>{p.bidder >= 0 ? <>{tr("prompt.bidTop")}<b>{n0(p.bid)}</b>{tr("prompt.bidderOf", { who: bidderName })}</> : tr("prompt.noBids")}</div>
      {p.bidder === playerId ? <p className={s.sub}>{tr("prompt.topBidder")}</p> : (
        <div className={s.bidRow}>
          <TextInput type="number" min={min} step={100} value={bid} disabled={auto} onChange={(e) => setBid(Number(e.target.value))} className={s.bidInput} />
          <Btn kind="pink" disabled={auto} onClick={() => void answer({ value: bid })}>{tr("prompt.bid")}</Btn>
          <Btn disabled={auto} onClick={() => void answer({ value: -1 })}>{tr("prompt.pass")}</Btn>
        </div>
      )}
    </div>
  );
}

export function openPrompt(sess: GameSession, p: MatchPrompt): void {
  openModal(fmtMsg(p.title, namesOf(sess.view?.state)) || tr("prompt.title"), (close) => <Prompt sess={sess} id={p.id} close={close} />, { closable: false, key: "prompt", ...(isCardChoice(p) ? { size: "wide" as const } : {}) });
}
