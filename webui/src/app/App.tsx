// App shell: boot gate, History API routes with the scene fade (SceneFader: 0.18 s
// out, 0.3 s in), BGM per screen, popups and toasts.

import { type ReactNode, useEffect, useState } from "react";
import { playSceneBgm, sfx, unlockAudio } from "../core/audio";
import { useLangVersion, useProfile } from "../core/hooks";
import { Boot } from "../scenes/boot/Boot";
import { Deck } from "../scenes/deck/Deck";
import { Gallery } from "../scenes/gallery/Gallery";
import { Lobby } from "../scenes/lobby/Lobby";
import { Room } from "../scenes/lobby/Room";
import { Menu } from "../scenes/menu/Menu";
import { Play } from "../scenes/Play";
import { Replays } from "../scenes/replay/Replays";
import { ReplayPlayer } from "../scenes/replay/ReplayPlayer";
import { closeAllModals, ModalHost } from "../ui/Modal";
import { ToastHost } from "../ui/Toast";
import { href, navigate, parse, type Route, usePath } from "./router";
import { setBackdrop, Stage } from "./Stage";
import { Console } from "../console/Console";

const BACKDROP: Record<Route["name"], string> = { menu: "bg_common", lobby: "bg_live", room: "bg_live", play: "bg_band", gallery: "bg_band", deck: "bg_band", replay: "bg_common", replayView: "bg_common" };
const BGM: Record<Route["name"], string> = { menu: "menu", lobby: "lobby", room: "lobby", play: "select", gallery: "gallery", deck: "deck", replay: "menu", replayView: "board" };

function scene(r: Route): ReactNode {
  switch (r.name) {
    case "menu": return <Menu />;
    case "gallery": return <Gallery />;
    case "deck": return <Deck />;
    case "lobby": return <Lobby />;
    case "room": return <Room id={r.id} />;
    case "play": return <Play id={r.id} />;
    case "replay": return <Replays />;
    case "replayView": return <ReplayPlayer />;
  }
}

// TapFeedback: unlock audio on the first gesture, a tap sound on buttons.
document.addEventListener("pointerdown", () => unlockAudio(), { capture: true });
document.addEventListener("click", (e) => {
  if ((e.target as Element).closest("button:not(:disabled)")) sfx("tap", 40);
});

export function App() {
  const [ready, setReady] = useState(false);
  const profile = useProfile();
  const lang = useLangVersion();
  const path = usePath();
  const target = parse(path);
  const [shown, setShown] = useState<Route | null>(null);
  const [fading, setFading] = useState(false);

  // Unknown or empty routes go to the menu.
  useEffect(() => {
    if (ready && profile && !target) navigate({ name: "menu" }, { replace: true });
  }, [ready, profile, target]);

  // Fade out, swap the scene, fade in.
  const key = target ? href(target) : "";
  useEffect(() => {
    if (!ready || !target) return;
    if (shown && href(shown) === key) return;
    let cancelled = false;
    setFading(true);
    const t = window.setTimeout(() => {
      if (cancelled) return;
      closeAllModals();
      setBackdrop(BACKDROP[target.name]);
      playSceneBgm(BGM[target.name]);
      setShown(target);
      window.setTimeout(() => setFading(false), 30);
    }, shown ? 180 : 0);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, key]);

  const booted = ready && profile;
  return (
    <>
      <Stage fading={fading}>
        {/* Remount on language change so every already-rendered string is re-read. */}
        <div key={lang} style={{ position: "absolute", inset: 0 }}>
          {booted ? shown && <div key={href(shown)}>{scene(shown)}</div> : <Boot resume={!!target} onReady={() => setReady(true)} />}
        </div>
        <ModalHost />
        <ToastHost />
      </Stage>
      <Console />
    </>
  );
}
