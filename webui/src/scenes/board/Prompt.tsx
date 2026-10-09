// A prompt waiting on you (PromptView): choice, tile (also pickable on the
// board), mortgage, pick cards, auction. Reads the live prompt; closes itself
// when it is answered or replaced. Rendered as the old prompt card: pink
// title, body, a thin timer bar, and outline pill options -- presented as the
// bottom sheet over the hand (Modal `chrome: "prompt"`): content-sized, up to
// the centre column, retractable to its title strip.

import { useEffect, useRef, useState } from "react";
import { cardArt } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle, cardText, cardColor } from "../../core/data";
import { n0, plain } from "../../core/format";
import { useAutoplay, useMatchView, useTick } from "../../core/hooks";
import type { Command, MatchPrompt } from "../../core/types";
import type { Names } from "../../i18n/msg";
import type { GameSession } from "../../game/session";
import { InspectCard, showCard, TagChip } from "../../ui/Card";
import { previewFrom, previewHide } from "../../ui/CardPreview";
import { bandColor } from "../../ui/Character";
import { SkillBody } from "../../ui/SkillBody";
import { TextInput } from "../../ui/Form";
import { openModal, useRaiseSheetOn } from "../../ui/Modal";
import { act } from "./model";
import s from "./Prompt.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg, type Msg } from "../../i18n/msg";
import { namesOf } from "../../core/names";
import { toast } from "../../ui/Toast";
import { msgCards, optionCard, returnsPickCount, returnsPickNumber, submitReturnsSelection } from "./returnsSelection";

/** Is this prompt waiting on `playerId`? */
export function waitingOn(p: MatchPrompt, playerId: number): boolean {
  const k = p.players.indexOf(playerId);
  return p.id > 0 && k >= 0 && p.answers[k] < 0;
}

/**
 * Compact prompts keep the small sheet (`fit`); the multi-select card grids
 * (Returns' eight-pick, `pick`) take the wide one -- a checkbox grid plus a
 * detail column wants the sheet up to the centre column's width.
 */
export function fitsInline(p: MatchPrompt): boolean {
  if (returnsPickNumber(p) !== null || p.kind === "pick") return false;
  // A very long card row also goes wide (8 tiles per row is the most the
  // 856px column can show beside the panel's padding).
  return p.options.filter((o) => msgCards(o).length > 0).length <= 8;
}

/** True for `ms` after `on` drops: feeds the sheet's slide-down with content. */
function useLinger(on: boolean, ms = 180): boolean {
  const [held, setHeld] = useState(on);
  useEffect(() => {
    if (on) {
      setHeld(true);
      return;
    }
    const t = window.setTimeout(() => setHeld(false), ms);
    return () => window.clearTimeout(t);
  }, [on, ms]);
  return on || held;
}

/**
 * The answer deadline, as the sheet strip's thin pink bar (online matches only;
 * solo has no deadline). Lives in the title strip so it stays visible when the
 * sheet is retracted -- and keeps running there.
 */
function PromptTimer({ sess }: { sess: GameSession }) {
  const { view, at } = useMatchView(sess);
  useTick(500);
  // The deadline is measured against the time the prompt was first shown.
  const total = useRef(0);
  const p = view?.state.prompt;
  if (sess.kind !== "online" || !view || !p) return null;
  const left = Math.max(0, Math.ceil(p.timeLeft - (performance.now() - at) / 1000));
  if (!total.current) total.current = Math.max(1, left);
  return (
    <div className={s.timerRow}>
      <div className={s.timerBar} role="progressbar" aria-valuemin={0} aria-valuemax={total.current} aria-valuenow={left}>
        <div className={s.timerFill} style={{ width: `${Math.min(1, left / total.current) * 100}%` }} />
      </div>
      <span className={s.timerNum}>{left}{tr("common.unitSec")}</span>
    </div>
  );
}

function Prompt({ sess, id, close }: { sess: GameSession; id: number; close: () => void }) {
  const { view } = useMatchView(sess);
  const auto = useAutoplay(sess); // 托管: answers are read-only
  const p = view?.state.prompt;
  // Keep one modal mounted through Returns' setup sequence. The player sees
  // one checklist and one confirmation, while the session advances its asks.
  const returnsGroup = useRef(returnsPickNumber(p) !== null).current;
  const live = !!view && !!p && (p.id === id || (returnsGroup && returnsPickNumber(p) !== null)) && waitingOn(p, view.playerId);
  useEffect(() => {
    if (!live) close();
  }, [live, close]);
  // A new question raises the sheet even mid-sequence (Returns' chained asks).
  useRaiseSheetOn(live ? p?.id : id);
  // Keep the panel's content through the close slide (the options go inert).
  const showing = useLinger(live);
  if (!showing || !view || !p) return null;
  const answer = (extra: Partial<Command>) => (auto ? Promise.resolve(false) : act(sess, { act: "answer", prompt: p.id, ...extra }));

  const bodyCards = uniqCards([...(p.card ? [p.card] : []), ...msgCards(p.title), ...msgCards(p.text)]);

  // One layout rule for every prompt that shows cards: a panel that is just a
  // row of card faces (the hand), or more than one related-card detail, stacks
  // ABOVE the question and lays its cards out across the width -- a 220px side
  // column forced pairs to stack into a tall thin strip of wasted space. A
  // single related-card detail keeps the side column (it reads as a caption).
  const showsHand = /hand|mulligan/.test(p.title.k) && !p.options.some((o) => msgCards(o).length);
  const relatedCards = showsHand ? [] : bodyCards;
  const stacked = showsHand || relatedCards.length > 1;

  let side = null;
  if (showsHand) {
    side = (
      <div className={cx(s.side, s.sideTop)}>
        <div className={s.sideTitle}>{tr("prompt.hand")}</div>
        <div className={s.cardStrip}>{view.hand.map((h, k) => <InspectCard key={k} id={h} size="tile" />)}</div>
        <div className={s.sideHint}>{tr("prompt.handHint")}</div>
      </div>
    );
  } else if (relatedCards.length) {
    side = (
      <div className={cx(s.side, stacked && s.sideTop)}>
        <div className={s.sideTitle}>{tr("prompt.related")}</div>
        <div className={stacked ? s.relatedRow : undefined}>
          {relatedCards.map((cid) => {
            const c = D.card(cid);
            return (
              <div
                key={cid}
                className={s.sideCardWrap}
                onMouseEnter={(e) => previewFrom(e.currentTarget, cid)}
                onMouseLeave={previewHide}
                onFocus={(e) => previewFrom(e.currentTarget, cid)}
                onBlur={previewHide}
                tabIndex={0}
              >
                <div className={s.dArt} style={{ borderColor: cardColor(cid) }}>
                  <img src={cardArt(cid)} alt="" />
                </div>
                <div className={s.dTitle}>{cardTitle(cid)}</div>
                {!!c?.tags.length && <div className={s.dTags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
                <div className={s.dText}><SkillBody text={cardText(cid)} /></div>
                <button type="button" className={s.dMore} onClick={() => showCard(cid)}>{tr("prompt.cardDetail")}</button>
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className={cx(s.prompt, side && s.withSide, stacked && s.stacked)}>
      {side}
      <div className={s.main}>
        {!returnsGroup && <div className={s.text}>{fmtMsg(p.text, namesOf(view.state))}</div>}
        <div className={s.options}>
          {returnsGroup && <ReturnsOptions p={p} sess={sess} auto={auto} />}
          {p.kind === "tile" && <TileOptions p={p} answer={answer} names={namesOf(view.state)} auto={auto} />}
          {p.kind === "mortgage" && <MortgageOptions p={p} answer={answer} auto={auto} names={namesOf(view.state)} />}
          {p.kind === "pick" && <PickOptions p={p} answer={answer} auto={auto} />}
          {p.kind === "auction" && <Auction p={p} playerId={view.playerId} bidderName={p.bidder >= 0 ? namesOf(view.state).playerId(p.bidder) : ""} answer={answer} auto={auto} />}
          {p.kind !== "tile" && p.kind !== "mortgage" && p.kind !== "pick" && p.kind !== "auction" && !returnsGroup && (
            <Options p={p} answer={answer} auto={auto} names={namesOf(view.state)} />
          )}
        </div>
      </div>
    </div>
  );
}

type Answer = (extra: Partial<Command>) => Promise<boolean>;

/** First occurrence of each card id, order preserved. */
function uniqCards(ids: string[]): string[] {
  return ids.filter((id, k) => id && ids.indexOf(id) === k);
}

/**
 * One option pill -- a text-only option (「不选」, 「返回」, a buy, a direction).
 * Card-bearing options are not pills: they are card tiles in the row above.
 */
function OptionPill({ o, names, disabled, onAnswer }: {
  o: Msg; names: Names; disabled: boolean; onAnswer: () => void;
}) {
  return (
    <button type="button" className={s.opt} disabled={disabled} onClick={onAnswer}>
      {fmtMsg(o, names)}
    </button>
  );
}

/**
 * Every option laid out as the prompt's card row plus a pill row:
 *   * an option that names a card becomes a uniform card tile (art + the card
 *     title + the option's own caption, e.g. the player name), hover previews
 *     it, clicking the face inspects it, clicking the caption answers;
 *   * an option that names no card stays an outline pill in the row beneath.
 * Single-select only -- the multi-select grids (Returns' eight-pick) keep
 * their checkbox grid and confirm.
 */
function Options({ p, answer, auto, names }: { p: MatchPrompt; answer: Answer; auto: boolean; names: Names }) {
  const carded = p.options.flatMap((o, i) => {
    const id = msgCards(o)[0];
    return id ? [{ o, i, id }] : [];
  });
  const plain = p.options.flatMap((o, i) => (msgCards(o).length ? [] : [{ o, i }]));
  return (
    <>
      {carded.length > 0 && (
        <>
          <p className={s.sub}>{tr("prompt.pickOne")}</p>
          <div className={s.cardRow}>
            {carded.map(({ o, i, id }) => {
              const label = fmtMsg(o, names);
              // A `{{card}}`-only label repeats the card's own title, so the
              // caption falls back to a plain 「选择」.
              const caption = label === cardTitle(id) ? tr("prompt.choose") : label;
              return (
                <div key={i} className={s.cardOpt}>
                  <InspectCard
                    id={id}
                    size="tile"
                    className={s.cardOptFace}
                    actions={[{
                      label: tr("prompt.pickThis", { card: cardTitle(id) }),
                      enabled: !auto,
                      kind: "pink",
                      run: () => answer({ value: i }),
                    }]}
                  />
                  <button type="button" className={s.cardOptCap} disabled={auto} onClick={() => void answer({ value: i })}>
                    {caption}
                  </button>
                </div>
              );
            })}
          </div>
        </>
      )}
      {plain.map(({ o, i }) => (
        <OptionPill key={i} o={o} names={names} disabled={auto} onAnswer={() => void answer({ value: i })} />
      ))}
    </>
  );
}

function ReturnsOptions({ p, sess, auto }: { p: MatchPrompt; sess: GameSession; auto: boolean }) {
  const seed = (prompt: MatchPrompt) => ({
    ids: prompt.options.map(optionCard).filter((c): c is string => c !== null),
    count: returnsPickCount(prompt),
  });
  const [pool, setPool] = useState(() => seed(p));
  const [picked, setPicked] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(0);
  const sending = useRef(false);
  const toggle = (k: number) => {
    if (auto || sending.current) return;
    setPicked((xs) => xs.includes(k) ? xs.filter((x) => x !== k) : xs.length < pool.count ? [...xs, k] : xs);
  };
  const submit = async () => {
    if (auto || sending.current || picked.length !== pool.count) return;
    sending.current = true;
    setBusy(true);
    setSent(0);
    try {
      const ok = await submitReturnsSelection({
        current: () => sess.view?.state.prompt,
        answer: (prompt, value) => act(sess, { act: "answer", prompt, value }),
        subscribe: (changed) => sess.subscribe(changed),
      }, picked.map((k) => pool.ids[k]), setSent);
      if (!ok) {
        const now = sess.view?.state.prompt;
        if (now && returnsPickNumber(now) !== null) {
          setPool(seed(now));
          setPicked([]);
          toast(tr("prompt.pickRetry"));
        }
      }
    } catch {
      toast(tr("prompt.pickRetry"), "error");
      const now = sess.view?.state.prompt;
      if (now && returnsPickNumber(now) !== null) {
        setPool(seed(now));
        setPicked([]);
      }
    } finally {
      sending.current = false;
      setBusy(false);
    }
  };
  return <>
    <div className={s.text}>{tr("prompt.addToDraw", { n: pool.count })}</div>
    <CardGrid ids={pool.ids} picked={picked} onPick={toggle} multiple disabled={auto || busy} />
    <button type="button" className={cx(s.opt, s.optMain, s.cardConfirm)} disabled={auto || busy || picked.length !== pool.count} onClick={() => void submit()}>
      <span className={s.confirmLabel}>{busy ? tr("prompt.addingToDraw", { n: sent, total: pool.count }) : tr("prompt.confirmDraw", { n: picked.length, total: pool.count })}</span>
    </button>
  </>;
}

function TileOptions({ p, answer, names, auto }: { p: MatchPrompt; answer: Answer; names: Names; auto: boolean }) {
  return (
    <>
      <p className={s.sub}>{tr("prompt.tapTiles")}</p>
      {p.options.map((o, i) => (
        <OptionPill key={i} o={o} names={names} disabled={auto} onAnswer={() => void answer({ value: i })} />
      ))}
      <button type="button" className={s.opt} disabled={auto} onClick={() => void answer({ value: p.items.length })}>{tr("prompt.none")}</button>
    </>
  );
}

function MortgageOptions({ p, answer, auto, names }: { p: MatchPrompt; answer: Answer; auto: boolean; names: Names }) {
  const [picked, setPicked] = useState<string[]>([]);
  const cancellable = p.options.length > 0;
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
      <button type="button" className={cx(s.opt, s.optMain)} disabled={auto || (cancellable && sum < p.bid)} onClick={() => void answer({ cards: picked })}>{tr("prompt.mortgage")}</button>
      {cancellable && <OptionPill o={p.options[0]} names={names} disabled={auto} onAnswer={() => void answer({ value: 1 })} />}
    </>
  );
}

function PickOptions({ p, answer, auto }: { p: MatchPrompt; answer: Answer; auto: boolean }) {
  const [picked, setPicked] = useState<number[]>([]);
  const toggle = (k: number) => setPicked(picked.includes(k) ? picked.filter((x) => x !== k) : picked.length < p.count ? [...picked, k] : picked);
  return (
    <>
      <CardGrid ids={p.items} picked={picked} onPick={(k) => !auto && toggle(k)} multiple disabled={auto} />
      <button type="button" className={cx(s.opt, s.optMain, s.cardConfirm)} disabled={auto || picked.length !== p.count} onClick={() => void answer({ cards: picked.map((k) => p.items[k]) })}>
        <span className={s.confirmLabel}>{tr("prompt.pickCards", { n: picked.length, total: p.count })}</span>
      </button>
    </>
  );
}

/** Prompts whose options are cards: they render as card tiles (or the
 *  multi-select grid), never as text-only buttons. */
export function isCardChoice(p: MatchPrompt): boolean {
  // This window chooses between a hand card and a paid skill. Keep the full
  // source labels together, including the fire cost, instead of presenting
  // only the card as a large card-picking panel.
  if (p.text.k === "ask.counteract.move_extension" || p.text.k === "ask.counteract.move_extension_payment") return false;
  return p.kind === "pick" || (p.kind !== "tile" && p.kind !== "mortgage" && p.kind !== "auction" && p.options.some((o) => optionCard(o) !== null));
}

/** Cards laid out like the hand, but larger, with a detail panel: hovering
 *  previews a card, clicking selects it and keeps it in the panel. The detail
 *  panel is the hover preview here, so the floating one is off. */
function CardGrid({ ids, picked, onPick, onConfirm, multiple = false, disabled = false }: { ids: string[]; picked: number[]; onPick: (k: number) => void; onConfirm?: (k: number) => void; multiple?: boolean; disabled?: boolean }) {
  const [hover, setHover] = useState<number | null>(null);
  const focus = hover ?? picked[picked.length - 1] ?? null;
  const id = focus === null ? null : ids[focus];
  const c = id ? D.card(id) : undefined;
  return (
    <div className={s.cardChoice}>
      <div className={s.cardGrid}>
        {ids.map((cid, k) => (
          <InspectCard
            key={`${cid}:${k}`}
            id={cid}
            size="tile"
            className={s.choiceCard}
            on={picked.includes(k)}
            preview={false}
            onClick={() => onPick(k)}
            onHover={(h) => setHover(h === null ? null : k)}
          >
            {multiple && <input type="checkbox" className={s.choiceCheck} aria-label={cardTitle(cid)} checked={picked.includes(k)} disabled={disabled} onChange={() => onPick(k)} onClick={(e) => e.stopPropagation()} onFocus={() => setHover(k)} onBlur={() => setHover(null)} />}
          </InspectCard>
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
          <button type="button" className={cx(s.opt, s.optMain)} disabled={auto} onClick={() => void answer({ value: bid })}>{tr("prompt.bid")}</button>
          <button type="button" className={s.opt} disabled={auto} onClick={() => void answer({ value: -1 })}>{tr("prompt.pass")}</button>
        </div>
      )}
    </div>
  );
}

export function openPrompt(sess: GameSession, p: MatchPrompt): void {
  openModal(fmtMsg(p.title, namesOf(sess.view?.state)) || tr("prompt.title"), (close) => <Prompt sess={sess} id={p.id} close={close} />, {
    closable: false,
    key: "prompt",
    chrome: "prompt",
    size: fitsInline(p) ? "fit" : "wide",
    // The countdown rides the retract strip (Modal head), so it stays visible
    // while the sheet is down -- and keeps running there.
    headExtra: <PromptTimer sess={sess} />,
  });
}