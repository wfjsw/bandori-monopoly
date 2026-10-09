// Compact turn controls at the bottom left; the Hand is docked below the map.

import { useEffect, useRef, useState } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { useAutoplay, useSessionOther } from "../../core/hooks";
import type { GameSession } from "../../game/session";
import { AutoToggle, ThinkingPill, autoFloat } from "../../ui/AutoToggle";
import { Btn } from "../../ui/Button";
import { CardFace, type CardAction, showCard, TagChip } from "../../ui/Card";
import { bandColor } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { SkillBody } from "../../ui/SkillBody";
import type { Animator } from "./anim";
import { act, type Model } from "./model";
import { byTitle, showDeck, showDeedList, showSkills } from "./Popups";
import { movementControl } from "./turnFlow";
import s from "./Side.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateOf } from "../../core/names";

export function Side({ m, sess, anim }: { m: Model; sess: GameSession; anim: Animator }) {
  const auto = useAutoplay(sess);
  // A replay is read-only: every action control (skill, roll / continue,
  // mortgage, redeem) is hidden, not disabled, and the grid collapses away --
  // the left column is just the players. Inspection (tiles, cards, log) stays.
  if (sess.readOnly) return null;
  const S = m.S;
  const cur = S.players[S.turn];
  const names = namesOf(S);
  const control = movementControl(m, { auto, animating: anim.animating, readOnly: sess.readOnly, connected: sess.connected });
  const can = control !== null;
  const advance = control === "end";

  let hint: string;
  if (S.phase !== "play") hint = "";
  else if (m.out) hint = m.me.bankrupt ? tr("board.spectating") : tr("board.youLeft");
  else if (S.turn < 0) hint = m.asking ? tr("board.redrawAsk") : tr("board.startingSoon");
  else if (S.busy || anim.animating) hint = tr("board.settling");
  else if (S.roller === m.playerId && !m.myTurn && S.step === 2) hint = tr("board.rolledFor", { who: names.playerId(S.turn) });
  else if (m.myTurn && S.step === 2 && S.skipMove) hint = tr("board.stayHint");
  else if (advance) hint = tr("board.moved");
  else if (m.myTurn && S.step === 2 && S.roller !== m.playerId && S.roller >= 0) hint = tr("board.waitRoller", { who: names.playerId(S.roller) });
  else if (m.myTurn) hint = S.step === 2 ? tr("board.clickRoll") : S.step >= 3 ? tr("board.moved") : tr("board.turnStart");
  else hint = tr("board.waiting", { who: names.playerId(S.turn), bot: cur?.bot ? tr("board.botSuffix") : "" });

  const hasSkill = m.me.actions?.some((a) => a.enabled);
  const caption = advance ? tr(S.skipMove ? "board.skipMovement" : "board.continueTurn") : tr("board.dice");
  return (
    <div className={s.actions}>
      <Btn icon="auto_awesome" className={cx(s.act, !hasSkill && m.myTurn && s.dim)} disabled={auto} onClick={() => showSkills(sess)}>{tr("board.useSkill")}</Btn>
      <button type="button" className={cx(s.dice, can && s.can, anim.rolling && s.rolling)} title={hint} disabled={!can} onClick={() => control && void act(sess, { act: control })}>
        {advance ? <Icon name="arrow_forward" className={s.advanceIcon} /> : <img className={s.diceImg} src={sceneImg("dice_d20")} alt="" />}
        {!advance && <div className={s.diceNum}>{anim.dice || ""}</div>}
        <div className={s.diceCap}>{caption}{!advance && <small>{" "}1d20</small>}</div>
        <div className={s.diceHint}>{hint}</div>
      </button>
      <Btn icon="account_balance" className={s.act} disabled={auto} onClick={() => showDeedList(sess, false)}>{tr("board.mortgageDeeds")}</Btn>
      <Btn icon="redo" className={s.act} disabled={auto} onClick={() => showDeedList(sess, true)}>{tr("board.redeemDeeds")}</Btn>
    </div>
  );
}

/** Only appears when somebody has already requested an online settlement. */
export function SettleVote({ m, sess }: { m: Model; sess: GameSession }) {
  const auto = useAutoplay(sess);
  const vote = m.S.vote;
  const k = vote.players.indexOf(m.playerId);
  if (!vote.id) return null;
  return (
    <div className={s.vote}>
      <span>{tr("board.voteStatus", { n: vote.answers.filter((a) => a === 1).length, total: vote.players.length })}</span>
      {/* Replay keeps the tally as read-out and drops the buttons. */}
      {!sess.readOnly && k >= 0 && vote.answers[k] < 0 && <div className={s.voteActions}>
        <Btn kind="pink" size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 1 })}>{tr("board.voteFor")}</Btn>
        <Btn size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 0 })}>{tr("board.voteAgainst")}</Btn>
      </div>}
    </div>
  );
}

/**
 * The match hand, docked at the bottom of the middle column like Master Duel's:
 * a fanned row of full card faces, retracted so only the top strip of each card
 * peeks above the bottom edge. Hovering the dock (or focusing a card) raises
 * the hand; leaving retracts it after a short delay. Hovering one card lifts it
 * clear of its neighbours and floats the full text. The slim bar under the fan
 * (hand count, hint, draw pile, 托管) stays visible either way.
 */
export function Hand({ m, sess, busy }: { m: Model; sess: GameSession; busy: boolean }) {
  const [hover, setHover] = useState<{ k: number; id: string; note: string } | null>(null);
  const [peek, setPeek] = useState(false);
  const [raised, setRaised] = useState(false);
  const auto = useAutoplay(sess); // 托管: play / discard are locked (inspect stays)
  const S = m.S;
  const limit = stateOf(m.me, "handLimit") || 5;
  const canPlay = m.myTurn && S.step === 2 && !S.busy && !m.asking && !busy && !auto;
  const detail = (id: string, k: number) => {
    setHover(null);
    const note = fmtMsg(m.v.handNotes[k], namesOf(m.S));
    // Replay: the sheet is inspection only -- no play / discard buttons.
    if (sess.readOnly) return void showCard(id, [], note);
    const acts: CardAction[] = [];
    if (m.overHand) acts.push({ label: tr("board.discardThis"), enabled: !auto, kind: "white", run: () => act(sess, { act: "discard", card: id }) });
    acts.push({ label: canPlay ? tr("board.play") : tr("board.playOnlyOps"), enabled: canPlay, run: () => act(sess, { act: "play", card: id }) });
    showCard(id, acts, note);
  };
  const hc = hover ? D.card(hover.id) : undefined;
  const deck = byTitle(m.v.draw ?? []);
  // Raise on hover / focus; retract only after a short delay, so moving between
  // cards (or to the bar and back) does not drop the hand out from under you.
  const closeTimer = useRef(0);
  const raise = () => {
    window.clearTimeout(closeTimer.current);
    setRaised(true);
  };
  const retract = () => {
    window.clearTimeout(closeTimer.current);
    closeTimer.current = window.setTimeout(() => setRaised(false), 200);
  };
  useEffect(() => () => window.clearTimeout(closeTimer.current), []);
  // The preview is `pointer-events: none` (it sits above the hand, not under
  // the cursor), so a long card text is scrolled from the hovered hand card:
  // the wheel over the hand, or PgUp/PgDn/↑/↓ while a card is hovered.
  const fanRef = useRef<HTMLDivElement>(null);
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
    const fan = fanRef.current;
    fan?.addEventListener("wheel", onWheel, { passive: false });
    window.addEventListener("keydown", onKey);
    return () => {
      fan?.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKey);
    };
  }, [hover]);
  return (
    <>
      <div
        className={cx(s.dock, raised && s.dockUp)}
        onMouseEnter={raise}
        onMouseLeave={retract}
        onFocus={raise}
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) retract(); }}
      >
        <div className={s.fan} ref={fanRef}>
          {m.v.hand.map((id, k) => (
            <div key={`${id}:${k}`} className={s.fanCard}>
              <CardFace
                id={id}
                size="mini"
                title={cardTitle(id)}
                onClick={() => detail(id, k)}
                onMouseEnter={() => setHover({ k, id, note: fmtMsg(m.v.handNotes[k], namesOf(S)) })}
                onMouseLeave={() => setHover(null)}
                onFocus={() => setHover({ k, id, note: fmtMsg(m.v.handNotes[k], namesOf(S)) })}
                onBlur={() => setHover(null)}
              />
            </div>
          ))}
        </div>
        <div className={s.bar}>
          <PanelTab>{tr("board.hand", { n: m.v.hand.length, max: limit })}</PanelTab>
          <span className={cx(s.handHint, m.overHand && s.warn)}>{m.overHand ? tr("board.overHand") : canPlay ? tr("board.canPlay") : tr("board.tapCard")}</span>
          {/* Your draw pile, at the head of the hand it feeds (it used to sit in
              the board centre). Hover lists what is left; click opens the cards. */}
          <div className={cx(s.handTools, !sess.readOnly && s.withAuto)}>
            <button type="button" className={s.deck} title={tr("board.drawPile")} onMouseEnter={() => setPeek(true)} onMouseLeave={() => setPeek(false)} onFocus={() => setPeek(true)} onBlur={() => setPeek(false)} onClick={() => { setPeek(false); showDeck(m); }}>
              <img src={sceneImg("card_back")} alt="" /><b>{m.me.draw}</b>
            </button>
            <div className={cx(autoFloat, s.autoSlot)}>
              {!sess.readOnly && <AnimationSpeed sess={sess} />}
              <AutoToggle sess={sess} compact />
            </div>
            <div className={s.thinkingSlot}><ThinkingPill sess={sess} /></div>
          </div>
        </div>
        {/* Hovering or focusing a card reveals the full card above the raised hand. */}
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

function AnimationSpeed({ sess }: { sess: GameSession }) {
  useSessionOther(sess);
  return (
    <div className={s.speeds} role="group" aria-label={tr("board.animationSpeed")}>
      {[1, 2, 4].map((speed) => (
        <button key={speed} type="button" className={cx(s.speed, sess.animSpeed === speed && s.speedOn)} aria-pressed={sess.animSpeed === speed} title={tr("board.animationSpeedAt", { n: speed })} onClick={() => sess.setAnimSpeed(speed)}>{speed}×</button>
      ))}
    </div>
  );
}
