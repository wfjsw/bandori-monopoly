// /play/solo and /play/<roomId>: the match screen. Shows character select
// or the board depending on the phase; resumes after a refresh (solo from the
// saved snapshot, online by re-attaching to the player).

import { useEffect, useState } from "react";
import { navigate } from "../app/router";
import { useBackdrop } from "../app/Stage";
import { playSceneBgm } from "../core/audio";
import { useMatchView } from "../core/hooks";
import { useCrossfade } from "../hooks/scene";
import { type GameSession, discardSolo, matchScene, resumeSolo } from "../game/session";
import { toast } from "../ui/Toast";
import { Board } from "./board/Board";
import { useOnline } from "./lobby/Room";
import { Select } from "./select/Select";
import { t as tr } from "../i18n/t";
import { useConsoleSession } from "../console/context";

/**
 * Resumed solo match: keep it (and its engine worker) for the life of the
 * screen, and bail back to the menu when there is no save -- or when the
 * worker failed to restore the one there was. The worker boots (and restores)
 * asynchronously, so a failure there is the old "solo save dropped" path, one
 * tick later.
 */
function useSoloResume(sess: ReturnType<typeof resumeSolo>): void {
  useEffect(() => {
    if (!sess) {
      toast(tr("play.noSolo"));
      navigate({ name: "menu" }, { replace: true });
      return;
    }
    let alive = true;
    void sess.ready.catch((e) => {
      if (!alive) return;
      console.warn("solo save dropped:", e);
      toast(tr("play.noSolo"));
      discardSolo();
      navigate({ name: "menu" }, { replace: true });
    });
    return () => {
      alive = false;
    };
  }, [sess]);
}

function SoloPlay() {
  const [sess] = useState(() => resumeSolo());
  useSoloResume(sess);
  return sess ? <Match sess={sess} /> : null;
}

function OnlinePlay({ id }: { id: string }) {
  const sess = useOnline(id);
  return sess ? <Match sess={sess} /> : null;
}

/**
 * An online match that never started (or has ended) drops back to the room.
 * Runs after every render: `view` and `room.playing` arrive from two different
 * channels and either may land first.
 */
function useLeaveFinishedOnline(sess: GameSession, view: ReturnType<typeof useMatchView>["view"]): void {
  useEffect(() => {
    if (sess.kind === "online" && view === null && sess.room && !sess.room.playing) navigate({ name: "room", id: sess.id }, { replace: true });
  });
}

function Match({ sess }: { sess: GameSession }) {
  useConsoleSession(sess);
  const { view } = useMatchView(sess);
  const scene = matchScene(view);
  useLeaveFinishedOnline(sess, view);

  // Fade between select and board like a scene change.
  const { shown, fading } = useCrossfade(scene, { initial: scene, outMs: 180, inMs: 0 });
  // The board and character select each have their own stage plate and BGM
  // (the App shell already set the /play plate; this refines it per phase).
  useBackdrop(shown === "board" ? "bg_common" : "bg_band");
  useEffect(() => {
    playSceneBgm(shown === "board" ? "board" : "select");
  }, [shown]);

  return (
    <div style={{ opacity: fading ? 0 : 1, transition: "opacity 0.18s" }}>
      {shown === "select" && <Select sess={sess} />}
      {shown === "board" && <Board sess={sess} />}
    </div>
  );
}

export function Play({ id }: { id: string }) {
  return id === "solo" ? <SoloPlay /> : <OnlinePlay id={id} />;
}