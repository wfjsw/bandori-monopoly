// App shell: boot gate, History API routes with the scene fade (SceneFader: 0.18 s
// out, 0.3 s in), BGM per screen, popups and toasts.

import { type ReactNode, useEffect, useMemo, useState } from "react";
import { playSceneBgm, sfx, unlockAudio } from "../core/audio";
import { useLangVersion, useProfile } from "../core/hooks";
import { useCrossfade } from "../hooks/scene";
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
import { InspectPreviewHost } from "../ui/CardPreview";
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
  // Parsed once per path: `useCrossfade` keys on this object, and a fresh one
  // every render would re-run its effect (and cancel its timers) on every
  // profile bump under a live match.
  const target = useMemo(() => parse(path), [path]);

  // Unknown or empty routes go to the menu.
  useEffect(() => {
    if (ready && profile && !target) navigate({ name: "menu" }, { replace: true });
  }, [ready, profile, target]);

  // Fade out, swap the scene (backdrop + BGM), fade in. The first scene fades
  // in over 30ms from the boot screen; later ones cross-fade under the fader.
  const { shown, fading } = useCrossfade(ready ? target : null, {
    eq: (a, b) => href(a) === href(b),
    outMs: 180,
    inMs: 30,
    onSwap: (r) => {
      closeAllModals();
      setBackdrop(BACKDROP[r.name]);
      playSceneBgm(BGM[r.name]);
    },
  });

  const booted = ready && profile;
  return (
    <>
      <Stage fading={fading}>
        {/* Remount on language change so every already-rendered string is re-read. */}
        <div key={lang} style={{ position: "absolute", inset: 0 }}>
          {booted ? shown && <div key={href(shown)}>{scene(shown)}</div> : <Boot resume={!!target} onReady={() => setReady(true)} />}
        </div>
        <ModalHost />
        <InspectPreviewHost />
        <ToastHost />
      </Stage>
      <Console />
    </>
  );
}