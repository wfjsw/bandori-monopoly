// Right column: current turn card with the turn timer, phase steps, the d20,
// action buttons, end turn, and the hand (with a full-size hover preview).

import { useState } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle } from "../../core/data";
import { n0 } from "../../core/format";
import type { GameSession } from "../../game/session";
import { SoloSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { type CardAction, CardFace, showCard, TagChip } from "../../ui/Card";
import { Avatar, bandColor } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { showScoreWeights } from "../../ui/ScoreWeights";
import { toast } from "../../ui/Toast";
import type { Animator } from "./anim";
import { act, canBuildOn, type Model } from "./model";
import { openDeed, showDeedList, showSettle, showSkills } from "./Popups";
import s from "./Side.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateOf } from "../../core/names";

/** The turn stages, by the game's own names (开始 / 运营 / 移动 / 结束) -- the
 *  sweep effect that announces them is separate and unchanged. */

const phases = () => [tr("common.start"), tr("board.stepOps"), tr("board.stepMove"), tr("board.stepEnd")];

function timerOf(m: Model, elapsed: number, solo: boolean): { value: string; caption: string; frac: number; cls: string } {
  const S = m.S;
  // Solo has no deadlines at all (the engine never expires one), so a clock
  // counting down to nothing would only read as "about to time out".
  if (solo) return { value: tr("common.unlimited"), caption: "", frac: 0, cls: s.idle };
  const cur = S.players[S.turn];
  if (S.phase !== "play") return { value: "—", caption: "", frac: 0, cls: s.idle };
  if (S.turn < 0) return { value: "—", caption: tr("board.ready"), frac: 0, cls: s.idle };
  if (cur?.ai) return { value: "—", caption: cur.bot ? tr("solo.bot") : tr("board.afk"), frac: 0, cls: s.idle };
  const e = !S.busy && !m.asking ? elapsed : 0;
  const shield = Math.max(0, S.shield - e);
  const bank = Math.max(0, S.bank - Math.max(0, e - S.shield));
  if (shield > 0) return { value: String(Math.ceil(shield)), caption: tr("board.shield"), frac: Math.min(1, shield / 20), cls: s.shield };
  return { value: String(Math.ceil(bank)), caption: tr("board.remain"), frac: Math.min(1, bank / 60), cls: bank < 10 ? s.low : "" };
}

export function Side({ m, sess, anim, elapsed }: { m: Model; sess: GameSession; anim: Animator; elapsed: number }) {
  const S = m.S;
  const cur = S.players[S.turn];
  const c = S.turn >= 0 ? m.charOf(S.turn) : undefined;
  const t = timerOf(m, elapsed, sess.kind === "solo");
  // 移动 while a walk is still on screen. `main_move` sets MOVE, walks and sets
  // END in one call, so by the time the steps play out `S.step` already names
  // 结束 -- the marker would call a moving token a finished turn.
  const step =
    S.phase !== "play"
      ? -1
      : anim.phase?.key === "board.stepEnd"
        ? 3
        : anim.walking
          ? 2
          : Math.min(3, Math.max(0, S.step - 1));
  // The rulebook's four stages are 开始 / 运营 / 移动 / 结束 (`rulebook.txt:2957`)
  // and `phases()` lists them in that order. The engine's `step` carries the
  // rulebook's own stage number (0 before a turn, 1..4 after), so `step - 1`
  // indexes the labels. Off by one here and 运营 -- the stage players spend
  // their turn in -- reads as 开始.
  //
  // This marker is **not** allowed to lag the game state, even though the
  // transition sweep holds each name for its own 1.5s: `canRoll` and everything
  // else read the real `S.step`, so a marker one stage behind reads as "stuck in
  // 开始" while the dice are already live, and then names 运营 over the move. The
  // sweep is what makes a short stage readable -- this row just says where the
  // game is now.
  const shown = step;
  const animating = anim.animating;
  const canRoll = S.phase === "play" && S.roller === m.playerId && S.step === 2 && !S.skipMove && !S.busy && !m.asking && !animating;
  const can = m.myTurn && !S.busy && !m.asking && !animating;

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
      <div className={s.turnCard}>
        <Avatar c={c} size={92} />
        <div className={s.tcMid}>
          <div className={s.tcTag}>{tr("board.turn")}</div>
          <div className={s.tcName}>{cur ? c?.display ?? cur.player : "—"}</div>
          <div className={s.tcMoney}><img src={sceneImg("icon_coin")} alt="" />{cur ? n0(cur.money) : ""}</div>
        </div>
        <div className={cx(s.timer, t.cls)} style={{ ["--frac" as string]: t.frac }}><b>{t.value}</b><small>{t.caption}</small></div>
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
        <Btn icon="auto_awesome" className={cx(s.act, !hasSkill && m.myTurn && s.dim)} onClick={() => showSkills(sess)}>{tr("board.useSkill")}</Btn>
        <Btn icon="account_balance" className={s.act} onClick={() => showDeedList(sess, false)}>{tr("board.mortgageDeeds")}</Btn>
        <Btn icon="redo" className={s.act} onClick={() => showDeedList(sess, true)}>{tr("board.redeemDeeds")}</Btn>
        <Btn icon="construction" className={s.act} onClick={buildFromButton}>{tr("board.build")}</Btn>
      </div>
      {/* Matches `why_not_act`'s "end": legal anywhere the player's turn is
          theirs and quiet, except 运营 with a move still owed (roll first) and
          移动 (the walk is still running). The old gate demanded 结束 outright,
          which left the button dead through 开始 and through a [停留] 运营. */}
      <Btn kind="blue" icon="check" className={s.end} disabled={!(can && S.step !== 3 && (S.step !== 2 || S.skipMove) && !m.overHand)} onClick={() => { anim.showPhase("board.stepEnd"); void act(sess, { act: "end" }); }}>
        {m.myTurn && S.step === 2 && S.skipMove ? tr("board.endTurnSkip") : tr("board.endTurn")}
      </Btn>

      <Hand m={m} sess={sess} busy={animating} />

      <div className={s.extra}>
        {vote.id ? (
          <div className={s.vote}>
            <span>{tr("board.voteStatus", { n: vote.answers.filter((a) => a === 1).length, total: vote.players.length })}</span>
            {k >= 0 && vote.answers[k] < 0 && <Btn kind="pink" size="small" onClick={() => void act(sess, { act: "vote", value: 1 })}>{tr("board.voteFor")}</Btn>}
            {k >= 0 && vote.answers[k] < 0 && <Btn size="small" onClick={() => void act(sess, { act: "vote", value: 0 })}>{tr("board.voteAgainst")}</Btn>}
          </div>
        ) : (
          <Btn size="small" icon="leaderboard" disabled={S.phase !== "play" || m.out} onClick={() => showSettle(sess)}>{sess.kind === "solo" ? tr("board.settle") : tr("board.voteEnd")}</Btn>
        )}
        <Btn size="small" icon="leaderboard" onClick={() => showScoreWeights(weights, false, undefined, tr("board.scoreRulesLocked"))}>{tr("solo.scoreRules")}</Btn>
      </div>
    </>
  );
}

function Hand({ m, sess, busy }: { m: Model; sess: GameSession; busy: boolean }) {
  const [hover, setHover] = useState<{ id: string; note: string } | null>(null);
  const S = m.S;
  const limit = stateOf(m.me, "handLimit") || 5;
  const canPlay = m.myTurn && S.step === 2 && !S.busy && !m.asking && !busy;
  const detail = (id: string, k: number) => {
    setHover(null);
    const acts: CardAction[] = [];
    if (m.overHand) acts.push({ label: tr("board.discardThis"), enabled: true, kind: "white", run: () => act(sess, { act: "discard", card: id }) });
    acts.push({ label: canPlay ? tr("board.play") : tr("board.playOnlyOps"), enabled: canPlay, run: () => act(sess, { act: "play", card: id }) });
    showCard(id, acts, fmtMsg(m.v.handNotes[k], namesOf(m.S)));
  };
  const hc = hover ? D.card(hover.id) : undefined;
  return (
    <>
      <div className={s.hand}>
        <div className={s.handHead}>
          <PanelTab>{tr("board.hand", { n: m.v.hand.length, max: limit })}</PanelTab>
          <span className={cx(s.handHint, m.overHand && s.warn)}>{m.overHand ? tr("board.overHand") : canPlay ? tr("board.canPlay") : tr("board.tapCard")}</span>
        </div>
        <div className={s.cards}>
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
            <div className={s.pText}>{hc?.text ?? ""}</div>
            {hover.note && <div className={s.pNote}>{hover.note}</div>}
          </>
        )}
      </div>
    </>
  );
}
