// The match board (BoardDemoController). Left: players. Center: the ring
// with the field panel and the event deck inside, and the collapsed hand
// underneath. Controls sit below the players; the right is a full match log.

import { useEffect, useRef } from "react";
import { navigate } from "../../app/router";
import { useStageFill } from "../../app/Stage";
import { t as tr } from "../../i18n/t";
import { CommitChip } from "../../ui/CommitChip";
import { cx } from "../../core/cx";
import { useAutoplay, useTick, useWakeLock } from "../../core/hooks";
import { endSession, type GameSession } from "../../game/session";
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
}

export function Board({ sess }: { sess: GameSession }) {
  const { view, at, anim } = useBoardSession(sess);
  useTick(500); // turn timer
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

  const m = view ? model(view) : null;
  useMatchDriver({ sess, m, anim, auto, exit });

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
  // The die lives in the board's roll zone (Ring); replays hide it entirely.
  const control = movementControl(m, { auto, animating: anim.animating, readOnly: sess.readOnly, connected: sess.connected });
  const roll: RollControl | null = sess.readOnly ? null : {
    enabled: control === "roll",
    rolling: anim.rolling,
    dice: anim.dice,
    hint: control === "roll" ? tr("board.clickRoll") : "",
    onClick: () => control === "roll" && void act(sess, { act: "roll" }),
  };

  return (
    <>
      <TopBar
        compact
        help={false}
        section={tr("board.mode", { mode: modeName(S.mode), n: S.players.length })}
        title={tr("board.round", { n: Math.max(1, S.round) })}
        onBack={leave}
        right={sess.kind === "online" && sess.room?.fair ? <CommitChip commit={sess.room.fair.commit} /> : <></>}
      />
      <AutoBanner sess={sess} />
      <div className={s.body}>
        <div className={s.left}>
          <Players m={m} solo={sess.kind !== "online"} elapsed={(performance.now() - at) / 1000} />
          <Side m={m} sess={sess} anim={anim} />
        </div>
        <div className={s.middle}>
          {/* The cards in play, as a fold-away strip at the top of this column
              (under the FX layer). The board's roll-zone die takes the roll
              action from the old side-column button. */}
          <FieldSheet m={m} />
          <div className={s.mapSlot}>
            <Ring
              m={m}
              anim={anim}
              pickable={tilePick}
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
