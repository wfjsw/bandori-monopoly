// The match board (BoardDemoController). Left: players. Center: the ring
// with the cards-in-play row over its top edge, and the collapsed hand
// underneath. Controls sit below the players; the right is a full match log.

import { useCallback, useEffect, useMemo, useRef } from "react";
import { navigate } from "../../app/router";
import { useStageFill } from "../../app/Stage";
import { namesOf } from "../../core/names";
import { t as tr } from "../../i18n/t";
import { cx } from "../../core/cx";
import { useAutoplay, useTick, useWakeLock } from "../../core/hooks";
import { endSession, type GameSession } from "../../game/session";
import { useSecondBoundary } from "../../hooks/timers";
import { isModalOpen } from "../../ui/Modal";
import { sfx } from "../../core/audio";
import { AutoBanner } from "../../ui/AutoToggle";
import { CardFace } from "../../ui/Card";
import { TopBar } from "../../ui/TopBar";
import { useBoardSession, type Animator } from "./anim";
import { act, buyable, canBuildOn, modeName, model, type Model } from "./model";
import { Log, Players } from "./Players";
import { openDeed, showLeave } from "./Popups";
import { openPrompt, waitingOn } from "./Prompt";
import { showResults } from "./Results";
import { Ring } from "./Ring";
import { Hand, SettleVote, Side } from "./Side";
import { shouldFinishTurn, movementControl } from "./turnFlow";
import { FieldSheet } from "./FieldSheet";
import { CardStand } from "./CardStand";
import { useStandingPreviewOn } from "../../ui/CardPreview";
import type { RollControl } from "./Ring";
import s from "./Board.module.css";

/**
 * The match driver: opens the prompt / landing-deed popups and the results
 * once the animation queue has caught up, and auto-ends a settled turn. Runs
 * after every render on purpose -- `view`, `anim.animating` and the modal stack
 * arrive from three different channels and either may land first. While 托管
 * is on the prompt modal is not opened at all (it would block the board) and
 * the landing deed is left alone -- the autopilot answers both.
 */
function useMatchDriver(opts: {
  sess: GameSession;
  m: Model | null;
  anim: Animator;
  auto: boolean;
  exit: () => void;
}): void {
  const { sess, m, anim, auto, exit } = opts;
  const promptFor = useRef(0);
  const autoDeed = useRef(-1);
  const autoEnd = useRef(-1);
  const resultsShown = useRef(false);
  // Once per state version, when the Animator queue has drained: tell the
  // server the acting seat has caught up so its clock can start (or resume).
  // Replay / solo have no server clock hold. Debounced by the animating flag
  // itself -- we only fire on the false edge plus a fresh seq.
  const ackedSeq = useRef(-1);
  useEffect(() => {
    if (anim.animating) return;
    if (sess.kind !== "online") return;
    const seq = m?.S.seq;
    if (seq == null || ackedSeq.current === seq) return;
    ackedSeq.current = seq;
    void sess.animDone(seq);
  }, [anim.animating, m?.S.seq, sess]);
  useEffect(() => {
    if (!m) return;
    const S = m.S;
    // An extra turn may keep the same round and player. Re-arm when the
    // engine leaves settlement, before waiting for its animation to finish.
    if (S.step !== 4) {
      autoDeed.current = -1;
      autoEnd.current = -1;
    }
    if (anim.animating) return;
    if (!auto && waitingOn(S.prompt, m.playerId) && promptFor.current !== S.prompt.id && !isModalOpen("prompt")) {
      promptFor.current = S.prompt.id;
      sfx("prompt");
      openPrompt(sess, S.prompt);
    }
    if (!auto && m.myTurn && S.step === 4 && !S.busy && !m.asking && S.landed >= 0) {
      const key = S.round * 100 + S.turn;
      if (autoDeed.current !== key && !isModalOpen() && (buyable(m, S.landed) || canBuildOn(m, S.landed))) {
        autoDeed.current = key;
        openDeed(sess, S.landed);
        return;
      }
    }
    const key = S.round * 100 + S.turn;
    if (autoEnd.current !== key && shouldFinishTurn(m, {
      auto, animating: anim.animating, readOnly: sess.readOnly,
      connected: sess.connected, modalOpen: isModalOpen(),
    })) {
      autoEnd.current = key;
      void act(sess, { act: "end" }).then((ok) => {
        if (!ok && autoEnd.current === key) autoEnd.current = -1;
      });
    }
    // A replay ends on its last frame with the transport still up (scrub
    // back, or exit); the results popup belongs to a live match only.
    if (S.phase === "ended" && !resultsShown.current && sess.kind !== "replay") {
      resultsShown.current = true;
      showResults(sess, m, exit);
    }
  });
}

export function Board({ sess }: { sess: GameSession }) {
  const { view, at, anim } = useBoardSession(sess);
  // Turn timer: online frames carry absolute deadlines, so repaint exactly
  // when the displayed second flips (not on a 500ms grid). Fallback tick for
  // paths with no deadline (replay / older server / local countdowns).
  // Bonus drains first; `baseDeadlineMs` is the end of the whole clock, so it
  // is the one that keeps the second-boundary ticker running through both
  // phases (bonus then base).
  const clockDeadline = view?.clock ? view.clock.baseDeadlineMs : null;
  useSecondBoundary(clockDeadline);
  useTick(500);
  const auto = useAutoplay(sess); // 托管 -- one shared input-lock
  // The match screen is the one scene that fills the window: no top / bottom
  // letterbox, so the columns stretch and the ring centres in the middle one.
  useStageFill();
  // Screen stays on during a live match only -- not in a replay (which also
  // renders this Board) and not once the match has ended.
  useWakeLock(sess.kind !== "replay" && view?.state.phase !== "ended");
  // The standing card panel replaces the floating hover popups on this screen.
  useStandingPreviewOn();

  const exit = () => {
    if (sess.kind === "replay") navigate({ name: "replay" });
    else if (sess.kind === "solo") {
      endSession();
      navigate({ name: "menu" });
    } else navigate({ name: "room", id: sess.id });
  };

  // Stable per snapshot, so the memoized panels below skip the 500 ms timer
  // and animation-only re-renders.
  const m = useMemo(() => (view ? model(view) : null), [view]);
  // The state already holds the next turn while the last one's events still
  // play; panels that show whose turn it is follow the animation instead.
  const shown = useMemo(() => (m && anim.turn != null && anim.turn !== m.S.turn ? { ...m, S: { ...m.S, turn: anim.turn } } : m), [m, anim.turn]);
  useMatchDriver({ sess, m, anim, auto, exit });
  // A stable tile-click, so the memoized ring tiles are not re-rendered by a
  // fresh closure each time; it reads the latest handler below.
  const tileClick = useRef<(i: number) => void>(() => undefined);
  const onTile = useCallback((i: number) => tileClick.current(i), []);

  if (!m) return null;
  const S = m.S;
  // Tile-pick clicks answer a `tile` prompt; while 托管 is on they must not
  // (the deed-inspect click stays -- that is read-only).
  const tilePick = !auto && waitingOn(S.prompt, m.playerId) && S.prompt.kind === "tile" && !anim.animating ? S.prompt.items.map(Number) : [];
  tileClick.current = (i: number) => {
    const k = tilePick.indexOf(i);
    if (k >= 0) return void act(sess, { act: "answer", prompt: S.prompt.id, value: k });
    openDeed(sess, i);
  };
  const leave = () => (sess.kind === "replay" || S.phase === "ended" || m.out ? exit() : showLeave(sess, exit));
  // The die lives in the board's roll zone (Ring); replays hide it entirely.
  const control = movementControl(m, { auto, animating: anim.animating, readOnly: sess.readOnly, connected: sess.connected });
  const advance = control === "end";
  const names = namesOf(S);
  const cur = S.players[S.turn];
  // Why the button is (or is not) live, shown under it.
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
  const roll: RollControl | null = sess.readOnly ? null : {
    enabled: control !== null,
    rolling: anim.rolling,
    dice: anim.dice,
    caption: advance ? tr(S.skipMove ? "board.skipMovement" : "board.continueTurn") : tr("board.dice"),
    hint,
    onClick: () => control && void act(sess, { act: control }),
  };

  const nLinks = anim.chain?.links.length ?? 0;
  const cardW = nLinks <= 2 ? 150 : nLinks <= 4 ? 125 : nLinks <= 6 ? 105 : nLinks <= 8 ? 90 : 78;
  return (
    <>
      <TopBar
        compact
        help={false}
        section={tr("board.mode", { mode: modeName(S.mode), n: S.players.length })}
        title={tr("board.round", { n: Math.max(1, S.round) })}
        onBack={leave}
        right={<></>}
      />
      <AutoBanner sess={sess} />
      <div className={s.body}>
        <div className={s.left}>
          <Players m={shown ?? m} solo={sess.kind !== "online"} elapsed={(performance.now() - at) / 1000} />
          <Side sess={sess} />
        </div>
        <div className={s.middle}>
          {/* The cards in play, as a top-docked card row over this column's
              upper edge (under the FX layer). The board's roll-zone die takes
              the roll action from the old side-column button. */}
          <FieldSheet m={m} pulse={anim.pulse} />
          <div className={s.mapSlot}>
            <Ring
              m={shown ?? m}
              anim={anim}
              pickable={tilePick}
              walk={tilePick.length > 0 && S.prompt.walk}
              onTile={onTile}
              roll={roll}
            />
          </div>
          <Hand m={m} sess={sess} busy={anim.animating} />
          {/* Match-screen FX layer: the stage banner (new-turn announcement),
              the action banner and the card activation flash are detached from
              the map -- sized and placed by this column, never clipped or
              scaled by the ring's window or zoom. */}
          <div className={s.fx}>
            {anim.banner && (
              <div key={anim.banner.id} className={s.banner}>
                <b>{anim.banner.title}</b>
                {anim.banner.body && <span>{anim.banner.body}</span>}
              </div>
            )}
            {anim.turnAnnouncement && (
              <div key={anim.turnAnnouncement.id} className={s.phaseFlash}>
                <i className={s.link} /><i className={s.link} /><i className={s.link} /><i className={s.link} />
                <span>{anim.turnAnnouncement.label}</span>
              </div>
            )}
            {anim.chain?.shown && (
              <div className={cx(s.chain, anim.chain.resolving && s.chainResolving)} style={{ width: cardW * 3.2 }}>
                {anim.chain.links.filter((l) => !anim.chain!.done.includes(l.seq) || l.seq === anim.chain!.active).map((l) => {
                  const ch = anim.chain!;
                  const done = ch.done.includes(l.seq);
                  return (
                    <div
                      key={l.seq}
                      className={cx(
                        s.chainLink,
                        l.owner >= 0 && l.owner === m.playerId ? s.chainMine : s.chainTheirs,
                        !ch.resolving && l.seq === ch.links.length && s.chainIn,
                        ch.active === l.seq && !done && s.chainActive,
                        done && s.chainDone,
                      )}
                      style={{ ["--owner" as string]: l.owner >= 0 ? m.colorOf(l.owner) : "var(--pink)", marginTop: l.seq > 1 ? -cardW * 0.55 : 0 }}
                    >
                      {l.card
                        ? <CardFace id={l.card} size="big" width={cardW} />
                        : <div className={s.chainOrigin} style={{ width: cardW, height: cardW * 1.5 }}>{tr("anim.chainOrigin")}</div>}
                      <span className={s.chainNo}>{l.seq}</span>
                    </div>
                  );
                })}
              </div>
            )}
            {anim.chainTitle && (
              <div key={anim.chainTitle.id} className={s.chainTitle}>
                {[...anim.chainTitle.text].map((ch, k) => (
                  <span key={k} style={{ animationDelay: `${k * 70}ms` }}>{ch}</span>
                ))}
              </div>
            )}
            {anim.flash && (
              <div key={anim.flash.id} className={cx(s.flash, anim.flash.out && s.flashOut)}>
                <div
                  className={s.flashFace}
                  style={{ ["--owner" as string]: anim.flash.owner >= 0 ? m.colorOf(anim.flash.owner) : "var(--pink)" }}
                >
                  <CardFace id={anim.flash.card} size="big" />
                  {(anim.flash.caption || anim.flash.detail.length > 0) && (
                    <div className={s.flashCaption}>
                      {anim.flash.caption && <div>{anim.flash.caption}</div>}
                      {anim.flash.detail.map((d, k) => <div key={k} className={s.flashDetail}>{d}</div>)}
                    </div>
                  )}
                  {anim.flash.negated && <div className={s.flashNegated}><span>{tr("board.cardNegated")}</span></div>}
                </div>
              </div>
            )}
          </div>
        </div>
        <div className={s.right}>
          {/* Half the sidebar: the standing card detail (hover / click / flash).
              The log keeps the other half. */}
          <div className={s.standSlot}>
            <CardStand flash={anim.flash?.card} />
          </div>
          <div className={s.logSlot}>
            <Log lines={anim.log} colorOf={m.colorOf}><SettleVote m={m} sess={sess} /></Log>
          </div>
        </div>
      </div>
    </>
  );
}
