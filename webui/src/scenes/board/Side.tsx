// Right column: current turn card with the turn timer, phase steps, the d20,
// action buttons, end turn, and the hand (with a full-size hover preview) and
// your draw pile.

import { useEffect, useRef, useState } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { n0 } from "../../core/format";
import { useAutoplay } from "../../core/hooks";
import type { GameSession } from "../../game/session";
import { SoloSession } from "../../game/session";
import { AutoToggle, ThinkingPill, autoFloat } from "../../ui/AutoToggle";
import { Btn } from "../../ui/Button";
import { type CardAction, CardFace, showCard, TagChip } from "../../ui/Card";
import { Avatar, bandColor } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { SkillBody } from "../../ui/SkillBody";
import { showScoreWeights } from "../../ui/ScoreWeights";
import { toast } from "../../ui/Toast";
import type { Animator } from "./anim";
import { act, canBuildOn, type Model } from "./model";
import { byTitle, openDeed, showDeck, showDeedList, showSettle, showSkills } from "./Popups";
import s from "./Side.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateMax, stateOf } from "../../core/names";
import { statusChips } from "./Players";

/** The turn stages, by the game's own names (开始 / 运营 / 移动 / 结束) -- the
 *  sweep effect that announces them is separate and unchanged. */

const phases = () => [tr("common.start"), tr("board.stepOps"), tr("board.stepMove"), tr("board.stepEnd")];

export function Side({ m, sess, anim }: { m: Model; sess: GameSession; anim: Animator }) {
  const auto = useAutoplay(sess); // 托管: every input here is locked
  const S = m.S;
  const cur = S.players[S.turn];
  const myChar = m.charOf(m.playerId);
  // The rulebook's four stages are 开始 / 运营 / 移动 / 结束 (`rulebook.txt:2957`)
  // and `phases()` lists them in that order. The engine's `step` carries the
  // rulebook's own stage number (0 before a turn, 1..4 after), so `step - 1`
  // indexes the labels. Off by one here and 运营 -- the stage players spend
  // their turn in -- reads as 开始.
  //
  // While events play, the marker follows the animation (`anim.stage`), not
  // the state: one roll comes back as dice + walk + `step = 结束` together, and
  // the state alone would name 结束 before the dice land. Once the queue drains
  // it follows `S.step` again, with no lag of its own -- `canRoll` and
  // everything else read the real step, so a marker held behind it reads as
  // "stuck in 开始" while the dice are already live.
  const shown = S.phase !== "play" ? -1 : Math.min(3, Math.max(0, (anim.stage ?? S.step) - 1));
  const animating = anim.animating;
  const canRoll = S.phase === "play" && S.roller === m.playerId && S.step === 2 && !S.skipMove && !S.busy && !m.asking && !animating && !auto;
  const can = m.myTurn && !S.busy && !m.asking && !animating && !auto;

  let hint: string;
  if (S.phase !== "play") hint = "";
  else if (m.out) hint = m.me.bankrupt ? tr("board.spectating") : tr("board.youLeft");
  else if (S.turn < 0) hint = m.asking ? tr("board.redrawAsk") : tr("board.startingSoon");
  else if (S.busy || animating) hint = tr("board.settling");
  else if (S.roller === m.playerId && !m.myTurn && S.step === 2) hint = tr("board.rolledFor", { who: cur?.player ?? "" });
  else if (m.myTurn && S.step === 2 && S.skipMove) hint = tr("board.stayHint");
  else if (m.myTurn && S.step === 2 && S.roller !== m.playerId && S.roller >= 0) hint = tr("board.waitRoller", { who: S.players[S.roller].player });
  else if (m.myTurn) hint = S.step === 2 ? tr("board.clickRoll") : S.step >= 3 ? tr("board.moved") : tr("board.turnStart");
  else hint = tr("board.waiting", { who: cur?.player ?? "", bot: cur?.bot ? tr("board.botSuffix") : "" });

  const buildFromButton = () => {
    if (!m.myTurn) return toast(tr("board.buildOwnOnly"));
    const p = m.me.pos;
    if (!canBuildOn(m, p)) return toast(S.owners[p] === m.playerId ? tr("board.buildOnlySettling") : tr("board.buildNotOnOwn"));
    openDeed(sess, p);
  };
  const hasSkill = m.me.actions?.some((a) => a.enabled);
  const weights = sess instanceof SoloSession ? sess.weights : sess.room?.weights ?? { money: 1, property: 1, houses: 1 };
  const vote = S.vote;
  const k = vote.players.indexOf(m.playerId);

  return (
    <>
      {/* Your own status only -- whose turn it is, and its clock, live on the
          player panels (Players.tsx). */}
      <div className={cx(s.turnCard, m.myTurn && s.myTurn)}>
        <Avatar c={myChar} size={64} />
        <div className={s.tcMid}>
          <div className={s.tcTop}>
            <span className={s.tcName}>{myChar?.display ?? m.me.player}</span>
            {m.myTurn && <span className={s.tcTag}>{tr("board.turn")}</span>}
          </div>
          <div className={s.tcMoney}><img src={sceneImg("icon_coin")} alt="" />{n0(m.me.money)}</div>
          <div className={s.tcStats}>
            <span className={s.tcFire}><img src={sceneImg("icon_fire")} alt="" />{stateOf(m.me, "fire")}/{stateMax(m.me, "fire")}</span>
            {statusChips(m.me).map((x) => <span key={x} className={s.tcStatus}>{x}</span>)}
          </div>
        </div>
      </div>
      {/* 托管 / 混沌 / 进阶: your own seat's mode. Drawn over the card's right end but
          outside it, so it can sit above any open modal -- it has to be
          clickable at any time. The 进阶 search's "thinking…" pill sits beside it. */}
      <div className={cx(autoFloat, s.autoSlot)}>
        <ThinkingPill sess={sess} />
        <AutoToggle sess={sess} compact />
      </div>

      <div className={s.steps}>
        {phases().map((label, i) => <span key={label} className={cx(s.step, i === shown ? s.stepOn : i < shown && s.stepDone)}>{label}</span>)}
      </div>

      <button type="button" className={cx(s.dice, canRoll && s.can, anim.rolling && s.rolling)} disabled={!canRoll} onClick={() => void act(sess, { act: "roll" })}>
        <img className={s.diceImg} src={sceneImg("dice_d20")} alt="" />
        <div className={s.diceNum}>{anim.dice || ""}</div>
        <div className={s.diceHint}>{hint}</div>
        <div className={s.diceCap}>{tr("board.dice")}<small>{" "}1d20</small></div>
        <img className={cx(s.star, s.starA)} src={sceneImg("star5")} alt="" />
        <img className={cx(s.star, s.note)} src={sceneImg("ic_music_note")} alt="" />
      </button>

      <div className={s.actions}>
        <Btn icon="auto_awesome" className={cx(s.act, !hasSkill && m.myTurn && s.dim)} disabled={auto} onClick={() => showSkills(sess)}>{tr("board.useSkill")}</Btn>
        <Btn icon="construction" className={s.act} disabled={auto} onClick={buildFromButton}>{tr("board.build")}</Btn>
        <Btn icon="account_balance" className={s.act} disabled={auto} onClick={() => showDeedList(sess, false)}>{tr("board.mortgageDeeds")}</Btn>
        <Btn icon="redo" className={s.act} disabled={auto} onClick={() => showDeedList(sess, true)}>{tr("board.redeemDeeds")}</Btn>
      </div>
      {/* Matches `why_not_act`'s "end": legal anywhere the player's turn is
          theirs and quiet, except 运营 with a move still owed (roll first) and
          移动 (the walk is still running). The old gate demanded 结束 outright,
          which left the button dead through 开始 and through a [停留] 运营. */}
      <Btn kind="blue" icon="check" className={s.end} disabled={!(can && S.step !== 3 && (S.step !== 2 || S.skipMove) && !m.overHand)} onClick={() => void act(sess, { act: "end" })}>
        {m.myTurn && S.step === 2 && S.skipMove ? tr("board.endTurnSkip") : tr("board.endTurn")}
      </Btn>

      <Hand m={m} sess={sess} busy={animating} />

      <div className={s.extra}>
        {vote.id ? (
          <div className={s.vote}>
            <span>{tr("board.voteStatus", { n: vote.answers.filter((a) => a === 1).length, total: vote.players.length })}</span>
            {k >= 0 && vote.answers[k] < 0 && <Btn kind="pink" size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 1 })}>{tr("board.voteFor")}</Btn>}
            {k >= 0 && vote.answers[k] < 0 && <Btn size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 0 })}>{tr("board.voteAgainst")}</Btn>}
          </div>
        ) : (
          <Btn size="small" icon="leaderboard" disabled={S.phase !== "play" || m.out || auto} onClick={() => showSettle(sess)}>{sess.kind !== "online" ? tr("board.settle") : tr("board.voteEnd")}</Btn>
        )}
        <Btn size="small" icon="leaderboard" onClick={() => showScoreWeights(weights, false, undefined, tr("board.scoreRulesLocked"))}>{tr("solo.scoreRules")}</Btn>
      </div>
    </>
  );
}

function Hand({ m, sess, busy }: { m: Model; sess: GameSession; busy: boolean }) {
  const [hover, setHover] = useState<{ id: string; note: string } | null>(null);
  const [peek, setPeek] = useState(false);
  const auto = useAutoplay(sess); // 托管: play / discard are locked (inspect stays)
  const S = m.S;
  const limit = stateOf(m.me, "handLimit") || 5;
  const canPlay = m.myTurn && S.step === 2 && !S.busy && !m.asking && !busy && !auto;
  const detail = (id: string, k: number) => {
    setHover(null);
    const acts: CardAction[] = [];
    if (m.overHand) acts.push({ label: tr("board.discardThis"), enabled: !auto, kind: "white", run: () => act(sess, { act: "discard", card: id }) });
    acts.push({ label: canPlay ? tr("board.play") : tr("board.playOnlyOps"), enabled: canPlay, run: () => act(sess, { act: "play", card: id }) });
    showCard(id, acts, fmtMsg(m.v.handNotes[k], namesOf(m.S)));
  };
  const hc = hover ? D.card(hover.id) : undefined;
  const deck = byTitle(m.v.draw ?? []);
  // The preview is `pointer-events: none` (it sits beside the hand, not under
  // the cursor), so a long card text is scrolled from the hovered hand card:
  // the wheel over the hand, or PgUp/PgDn/↑/↓ while a card is hovered.
  const cardsRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (textRef.current) textRef.current.scrollTop = 0;
  }, [hover?.id]);
  useEffect(() => {
    if (!hover) return;
    const text = () => {
      const el = textRef.current;
      return el && el.scrollHeight > el.clientHeight ? el : null;
    };
    const onWheel = (e: WheelEvent) => {
      const el = text();
      if (!el) return;
      e.preventDefault();
      el.scrollTop += e.deltaMode === 1 ? e.deltaY * 21 : e.deltaMode === 2 ? e.deltaY * el.clientHeight : e.deltaY;
    };
    const onKey = (e: KeyboardEvent) => {
      const el = text();
      if (!el || e.altKey || e.ctrlKey || e.metaKey) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      const line = 21; // `.pText` line-height: 13px × 1.6
      const page = Math.max(line, el.clientHeight - line);
      const delta = { ArrowDown: line, ArrowUp: -line, PageDown: page, PageUp: -page }[e.key];
      if (delta === undefined) return;
      e.preventDefault();
      el.scrollTop += delta;
    };
    const cards = cardsRef.current;
    cards?.addEventListener("wheel", onWheel, { passive: false });
    window.addEventListener("keydown", onKey);
    return () => {
      cards?.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKey);
    };
  }, [hover]);
  return (
    <>
      <div className={s.hand}>
        <div className={s.handHead}>
          <PanelTab>{tr("board.hand", { n: m.v.hand.length, max: limit })}</PanelTab>
          <span className={cx(s.handHint, m.overHand && s.warn)}>{m.overHand ? tr("board.overHand") : canPlay ? tr("board.canPlay") : tr("board.tapCard")}</span>
          {/* Your draw pile, at the head of the hand it feeds (it used to sit in
              the board centre). Hover lists what is left; click opens the cards. */}
          <button type="button" className={s.deck} title={tr("board.drawPile")} onMouseEnter={() => setPeek(true)} onMouseLeave={() => setPeek(false)} onClick={() => { setPeek(false); showDeck(m); }}>
            <img src={sceneImg("card_back")} alt="" /><b>{m.me.draw}</b>
          </button>
        </div>
        <div className={s.cards} ref={cardsRef}>
          {m.v.hand.map((id, k) => (
            <CardFace key={`${id}:${k}`} id={id} size="hand" onClick={() => detail(id, k)} onMouseEnter={() => setHover({ id, note: fmtMsg(m.v.handNotes[k], namesOf(m.S)) })} onMouseLeave={() => setHover(null)} />
          ))}
        </div>
      </div>
      {/* Card-game improvement: hovering a hand card shows it full size next to the hand. */}
      <div className={cx(s.preview, hover && s.previewOn)}>
        {hover && (
          <>
            <div className={s.pArt} style={{ borderColor: hc ? bandColor(hc.band) : "#ED4E76" }}><img src={cardArt(hover.id)} alt="" /></div>
            <div className={s.pTitle}>{cardTitle(hover.id)}</div>
            {!!hc?.tags.length && <div className={s.pTags}>{hc.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
            <div className={s.pText} ref={textRef}><SkillBody text={hc?.text ?? ""} /></div>
            {hover.note && <div className={s.pNote}>{hover.note}</div>}
          </>
        )}
      </div>
      {/* The draw pile, alphabetical -- never in the order it will be drawn. */}
      <div className={cx(s.peek, peek && s.peekOn)}>
        <div className={s.peekHead}>{tr("board.drawPile")}<b>×{m.me.draw}</b></div>
        {deck.length
          ? deck.map((id, k) => <div key={k} className={s.peekRow}><img src={cardArt(id)} alt="" /><span>{cardTitle(id)}</span></div>)
          : <div className={s.peekEmpty}>{tr("board.drawPileEmpty")}</div>}
      </div>
    </>
  );
}
