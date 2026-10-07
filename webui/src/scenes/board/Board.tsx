// The match board (BoardDemoController). Left: players + log. Center: the ring
// with the field panel and the event deck inside. Right: turn card, steps, d20,
// actions, end turn, and the hand with your draw pile.

import { useEffect, useRef } from "react";
import { navigate } from "../../app/router";
import { t as tr } from "../../i18n/t";
import { useAutoplay, useTick } from "../../core/hooks";
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
import { Side } from "./Side";
import s from "./Board.module.css";

export function Board({ sess }: { sess: GameSession }) {
  const { view, at, anim } = useBoardSession(sess);
  useTick(500); // turn timer
  const auto = useAutoplay(sess); // 托管 -- one shared input-lock
  const promptFor = useRef(0);
  const autoDeed = useRef(-1);
  const resultsShown = useRef(false);

  const exit = () => {
    if (sess.kind === "solo") {
      endSession();
      navigate({ name: "menu" });
    } else navigate({ name: "room", id: sess.id });
  };

  const m = view ? model(view) : null;

  // Prompts, the landing deed popup and the results open once the animation has caught up.
  // While 托管 is on the prompt modal is not opened at all (it would block the
  // board) and the landing deed is left alone -- the autopilot answers both.
  useEffect(() => {
    if (!m || anim.animating) return;
    const S = m.S;
    if (!auto && waitingOn(S.prompt, m.playerId) && promptFor.current !== S.prompt.id && !isModalOpen("prompt")) {
      promptFor.current = S.prompt.id;
      sfx("prompt");
      openPrompt(sess, S.prompt);
    }
    if (!auto && m.myTurn && S.step === 4 && !S.busy && !m.asking && S.landed >= 0) {
      const key = S.round * 100 + S.turn;
      if (autoDeed.current !== key && !isModalOpen("deed") && (buyable(m, S.landed) || canBuildOn(m, S.landed))) {
        autoDeed.current = key;
        openDeed(sess, S.landed);
      }
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
  const leave = () => (S.phase === "ended" || m.out ? exit() : showLeave(sess, exit));

  return (
    <>
      <TopBar compact help={false} section={tr("board.mode", { mode: modeName(S.mode), n: S.players.length })} title={tr("board.round", { n: Math.max(1, S.round) })} onBack={leave} right={<></>} />
      <AutoBanner sess={sess} />
      <div className={s.body}>
        <div className={s.left}>
          <Players m={m} solo={sess.kind === "solo"} elapsed={(performance.now() - at) / 1000} />
          <Log lines={anim.log} />
        </div>
        <Ring m={m} anim={anim} pickable={tilePick} onTile={onTile} />
        <div className={s.right}>
          <Side m={m} sess={sess} anim={anim} />
        </div>
      </div>
    </>
  );
}
