// /play/solo and /play/<roomId>: the match screen. Shows character select,
// the full-screen deck adjuster, or the board depending on the phase; resumes
// after a refresh (solo from the saved snapshot, online by re-attaching to the
// player).

import { useEffect, useState } from "react";
import { navigate } from "../app/router";
import { useBackdrop } from "../app/Stage";
import { playSceneBgm } from "../core/audio";
import { useMatchView } from "../core/hooks";
import { useCrossfade } from "../hooks/scene";
import { type GameSession, type SoloSession, discardSolo, matchScene, resumeSolo } from "../game/session";
import { toast } from "../ui/Toast";
import { Board } from "./board/Board";
import { useOnline } from "./lobby/Room";
import { DeckAdjust } from "./select/DeckAdjust";
import { Select } from "./select/Select";
import { t as tr } from "../i18n/t";
import s from "./Play.module.css";
import { useConsoleSession } from "../console/context";

/**
 * Resumed solo match: keep it (and its engine worker) for the life of the
 * screen, and bail back to the menu when there is no save -- or when the
 * worker failed to restore the one there was. The worker boots (and restores)
 * asynchronously, so a failure there is the old "solo save dropped" path, one
 * tick later.
 */
function useSoloResume(sess: SoloSession | null, loaded: boolean): void {
  useEffect(() => {
    // The save lives in IndexedDB now, so the load is asynchronous: wait for
    // it before deciding there is nothing to resume.
    if (!loaded) return;
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
  }, [sess, loaded]);
}

/** Shown while the solo engine worker boots and opens the match. */
function Opening() {
  return (
    <div className={s.opening} role="status">
      <div className={s.spinner} />
      <div>{tr("play.opening")}</div>
    </div>
  );
}

function SoloPlay() {
  const [sess, setSess] = useState<SoloSession | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [open, setOpen] = useState(false);
  useSoloResume(sess, loaded);
  useEffect(() => {
    let alive = true;
    void resumeSolo().then((s) => {
      if (!alive) return;
      setSess(s);
      setLoaded(true);
    });
    return () => {
      alive = false;
    };
  }, []);
  useEffect(() => {
    let alive = true;
    void sess?.ready.then(() => alive && setOpen(true), () => undefined);
    return () => {
      alive = false;
    };
  }, [sess]);
  if (!loaded) return <Opening />;
  if (!sess) return null;
  return open ? <Match sess={sess} /> : <Opening />;
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

  // Fade between select / deck / board like a scene change.
  const { shown, fading } = useCrossfade(scene, { initial: scene, outMs: 180, inMs: 0 });
  // Character select and the deck window share the select plate and BGM; only
  // the board differs (the App shell already set the /play plate).
  useBackdrop(shown === "board" ? "bg_common" : "bg_band");
  useEffect(() => {
    playSceneBgm(shown === "board" ? "board" : "select");
  }, [shown]);

  return (
    <div style={{ opacity: fading ? 0 : 1, transition: "opacity 0.18s" }}>
      {shown === "select" && <Select sess={sess} />}
      {shown === "deck" && <DeckAdjust sess={sess} />}
      {shown === "board" && <Board sess={sess} />}
    </div>
  );
}

export function Play({ id }: { id: string }) {
  return id === "solo" ? <SoloPlay /> : <OnlinePlay id={id} />;
}