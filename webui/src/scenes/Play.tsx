// /play/solo and /play/<roomId>: the match screen. Shows character select
// or the board depending on the phase; resumes after a refresh (solo from the
// saved snapshot, online by re-attaching to the player).

import { useEffect, useState } from "react";
import { navigate } from "../app/router";
import { setBackdrop } from "../app/Stage";
import { playSceneBgm } from "../core/audio";
import { useMatchView } from "../core/hooks";
import { type GameSession, matchScene, resumeSolo } from "../game/session";
import { toast } from "../ui/Toast";
import { Board } from "./board/Board";
import { useOnline } from "./lobby/Room";
import { Select } from "./select/Select";
import { t as tr } from "../i18n/t";
import { bindConsoleSession } from "../console/context";

function SoloPlay() {
  const [sess] = useState(() => resumeSolo());
  useEffect(() => {
    if (!sess) {
      toast(tr("play.noSolo"));
      navigate({ name: "menu" }, { replace: true });
    }
  }, [sess]);
  return sess ? <Match sess={sess} /> : null;
}

function OnlinePlay({ id }: { id: string }) {
  const sess = useOnline(id);
  return sess ? <Match sess={sess} /> : null;
}

function Match({ sess }: { sess: GameSession }) {
  useEffect(() => bindConsoleSession(sess), [sess, bindConsoleSession]);
  const { view } = useMatchView(sess);
  const scene = matchScene(view);
  const [fade, setFade] = useState(false);
  const [shown, setShown] = useState(scene);

  // Fade between select and board like a scene change.
  useEffect(() => {
    if (scene === shown) return;
    setFade(true);
    const t = window.setTimeout(() => {
      setShown(scene);
      setFade(false);
    }, 180);
    return () => clearTimeout(t);
  }, [scene, shown]);

  useEffect(() => {
    setBackdrop(shown === "board" ? "bg_common" : "bg_band");
    playSceneBgm(shown === "board" ? "board" : "select");
  }, [shown]);

  // Online room went back to waiting (match over, or never started).
  useEffect(() => {
    if (sess.kind === "online" && view === null && sess.room && !sess.room.playing) navigate({ name: "room", id: sess.id }, { replace: true });
  });

  return (
    <div style={{ opacity: fade ? 0 : 1, transition: "opacity 0.18s" }}>
      {shown === "select" && <Select sess={sess} />}
      {shown === "board" && <Board sess={sess} />}
    </div>
  );
}

export function Play({ id }: { id: string }) {
  return id === "solo" ? <SoloPlay /> : <OnlinePlay id={id} />;
}
