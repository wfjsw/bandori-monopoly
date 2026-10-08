// The match board (BoardDemoController). Left: players. Center: the ring
// with the field panel and the event deck inside, and the collapsed hand
// underneath. Controls sit below the players; the right is a full match log.

import { useEffect, useRef } from "react";
import { navigate } from "../../app/router";
import { t as tr } from "../../i18n/t";
import { useAutoplay, useTick, useWakeLock } from "../../core/hooks";
import { endSession, type GameSession } from "../../game/session";
import { isModalOpen } from "../../ui/Modal";
import { sfx } from "../../core/audio";
import { AutoBanner } from "../../ui/AutoToggle";
import { TopBar } from "../../ui/TopBar";
import { useBoardSession } from "./anim";
import { act, buyable, canBuildOn, modeName, model } from "./model";
import { Log, Players } from "./Players";
import { openDeed, showLeave } from "./Popups";
import { openPrompt, waitingOn } from "./Prompt";
import { showResults } from "./Results";
import { Ring } from "./Ring";
import { Hand, SettleVote, Side } from "./Side";
import { shouldFinishTurn } from "./turnFlow";
import s from "./Board.module.css";

export function Board({ sess }: { sess: GameSession }) {
  const { view, at, anim } = useBoardSession(sess);
  useTick(500); // turn timer
  const auto = useAutoplay(sess); // 托管 -- one shared input-lock
  // Screen stays on during a live match only -- not in a replay (which also
  // renders this Board) and not once the match has ended.
  useWakeLock(sess.kind !== "replay" && view?.state.phase !== "ended");
  const promptFor = useRef(0);
  const autoDeed = useRef(-1);
  const autoEnd = useRef(-1);
  const resultsShown = useRef(false);

  const exit = () => {
    if (sess.kind === "replay") navigate({ name: "replay" });
    else if (sess.kind === "solo") {
      endSession();
      navigate({ name: "menu" });
    } else navigate({ name: "room", id: sess.id });
  };

  const m = view ? model(view) : null;

  // Prompts, the landing deed popup and the results open once the animation has caught up.
  // While 托管 is on the prompt modal is not opened at all (it would block the
  // board) and the landing deed is left alone -- the autopilot answers both.
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
    if (S.phase === "ended" && !resultsShown.current) {
      resultsShown.current = true;
      showResults(sess, m, exit);
    }
  });

  if (!m) return null;
  const S = m.S;
  // Tile-pick clicks answer a `tile` prompt; while 托管 is on they must not
  // (the deed-inspect click stays -- that is read-only).
  const tilePick = !auto && waitingOn(S.prompt, m.playerId) && S.prompt.kind === "tile" && !anim.animating ? S.prompt.items.map(Number) : [];
  const onTile = (i: number) => {
    const k = tilePick.indexOf(i);
    if (k >= 0) return void act(sess, { act: "answer", prompt: S.prompt.id, value: k });
    openDeed(sess, i);
  };
  const leave = () => (sess.kind === "replay" || S.phase === "ended" || m.out ? exit() : showLeave(sess, exit));

  return (
    <>
      <TopBar compact help={false} section={tr("board.mode", { mode: modeName(S.mode), n: S.players.length })} title={tr("board.round", { n: Math.max(1, S.round) })} onBack={leave} right={<></>} />
      <AutoBanner sess={sess} />
      <div className={s.body}>
        <div className={s.left}>
          <Players m={m} solo={sess.kind !== "online"} elapsed={(performance.now() - at) / 1000} />
          <Side m={m} sess={sess} anim={anim} />
        </div>
        <div className={s.middle}>
          <Ring m={m} anim={anim} pickable={tilePick} onTile={onTile} />
          <Hand m={m} sess={sess} busy={anim.animating} />
        </div>
        <div className={s.right}>
          <div className={s.logSlot}>
            <Log lines={anim.log}><SettleVote m={m} sess={sess} /></Log>
          </div>
        </div>
      </div>
    </>
  );
}
